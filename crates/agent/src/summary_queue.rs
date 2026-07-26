use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use blockcell_core::system_event::{
    SessionSummary, SummaryCategory, SummaryItem, SummaryScope, SystemEvent,
};
use uuid::Uuid;

/// 安全获取锁，处理锁中毒情况
///
/// 如果锁中毒（持有锁的线程 panic），会恢复并返回内部状态。
/// 这是安全的，因为 SummaryQueue 的数据可以重建。
fn get_lock<T>(lock: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!("[summary_queue] Lock poisoned, recovering");
            poisoned.into_inner()
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SummaryQueueSnapshot {
    pub pending_count: usize,
    pub items: Vec<SummaryItem>,
}

#[derive(Clone)]
pub struct MainSessionSummaryQueue {
    items: Arc<Mutex<Vec<SummaryItem>>>,
    max_items_before_flush: usize,
    max_age_ms: i64,
    persistence_path: Option<Arc<PathBuf>>,
}

impl MainSessionSummaryQueue {
    pub fn with_policy(max_items_before_flush: usize, max_age_ms: i64) -> Self {
        Self {
            items: Arc::new(Mutex::new(Vec::new())),
            max_items_before_flush,
            max_age_ms,
            persistence_path: None,
        }
    }

    pub fn with_persistence(
        max_items_before_flush: usize,
        max_age_ms: i64,
        path: PathBuf,
    ) -> std::io::Result<Self> {
        let items = if path.exists() {
            let content = std::fs::read(&path)?;
            serde_json::from_slice(&content).map_err(|error| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
            })?
        } else {
            Vec::new()
        };
        Ok(Self {
            items: Arc::new(Mutex::new(items)),
            max_items_before_flush,
            max_age_ms,
            persistence_path: Some(Arc::new(path)),
        })
    }

    fn persist_locked(&self, items: &[SummaryItem]) {
        let Some(path) = self.persistence_path.as_deref() else {
            return;
        };
        let persistent: Vec<&SummaryItem> = items.iter().filter(|item| item.persist).collect();
        let result = serde_json::to_vec_pretty(&persistent)
            .map_err(std::io::Error::other)
            .and_then(|bytes| blockcell_core::file_store::atomic_write(path, &bytes));
        if let Err(error) = result {
            tracing::warn!(path = %path.display(), error = %error, "Failed to persist summary queue");
        }
    }

    pub fn enqueue(&self, item: SummaryItem) {
        let mut items = get_lock(&self.items);
        if items.iter().any(|existing| {
            existing
                .source_event_ids
                .iter()
                .any(|event_id| item.source_event_ids.contains(event_id))
        }) {
            return;
        }
        if let Some(merge_key) = item.merge_key.as_deref() {
            if let Some(existing) = items.iter_mut().find(|existing| {
                existing.scope == item.scope && existing.merge_key.as_deref() == Some(merge_key)
            }) {
                for source_event_id in item.source_event_ids {
                    if !existing.source_event_ids.contains(&source_event_id) {
                        existing.source_event_ids.push(source_event_id);
                    }
                }
                existing.title = item.title;
                existing.body = item.body;
                existing.created_at_ms = item.created_at_ms;
                existing.priority = existing.priority.max(item.priority);
                existing.category = item.category;
                existing.persist |= item.persist;
                self.persist_locked(&items);
                return;
            }
        }
        items.push(item);
        self.persist_locked(&items);
    }

    pub fn enqueue_event_as_summary_item(&self, event: &SystemEvent) -> SummaryItem {
        let item = SummaryItem {
            id: format!("sum_{}", Uuid::new_v4()),
            scope: match &event.scope {
                blockcell_core::system_event::EventScope::Global
                | blockcell_core::system_event::EventScope::MainSession => {
                    SummaryScope::MainSession
                }
                blockcell_core::system_event::EventScope::Channel { channel, chat_id } => {
                    SummaryScope::Channel {
                        channel: channel.clone(),
                        chat_id: chat_id.clone(),
                    }
                }
                blockcell_core::system_event::EventScope::Session {
                    channel,
                    account_id,
                    chat_id,
                    session_key,
                } => SummaryScope::Session {
                    channel: channel.clone(),
                    account_id: account_id.clone(),
                    chat_id: chat_id.clone(),
                    session_key: session_key.clone(),
                },
            },
            category: category_for_event(event),
            title: event.title.clone(),
            body: event.summary.clone(),
            source_event_ids: vec![event.id.clone()],
            created_at_ms: event.created_at_ms,
            priority: event.priority,
            merge_key: event.dedup_key.clone(),
            persist: event.delivery.persist,
        };
        self.enqueue(item.clone());
        item
    }

    pub fn flush_due_items(&self, now_ms: i64) -> Vec<SummaryItem> {
        let items = get_lock(&self.items);
        if items.is_empty() {
            return Vec::new();
        }

        let oldest_created_at = items
            .iter()
            .map(|item| item.created_at_ms)
            .min()
            .unwrap_or(now_ms);
        let age_due = now_ms.saturating_sub(oldest_created_at) >= self.max_age_ms;
        let count_due = items.len() >= self.max_items_before_flush;

        if !age_due && !count_due {
            return Vec::new();
        }

        let mut flushed = items.clone();
        flushed.sort_by_key(|item| item.created_at_ms);
        flushed
    }

    pub fn acknowledge_items(&self, item_ids: &[String]) {
        let mut items = get_lock(&self.items);
        items.retain(|item| !item_ids.iter().any(|item_id| item_id == &item.id));
        self.persist_locked(&items);
    }

    pub fn snapshot(&self) -> SummaryQueueSnapshot {
        let items = get_lock(&self.items);
        let mut cloned = items.clone();
        cloned.sort_by_key(|item| item.created_at_ms);
        SummaryQueueSnapshot {
            pending_count: cloned.len(),
            items: cloned,
        }
    }

    pub fn build_session_summary(&self, items: Vec<SummaryItem>) -> SessionSummary {
        let compact_text = items
            .iter()
            .map(|item| format!("- {}", item.title))
            .collect::<Vec<_>>()
            .join("\n");
        SessionSummary {
            title: "System updates".to_string(),
            items,
            compact_text,
        }
    }
}

fn category_for_event(event: &SystemEvent) -> SummaryCategory {
    if event.kind.starts_with("task.") {
        SummaryCategory::Task
    } else if event.kind.starts_with("cron.") {
        SummaryCategory::Cron
    } else if event.kind.starts_with("ghost.") {
        SummaryCategory::Ghost
    } else {
        SummaryCategory::System
    }
}
