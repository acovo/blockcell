use blockcell_core::system_event::{
    EventPriority, EventScope, SessionSummary, SummaryItem, SystemEvent,
};

use crate::summary_queue::{MainSessionSummaryQueue, SummaryQueueSnapshot};
use crate::system_event_store::{InMemorySystemEventStore, SystemEventStoreOps};

#[derive(Debug, Clone)]
pub struct NotificationRequest {
    pub event_id: String,
    pub scope: EventScope,
    pub title: String,
    pub body: String,
    pub priority: EventPriority,
}

#[derive(Debug, Clone)]
pub struct SummaryEnqueueRequest {
    pub event_id: String,
    pub item: SummaryItem,
}

#[derive(Debug, Clone, Default)]
pub struct HeartbeatDecision {
    pub immediate_notifications: Vec<NotificationRequest>,
    pub summary_updates: Vec<SummaryEnqueueRequest>,
    pub flushed_summaries: Vec<SessionSummary>,
    pub emitted_events: Vec<SystemEvent>,
    pub ack_event_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HeartbeatContext {
    pub now_ms: i64,
    pub pending_events: Vec<SystemEvent>,
    pub queue_state: SummaryQueueSnapshot,
}

#[derive(Clone)]
pub struct SystemEventOrchestrator {
    store: InMemorySystemEventStore,
    queue: MainSessionSummaryQueue,
}

impl SystemEventOrchestrator {
    pub fn new(store: InMemorySystemEventStore, queue: MainSessionSummaryQueue) -> Self {
        Self { store, queue }
    }

    pub fn queue(&self) -> &MainSessionSummaryQueue {
        &self.queue
    }

    pub fn store(&self) -> &InMemorySystemEventStore {
        &self.store
    }

    pub fn build_context(&self, now_ms: i64) -> HeartbeatContext {
        HeartbeatContext {
            now_ms,
            pending_events: self.store.list_pending(256),
            queue_state: self.queue.snapshot(),
        }
    }

    pub fn process_tick(&self, now_ms: i64) -> HeartbeatDecision {
        let context = self.build_context(now_ms);
        let mut decision = HeartbeatDecision::default();

        for event in context.pending_events {
            if event.priority == EventPriority::Low || !event.delivery.notify_user {
                decision.ack_event_ids.push(event.id.clone());
                continue;
            }

            if event.priority == EventPriority::Critical || event.delivery.immediate {
                decision.immediate_notifications.push(NotificationRequest {
                    event_id: event.id.clone(),
                    scope: event.scope.clone(),
                    title: event.title.clone(),
                    body: event.summary.clone(),
                    priority: event.priority,
                });
            }

            if event.delivery.include_in_summary {
                let item = self.queue.enqueue_event_as_summary_item(&event);
                decision.summary_updates.push(SummaryEnqueueRequest {
                    event_id: event.id.clone(),
                    item,
                });
            }
        }

        let flushed = self.queue.flush_due_items(now_ms);
        if !flushed.is_empty() {
            let mut scoped_groups: Vec<Vec<SummaryItem>> = Vec::new();
            for item in flushed {
                if let Some(group) = scoped_groups
                    .iter_mut()
                    .find(|group| group.first().is_some_and(|first| first.scope == item.scope))
                {
                    group.push(item);
                } else {
                    scoped_groups.push(vec![item]);
                }
            }
            decision.flushed_summaries.extend(
                scoped_groups
                    .into_iter()
                    .map(|items| self.queue.build_session_summary(items)),
            );
        }

        decision
    }
}
