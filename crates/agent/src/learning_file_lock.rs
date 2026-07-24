use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct OwnerAwareFileLock {
    path: PathBuf,
    owner: String,
}

impl OwnerAwareFileLock {
    pub fn acquire(path: &Path) -> Result<Self> {
        Self::acquire_with_timeout(path, Duration::from_secs(10))
    }

    pub fn acquire_with_timeout(path: &Path, timeout: Duration) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let deadline = Instant::now() + timeout;
        loop {
            match fs::create_dir(path) {
                Ok(()) => {
                    let owner = format!("{} {}", std::process::id(), uuid::Uuid::new_v4());
                    if let Err(error) = fs::write(path.join("owner.pid"), &owner) {
                        let _ = fs::remove_dir_all(path);
                        return Err(error);
                    }
                    sync_parent(path);
                    return Ok(Self {
                        path: path.to_path_buf(),
                        owner,
                    });
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    if owner_is_dead(path) {
                        match fs::remove_dir_all(path) {
                            Ok(()) => {
                                sync_parent(path);
                                continue;
                            }
                            Err(remove_error) if remove_error.kind() == ErrorKind::NotFound => {
                                continue;
                            }
                            Err(remove_error) => return Err(remove_error),
                        }
                    }
                    if Instant::now() >= deadline {
                        return Err(Error::new(
                            ErrorKind::WouldBlock,
                            format!(
                                "timed out waiting for learning file lock: {}",
                                path.display()
                            ),
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for OwnerAwareFileLock {
    fn drop(&mut self) {
        let owner_path = self.path.join("owner.pid");
        if fs::read_to_string(&owner_path).ok().as_deref() == Some(self.owner.as_str()) {
            let _ = fs::remove_dir_all(&self.path);
            sync_parent(&self.path);
        }
    }
}

fn owner_is_dead(path: &Path) -> bool {
    let Ok(owner) = fs::read_to_string(path.join("owner.pid")) else {
        return false;
    };
    let Some(pid) = owner
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<u32>().ok())
    else {
        return false;
    };
    !is_pid_alive(pid)
}

fn is_pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        true
    }
}

fn sync_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn recovers_lock_directory_owned_by_dead_pid() {
        let root = std::env::temp_dir().join(format!(
            "blockcell-dead-learning-lock-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("owner.pid"), "999999").unwrap();

        let guard = OwnerAwareFileLock::acquire_with_timeout(&root, Duration::from_millis(50))
            .expect("dead owner lock should be recovered");
        assert!(std::fs::read_to_string(root.join("owner.pid"))
            .unwrap()
            .starts_with(&std::process::id().to_string()));
        drop(guard);
        assert!(!root.exists());
    }

    #[test]
    fn refuses_to_steal_lock_directory_owned_by_live_pid() {
        let root = std::env::temp_dir().join(format!(
            "blockcell-live-learning-lock-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("owner.pid"), std::process::id().to_string()).unwrap();

        let error = OwnerAwareFileLock::acquire_with_timeout(&root, Duration::from_millis(20))
            .expect_err("live owner lock must not be stolen");
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
        assert!(root.exists());

        std::fs::remove_dir_all(root).unwrap();
    }
}
