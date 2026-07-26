use blockcell_agent::system_event_store::{InMemorySystemEventStore, SystemEventStoreOps};
use blockcell_core::system_event::{EventPriority, EventScope, SystemEvent};

fn build_event(id: &str, created_at_ms: i64, dedup_key: Option<&str>) -> SystemEvent {
    let mut event = SystemEvent::new_main_session(
        "task.completed",
        "task_manager",
        EventPriority::Normal,
        format!("event-{id}"),
        format!("summary-{id}"),
    );
    event.id = id.to_string();
    event.created_at_ms = created_at_ms;
    event.dedup_key = dedup_key.map(str::to_string);
    event
}

#[test]
fn system_event_store_lists_pending_in_created_order() {
    let store = InMemorySystemEventStore::default();
    let first = build_event("evt-1", 100, None);
    let second = build_event("evt-2", 200, None);

    store.emit(second);
    store.emit(first);

    let pending = store.list_pending(10);
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].id, "evt-1");
    assert_eq!(pending[1].id, "evt-2");
}

#[test]
fn system_event_store_marks_delivered_and_acked() {
    let store = InMemorySystemEventStore::default();
    let first = build_event("evt-1", 100, None);
    let second = build_event("evt-2", 200, None);

    store.emit(first);
    store.emit(second);
    store.mark_delivered(&["evt-1".to_string()]);
    store.mark_acked(&["evt-2".to_string()]);

    let pending = store.list_pending(10);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "evt-2");

    let recent = store.list_recent(&EventScope::MainSession, 10);
    assert!(recent
        .iter()
        .any(|event| event.id == "evt-1" && event.delivered));
    assert!(recent
        .iter()
        .any(|event| event.id == "evt-2" && event.acked));
}

#[test]
fn system_event_store_dedups_matching_pending_keys() {
    let store = InMemorySystemEventStore::default();
    let first = build_event("evt-1", 100, Some("task:alpha"));
    let replacement = build_event("evt-2", 200, Some("task:alpha"));

    store.emit(first);
    store.dedup_or_merge(replacement);

    let pending = store.list_pending(10);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "evt-2");
    assert_eq!(pending[0].dedup_key.as_deref(), Some("task:alpha"));
}

#[test]
fn system_event_store_restores_only_persistent_events_after_restart() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("system_events.json");
    let store = InMemorySystemEventStore::with_persistence(path.clone()).unwrap();
    let persistent = build_event("persistent", 100, None);
    let mut transient = build_event("transient", 200, None);
    transient.delivery.persist = false;

    store.emit(persistent);
    store.emit(transient);
    drop(store);

    let restored = InMemorySystemEventStore::with_persistence(path).unwrap();
    let pending = restored.list_pending(10);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "persistent");
}
