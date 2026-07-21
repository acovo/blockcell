use blockcell_core::{Error, Result};
use chrono::Timelike;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

static REPLACEMENT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 原子切换管理器
pub struct AtomicSwitcher {
    install_dir: PathBuf,
    current_binary: PathBuf,
    backup_dir: PathBuf,
}

impl AtomicSwitcher {
    pub fn new(install_dir: PathBuf) -> Self {
        let binary_name = std::env::current_exe()
            .ok()
            .and_then(|path| path.file_name().map(|name| name.to_os_string()))
            .unwrap_or_else(|| "blockcell".into());
        Self::for_binary(install_dir.join(binary_name))
    }

    #[doc(hidden)]
    pub fn for_binary(current_binary: PathBuf) -> Self {
        let install_dir = current_binary
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let backup_dir = install_dir.join("backups");
        Self {
            install_dir,
            current_binary,
            backup_dir,
        }
    }

    /// 原子切换到新版本
    pub async fn switch_to_new(&self, new_binary: &Path, version: &str) -> Result<()> {
        info!(version = %version, "Starting atomic switch");
        let _lock = UpdateLock::acquire(self.install_dir.join(".blockcell-update.lock"))?;

        // 在产生备份之前先拒绝明显无效的更新文件。
        self.verify_binary(new_binary)?;

        // 1. 确保备份目录存在
        std::fs::create_dir_all(&self.backup_dir)?;

        // 2. 备份当前版本
        let current_binary = self.get_current_binary_path()?;
        let backup_path = self.backup_dir.join(format!(
            "blockcell-{}-{}",
            self.get_current_version()?,
            unique_timestamp()
        ));

        if current_binary.exists() {
            copy_file_durable(&current_binary, &backup_path)?;
            info!(backup = %backup_path.display(), "Current version backed up");
        }

        // 3. 在目标目录创建唯一临时文件并原子替换。
        replace_from_source(new_binary, &current_binary)?;
        info!("Binary replaced atomically");

        // 6. 验证替换成功
        if !current_binary.exists() {
            return Err(Error::Other("Binary replacement failed".to_string()));
        }

        // 7. 清理旧备份（保留最近 N 个）
        self.cleanup_old_backups(5)?;

        info!("Atomic switch completed successfully");
        Ok(())
    }

    /// 回滚到上一个版本
    pub async fn rollback(&self, version: Option<&str>) -> Result<()> {
        warn!("Rolling back to previous version");
        let _lock = UpdateLock::acquire(self.install_dir.join(".blockcell-update.lock"))?;

        // 1. 找到最新的备份
        let latest_backup = self.find_backup(version)?;

        // 2. 获取当前二进制路径
        let current_binary = self.get_current_binary_path()?;

        // 3. 备份失败的版本
        let failed_backup = self
            .backup_dir
            .join(format!("blockcell-failed-{}", unique_timestamp()));
        if current_binary.exists() {
            copy_file_durable(&current_binary, &failed_backup)?;
        }

        // 4. 恢复备份
        replace_from_source(&latest_backup, &current_binary)?;

        info!(backup = %latest_backup.display(), "Rolled back successfully");
        Ok(())
    }

    /// 验证二进制文件
    fn verify_binary(&self, path: &Path) -> Result<()> {
        // 1. 检查文件存在
        if !path.exists() {
            return Err(Error::NotFound("Binary not found".to_string()));
        }

        // 2. 检查文件大小（至少应该有几 MB）
        let metadata = std::fs::metadata(path)?;
        if metadata.len() < 1024 * 1024 {
            return Err(Error::Validation("Binary too small".to_string()));
        }

        // 3. 检查文件头（ELF/Mach-O/PE）
        let mut file = std::fs::File::open(path)?;
        use std::io::Read;
        let mut magic = [0u8; 4];
        file.read_exact(&mut magic)?;

        #[cfg(target_os = "linux")]
        if &magic != b"\x7fELF" {
            return Err(Error::Validation("Not a valid ELF binary".to_string()));
        }

        #[cfg(target_os = "macos")]
        {
            // Mach-O 魔数：小端 64-bit (\xcf\xfa\xed\xfe), 小端 32-bit (\xce\xfa\xed\xfe),
            // 大端 64-bit (\xfe\xed\xfa\xcf), 大端 32-bit (\xfe\xed\xfa\xce),
            // Fat Binary (\xca\xfe\xba\xbe)
            let is_macho = matches!(
                magic,
                [0xcf, 0xfa, 0xed, 0xfe]  // 小端 64-bit
                | [0xce, 0xfa, 0xed, 0xfe]  // 小端 32-bit
                | [0xfe, 0xed, 0xfa, 0xcf]  // 大端 64-bit
                | [0xfe, 0xed, 0xfa, 0xce]  // 大端 32-bit
                | [0xca, 0xfe, 0xba, 0xbe] // Fat Binary (Universal)
            );
            if !is_macho {
                return Err(Error::Validation("Not a valid Mach-O binary".to_string()));
            }
        }

        #[cfg(target_os = "windows")]
        if &magic[0..2] != b"MZ" {
            return Err(Error::Validation("Not a valid PE binary".to_string()));
        }

        debug!("Binary verification passed");
        Ok(())
    }

    fn get_current_binary_path(&self) -> Result<PathBuf> {
        Ok(self.current_binary.clone())
    }

    fn get_current_version(&self) -> Result<String> {
        Ok(env!("CARGO_PKG_VERSION").to_string())
    }

    /// 返回按修改时间排序的备份文件列表（不包含失败备份）
    fn list_backups_sorted(&self) -> Result<Vec<std::fs::DirEntry>> {
        let mut backups: Vec<_> = std::fs::read_dir(&self.backup_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                name.starts_with("blockcell-") && !name.contains("-failed-")
            })
            .collect();

        backups.sort_by_key(|e| {
            e.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });

        Ok(backups)
    }

    fn find_backup(&self, version: Option<&str>) -> Result<PathBuf> {
        let mut backups = self.list_backups_sorted()?;

        if let Some(version) = version {
            let expected_prefix = format!("blockcell-{}-", version);
            backups.retain(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&expected_prefix)
            });
        }

        if backups.is_empty() {
            return Err(Error::NotFound(match version {
                Some(version) => format!("No backup found for rollback version {}", version),
                None => "No backup found for rollback".to_string(),
            }));
        }

        Ok(backups.last().unwrap().path())
    }

    fn cleanup_old_backups(&self, keep_count: usize) -> Result<()> {
        let backups = self.list_backups_sorted()?;

        if backups.len() <= keep_count {
            return Ok(());
        }

        let to_remove = backups.len() - keep_count;
        for backup in backups.iter().take(to_remove) {
            if let Err(e) = std::fs::remove_file(backup.path()) {
                warn!(path = %backup.path().display(), error = %e, "Failed to remove old backup");
            } else {
                debug!(path = %backup.path().display(), "Removed old backup");
            }
        }

        Ok(())
    }
}

struct UpdateLock {
    path: PathBuf,
}

impl UpdateLock {
    fn acquire(path: PathBuf) -> Result<Self> {
        use std::io::Write;

        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    Error::Other("Another update or rollback is already running".to_string())
                } else {
                    error.into()
                }
            })?;
        writeln!(file, "{}", std::process::id())?;
        file.sync_all()?;
        Ok(Self { path })
    }
}

impl Drop for UpdateLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn unique_timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn unique_replacement_temp_path(target: &Path) -> PathBuf {
    let directory = target.parent().unwrap_or_else(|| Path::new("."));
    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("blockcell");
    let counter = REPLACEMENT_COUNTER.fetch_add(1, Ordering::Relaxed);
    directory.join(format!(
        ".{}-{}-{}-{}.tmp",
        file_name,
        std::process::id(),
        unique_timestamp(),
        counter
    ))
}

fn copy_file_durable(source: &Path, destination: &Path) -> Result<()> {
    let mut source_file = std::fs::File::open(source)?;
    let mut destination_file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)?;
    std::io::copy(&mut source_file, &mut destination_file)?;
    destination_file.sync_all()?;
    std::fs::set_permissions(destination, std::fs::metadata(source)?.permissions())?;
    Ok(())
}

fn replace_from_source(source: &Path, target: &Path) -> Result<()> {
    let temp_path = unique_replacement_temp_path(target);
    let result = (|| {
        copy_file_durable(source, &temp_path)?;
        replace_file_atomic(&temp_path, target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result
}

#[cfg(windows)]
fn replace_file_atomic(source: &Path, target: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target_wide: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            target_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error().into())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file_atomic(source: &Path, target: &Path) -> Result<()> {
    std::fs::rename(source, target)?;
    Ok(())
}

/// 维护窗口检查器
pub struct MaintenanceWindow {
    window: String, // 格式: "HH:MM-HH:MM"
}

impl MaintenanceWindow {
    pub fn new(window: String) -> Self {
        Self { window }
    }

    /// 检查当前时间是否在维护窗口内
    pub fn is_in_window(&self) -> bool {
        if self.window.is_empty() {
            return true; // 没有配置维护窗口，任何时间都可以
        }

        let parts: Vec<&str> = self.window.split('-').collect();
        if parts.len() != 2 {
            warn!(window = %self.window, "Invalid maintenance window format");
            return false;
        }

        let start = match self.parse_time(parts[0]) {
            Some(t) => t,
            None => return false,
        };

        let end = match self.parse_time(parts[1]) {
            Some(t) => t,
            None => return false,
        };

        let now = chrono::Local::now();
        let current = (now.hour(), now.minute());

        // 处理跨午夜的情况
        if start <= end {
            current >= start && current < end
        } else {
            current >= start || current < end
        }
    }

    fn parse_time(&self, time_str: &str) -> Option<(u32, u32)> {
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let hour = parts[0].trim().parse::<u32>().ok()?;
        let minute = parts[1].trim().parse::<u32>().ok()?;

        if hour >= 24 || minute >= 60 {
            return None;
        }

        Some((hour, minute))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_maintenance_window() {
        let window = MaintenanceWindow::new("03:00-05:00".to_string());
        // 这个测试依赖于当前时间，所以只是验证不会 panic
        let _ = window.is_in_window();
    }

    #[test]
    fn test_parse_time() {
        let window = MaintenanceWindow::new("03:00-05:00".to_string());
        assert_eq!(window.parse_time("03:00"), Some((3, 0)));
        assert_eq!(window.parse_time("23:59"), Some((23, 59)));
        assert_eq!(window.parse_time("24:00"), None);
        assert_eq!(window.parse_time("invalid"), None);
    }

    #[tokio::test]
    async fn rollback_restores_requested_version_atomically() {
        let temp = TempDir::new().unwrap();
        let current = temp.path().join("blockcell-test");
        std::fs::write(&current, b"failed version").unwrap();
        let backup_dir = temp.path().join("backups");
        std::fs::create_dir_all(&backup_dir).unwrap();
        std::fs::write(backup_dir.join("blockcell-1.0.0-100"), b"version one").unwrap();
        std::fs::write(backup_dir.join("blockcell-2.0.0-200"), b"version two").unwrap();
        let switcher = AtomicSwitcher::for_binary(current.clone());

        switcher.rollback(Some("1.0.0")).await.unwrap();

        assert_eq!(std::fs::read(&current).unwrap(), b"version one");
        assert_eq!(
            std::fs::read(backup_dir.join("blockcell-1.0.0-100")).unwrap(),
            b"version one"
        );
    }

    #[tokio::test]
    async fn rollback_rejects_unknown_requested_version() {
        let temp = TempDir::new().unwrap();
        let current = temp.path().join("blockcell-test");
        std::fs::write(&current, b"failed version").unwrap();
        std::fs::create_dir_all(temp.path().join("backups")).unwrap();
        let switcher = AtomicSwitcher::for_binary(current);

        let error = switcher.rollback(Some("3.0.0")).await.unwrap_err();

        assert!(error.to_string().contains("3.0.0"), "{error}");
    }

    #[test]
    fn replacement_temp_paths_are_unique_and_share_target_directory() {
        let temp = TempDir::new().unwrap();
        let target = temp.path().join("blockcell-test");

        let first = unique_replacement_temp_path(&target);
        let second = unique_replacement_temp_path(&target);

        assert_ne!(first, second);
        assert_eq!(first.parent(), target.parent());
        assert_eq!(second.parent(), target.parent());
    }
}
