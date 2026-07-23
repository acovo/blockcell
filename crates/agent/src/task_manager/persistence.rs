//! TaskManager 的磁盘持久化与回收逻辑。
//!
//! 任务的落盘、重启恢复、文件清理、后台回收循环从 `task_manager.rs` 抽出，
//! 作为 `TaskManager` 的独立 impl 块（子模块可访问父类型私有成员）。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use super::{is_terminal_status, TaskInfo, TaskManager, TaskStatus};

impl TaskManager {
    /// 任务持久化目录
    pub(super) fn tasks_dir(workspace_dir: &Path) -> PathBuf {
        workspace_dir.join(".blockcell").join("tasks")
    }

    /// 持久化单个任务到 JSON 文件
    ///
    /// 在任务状态变更时调用：
    /// - 创建任务时
    /// - 状态变为 Running/Completed/Failed/Cancelled 时
    pub(super) async fn persist_task_to_disk(&self, workspace_dir: &Path, task: &TaskInfo) {
        let tasks_dir = Self::tasks_dir(workspace_dir);

        // 确保目录存在
        if let Err(e) = tokio::fs::create_dir_all(&tasks_dir).await {
            tracing::warn!(path = %tasks_dir.display(), error = %e, "Failed to create tasks dir");
            return;
        }

        let file_path = tasks_dir.join(format!("{}.json", task.id));
        let content = serde_json::to_string_pretty(task);

        match content {
            Ok(json) => {
                let write_path = file_path.clone();
                let write_result = tokio::task::spawn_blocking(move || {
                    crate::fs_util::atomic_write(&write_path, json.as_bytes())
                })
                .await;
                match write_result {
                    Ok(Ok(())) => {
                        tracing::debug!(task_id = %task.id, path = %file_path.display(), "Task persisted to disk");
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(task_id = %task.id, path = %file_path.display(), error = %e, "Failed to persist task to disk");
                    }
                    Err(e) => {
                        tracing::warn!(task_id = %task.id, path = %file_path.display(), error = %e, "Task persistence worker failed");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(task_id = %task.id, path = %file_path.display(), error = %e, "Failed to serialize task");
            }
        }
    }

    /// 从磁盘恢复未完成的任务
    ///
    /// agent 和 gateway 启动时都应调用。
    /// 只恢复未达到终止状态的任务，恢复为 Queued 状态。
    /// 限制最大恢复文件数，防止目录异常导致 OOM 或启动过慢。
    pub async fn restore_from_disk(&self, workspace_dir: &Path) -> usize {
        /// 最大恢复文件数限制
        const MAX_RESTORE_FILES: usize = 1000;

        let tasks_dir = Self::tasks_dir(workspace_dir);
        let mut count = 0;
        let mut total_scanned = 0;

        // 目录不存在则跳过
        if !tokio::fs::try_exists(&tasks_dir).await.unwrap_or(false) {
            return 0;
        }

        let mut entries = match tokio::fs::read_dir(&tasks_dir).await {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(path = %tasks_dir.display(), error = %e, "Failed to read persisted tasks directory");
                return 0;
            }
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            total_scanned += 1;
            if total_scanned > MAX_RESTORE_FILES {
                tracing::warn!(
                    limit = MAX_RESTORE_FILES,
                    "恢复文件数超过限制，跳过剩余文件"
                );
                break;
            }

            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) => match serde_json::from_str::<TaskInfo>(&content) {
                        Ok(task) => {
                            // 只恢复未完成的任务
                            if !is_terminal_status(&task.status) {
                                // 标记为 Failed 而非 Queued，因为没有机制重新执行恢复的任务
                                // 避免僵尸任务永远停留在 Queued 状态
                                let mut restored_task = task.clone();
                                restored_task.status = TaskStatus::Failed;
                                restored_task.started_at = None;
                                restored_task.completed_at = Some(Utc::now());
                                restored_task.progress = None;
                                restored_task.result = None;
                                restored_task.error = Some(
                                "Task restored from disk after restart; not re-executed automatically".to_string()
                            );

                                let restored_task_for_persist = restored_task.clone();
                                self.tasks
                                    .lock()
                                    .await
                                    .insert(task.id.clone(), restored_task);
                                self.persist_task_to_disk(
                                    workspace_dir,
                                    &restored_task_for_persist,
                                )
                                .await;
                                count += 1;

                                tracing::info!(
                                    task_id = %task.id,
                                    agent_type = ?task.agent_type,
                                    "Restored unfinished task"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!(path = %path.display(), error = %e, "Skipping invalid persisted task")
                        }
                    },
                    Err(e) => {
                        tracing::warn!(path = %path.display(), error = %e, "Failed to read persisted task")
                    }
                }
            }
        }

        if count > 0 {
            tracing::info!(count = count, "Restored unfinished tasks from disk");
        }
        count
    }

    /// 清理已完成的任务文件
    pub async fn cleanup_task_file(&self, workspace_dir: &Path, task_id: &str) {
        let file_path = Self::tasks_dir(workspace_dir).join(format!("{}.json", task_id));
        if tokio::fs::remove_file(&file_path).await.is_ok() {
            tracing::debug!(task_id = %task_id, "Cleaned up task file");
        }
    }

    /// 启动定期清理循环
    ///
    /// 每 60 秒清理 evict_after 已过期的任务，同时删除对应的 JSON 文件。
    /// agent 和 gateway 启动时都应调用。
    ///
    /// # Example
    /// ```rust,ignore
    /// let task_manager = Arc::new(TaskManager::new());
    /// let workspace_dir = paths.workspace();
    /// let cleanup_shutdown_rx = shutdown_tx.subscribe();
    /// task_manager.clone().spawn_cleanup_loop(&workspace_dir, cleanup_shutdown_rx);
    /// ```
    pub fn spawn_cleanup_loop(
        self: Arc<Self>,
        workspace_dir: &Path,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> JoinHandle<()> {
        let workspace_dir = workspace_dir.to_path_buf();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        self.cleanup_evicted_tasks(&workspace_dir).await;
                        tracing::debug!("Completed eviction cleanup cycle");
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("cleanup_loop shutting down");
                        break;
                    }
                }
            }
        })
    }

    /// 清理过期任务（evict_after 已过）
    ///
    /// 同时清理对应的 JSON 持久化文件。
    pub async fn cleanup_evicted_tasks(&self, workspace_dir: &Path) {
        let now = Utc::now();
        let evicted_ids: Vec<String> = {
            let mut tasks = self.tasks.lock().await;
            let ids: Vec<String> = tasks
                .iter()
                .filter(|(_, t)| {
                    is_terminal_status(&t.status) && t.evict_after.is_some_and(|dt| dt <= now)
                })
                .map(|(id, _)| id.clone())
                .collect();
            // 从内存中移除
            for id in &ids {
                tasks.remove(id);
            }
            ids
        };

        // 清理对应的 message_queues 条目，防止内存泄漏
        if !evicted_ids.is_empty() {
            let mut queues = match self.message_queues.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            for id in &evicted_ids {
                queues.remove(id);
            }
        }
        if !evicted_ids.is_empty() {
            let mut tokens = match self.abort_tokens.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            for id in &evicted_ids {
                tokens.remove(id);
            }
        }

        // 清理对应的 JSON 文件
        for task_id in evicted_ids {
            self.cleanup_task_file(workspace_dir, &task_id).await;
        }
    }
}
