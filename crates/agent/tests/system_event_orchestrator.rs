use blockcell_agent::summary_queue::MainSessionSummaryQueue;
use blockcell_agent::system_event_orchestrator::SystemEventOrchestrator;
use blockcell_agent::system_event_store::{InMemorySystemEventStore, SystemEventStoreOps};
use blockcell_core::system_event::{DeliveryPolicy, EventPriority, SystemEvent};

fn build_event(id: &str, priority: EventPriority) -> SystemEvent {
    let mut event = SystemEvent::new_main_session(
        "task.completed",
        "task_manager",
        priority,
        format!("title-{id}"),
        format!("summary-{id}"),
    );
    event.id = id.to_string();
    event
}

#[test]
fn orchestrator_sends_critical_events_as_immediate_notifications() {
    let store = InMemorySystemEventStore::default();
    let queue = MainSessionSummaryQueue::with_policy(5, 30_000);
    let orchestrator = SystemEventOrchestrator::new(store.clone(), queue.clone());

    let mut event = build_event("critical", EventPriority::Critical);
    event.delivery = DeliveryPolicy::critical();
    store.emit(event.clone());

    let decision = orchestrator.process_tick(1_000);

    assert_eq!(decision.immediate_notifications.len(), 1);
    assert_eq!(decision.immediate_notifications[0].title, event.title);
    assert!(!decision.ack_event_ids.contains(&event.id));
    assert_eq!(queue.snapshot().pending_count, 1);
    assert_eq!(store.list_pending(10).len(), 1);
}

#[test]
fn orchestrator_enqueues_normal_events_for_summary() {
    let store = InMemorySystemEventStore::default();
    let queue = MainSessionSummaryQueue::with_policy(5, 30_000);
    let orchestrator = SystemEventOrchestrator::new(store.clone(), queue.clone());

    let event = build_event("normal", EventPriority::Normal);
    store.emit(event.clone());

    let decision = orchestrator.process_tick(1_000);

    assert!(decision.immediate_notifications.is_empty());
    assert_eq!(decision.summary_updates.len(), 1);
    assert_eq!(queue.snapshot().pending_count, 1);
    assert!(!decision.ack_event_ids.contains(&event.id));
}

#[test]
fn orchestrator_keeps_low_events_silent_by_default() {
    let store = InMemorySystemEventStore::default();
    let queue = MainSessionSummaryQueue::with_policy(5, 30_000);
    let orchestrator = SystemEventOrchestrator::new(store.clone(), queue.clone());

    let event = build_event("low", EventPriority::Low);
    store.emit(event.clone());

    let decision = orchestrator.process_tick(1_000);

    assert!(decision.immediate_notifications.is_empty());
    assert!(decision.summary_updates.is_empty());
    assert!(decision.ack_event_ids.contains(&event.id));
    assert_eq!(queue.snapshot().pending_count, 0);
    assert_eq!(store.list_pending(10).len(), 1);
}

#[test]
fn orchestrator_only_returns_immediately_ackable_event_ids() {
    let store = InMemorySystemEventStore::default();
    let queue = MainSessionSummaryQueue::with_policy(5, 30_000);
    let orchestrator = SystemEventOrchestrator::new(store.clone(), queue.clone());

    let first = build_event("one", EventPriority::Normal);
    let second = build_event("two", EventPriority::Critical);
    store.emit(first.clone());
    store.emit(second.clone());

    let decision = orchestrator.process_tick(1_000);

    assert!(decision.ack_event_ids.is_empty());
}

#[test]
fn orchestrator_forces_notification_at_event_max_delay() {
    let store = InMemorySystemEventStore::default();
    let queue = MainSessionSummaryQueue::with_policy(5, 30_000);
    let orchestrator = SystemEventOrchestrator::new(store.clone(), queue);

    let mut event = build_event("deadline", EventPriority::Normal);
    event.created_at_ms = 1_000;
    event.delivery.max_delay_seconds = Some(1);
    store.emit(event);

    assert!(orchestrator
        .process_tick(1_999)
        .immediate_notifications
        .is_empty());
    assert_eq!(
        orchestrator
            .process_tick(2_000)
            .immediate_notifications
            .len(),
        1
    );
}

#[test]
fn orchestrator_does_not_persist_summary_for_transient_event() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("summary_queue.json");
    let store = InMemorySystemEventStore::default();
    let queue = MainSessionSummaryQueue::with_persistence(5, 30_000, path.clone()).unwrap();
    let orchestrator = SystemEventOrchestrator::new(store.clone(), queue);

    let mut event = build_event("transient", EventPriority::Normal);
    event.delivery.persist = false;
    store.emit(event);
    orchestrator.process_tick(1_000);
    drop(orchestrator);

    let restored = MainSessionSummaryQueue::with_persistence(5, 30_000, path).unwrap();
    assert_eq!(restored.snapshot().pending_count, 0);
}
