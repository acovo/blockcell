use blockcell_core::{Error, Result};
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

pub(crate) struct FileOwnerLock {
    path: PathBuf,
    owner: String,
}

impl FileOwnerLock {
    pub(crate) fn acquire(root: &Path, namespace: &str, key: &str) -> Result<Self> {
        let lock_dir = root.join(".locks");
        fs::create_dir_all(&lock_dir)?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        let path = lock_dir.join(format!("{}-{:016x}.lock", namespace, hasher.finish()));
        let owner = format!("{}:{}", std::process::id(), uuid::Uuid::new_v4());
        let deadline = Instant::now() + Duration::from_secs(10);

        loop {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    file.write_all(owner.as_bytes())?;
                    file.sync_all()?;
                    return Ok(Self { path, owner });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let stale = fs::metadata(&path)
                        .and_then(|metadata| metadata.modified())
                        .ok()
                        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                        .is_some_and(|age| age > Duration::from_secs(15 * 60));
                    if stale {
                        let _ = fs::remove_file(&path);
                        continue;
                    }
                    if Instant::now() >= deadline {
                        return Err(Error::Other(format!(
                            "Timed out acquiring file lock: {}",
                            path.display()
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

impl Drop for FileOwnerLock {
    fn drop(&mut self) {
        if fs::read_to_string(&self.path).ok().as_deref() == Some(self.owner.as_str()) {
            let _ = fs::remove_file(&self.path);
        }
    }
}
