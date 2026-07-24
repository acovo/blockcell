use anyhow::Context;
use blockcell_core::file_store::{atomic_write, ExclusiveFileLock};
use serde::{de::DeserializeOwned, Serialize};
use std::path::{Path, PathBuf};

pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read JSON store {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON store {}", path.display()))
}

pub(crate) fn update_json<T, R, D, F>(path: &Path, default: D, mutate: F) -> anyhow::Result<R>
where
    T: DeserializeOwned + Serialize,
    D: FnOnce() -> T,
    F: FnOnce(&mut T) -> anyhow::Result<R>,
{
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _lock = ExclusiveFileLock::acquire(&lock_path(path))
        .with_context(|| format!("Failed to lock JSON store {}", path.display()))?;
    let mut store = if path.exists() {
        read_json(path)?
    } else {
        default()
    };
    let result = mutate(&mut store)?;
    let content = serde_json::to_vec_pretty(&store)?;
    atomic_write(path, &content)
        .with_context(|| format!("Failed to atomically write JSON store {}", path.display()))?;
    Ok(result)
}

fn lock_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("store.json");
    path.with_file_name(format!(".{file_name}.lock"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::sync::{Arc, Barrier};

    fn temp_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "blockcell-cli-json-{name}-{}-{}.json",
            std::process::id(),
            uuid::Uuid::new_v4()
        ))
    }

    #[test]
    fn corrupt_json_is_not_replaced() {
        let path = temp_file("corrupt");
        std::fs::write(&path, "{broken").expect("write corrupt json");

        let result = update_json(&path, || json!({}), |_store: &mut Value| Ok(()));

        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{broken");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn concurrent_updates_do_not_lose_changes() {
        let path = temp_file("concurrent");
        let barrier = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();

        for key in ["first", "second"] {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                update_json(
                    &path,
                    || json!({}),
                    |store: &mut Value| {
                        store[key] = json!(true);
                        Ok(())
                    },
                )
                .expect("update json");
            }));
        }

        barrier.wait();
        for handle in handles {
            handle.join().expect("join update");
        }

        let stored: Value = read_json(&path).expect("read updated json");
        assert_eq!(stored["first"], json!(true));
        assert_eq!(stored["second"], json!(true));
        let _ = std::fs::remove_file(&path);
    }
}
