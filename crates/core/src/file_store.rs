use std::io::{Error, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A cross-process exclusive lock backed by an atomically-created file.
///
/// The lock file stores `pid:unix_timestamp:token`. The random token makes
/// releasing ownership-safe when a stale lock is replaced while an old holder
/// is still unwinding.
#[derive(Debug)]
pub struct ExclusiveFileLock {
    path: PathBuf,
    token: String,
    held: bool,
}

impl ExclusiveFileLock {
    const MAX_RETRIES: usize = 30;
    const RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

    pub fn try_acquire(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        match Self::create(path) {
            Ok(lock) => Ok(lock),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                let observed = std::fs::read_to_string(path).unwrap_or_default();
                if !holder_is_alive(&observed)
                    && std::fs::read_to_string(path).ok().as_deref() == Some(observed.as_str())
                {
                    match std::fs::remove_file(path) {
                        Ok(()) => Self::create(path),
                        Err(remove_error) if remove_error.kind() == ErrorKind::NotFound => {
                            Self::create(path)
                        }
                        Err(remove_error) => Err(remove_error),
                    }
                } else {
                    Err(Error::new(
                        ErrorKind::WouldBlock,
                        format!("file lock is already held: {}", path.display()),
                    ))
                }
            }
            Err(error) => Err(error),
        }
    }

    pub fn acquire(path: &Path) -> std::io::Result<Self> {
        for attempt in 0..Self::MAX_RETRIES {
            match Self::try_acquire(path) {
                Ok(lock) => return Ok(lock),
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    if attempt + 1 < Self::MAX_RETRIES {
                        std::thread::sleep(Self::RETRY_INTERVAL);
                    }
                }
                Err(error) => return Err(error),
            }
        }
        Err(Error::new(
            ErrorKind::WouldBlock,
            format!("timed out acquiring file lock: {}", path.display()),
        ))
    }

    fn create(path: &Path) -> std::io::Result<Self> {
        let token = uuid::Uuid::new_v4().to_string();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        write!(file, "{}:{}:{}", std::process::id(), timestamp, token)?;
        file.sync_all()?;
        Ok(Self {
            path: path.to_path_buf(),
            token,
            held: true,
        })
    }

    pub fn release(&mut self) -> std::io::Result<()> {
        if !self.held {
            return Ok(());
        }
        let owned = std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|content| lock_token(&content).map(str::to_owned))
            .is_some_and(|token| token == self.token);
        if owned {
            match std::fs::remove_file(&self.path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        self.held = false;
        Ok(())
    }
}

impl Drop for ExclusiveFileLock {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

fn lock_token(content: &str) -> Option<&str> {
    let mut fields = content.trim().splitn(3, ':');
    fields.next()?;
    fields.next()?;
    fields.next()
}

fn holder_is_alive(content: &str) -> bool {
    let pid = content
        .trim()
        .split(':')
        .next()
        .and_then(|value| value.parse::<u32>().ok());
    pid.is_some_and(process_is_alive)
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn process_is_alive(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
        .unwrap_or(false)
}

/// Durably writes a complete replacement in the target directory and then
/// atomically swaps it into place.
pub fn atomic_write(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("store");
    let temp_path = path.with_file_name(format!(
        ".{file_name}.tmp.{}.{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let mut temp = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)?;
    if let Err(error) = (|| {
        temp.write_all(data)?;
        temp.sync_all()?;
        drop(temp);
        replace_file(&temp_path, path)
    })() {
        let _ = std::fs::remove_file(&temp_path);
        return Err(error);
    }

    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        std::fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_file(temp_path: &Path, path: &Path) -> std::io::Result<()> {
    std::fs::rename(temp_path, path)
}

#[cfg(windows)]
fn replace_file(temp_path: &Path, path: &Path) -> std::io::Result<()> {
    let backup_path = path.with_file_name(format!(
        ".{}.bak.{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("store"),
        uuid::Uuid::new_v4()
    ));
    if path.exists() {
        std::fs::rename(path, &backup_path)?;
    }
    match std::fs::rename(temp_path, path) {
        Ok(()) => {
            let _ = std::fs::remove_file(backup_path);
            Ok(())
        }
        Err(error) => {
            if backup_path.exists() {
                let _ = std::fs::rename(backup_path, path);
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{atomic_write, ExclusiveFileLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "blockcell-file-store-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn exclusive_lock_rejects_second_live_holder() {
        let path = temp_path("exclusive.lock");
        let first = ExclusiveFileLock::try_acquire(&path).expect("first lock");
        let second = ExclusiveFileLock::try_acquire(&path);

        assert_eq!(
            second.expect_err("second holder must be rejected").kind(),
            std::io::ErrorKind::WouldBlock
        );

        drop(first);
        assert!(!path.exists());
    }

    #[test]
    fn exclusive_lock_recovers_dead_holder() {
        let path = temp_path("stale.lock");
        std::fs::write(&path, "4294967295:1:stale-token").expect("write stale lock");

        let lock = ExclusiveFileLock::try_acquire(&path).expect("replace stale lock");
        assert_ne!(
            std::fs::read_to_string(&path).expect("read current lock"),
            "4294967295:1:stale-token"
        );

        drop(lock);
    }

    #[test]
    fn release_does_not_remove_replacement_owned_by_another_token() {
        let path = temp_path("ownership.lock");
        let mut first = ExclusiveFileLock::try_acquire(&path).expect("first lock");
        std::fs::write(&path, "1:1:replacement-token").expect("replace lock contents");

        first.release().expect("release old token");

        assert!(path.exists(), "old owner must not delete replacement lock");
        std::fs::remove_file(path).expect("cleanup replacement");
    }

    #[test]
    fn atomic_write_replaces_file_with_complete_contents() {
        let path = temp_path("atomic.json");
        std::fs::write(&path, br#"{"version":1,"jobs":[]}"#).expect("write original");

        atomic_write(&path, br#"{"version":1,"jobs":[{"id":"new"}]}"#).expect("atomic write");

        let content = std::fs::read_to_string(&path).expect("read replacement");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("complete json");
        assert_eq!(parsed["jobs"][0]["id"], "new");
        std::fs::remove_file(path).expect("cleanup file");
    }
}
