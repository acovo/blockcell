use crate::atomic::{AtomicSwitcher, MaintenanceWindow};
use crate::manifest::Manifest;
use crate::verification::{HealthChecker, SignatureVerifier};
use blockcell_core::{Config, Error, Paths, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info, warn};

const MAX_ARTIFACT_SIZE: u64 = 256 * 1024 * 1024;
const STAGING_METADATA_FILE: &str = "current.json";
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct UpdateManager {
    config: Config,
    paths: Paths,
    client: Client,
    switcher: AtomicSwitcher,
}

#[derive(Debug)]
pub struct UpdateStatus {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub staging_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedUpdate {
    pub version: String,
    pub path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StagedUpdateMetadata {
    version: String,
    file_name: String,
}

impl UpdateManager {
    pub fn new(config: Config, paths: Paths) -> Self {
        let install_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        let switcher = AtomicSwitcher::new(install_dir);

        Self {
            config,
            paths,
            client: Client::new(),
            switcher,
        }
    }

    pub async fn check(&self) -> Result<Option<Manifest>> {
        let manifest_url = &self.config.auto_upgrade.manifest_url;
        if manifest_url.is_empty() {
            return Err(Error::Config("Manifest URL not configured".to_string()));
        }

        debug!(url = %manifest_url, "Checking for updates");

        let response = self
            .client
            .get(manifest_url)
            .send()
            .await
            .map_err(|e| Error::Other(format!("Failed to fetch manifest: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::Other(format!(
                "Failed to fetch manifest: HTTP {}",
                response.status()
            )));
        }

        let manifest: Manifest = response
            .json()
            .await
            .map_err(|e| Error::Other(format!("Failed to parse manifest: {}", e)))?;

        // Check if channel matches
        if manifest.channel != self.config.auto_upgrade.channel {
            debug!(
                manifest_channel = %manifest.channel,
                config_channel = %self.config.auto_upgrade.channel,
                "Channel mismatch"
            );
            return Ok(None);
        }

        let current_version = env!("CARGO_PKG_VERSION");
        if !Self::version_greater(&manifest.version, current_version) {
            debug!(
                current = %current_version,
                manifest = %manifest.version,
                "Already on latest version or manifest is not newer"
            );
            return Ok(None);
        }

        Ok(Some(manifest))
    }

    pub async fn download(&self, manifest: &Manifest) -> Result<PathBuf> {
        let (os, arch) = get_current_platform();

        let artifact = manifest
            .get_artifact(&os, &arch)
            .ok_or_else(|| Error::NotFound(format!("No artifact for {}/{}", os, arch)))?;

        info!(url = %artifact.url, "Downloading update");

        let mut response = self
            .client
            .get(&artifact.url)
            .send()
            .await
            .map_err(|e| Error::Other(format!("Download failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::Other(format!(
                "Download failed: HTTP {}",
                response.status()
            )));
        }

        if let Some(content_length) = response.content_length() {
            if content_length > MAX_ARTIFACT_SIZE {
                return Err(Error::Validation(format!(
                    "Update artifact is too large: {} bytes (maximum {})",
                    content_length, MAX_ARTIFACT_SIZE
                )));
            }
        }

        let version_file_name = Self::version_file_name(&manifest.version)?;
        let staging_dir = self.staging_dir();
        std::fs::create_dir_all(&staging_dir)?;
        let staging_path = staging_dir.join(&version_file_name);
        let temp_path = unique_temp_path(&staging_dir, "artifact");

        let result: Result<PathBuf> = async {
            let mut file = tokio::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp_path)
                .await?;
            let mut hasher = Sha256::new();
            let mut downloaded = 0u64;

            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|e| Error::Other(format!("Failed to read download: {}", e)))?
            {
                downloaded = downloaded.checked_add(chunk.len() as u64).ok_or_else(|| {
                    Error::Validation("Update artifact size overflow".to_string())
                })?;
                if downloaded > MAX_ARTIFACT_SIZE {
                    return Err(Error::Validation(format!(
                        "Update artifact is too large: more than {} bytes",
                        MAX_ARTIFACT_SIZE
                    )));
                }
                file.write_all(&chunk).await?;
                hasher.update(&chunk);
            }
            file.flush().await?;
            file.sync_all().await?;
            drop(file);

            let hash = format!("{:x}", hasher.finalize());
            if hash != artifact.sha256 {
                return Err(Error::Validation(format!(
                    "SHA256 mismatch: expected {}, got {}",
                    artifact.sha256, hash
                )));
            }
            info!("SHA256 verification passed");

            if self.config.auto_upgrade.require_signature {
                let bytes = tokio::fs::read(&temp_path).await?;
                self.verify_signature(manifest, &bytes, artifact.sig.as_deref())?;
            } else {
                warn!(
                    "requireSignature is disabled: installing update verified by SHA256 only. \
                     A tampered manifest could serve a malicious binary. \
                     Enable signature verification (requireSignature: true) for fail-closed safety."
                );
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&temp_path)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&temp_path, perms)?;
            }

            replace_staging_file(&temp_path, &staging_path)?;
            self.persist_staged_update(&StagedUpdateMetadata {
                version: manifest.version.clone(),
                file_name: version_file_name,
            })?;

            Ok(staging_path.clone())
        }
        .await;

        if result.is_err() {
            let _ = std::fs::remove_file(&temp_path);
        }
        let staging_path = result?;

        info!(path = %staging_path.display(), "Update downloaded and verified");

        Ok(staging_path)
    }

    pub fn staged_update(&self) -> Result<Option<StagedUpdate>> {
        let metadata_path = self.staging_dir().join(STAGING_METADATA_FILE);
        let bytes = match std::fs::read(&metadata_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let metadata: StagedUpdateMetadata = serde_json::from_slice(&bytes)?;
        let file_name = Path::new(&metadata.file_name);
        if file_name.components().count() != 1
            || !matches!(file_name.components().next(), Some(Component::Normal(_)))
        {
            return Err(Error::Validation(
                "Invalid staged update file name".to_string(),
            ));
        }
        let path = self.staging_dir().join(file_name);
        if !path.is_file() {
            return Ok(None);
        }
        Ok(Some(StagedUpdate {
            version: metadata.version,
            path,
        }))
    }

    fn staging_dir(&self) -> PathBuf {
        self.paths.update_dir().join("staging")
    }

    fn version_file_name(version: &str) -> Result<String> {
        if version.is_empty()
            || !version
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || ".-_+".contains(character))
        {
            return Err(Error::Validation(format!(
                "Invalid update version for staging: {}",
                version
            )));
        }
        Ok(format!("blockcell-{}", version))
    }

    fn persist_staged_update(&self, metadata: &StagedUpdateMetadata) -> Result<()> {
        use std::io::Write;

        let staging_dir = self.staging_dir();
        std::fs::create_dir_all(&staging_dir)?;
        let metadata_path = staging_dir.join(STAGING_METADATA_FILE);
        let temp_path = unique_temp_path(&staging_dir, "metadata");
        let result: Result<()> = (|| {
            let mut file = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp_path)?;
            serde_json::to_writer(&mut file, metadata)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            replace_staging_file(&temp_path, &metadata_path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp_path);
        }
        result
    }

    fn clear_staged_update_metadata(&self) -> Result<()> {
        match std::fs::remove_file(self.staging_dir().join(STAGING_METADATA_FILE)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn status(&self) -> Result<UpdateStatus> {
        let current_version = env!("CARGO_PKG_VERSION").to_string();
        let staged = self.staged_update()?;
        let remote = self.check().await?;
        let latest_version = remote
            .as_ref()
            .map(|manifest| manifest.version.clone())
            .or_else(|| staged.as_ref().map(|update| update.version.clone()));
        let update_available = remote.is_some() || staged.is_some();
        let staging_path = staged.map(|update| update.path);

        Ok(UpdateStatus {
            current_version,
            latest_version,
            update_available,
            staging_path,
        })
    }

    pub async fn apply(&self, staging_path: &std::path::Path, version: &str) -> Result<()> {
        info!(version = %version, "Applying update");

        // 1. 检查维护窗口
        let window = MaintenanceWindow::new(self.config.auto_upgrade.maintenance_window.clone());
        if !window.is_in_window() {
            return Err(Error::Other(
                "Not in maintenance window, update postponed".to_string(),
            ));
        }

        // 2. 运行 Healthcheck（在切换前）
        let checker = HealthChecker::new(staging_path.to_path_buf());
        let health_result = checker.check(30).await?;

        if !health_result.passed {
            error!("Healthcheck failed before switch");
            for check in &health_result.checks {
                if !check.passed {
                    error!(check = %check.name, message = %check.message, "Failed check");
                }
            }
            return Err(Error::Validation("Healthcheck failed".to_string()));
        }
        info!("Pre-switch healthcheck passed");

        // 3. 原子切换
        self.switcher.switch_to_new(staging_path, version).await?;
        self.clear_staged_update_metadata()?;

        // 4. 运行 Healthcheck（切换后）
        // 注意：这里需要重启进程，所以实际上这个检查应该在重启后由外部进程执行
        // 这里我们只是验证文件已正确替换
        info!("Update applied successfully. Restart required.");

        Ok(())
    }

    pub async fn rollback(&self, version: Option<&str>) -> Result<()> {
        warn!("Rolling back to previous version");

        self.switcher.rollback(version).await?;

        info!("Rollback completed. Restart required.");
        Ok(())
    }

    /// 验证签名
    fn verify_signature(
        &self,
        _manifest: &Manifest,
        data: &[u8],
        signature: Option<&str>,
    ) -> Result<()> {
        let sig = signature
            .ok_or_else(|| Error::Validation("Signature required but not provided".to_string()))?;

        // 从环境变量或配置获取公钥
        let public_key_hex = std::env::var("BLOCKCELL_PUBLIC_KEY")
            .or_else(|_| std::env::var("BLOCKCELL_VERIFY_KEY"))
            .map_err(|_| Error::Config("Public key not configured".to_string()))?;

        let verifier = SignatureVerifier::from_hex(&public_key_hex)?;
        verifier.verify(data, sig)?;

        info!("Signature verification passed");
        Ok(())
    }

    /// 执行完整的更新流程
    pub async fn update_flow(&self) -> Result<()> {
        // 1. 检查更新
        info!("Checking for updates...");
        let manifest = match self.check().await? {
            Some(m) => m,
            None => {
                info!("No updates available");
                return Ok(());
            }
        };

        let current_version = env!("CARGO_PKG_VERSION");
        // 使用语义版本比较：若 manifest 版本不高于当前版本，无需更新
        if !Self::version_greater(&manifest.version, current_version) {
            info!(
                current = %current_version,
                manifest = %manifest.version,
                "Already on latest version or manifest is older"
            );
            return Ok(());
        }

        // 检查最低主机版本兼容性
        if let Some(ref min_version) = manifest.min_host_version {
            if !Self::version_satisfies(current_version, min_version) {
                return Err(Error::Validation(format!(
                    "Current version {} does not meet minimum required version {}. Manual upgrade required.",
                    current_version, min_version
                )));
            }
        }

        info!(
            current = %current_version,
            latest = %manifest.version,
            "Update available"
        );

        // 2. 下载
        let staging_path = self.download(&manifest).await?;

        // 3. 应用
        self.apply(&staging_path, &manifest.version).await?;

        Ok(())
    }

    /// 检查当前版本是否满足最低版本要求 (semver 比较，正确处理 pre-release)
    fn version_satisfies(current: &str, minimum: &str) -> bool {
        let cur = current.trim_start_matches('v');
        let min = minimum.trim_start_matches('v');
        // 优先使用 semver 语义比较，正确处理 pre-release 标签
        if let (Ok(cv), Ok(mv)) = (semver::Version::parse(cur), semver::Version::parse(min)) {
            return cv >= mv;
        }
        // 回退到旧的数字比较
        Self::parse_numeric(cur) >= Self::parse_numeric(min)
    }

    /// 检查 candidate 版本是否严格大于 base 版本 (semver 比较，正确处理 pre-release)
    fn version_greater(candidate: &str, base: &str) -> bool {
        let c = candidate.trim_start_matches('v');
        let b = base.trim_start_matches('v');
        // 优先使用 semver 语义比较，正确处理 pre-release 标签
        // 例如: "1.0.0" > "1.0.0-beta.1" 为 true（正式版大于预发布版）
        if let (Ok(cv), Ok(bv)) = (semver::Version::parse(c), semver::Version::parse(b)) {
            return cv > bv;
        }
        // 回退到旧的数字比较
        Self::parse_numeric(c) > Self::parse_numeric(b)
    }

    /// 旧的数字回退解析: 逐段取数字部分，用于非标准 semver 格式
    fn parse_numeric(v: &str) -> Vec<u64> {
        v.split('.')
            .filter_map(|s| {
                s.split(|c: char| !c.is_ascii_digit())
                    .next()
                    .and_then(|n| n.parse::<u64>().ok())
            })
            .collect()
    }
}

fn get_current_platform() -> (String, String) {
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    };

    (os.to_string(), arch.to_string())
}

fn unique_temp_path(directory: &Path, label: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    directory.join(format!(
        ".blockcell-{}-{}-{}-{}.tmp",
        label,
        std::process::id(),
        timestamp,
        counter
    ))
}

fn replace_staging_file(source: &Path, destination: &Path) -> Result<()> {
    #[cfg(windows)]
    if destination.exists() {
        std::fs::remove_file(destination)?;
    }
    std::fs::rename(source, destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Artifact;
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn serve_once(status: &str, headers: &[(&str, String)], body: &[u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let status = status.to_string();
        let headers: Vec<(String, String)> = headers
            .iter()
            .map(|(name, value)| ((*name).to_string(), value.clone()))
            .collect();
        let body = body.to_vec();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0u8; 4096];
            let _ = socket.read(&mut request).await;

            let mut response = format!("HTTP/1.1 {status}\r\nConnection: close\r\n");
            for (name, value) in headers {
                response.push_str(&format!("{name}: {value}\r\n"));
            }
            response.push_str("\r\n");
            socket.write_all(response.as_bytes()).await.unwrap();
            socket.write_all(&body).await.unwrap();
        });

        format!("http://{addr}/artifact")
    }

    fn manager(temp: &TempDir) -> UpdateManager {
        let mut config = Config::default();
        config.auto_upgrade.require_signature = false;
        UpdateManager::new(config, Paths::with_base(temp.path().to_path_buf()))
    }

    fn manager_with_manifest_url(temp: &TempDir, manifest_url: String) -> UpdateManager {
        let mut config = Config::default();
        config.auto_upgrade.require_signature = false;
        config.auto_upgrade.manifest_url = manifest_url;
        UpdateManager::new(config, Paths::with_base(temp.path().to_path_buf()))
    }

    fn manifest(url: String, body: &[u8]) -> Manifest {
        let (os, arch) = get_current_platform();
        Manifest {
            channel: "stable".to_string(),
            version: "9.9.9".to_string(),
            published_at: "2026-07-21T00:00:00Z".to_string(),
            artifacts: vec![Artifact {
                os,
                arch,
                url,
                sha256: format!("{:x}", Sha256::digest(body)),
                sig: None,
            }],
            min_host_version: None,
            notes: String::new(),
        }
    }

    #[tokio::test]
    async fn download_rejects_non_success_http_status() {
        let temp = TempDir::new().unwrap();
        let body = b"not found";
        let url = serve_once(
            "404 Not Found",
            &[("Content-Length", body.len().to_string())],
            body,
        )
        .await;

        let error = manager(&temp)
            .download(&manifest(url, body))
            .await
            .unwrap_err();

        assert!(error.to_string().contains("HTTP 404"), "{error}");
    }

    #[tokio::test]
    async fn download_rejects_artifact_larger_than_limit() {
        let temp = TempDir::new().unwrap();
        let url = serve_once(
            "200 OK",
            &[("Content-Length", (MAX_ARTIFACT_SIZE + 1).to_string())],
            &[],
        )
        .await;

        let error = manager(&temp)
            .download(&manifest(url, &[]))
            .await
            .unwrap_err();

        assert!(error.to_string().contains("too large"), "{error}");
    }

    #[tokio::test]
    async fn successful_download_persists_staging_metadata() {
        let temp = TempDir::new().unwrap();
        let body = b"signed update bytes";
        let url = serve_once(
            "200 OK",
            &[("Content-Length", body.len().to_string())],
            body,
        )
        .await;
        let manager = manager(&temp);

        let staged_path = manager.download(&manifest(url, body)).await.unwrap();

        assert_eq!(std::fs::read(staged_path).unwrap(), body);
        let metadata_path = temp.path().join("update/staging/current.json");
        let metadata: serde_json::Value =
            serde_json::from_slice(&std::fs::read(metadata_path).unwrap()).unwrap();
        assert_eq!(metadata["version"], "9.9.9");
        assert_eq!(metadata["fileName"], "blockcell-9.9.9");
    }

    #[tokio::test]
    async fn status_reports_remote_update() {
        let temp = TempDir::new().unwrap();
        let remote_manifest = manifest("http://127.0.0.1/unused".to_string(), b"");
        let body = serde_json::to_vec(&remote_manifest).unwrap();
        let url = serve_once(
            "200 OK",
            &[
                ("Content-Length", body.len().to_string()),
                ("Content-Type", "application/json".to_string()),
            ],
            &body,
        )
        .await;

        let status = manager_with_manifest_url(&temp, url)
            .status()
            .await
            .unwrap();

        assert_eq!(status.latest_version.as_deref(), Some("9.9.9"));
        assert!(status.update_available);
        assert_eq!(status.staging_path, None);
    }

    #[tokio::test]
    async fn status_reports_locally_staged_update() {
        let temp = TempDir::new().unwrap();
        let mut remote_manifest = manifest("http://127.0.0.1/unused".to_string(), b"");
        remote_manifest.channel = "beta".to_string();
        let body = serde_json::to_vec(&remote_manifest).unwrap();
        let url = serve_once(
            "200 OK",
            &[
                ("Content-Length", body.len().to_string()),
                ("Content-Type", "application/json".to_string()),
            ],
            &body,
        )
        .await;
        let manager = manager_with_manifest_url(&temp, url);
        let staging_dir = temp.path().join("update/staging");
        std::fs::create_dir_all(&staging_dir).unwrap();
        let staged_path = staging_dir.join("blockcell-9.9.9");
        std::fs::write(&staged_path, b"update").unwrap();
        manager
            .persist_staged_update(&StagedUpdateMetadata {
                version: "9.9.9".to_string(),
                file_name: "blockcell-9.9.9".to_string(),
            })
            .unwrap();

        let status = manager.status().await.unwrap();

        assert_eq!(status.latest_version.as_deref(), Some("9.9.9"));
        assert!(status.update_available);
        assert_eq!(status.staging_path.as_deref(), Some(staged_path.as_path()));
    }
}
