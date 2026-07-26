use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use blockcell_core::system_event::{EventScope, SystemEvent};
use chrono::Utc;

pub trait SystemEventStoreOps: Send + Sync {
    fn emit(&self, event: SystemEvent);
    fn list_pending(&self, limit: usize) -> Vec<SystemEvent>;
    fn list_recent(&self, scope: &EventScope, limit: usize) -> Vec<SystemEvent>;
    fn mark_delivered(&self, event_ids: &[String]);
    fn mark_acked(&self, event_ids: &[String]);
    fn count_pending(&self) -> usize;
    fn cleanup_expired(&self, max_age_secs: u64) -> usize;
}

/// 安全获取锁，处理锁中毒情况
///
/// 如果锁中毒（持有锁的线程 panic），会恢复并返回内部状态。
/// 这是安全的，因为 SystemEventStore 的数据可以重建。
fn get_lock<T>(lock: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!("[system_event_store] Lock poisoned, recovering");
            poisoned.into_inner()
        }
    }
}

#[derive(Clone)]
pub struct InMemorySystemEventStore {
    events: Arc<Mutex<Vec<SystemEvent>>>,
    persistence_path: Option<Arc<PathBuf>>,
}

impl Default for InMemorySystemEventStore {
    fn default() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            persistence_path: None,
        }
    }
}

impl InMemorySystemEventStore {
    pub fn with_persistence(path: PathBuf) -> std::io::Result<Self> {
        let events = if path.exists() {
            let content = std::fs::read(&path)?;
            serde_json::from_slice(&content).map_err(|error| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
            })?
        } else {
            Vec::new()
        };
        Ok(Self {
            events: Arc::new(Mutex::new(events)),
            persistence_path: Some(Arc::new(path)),
        })
    }

    fn persist_locked(&self, events: &[SystemEvent]) {
        let Some(path) = self.persistence_path.as_deref() else {
            return;
        };
        let persistent: Vec<&SystemEvent> = events
            .iter()
            .filter(|event| event.delivery.persist)
            .collect();
        let result = serde_json::to_vec_pretty(&persistent)
            .map_err(std::io::Error::other)
            .and_then(|bytes| blockcell_core::file_store::atomic_write(path, &bytes));
        if let Err(error) = result {
            tracing::warn!(path = %path.display(), error = %error, "Failed to persist system events");
        }
    }

    pub fn dedup_or_merge(&self, event: SystemEvent) {
        let mut events = get_lock(&self.events);
        if let Some(dedup_key) = event.dedup_key.as_deref() {
            if let Some(existing) = events.iter_mut().find(|existing| {
                !existing.delivered
                    && !existing.acked
                    && existing.dedup_key.as_deref() == Some(dedup_key)
            }) {
                *existing = event;
                self.persist_locked(&events);
                return;
            }
        }
        events.push(event);
        self.persist_locked(&events);
    }
}

impl SystemEventStoreOps for InMemorySystemEventStore {
    fn emit(&self, event: SystemEvent) {
        self.dedup_or_merge(event);
    }

    fn list_pending(&self, limit: usize) -> Vec<SystemEvent> {
        let events = get_lock(&self.events);
        let mut pending: Vec<SystemEvent> = events
            .iter()
            .filter(|event| !event.delivered)
            .cloned()
            .collect();
        pending.sort_by_key(|event| event.created_at_ms);
        pending.truncate(limit);
        pending
    }

    fn list_recent(&self, scope: &EventScope, limit: usize) -> Vec<SystemEvent> {
        let events = get_lock(&self.events);
        let mut recent: Vec<SystemEvent> = events
            .iter()
            .filter(|event| &event.scope == scope)
            .cloned()
            .collect();
        recent.sort_by_key(|right| std::cmp::Reverse(right.created_at_ms));
        recent.truncate(limit);
        recent
    }

    fn mark_delivered(&self, event_ids: &[String]) {
        let mut events = get_lock(&self.events);
        for event in events.iter_mut() {
            if event_ids.iter().any(|event_id| event_id == &event.id) {
                event.delivered = true;
            }
        }
        self.persist_locked(&events);
    }

    fn mark_acked(&self, event_ids: &[String]) {
        let mut events = get_lock(&self.events);
        for event in events.iter_mut() {
            if event_ids.iter().any(|event_id| event_id == &event.id) {
                event.acked = true;
            }
        }
        self.persist_locked(&events);
    }

    fn count_pending(&self) -> usize {
        let events = get_lock(&self.events);
        events.iter().filter(|event| !event.delivered).count()
    }

    fn cleanup_expired(&self, max_age_secs: u64) -> usize {
        let cutoff = Utc::now().timestamp_millis() - (max_age_secs as i64 * 1000);
        let mut events = get_lock(&self.events);
        let before = events.len();
        // 先将过期事件标记为 delivered（释放"资源"——待处理事件是一种逻辑资源）
        // 这样依赖 pending 状态的消费者不会看到已过期但仍 pending 的事件
        for event in events.iter_mut() {
            if event.created_at_ms < cutoff && !event.delivered {
                event.delivered = true;
            }
        }
        // 然后移除已过期且已 delivered 或已 acked 的事件
        // 保留条件：未过期，或过期但仍未投递（需要继续投递）
        events.retain(|event| event.created_at_ms >= cutoff || !event.delivered);
        let removed = before.saturating_sub(events.len());
        self.persist_locked(&events);
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockcell_core::system_event::{DeliveryPolicy, EventPriority};

    fn make_event(id: &str, created_at_ms: i64, delivered: bool) -> SystemEvent {
        SystemEvent {
            id: id.to_string(),
            kind: "test".to_string(),
            source: "test".to_string(),
            scope: EventScope::Global,
            priority: EventPriority::Normal,
            title: "test event".to_string(),
            summary: "test summary".to_string(),
            details: serde_json::Value::Null,
            created_at_ms,
            correlation_id: None,
            dedup_key: None,
            delivery: DeliveryPolicy::default(),
            delivered,
            acked: false,
        }
    }

    /// 测试：cleanup_expired 先标记过期事件为 delivered 再移除
    ///
    /// 验证：
    /// - 过期且未 delivered 的事件先被标记为 delivered（释放资源）
    /// - 然后已 delivered 的过期事件被移除
    /// - 非 delivered 的未过期事件不受影响
    #[test]
    fn test_cleanup_expired_marks_delivered_before_removing() {
        let store = InMemorySystemEventStore::default();
        let now = Utc::now().timestamp_millis();

        // 两个过期事件（一个 delivered，一个未 delivered）
        store.emit(make_event("old-1", now - 200_000, true));
        store.emit(make_event("old-2", now - 200_000, false));
        // 一个未过期事件
        store.emit(make_event("new-1", now, false));

        // 清理前：old-2 仍 pending
        assert_eq!(store.count_pending(), 2); // old-2 + new-1

        // 清理 100 秒前的事件
        let removed = store.cleanup_expired(100);
        // old-1 (delivered+expired) 被移除
        // old-2 (pending+expired) 先被标记为 delivered，然后也被移除
        assert_eq!(removed, 2);
        // new-1 仍 pending
        assert_eq!(store.count_pending(), 1);
    }
}
