use super::*;
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    #[test]
    fn external_skill_name_rejects_path_syntax() {
        assert!(sanitize_skill_name("../escape").is_err());
        assert!(sanitize_skill_name("nested/skill").is_err());
        assert!(sanitize_skill_name(r"nested\skill").is_err());
    }

    #[test]
    fn installed_skill_name_must_be_one_safe_component() {
        assert_eq!(validate_installed_skill_name("weather").unwrap(), "weather");
        for invalid in ["", ".", "..", "../escape", "nested/skill", r"nested\skill"] {
            assert!(
                validate_installed_skill_name(invalid).is_err(),
                "accepted invalid installed skill name: {invalid}"
            );
        }
    }

    #[test]
    fn hub_download_url_must_remain_same_origin() {
        let hub = reqwest::Url::parse("https://hub.example/api").unwrap();

        assert_eq!(
            resolve_hub_download_url(&hub, "/downloads/weather.zip")
                .unwrap()
                .as_str(),
            "https://hub.example/api/downloads/weather.zip"
        );
        assert_eq!(
            resolve_hub_download_url(&hub, "v1/skills/weather/download")
                .unwrap()
                .as_str(),
            "https://hub.example/api/v1/skills/weather/download"
        );
        assert!(resolve_hub_download_url(&hub, "https://evil.example/steal").is_err());
        assert!(resolve_hub_download_url(&hub, "http://hub.example/downgrade").is_err());
    }

    #[tokio::test]
    async fn streaming_download_stops_at_the_byte_limit() {
        let chunks = stream::iter(vec![
            Ok::<_, std::io::Error>(b"1234".to_vec()),
            Ok(b"5678".to_vec()),
        ]);

        let err = collect_stream_limited(chunks, 7)
            .await
            .expect_err("an eighth byte must be rejected");
        assert!(err.contains("exceeds"), "unexpected error: {err}");
    }

    #[test]
    fn mapped_private_ipv4_is_blocked() {
        let mapped = "::ffff:127.0.0.1".parse().unwrap();
        assert!(is_blocked_ip(mapped));
    }

    #[test]
    fn archive_budget_rejects_too_many_or_too_large_entries() {
        assert!(check_archive_budget(1, 10, 10, 10).is_ok());
        assert!(check_archive_budget(11, 10, 1, 10).is_err());
        assert!(check_archive_budget(1, 10, 11, 10).is_err());
    }

    #[test]
    fn hub_extraction_propagates_filesystem_errors() {
        use std::io::Write;

        let mut zip_bytes = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut zip_bytes);
            let mut writer = zip::ZipWriter::new(cursor);
            writer
                .start_file("skill/SKILL.md", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"hello").unwrap();
            writer.finish().unwrap();
        }

        let temp = std::env::temp_dir().join(format!("blockcell-extract-{}", uuid::Uuid::new_v4()));
        std::fs::write(&temp, b"not a directory").unwrap();
        let result = extract_hub_package(&zip_bytes, &temp);
        let _ = std::fs::remove_file(&temp);

        assert!(result.is_err(), "filesystem failure must not be ignored");
    }

    #[tokio::test]
    async fn external_staging_write_propagates_filesystem_errors() {
        let temp = std::env::temp_dir().join(format!("blockcell-staging-{}", uuid::Uuid::new_v4()));
        tokio::fs::write(&temp, b"not a directory").await.unwrap();
        let files = vec![DownloadedFile {
            name: "SKILL.md".to_string(),
            content: "hello".to_string(),
        }];

        let result = write_external_staging_files(&temp, &files, "demo", None, None).await;
        let _ = tokio::fs::remove_file(&temp).await;

        assert!(result.is_err(), "staging write failure must be returned");
    }
}
// Skills management — delete / hub proxy / install external
// ---------------------------------------------------------------------------

/// DELETE /v1/skills/:name — delete a user skill
pub(super) async fn handle_skill_delete(
    State(state): State<GatewayState>,
    AxumPath(skill_name): AxumPath<String>,
) -> impl IntoResponse {
    let skill_name = match validate_installed_skill_name(&skill_name) {
        Ok(name) => name,
        Err(message) => return Json(serde_json::json!({ "status": "error", "message": message })),
    };
    let skill_dir = state.paths.skills_dir().join(&skill_name);
    if !skill_dir.exists() {
        return Json(serde_json::json!({ "status": "not_found", "skill": skill_name }));
    }
    match tokio::fs::remove_dir_all(&skill_dir).await {
        Ok(_) => Json(serde_json::json!({ "status": "deleted", "skill": skill_name })),
        Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
    }
}

/// GET /v1/hub/skills — proxy community hub skills list
pub(super) async fn handle_hub_skills(State(state): State<GatewayState>) -> impl IntoResponse {
    let hub_url = match state.config.community_hub_url() {
        Some(u) => u,
        None => {
            return Json(
                serde_json::json!({ "error": "Community hub not configured", "skills": [] }),
            )
        }
    };
    let api_key = state.config.community_hub_api_key();
    let url = format!("{}/v1/skills/trending", hub_url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap_or_default();

    let mut req = client.get(&url);
    if let Some(k) = &api_key {
        req = req.header("Authorization", format!("Bearer {}", k));
    }

    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = response_text_limited(resp, HUB_MAX_METADATA_BYTES)
                .await
                .unwrap_or_default();
            let val: serde_json::Value =
                serde_json::from_str(&body).unwrap_or(serde_json::json!({ "skills": [] }));
            Json(val)
        }
        Ok(resp) => {
            let status = resp.status().as_u16();
            Json(serde_json::json!({ "error": format!("Hub returned {}", status), "skills": [] }))
        }
        Err(e) => Json(serde_json::json!({ "error": e.to_string(), "skills": [] })),
    }
}

/// POST /v1/hub/skills/:name/install — install a skill from community hub
pub(super) async fn handle_hub_skill_install(
    State(state): State<GatewayState>,
    AxumPath(skill_name): AxumPath<String>,
) -> impl IntoResponse {
    let skill_name = match validate_installed_skill_name(&skill_name) {
        Ok(name) => name,
        Err(message) => return Json(serde_json::json!({ "status": "error", "message": message })),
    };
    let hub_url = match state.config.community_hub_url() {
        Some(u) => match reqwest::Url::parse(&u) {
            Ok(url) => url,
            Err(e) => {
                return Json(serde_json::json!({
                    "status": "error",
                    "message": format!("Invalid community hub URL: {e}")
                }))
            }
        },
        None => {
            return Json(
                serde_json::json!({ "status": "error", "message": "Community hub not configured" }),
            )
        }
    };
    let api_key = state.config.community_hub_api_key();
    let skills_dir = state.paths.skills_dir();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap_or_default();

    // Fetch skill metadata
    let info_url = format!(
        "{}/v1/skills/{}/latest",
        hub_url.as_str().trim_end_matches('/'),
        urlencoding::encode(&skill_name)
    );
    let mut req = client.get(&info_url);
    if let Some(k) = &api_key {
        req = req.header("Authorization", format!("Bearer {}", k));
    }
    let info: serde_json::Value = match req.send().await {
        Ok(r) if r.status().is_success() => response_text_limited(r, HUB_MAX_METADATA_BYTES)
            .await
            .ok()
            .and_then(|body| serde_json::from_str(&body).ok())
            .unwrap_or(serde_json::json!({})),
        _ => serde_json::json!({}),
    };

    // Resolve download URL
    let dist_url = info
        .get("dist_url")
        .and_then(|v| v.as_str())
        .or_else(|| info.get("source_url").and_then(|v| v.as_str()));
    let download_candidate = dist_url
        .map(str::to_string)
        .unwrap_or_else(|| format!("v1/skills/{}/download", urlencoding::encode(&skill_name)));
    let download_url = match resolve_hub_download_url(&hub_url, &download_candidate) {
        Ok(url) => url,
        Err(message) => return Json(serde_json::json!({ "status": "error", "message": message })),
    };

    let mut dl_req = client.get(download_url);
    if let Some(k) = &api_key {
        dl_req = dl_req.header("Authorization", format!("Bearer {}", k));
    }

    let resp = match dl_req.send().await {
        Ok(r) => r,
        Err(e) => return Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
    };

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        return Json(
            serde_json::json!({ "status": "error", "message": format!("Download failed: HTTP {}", status) }),
        );
    }

    let bytes = match response_bytes_limited(resp, HUB_MAX_DOWNLOAD_BYTES).await {
        Ok(bytes) => bytes,
        Err(e) => return Json(serde_json::json!({ "status": "error", "message": e })),
    };

    let size_bytes = bytes.len();
    if let Err(e) = install_hub_package_transactionally(&skills_dir, &skill_name, bytes).await {
        return Json(serde_json::json!({ "status": "error", "message": e }));
    }

    Json(serde_json::json!({
        "status": "installed",
        "skill": skill_name,
        "size_bytes": size_bytes,
    }))
}

/// POST /v1/skills/install-external — import an external skill package and queue evolution
#[derive(Deserialize)]
pub(super) struct InstallExternalRequest {
    url: String,
}

/// Represents a downloaded file (name + text content).
pub(super) struct DownloadedFile {
    name: String,
    content: String,
}

const EXTERNAL_MAX_DOWNLOAD_BYTES: usize = 5 * 1024 * 1024; // 5MB
const EXTERNAL_MAX_FILES: usize = 200;
const EXTERNAL_MAX_GITHUB_DEPTH: usize = 6;
const HUB_MAX_METADATA_BYTES: usize = 1024 * 1024;
const HUB_MAX_DOWNLOAD_BYTES: usize = 10 * 1024 * 1024;
const HUB_MAX_ARCHIVE_FILES: usize = 500;
const HUB_MAX_UNPACKED_BYTES: u64 = 50 * 1024 * 1024;

async fn collect_stream_limited<S, B, E>(mut stream: S, max_bytes: usize) -> Result<Vec<u8>, String>
where
    S: futures::Stream<Item = Result<B, E>> + Unpin,
    B: AsRef<[u8]>,
    E: std::fmt::Display,
{
    use futures::StreamExt;

    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download failed: {e}"))?;
        let bytes = chunk.as_ref();
        if body.len().saturating_add(bytes.len()) > max_bytes {
            return Err(format!("Download exceeds {max_bytes} byte limit"));
        }
        body.extend_from_slice(bytes);
    }
    Ok(body)
}

async fn response_bytes_limited(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(format!("Download exceeds {max_bytes} byte limit"));
    }
    collect_stream_limited(response.bytes_stream(), max_bytes).await
}

async fn response_text_limited(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<String, String> {
    let bytes = response_bytes_limited(response, max_bytes).await?;
    String::from_utf8(bytes).map_err(|e| format!("Downloaded content is not UTF-8: {e}"))
}

fn check_archive_budget(
    entry_count: usize,
    max_entries: usize,
    total_unpacked: u64,
    max_unpacked: u64,
) -> Result<(), String> {
    if entry_count > max_entries {
        return Err(format!("Archive contains more than {max_entries} entries"));
    }
    if total_unpacked > max_unpacked {
        return Err(format!("Archive expands beyond {max_unpacked} bytes"));
    }
    Ok(())
}

fn extract_hub_package(bytes: &[u8], destination: &std::path::Path) -> Result<(), String> {
    let cursor = std::io::Cursor::new(bytes);
    let Ok(mut archive) = zip::ZipArchive::new(cursor) else {
        std::fs::create_dir_all(destination).map_err(|e| e.to_string())?;
        return std::fs::write(destination.join("raw.bin"), bytes).map_err(|e| e.to_string());
    };

    let entry_count = archive.len();
    check_archive_budget(
        entry_count,
        HUB_MAX_ARCHIVE_FILES,
        0,
        HUB_MAX_UNPACKED_BYTES,
    )?;
    let mut unpacked_bytes = 0u64;

    for i in 0..entry_count {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let declared_total = unpacked_bytes.saturating_add(file.size());
        check_archive_budget(
            entry_count,
            HUB_MAX_ARCHIVE_FILES,
            declared_total,
            HUB_MAX_UNPACKED_BYTES,
        )?;
        let Some(enclosed) = file.enclosed_name() else {
            return Err(format!("Archive entry has unsafe path: {}", file.name()));
        };
        let components: Vec<_> = enclosed.components().collect();
        let relative = if components.len() > 1 {
            components[1..].iter().collect::<std::path::PathBuf>()
        } else {
            enclosed.to_path_buf()
        };
        let out_path = destination.join(relative);
        if file.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut outfile = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
        let remaining = HUB_MAX_UNPACKED_BYTES.saturating_sub(unpacked_bytes);
        let mut limited = std::io::Read::take(&mut file, remaining.saturating_add(1));
        let copied = std::io::copy(&mut limited, &mut outfile).map_err(|e| e.to_string())?;
        unpacked_bytes = unpacked_bytes.saturating_add(copied);
        check_archive_budget(
            entry_count,
            HUB_MAX_ARCHIVE_FILES,
            unpacked_bytes,
            HUB_MAX_UNPACKED_BYTES,
        )?;
    }
    Ok(())
}

async fn install_hub_package_transactionally(
    skills_dir: &std::path::Path,
    skill_name: &str,
    bytes: Vec<u8>,
) -> Result<(), String> {
    tokio::fs::create_dir_all(skills_dir)
        .await
        .map_err(|e| e.to_string())?;
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let staging = skills_dir.join(format!(".install-{skill_name}-{nonce}"));
    let backup = skills_dir.join(format!(".backup-{skill_name}-{nonce}"));
    let target = skills_dir.join(skill_name);

    tokio::fs::create_dir(&staging)
        .await
        .map_err(|e| e.to_string())?;
    let extract_path = staging.clone();
    let extract_result =
        tokio::task::spawn_blocking(move || extract_hub_package(&bytes, &extract_path))
            .await
            .map_err(|e| format!("Archive extraction task failed: {e}"))?;
    if let Err(error) = extract_result {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(error);
    }

    let had_target = target.exists();
    if had_target {
        tokio::fs::rename(&target, &backup)
            .await
            .map_err(|e| format!("Failed to preserve existing skill: {e}"))?;
    }
    if let Err(error) = tokio::fs::rename(&staging, &target).await {
        if had_target {
            let _ = tokio::fs::rename(&backup, &target).await;
        }
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(format!("Failed to activate installed skill: {error}"));
    }
    if had_target {
        if let Err(error) = tokio::fs::remove_dir_all(&backup).await {
            tracing::warn!(
                path = %backup.display(),
                error = %error,
                "Installed skill but could not remove backup directory"
            );
        }
    }
    Ok(())
}

async fn write_external_staging_files(
    skill_dir: &std::path::Path,
    downloaded_files: &[DownloadedFile],
    skill_name: &str,
    display_name: Option<&str>,
    description: Option<&str>,
) -> Result<usize, String> {
    tokio::fs::create_dir_all(skill_dir)
        .await
        .map_err(|e| format!("Cannot create skill dir: {e}"))?;
    let mut total_bytes = 0usize;
    for file in downloaded_files {
        total_bytes = total_bytes.saturating_add(file.content.len());
        if total_bytes > EXTERNAL_MAX_DOWNLOAD_BYTES {
            return Err(format!(
                "Downloaded content too large (>{EXTERNAL_MAX_DOWNLOAD_BYTES} bytes)"
            ));
        }
        let rel = normalize_relative_path(&file.name)
            .ok_or_else(|| format!("Unsafe downloaded path: {}", file.name))?;
        let out_path = skill_dir.join(rel);
        if let Some(parent) = out_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Cannot create staging directory: {e}"))?;
        }
        tokio::fs::write(&out_path, &file.content)
            .await
            .map_err(|e| format!("Cannot write {}: {e}", out_path.display()))?;
    }

    if !skill_dir.join("meta.yaml").exists() {
        let meta_value = serde_json::json!({
            "name": display_name.unwrap_or(skill_name),
            "description": description.unwrap_or("External skill (evolving)"),
            "tools": [],
        });
        let meta_content = serde_yaml::to_string(&meta_value)
            .map_err(|e| format!("Cannot serialize meta.yaml: {e}"))?;
        tokio::fs::write(skill_dir.join("meta.yaml"), meta_content)
            .await
            .map_err(|e| format!("Cannot write meta.yaml: {e}"))?;
    }
    Ok(total_bytes)
}

fn is_blocked_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_unspecified()
                || v4.octets()[0] == 0
        }
        std::net::IpAddr::V6(v6) => {
            v6.to_ipv4_mapped()
                .is_some_and(|mapped| is_blocked_ip(std::net::IpAddr::V4(mapped)))
                || v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
        }
    }
}

async fn validated_external_addrs(url: &reqwest::Url) -> Result<Vec<std::net::SocketAddr>, String> {
    match url.scheme() {
        "http" | "https" => {}
        s => return Err(format!("Unsupported URL scheme: {}", s)),
    }

    let host = url.host_str().ok_or("URL host is required")?.to_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        return Err("Blocked host: localhost".to_string());
    }
    if host.ends_with(".local") {
        return Err("Blocked host: .local".to_string());
    }
    let port = url.port_or_known_default().unwrap_or(443);

    // If it's already an IP literal, validate directly.
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        if is_blocked_ip(ip) {
            return Err(format!("Blocked IP: {}", ip));
        }
        return Ok(vec![std::net::SocketAddr::new(ip, port)]);
    }

    let addrs: Vec<_> = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|e| format!("DNS lookup failed: {}", e))?
        .collect();
    if addrs.is_empty() {
        return Err("DNS lookup returned no addresses".to_string());
    }
    for addr in &addrs {
        if is_blocked_ip(addr.ip()) {
            return Err(format!("Blocked resolved IP: {}", addr.ip()));
        }
    }
    Ok(addrs)
}

async fn build_external_client(url: &reqwest::Url) -> Result<reqwest::Client, String> {
    let addrs = validated_external_addrs(url).await?;
    let host = url.host_str().ok_or("URL host is required")?;
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(host, &addrs)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

fn sanitize_skill_name(raw: &str) -> Result<String, String> {
    if raw.contains(['/', '\\']) || matches!(raw.trim(), "." | "..") {
        return Err("Invalid skill name (path syntax is not allowed)".to_string());
    }
    let mut out = String::new();
    for ch in raw.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if matches!(c, ' ' | '-' | '.' | '_') && !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        return Err("Invalid skill name (empty after sanitization)".to_string());
    }
    if out.len() > 64 {
        return Err("Invalid skill name (too long)".to_string());
    }
    if out.contains("__") {
        // Not a security issue, but avoid pathological names.
        // Keep as-is; consumers may rely on underscores.
    }
    Ok(out)
}

fn validate_installed_skill_name(raw: &str) -> Result<String, String> {
    if raw.is_empty()
        || raw.len() > 64
        || raw.trim() != raw
        || matches!(raw, "." | "..")
        || raw.contains(['/', '\\'])
        || raw.chars().any(|c| c.is_control())
    {
        return Err("Invalid skill name: expected one safe path component".to_string());
    }
    Ok(raw.to_string())
}

fn resolve_hub_download_url(
    hub_url: &reqwest::Url,
    candidate: &str,
) -> Result<reqwest::Url, String> {
    let resolved = match reqwest::Url::parse(candidate) {
        Ok(url) => url,
        Err(_) => reqwest::Url::parse(&format!(
            "{}/{}",
            hub_url.as_str().trim_end_matches('/'),
            candidate.trim_start_matches('/')
        ))
        .map_err(|e| format!("Invalid Hub download URL: {e}"))?,
    };
    let same_origin = resolved.scheme() == hub_url.scheme()
        && resolved.host_str() == hub_url.host_str()
        && resolved.port_or_known_default() == hub_url.port_or_known_default();
    if !same_origin {
        return Err("Hub download URL must use the configured Hub origin".to_string());
    }
    Ok(resolved)
}

fn normalize_relative_path(rel: &str) -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(rel);
    let mut clean = std::path::PathBuf::new();
    for comp in p.components() {
        match comp {
            std::path::Component::Normal(s) => clean.push(s),
            std::path::Component::CurDir => {}
            // Block absolute paths and any parent traversal.
            std::path::Component::RootDir
            | std::path::Component::Prefix(_)
            | std::path::Component::ParentDir => return None,
        }
    }
    if clean.as_os_str().is_empty() {
        None
    } else {
        Some(clean)
    }
}

fn ensure_within_dir(root: &std::path::Path, path: &std::path::Path) -> bool {
    if let (Ok(r), Ok(p)) = (root.canonicalize(), path.canonicalize()) {
        return p.starts_with(r);
    }
    // If canonicalize fails (e.g. path doesn't exist yet), fall back to lexical check.
    path.starts_with(root)
}

/// Convert a GitHub HTML URL to the GitHub API tree URL for directory listing.
/// e.g. https://github.com/openclaw/skills/tree/main/skills/foo/bar
///   -> https://api.github.com/repos/openclaw/skills/contents/skills/foo/bar?ref=main
fn github_html_to_api_url(url: &str) -> Option<String> {
    // Match: github.com/{owner}/{repo}/tree/{branch}/{path}
    let stripped = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    if !stripped.starts_with("github.com/") {
        return None;
    }
    let parts: Vec<&str> = stripped
        .trim_start_matches("github.com/")
        .splitn(5, '/')
        .collect();
    if parts.len() < 4 || parts[2] != "tree" {
        return None;
    }
    let owner = parts[0];
    let repo = parts[1];
    let branch = parts[3];
    let path = if parts.len() == 5 { parts[4] } else { "" };
    Some(format!(
        "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
        owner, repo, path, branch
    ))
}

/// Convert a GitHub blob URL to the raw content URL.
/// e.g. https://github.com/openclaw/skills/blob/main/skills/foo/SKILL.md
///   -> https://raw.githubusercontent.com/openclaw/skills/main/skills/foo/SKILL.md
fn github_blob_to_raw_url(url: &str) -> Option<String> {
    let stripped = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    if !stripped.starts_with("github.com/") {
        return None;
    }
    let rest = stripped.trim_start_matches("github.com/");
    let parts: Vec<&str> = rest.splitn(5, '/').collect();
    if parts.len() < 5 || parts[2] != "blob" {
        return None;
    }
    let owner = parts[0];
    let repo = parts[1];
    let branch = parts[3];
    let path = parts[4];
    Some(format!(
        "https://raw.githubusercontent.com/{}/{}/{}/{}",
        owner, repo, branch, path
    ))
}

/// Extract skill name and description from OpenClaw SKILL.md YAML frontmatter.
/// Returns (name, description).
fn parse_openclaw_frontmatter(content: &str) -> (Option<String>, Option<String>) {
    if !content.starts_with("---") {
        return (None, None);
    }
    let after_open = &content[3..];
    let end = after_open.find("\n---").unwrap_or(0);
    if end == 0 {
        return (None, None);
    }
    let frontmatter = &after_open[..end];
    let mut name: Option<String> = None;
    let mut desc: Option<String> = None;
    let mut in_desc_block = false;
    let mut desc_lines: Vec<String> = Vec::new();

    for line in frontmatter.lines() {
        if in_desc_block {
            if line.starts_with("  ") || line.starts_with('\t') {
                desc_lines.push(line.trim().to_string());
                continue;
            } else {
                in_desc_block = false;
                if !desc_lines.is_empty() {
                    desc = Some(desc_lines.join(" "));
                }
            }
        }
        if let Some(v) = line.strip_prefix("name:") {
            name = Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(v) = line.strip_prefix("description:") {
            let trimmed = v.trim();
            if trimmed == "|" || trimmed == ">" {
                in_desc_block = true;
                desc_lines.clear();
            } else if !trimmed.is_empty() {
                desc = Some(trimmed.trim_matches('"').trim_matches('\'').to_string());
            }
        }
    }
    if in_desc_block && !desc_lines.is_empty() {
        desc = Some(desc_lines.join(" "));
    }
    (name, desc)
}

/// Download text files from a GitHub directory via the GitHub Contents API.
/// Traverses subdirectories up to a fixed depth (iterative, avoids async recursion).
async fn fetch_github_directory_recursive(
    api_url: &str,
    root_prefix: &str,
    depth: usize,
    remaining_files: &mut usize,
    remaining_bytes: &mut usize,
) -> Result<Vec<DownloadedFile>, String> {
    let mut result: Vec<DownloadedFile> = Vec::new();
    let mut stack: Vec<(String, usize)> = vec![(api_url.to_string(), depth)];

    while let Some((url, d)) = stack.pop() {
        if d > EXTERNAL_MAX_GITHUB_DEPTH {
            continue;
        }
        if *remaining_files == 0 {
            return Err(format!(
                "Too many files in GitHub directory (max {})",
                EXTERNAL_MAX_FILES
            ));
        }
        if *remaining_bytes == 0 {
            return Err(format!(
                "Downloaded content too large (max {} bytes)",
                EXTERNAL_MAX_DOWNLOAD_BYTES
            ));
        }

        let parsed_url =
            reqwest::Url::parse(&url).map_err(|e| format!("Invalid GitHub API URL: {e}"))?;
        let client = build_external_client(&parsed_url).await?;
        let resp = client
            .get(&url)
            .header("User-Agent", "blockcell-agent/1.0")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(|e| format!("GitHub API request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!(
                "GitHub API returned HTTP {}",
                resp.status().as_u16()
            ));
        }

        let response_body = response_text_limited(resp, HUB_MAX_METADATA_BYTES).await?;
        let entries: serde_json::Value = serde_json::from_str(&response_body)
            .map_err(|e| format!("Failed to parse GitHub API response: {e}"))?;

        let files_array = entries
            .as_array()
            .ok_or("GitHub API returned non-array response")?;

        for entry in files_array {
            if *remaining_files == 0 {
                return Err(format!(
                    "Too many files in GitHub directory (max {})",
                    EXTERNAL_MAX_FILES
                ));
            }
            if *remaining_bytes == 0 {
                return Err(format!(
                    "Downloaded content too large (max {} bytes)",
                    EXTERNAL_MAX_DOWNLOAD_BYTES
                ));
            }

            let file_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let file_name = entry
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let download_url = entry
                .get("download_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let entry_path = entry
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if file_type == "dir" {
                if let Some(next_url) = entry.get("url").and_then(|v| v.as_str()) {
                    stack.push((next_url.to_string(), d + 1));
                }
                continue;
            }

            if file_type != "file" || download_url.is_empty() {
                continue;
            }

            let ext = file_name.rsplit('.').next().unwrap_or("").to_lowercase();
            let is_text = matches!(
                ext.as_str(),
                "md" | "rhai"
                    | "yaml"
                    | "yml"
                    | "json"
                    | "toml"
                    | "sh"
                    | "py"
                    | "ts"
                    | "js"
                    | "txt"
            ) || file_name == "SKILL.md"
                || file_name == "SKILL.rhai"
                || file_name == "meta.yaml";

            if !is_text {
                continue;
            }

            let mut rel = file_name.clone();
            if !root_prefix.is_empty() {
                let prefix = format!("{}/", root_prefix.trim_end_matches('/'));
                if entry_path.starts_with(&prefix) {
                    rel = entry_path[prefix.len()..].to_string();
                }
            }
            let Some(rel_path) = normalize_relative_path(&rel) else {
                continue;
            };

            let download_parsed = reqwest::Url::parse(&download_url)
                .map_err(|e| format!("Invalid GitHub download URL: {e}"))?;
            let download_client = build_external_client(&download_parsed).await?;
            match download_client
                .get(&download_url)
                .header("User-Agent", "blockcell-agent/1.0")
                .send()
                .await
            {
                Ok(r) if r.status().is_success() => {
                    if let Ok(text) = response_text_limited(r, *remaining_bytes).await {
                        if text.len() > *remaining_bytes {
                            return Err(format!(
                                "Downloaded content too large (max {} bytes)",
                                EXTERNAL_MAX_DOWNLOAD_BYTES
                            ));
                        }
                        *remaining_bytes = remaining_bytes.saturating_sub(text.len());
                        *remaining_files = remaining_files.saturating_sub(1);
                        result.push(DownloadedFile {
                            name: rel_path.to_string_lossy().to_string(),
                            content: text,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    Ok(result)
}

pub(super) async fn handle_skill_install_external(
    State(state): State<GatewayState>,
    Json(req): Json<InstallExternalRequest>,
) -> impl IntoResponse {
    let url = req.url.trim().to_string();
    if url.is_empty() {
        return Json(serde_json::json!({ "status": "error", "message": "url is required" }));
    }

    let parsed_url = match reqwest::Url::parse(&url) {
        Ok(u) => u,
        Err(e) => {
            return Json(serde_json::json!({
                "status": "error",
                "message": format!("Invalid URL: {}", e)
            }))
        }
    };
    let client = match build_external_client(&parsed_url).await {
        Ok(client) => client,
        Err(e) => return Json(serde_json::json!({ "status": "error", "message": e })),
    };

    // ── Step 1: Download skill files ────────────────────────────────────────

    let mut downloaded_files: Vec<DownloadedFile> = Vec::new();

    if url.ends_with(".zip") || url.contains(".zip?") {
        // zip bundle download
        let resp: reqwest::Response = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                return Json(
                    serde_json::json!({ "status": "error", "message": format!("Download failed: {}", e) }),
                )
            }
        };
        if !resp.status().is_success() {
            return Json(
                serde_json::json!({ "status": "error", "message": format!("HTTP {}", resp.status().as_u16()) }),
            );
        }
        if let Some(len) = resp.content_length() {
            if len as usize > EXTERNAL_MAX_DOWNLOAD_BYTES {
                return Json(serde_json::json!({
                    "status": "error",
                    "message": format!("ZIP too large ({} bytes, max {})", len, EXTERNAL_MAX_DOWNLOAD_BYTES)
                }));
            }
        }

        let bytes = match response_bytes_limited(resp, EXTERNAL_MAX_DOWNLOAD_BYTES).await {
            Ok(bytes) => bytes,
            Err(e) => return Json(serde_json::json!({ "status": "error", "message": e })),
        };
        let cursor = std::io::Cursor::new(&bytes);
        if let Ok(mut archive) = zip::ZipArchive::new(cursor) {
            let mut files_left = EXTERNAL_MAX_FILES;
            let mut remaining_bytes = EXTERNAL_MAX_DOWNLOAD_BYTES;
            for i in 0..archive.len() {
                if files_left == 0 {
                    return Json(serde_json::json!({
                        "status": "error",
                        "message": format!("Too many files in ZIP (max {})", EXTERNAL_MAX_FILES)
                    }));
                }
                if let Ok(mut file) = archive.by_index(i) {
                    if file.is_dir() {
                        continue;
                    }

                    let raw_name = file.name();
                    // Skip common junk directories
                    if raw_name.starts_with("__MACOSX/") {
                        continue;
                    }
                    let Some(rel_path) = normalize_relative_path(raw_name) else {
                        continue;
                    };

                    if file.size() > remaining_bytes as u64 {
                        return Json(serde_json::json!({
                            "status": "error",
                            "message": format!("Downloaded content too large (max {} bytes)", EXTERNAL_MAX_DOWNLOAD_BYTES)
                        }));
                    }

                    let mut content = String::new();
                    use std::io::Read;
                    if file
                        .by_ref()
                        .take(remaining_bytes.saturating_add(1) as u64)
                        .read_to_string(&mut content)
                        .is_ok()
                    {
                        if content.len() > remaining_bytes {
                            return Json(serde_json::json!({
                                "status": "error",
                                "message": format!("Downloaded content too large (max {} bytes)", EXTERNAL_MAX_DOWNLOAD_BYTES)
                            }));
                        }
                        remaining_bytes = remaining_bytes.saturating_sub(content.len());
                        files_left = files_left.saturating_sub(1);
                        downloaded_files.push(DownloadedFile {
                            name: rel_path.to_string_lossy().to_string(),
                            content,
                        });
                    }
                }
            }
        }
    } else if let Some(api_url) = github_html_to_api_url(&url) {
        // GitHub directory URL → use Contents API
        let root_prefix = url
            .split("/tree/")
            .nth(1)
            .and_then(|s| s.split_once('/').map(|x| x.1))
            .unwrap_or("")
            .trim_matches('/')
            .to_string();
        let mut remaining = EXTERNAL_MAX_FILES;
        let mut remaining_bytes = EXTERNAL_MAX_DOWNLOAD_BYTES;
        match fetch_github_directory_recursive(
            &api_url,
            &root_prefix,
            0,
            &mut remaining,
            &mut remaining_bytes,
        )
        .await
        {
            Ok(files) => downloaded_files = files,
            Err(e) => return Json(serde_json::json!({ "status": "error", "message": e })),
        }
    } else {
        // Single file URL (blob or raw)
        let raw_url = if url.contains("github.com/") && url.contains("/blob/") {
            github_blob_to_raw_url(&url).unwrap_or_else(|| url.clone())
        } else {
            url.clone()
        };

        let raw_parsed = match reqwest::Url::parse(&raw_url) {
            Ok(u) => u,
            Err(e) => {
                return Json(serde_json::json!({
                    "status": "error",
                    "message": format!("Invalid URL: {}", e)
                }))
            }
        };
        let raw_client = match build_external_client(&raw_parsed).await {
            Ok(client) => client,
            Err(e) => return Json(serde_json::json!({ "status": "error", "message": e })),
        };

        let resp: reqwest::Response = match raw_client.get(&raw_url).send().await {
            Ok(r) => r,
            Err(e) => {
                return Json(
                    serde_json::json!({ "status": "error", "message": format!("Download failed: {}", e) }),
                )
            }
        };
        if !resp.status().is_success() {
            return Json(
                serde_json::json!({ "status": "error", "message": format!("HTTP {}", resp.status().as_u16()) }),
            );
        }

        if let Some(len) = resp.content_length() {
            if len as usize > EXTERNAL_MAX_DOWNLOAD_BYTES {
                return Json(serde_json::json!({
                    "status": "error",
                    "message": format!("File too large ({} bytes, max {})", len, EXTERNAL_MAX_DOWNLOAD_BYTES)
                }));
            }
        }
        let content = match response_text_limited(resp, EXTERNAL_MAX_DOWNLOAD_BYTES).await {
            Ok(content) => content,
            Err(e) => return Json(serde_json::json!({ "status": "error", "message": e })),
        };
        if content.len() > EXTERNAL_MAX_DOWNLOAD_BYTES {
            return Json(serde_json::json!({
                "status": "error",
                "message": format!("File too large ({} bytes, max {})", content.len(), EXTERNAL_MAX_DOWNLOAD_BYTES)
            }));
        }
        let fname = raw_url.rsplit('/').next().unwrap_or("SKILL.md").to_string();
        let rel =
            normalize_relative_path(&fname).unwrap_or_else(|| std::path::PathBuf::from("SKILL.md"));
        downloaded_files.push(DownloadedFile {
            name: rel.to_string_lossy().to_string(),
            content,
        });
    }

    if downloaded_files.is_empty() {
        return Json(
            serde_json::json!({ "status": "error", "message": "No skill files could be downloaded from the provided URL" }),
        );
    }

    // ── Step 2: Determine skill name ─────────────────────────────────────────

    // Try to parse from SKILL.md frontmatter first
    let skill_md_content = downloaded_files
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case("SKILL.md"))
        .map(|f| f.content.as_str())
        .unwrap_or("");

    let (fm_name, fm_description) = parse_openclaw_frontmatter(skill_md_content);

    // Derive a filesystem-safe skill name
    let raw_skill_name = fm_name.clone().unwrap_or_else(|| {
        // Fall back to last path segment from the URL
        url.trim_end_matches('/')
            .rsplit('/')
            .find(|s| !s.is_empty())
            .unwrap_or("external_skill")
            .trim_end_matches(".zip")
            .trim_end_matches(".md")
            .to_string()
    });
    let skill_name = match sanitize_skill_name(&raw_skill_name) {
        Ok(s) => s,
        Err(e) => {
            return Json(serde_json::json!({
                "status": "error",
                "message": format!("Invalid skill name: {}", e)
            }))
        }
    };

    let existing_dir = state.paths.skills_dir().join(&skill_name);
    if existing_dir.exists() {
        return Json(serde_json::json!({
            "status": "error",
            "message": format!("Skill '{}' already exists. Please rename it (e.g. change frontmatter name) before importing.", skill_name)
        }));
    }

    let staging_dir_existing = state.paths.import_staging_skills_dir().join(&skill_name);
    if staging_dir_existing.exists() {
        return Json(serde_json::json!({
            "status": "error",
            "message": format!("Skill '{}' is already staged for import. If it is still evolving, please wait for it to complete.", skill_name)
        }));
    }

    {
        let svc = state.evolution_service.lock().await;
        if let Ok(records) = svc.list_all_records() {
            for r in records {
                if r.skill_name != skill_name {
                    continue;
                }
                let status = r.status.normalize();
                let in_progress = matches!(
                    *status,
                    blockcell_skills::evolution::EvolutionStatus::Triggered
                        | blockcell_skills::evolution::EvolutionStatus::Generating
                        | blockcell_skills::evolution::EvolutionStatus::Generated
                        | blockcell_skills::evolution::EvolutionStatus::Auditing
                        | blockcell_skills::evolution::EvolutionStatus::AuditPassed
                        | blockcell_skills::evolution::EvolutionStatus::CompilePassed
                        | blockcell_skills::evolution::EvolutionStatus::Observing
                        | blockcell_skills::evolution::EvolutionStatus::RollingOut
                );
                if in_progress {
                    return Json(serde_json::json!({
                        "status": "error",
                        "message": format!("Skill '{}' has an in-progress evolution record ({}, {:?}). Please wait for it to complete or clean it up first.", skill_name, r.id, status),
                        "skill": skill_name,
                        "evolution_id": r.id,
                    }));
                }
            }
        }
    }

    // ── Step 3: Write files to skill staging directory ───────────────────────

    let skill_dir = state.paths.import_staging_skills_dir().join(&skill_name);
    if skill_dir.exists() {
        let staging_root = state.paths.import_staging_skills_dir();
        if ensure_within_dir(&staging_root, &skill_dir) {
            if let Err(e) = tokio::fs::remove_dir_all(&skill_dir).await {
                return Json(serde_json::json!({
                    "status": "error",
                    "message": format!("Cannot clear previous staging directory: {e}")
                }));
            }
        } else {
            return Json(serde_json::json!({
                "status": "error",
                "message": "Refusing to delete directory outside staging root"
            }));
        }
    }
    let total_bytes = match write_external_staging_files(
        &skill_dir,
        &downloaded_files,
        &skill_name,
        fm_name.as_deref(),
        fm_description.as_deref(),
    )
    .await
    {
        Ok(total_bytes) => total_bytes,
        Err(message) => {
            let _ = tokio::fs::remove_dir_all(&skill_dir).await;
            return Json(serde_json::json!({ "status": "error", "message": message }));
        }
    };

    // ── Step 4: Build evolution context and trigger the self-evolution pipeline

    // Collect all file contents into a single description block for the LLM
    let mut openclaw_content = String::new();
    openclaw_content.push_str(&format!("## OpenClaw Skill Source (from {})\n\n", url));
    for df in &downloaded_files {
        openclaw_content.push_str(&format!("### {}\n```\n{}\n```\n\n", df.name, df.content));
    }

    // Detect skill type from downloaded files
    let has_py = downloaded_files.iter().any(|f| f.name.ends_with(".py"));
    let has_rhai = downloaded_files.iter().any(|f| f.name.ends_with(".rhai"));
    let ext_skill_type = if has_rhai {
        blockcell_skills::SkillType::Rhai
    } else if has_py {
        blockcell_skills::SkillType::Python
    } else {
        blockcell_skills::SkillType::PromptOnly
    };

    let description = match ext_skill_type {
        blockcell_skills::SkillType::Python => format!(
            "Convert the following OpenClaw-compatible skill into a Blockcell SKILL.py script.\n\
            Skill name: {}\n\
            {}\n\
            \n\
            Generate a COMPLETE SKILL.py and a minimal compatible meta.yaml.\n\
            Blockcell Python runtime contract:\n\
            - Script is executed as `python3 SKILL.py`\n\
            - User input is provided from stdin as plain text\n\
            - Additional JSON context is available in env `BLOCKCELL_SKILL_CONTEXT`\n\
            - Output final user-facing result to stdout\n\
            - Do NOT require command-line JSON arguments\n\
            - meta.yaml should stay minimal: keep `name`, `description`, and only add `tools`/`requires`/`permissions`/`fallback` when truly needed\n\
            - Do NOT generate any legacy routing or formatting fields\n\
            \n\
            Reuse useful logic from legacy OpenClaw scripts (e.g. scripts/*.py),\n\
            but adapt the entrypoint and output format to Blockcell style.\n\
            \n\
            {}",
            fm_name.as_deref().unwrap_or(&skill_name),
            fm_description
                .as_deref()
                .map(|d| format!("Description: {}", d))
                .unwrap_or_default(),
            openclaw_content,
        ),
        blockcell_skills::SkillType::LocalScript => format!(
            "Convert the following OpenClaw-compatible skill into a Blockcell local script / CLI skill asset.\n\
            Skill name: {}\n\
            {}\n\
            Generate a COMPLETE local script entrypoint and a minimal compatible meta.yaml.\n\
            Blockcell local-script runtime contract:\n\
            - Script is executed through `exec_local` inside the active skill directory\n\
            - Use relative paths only; do not depend on absolute paths\n\
            - Read user input from stdin, args, or env when appropriate\n\
            - Write user-facing results to stdout\n\
            - meta.yaml should stay minimal: keep `name`, `description`, and only add `tools`/`requires`/`permissions`/`fallback` when truly needed\n\
            - Do NOT generate any legacy routing or formatting fields\n\
            Reuse useful logic from legacy OpenClaw scripts, but adapt the entrypoint and output format to Blockcell style.\n\
            \n\
            {}",
            fm_name.as_deref().unwrap_or(&skill_name),
            fm_description
                .as_deref()
                .map(|d| format!("Description: {}", d))
                .unwrap_or_default(),
            openclaw_content,
        ),
        blockcell_skills::SkillType::Rhai => format!(
            "Convert the following OpenClaw-compatible skill into a Blockcell SKILL.rhai script.\n\
            Skill name: {}\n\
            {}\n\
            \n\
            Generate a COMPLETE SKILL.rhai and a minimal compatible meta.yaml.\n\
            Use Blockcell tool-call style and produce clear user-facing output.\n\
            meta.yaml should stay minimal: keep `name`, `description`, and only add `tools`/`requires`/`permissions`/`fallback` when truly needed.\n\
            Do NOT generate any legacy routing or formatting fields.\n\
            \n\
            {}",
            fm_name.as_deref().unwrap_or(&skill_name),
            fm_description
                .as_deref()
                .map(|d| format!("Description: {}", d))
                .unwrap_or_default(),
            openclaw_content,
        ),
        blockcell_skills::SkillType::PromptOnly => format!(
            "Convert the following OpenClaw-compatible skill into a Blockcell SKILL.md document.\n\
            Skill name: {}\n\
            {}\n\
            \n\
            Generate an improved SKILL.md that describes how the AI agent should handle requests\n\
            for this skill, including: goal, tools to use, step-by-step scenarios, and fallback strategy.\n\
            Also generate a minimal meta.yaml with `name`, `description`, and optional `tools`/`requires`/`permissions`/`fallback` only when needed.\n\
            Do NOT generate any legacy routing or formatting fields.\n\
            Base the content on the OpenClaw SKILL.md instructions below.\n\
            \n\
            {}",
            fm_name.as_deref().unwrap_or(&skill_name),
            fm_description
                .as_deref()
                .map(|d| format!("Description: {}", d))
                .unwrap_or_default(),
            openclaw_content,
        ),
    };

    let context = blockcell_skills::EvolutionContext {
        skill_name: skill_name.clone(),
        current_version: "0.0.0".to_string(),
        trigger: blockcell_skills::TriggerReason::ManualRequest { description },
        error_stack: None,
        source_snippet: None,
        source_path: None,
        layout: match ext_skill_type {
            blockcell_skills::SkillType::Rhai => blockcell_skills::SkillLayout::RhaiOrchestration,
            blockcell_skills::SkillType::Python => blockcell_skills::SkillLayout::Hybrid,
            blockcell_skills::SkillType::LocalScript => blockcell_skills::SkillLayout::LocalScript,
            blockcell_skills::SkillType::PromptOnly => blockcell_skills::SkillLayout::PromptTool,
        },
        tool_schemas: vec![],
        timestamp: chrono::Utc::now().timestamp(),
        skill_type: ext_skill_type,
        staged: true,
        staging_skills_dir: Some(
            state
                .paths
                .import_staging_skills_dir()
                .to_string_lossy()
                .to_string(),
        ),
    };

    let evolution_id = {
        let svc = state.evolution_service.lock().await;
        match svc.trigger_external_evolution(context).await {
            Ok(id) => id,
            Err(e) => {
                let _ = tokio::fs::remove_dir_all(&skill_dir).await;
                return Json(serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to queue evolution: {}", e)
                }));
            }
        }
    };

    tracing::info!(
        skill = %skill_name,
        evolution_id = %evolution_id,
        files = downloaded_files.len(),
        "External skill queued for self-evolution"
    );

    Json(serde_json::json!({
        "status": "evolving",
        "skill": skill_name,
        "evolution_id": evolution_id,
        "files_downloaded": downloaded_files.len(),
        "size_bytes": total_bytes,
        "message": "技能包已导入 staging，并已加入自进化队列；系统会按当前 Blockcell skill 规范整理后再部署"
    }))
}

// ---------------------------------------------------------------------------
