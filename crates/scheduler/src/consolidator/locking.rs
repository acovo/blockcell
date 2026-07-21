use super::*;

impl DreamConsolidator {
    /// Atomically acquires the process-wide Dream lock.
    pub(crate) async fn acquire_lock(&self) -> Result<(), DreamError> {
        let lock_path = self.config_dir.join(LOCK_FILE_NAME);
        let guard = match ExclusiveFileLock::try_acquire(&lock_path) {
            Ok(guard) => guard,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                return Err(DreamError::LockAcquired)
            }
            Err(error) => return Err(DreamError::Io(error)),
        };

        let mut slot = self
            .dream_lock
            .lock()
            .map_err(|_| DreamError::Io(std::io::Error::other("dream lock mutex poisoned")))?;
        if slot.is_some() {
            return Err(DreamError::LockAcquired);
        }
        *slot = Some(guard);
        Ok(())
    }

    /// Releases the Dream lock only when this consolidator still owns its token.
    pub(crate) async fn release_lock(&self) -> Result<(), DreamError> {
        let mut guard = self
            .dream_lock
            .lock()
            .map_err(|_| DreamError::Io(std::io::Error::other("dream lock mutex poisoned")))?
            .take();
        if let Some(lock) = guard.as_mut() {
            lock.release()?;
        }
        Ok(())
    }

    /// 阶段 1: 定位现有内容
    pub(crate) async fn orient(&self) -> Result<(), DreamError> {
        tracing::debug!("[dream] Phase 1: Orienting");
        let memory_dir = self.config_dir.join("memory");
        if !fs::try_exists(&memory_dir).await? {
            fs::create_dir_all(&memory_dir).await?;
        }
        Ok(())
    }
}
