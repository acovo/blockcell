use blockcell_agent::summary_queue::MainSessionSummaryQueue;
use blockcell_core::system_event::{EventPriority, SummaryCategory, SummaryItem, SummaryScope};

fn build_item(id: &str, created_at_ms: i64, merge_key: Option<&str>, title: &str) -> SummaryItem {
    SummaryItem {
        id: id.to_string(),
        scope: SummaryScope::MainSession,
        category: SummaryCategory::Task,
        title: title.to_string(),
        body: format!("body-{id}"),
        source_event_ids: vec![format!("evt-{id}")],
        created_at_ms,
        priority: EventPriority::Normal,
        merge_key: merge_key.map(str::to_string),
        persist: true,
    }
}

#[test]
fn summary_queue_enqueues_items_for_main_session() {
    let queue = MainSessionSummaryQueue::with_policy(3, 30_000);
    queue.enqueue(build_item("one", 100, None, "Task one finished"));

    let snapshot = queue.snapshot();
    assert_eq!(snapshot.pending_count, 1);
    assert_eq!(snapshot.items[0].title, "Task one finished");
}

#[test]
fn summary_queue_merges_items_with_same_merge_key() {
    let queue = MainSessionSummaryQueue::with_policy(3, 30_000);
    queue.enqueue(build_item(
        "one",
        100,
        Some("task:alpha"),
        "Task alpha running",
    ));
    queue.enqueue(build_item(
        "two",
        200,
        Some("task:alpha"),
        "Task alpha completed",
    ));

    let snapshot = queue.snapshot();
    assert_eq!(snapshot.pending_count, 1);
    assert_eq!(snapshot.items[0].title, "Task alpha completed");
    assert_eq!(snapshot.items[0].source_event_ids.len(), 2);
}

#[test]
fn summary_queue_keeps_final_task_state_for_same_merge_key() {
    let queue = MainSessionSummaryQueue::with_policy(3, 30_000);
    queue.enqueue(build_item(
        "queued",
        100,
        Some("task:lifecycle"),
        "Task queued",
    ));
    queue.enqueue(build_item(
        "running",
        200,
        Some("task:lifecycle"),
        "Task running",
    ));
    queue.enqueue(build_item(
        "done",
        300,
        Some("task:lifecycle"),
        "Task completed",
    ));

    let snapshot = queue.snapshot();
    assert_eq!(snapshot.pending_count, 1);
    assert_eq!(snapshot.items[0].title, "Task completed");
}

#[test]
fn summary_queue_flushes_due_items_by_count_or_age() {
    let by_count = MainSessionSummaryQueue::with_policy(2, 30_000);
    by_count.enqueue(build_item("one", 100, None, "One"));
    by_count.enqueue(build_item("two", 200, None, "Two"));
    let flushed = by_count.flush_due_items(250);
    assert_eq!(flushed.len(), 2);
    assert_eq!(by_count.snapshot().pending_count, 2);
    by_count.acknowledge_items(
        &flushed
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>(),
    );
    assert_eq!(by_count.snapshot().pending_count, 0);

    let by_age = MainSessionSummaryQueue::with_policy(5, 1_000);
    by_age.enqueue(build_item("old", 100, None, "Old"));
    let flushed = by_age.flush_due_items(1_500);
    assert_eq!(flushed.len(), 1);
    assert_eq!(flushed[0].title, "Old");
}

#[test]
fn summary_queue_restores_pending_items_after_restart() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("summary_queue.json");
    let queue = MainSessionSummaryQueue::with_persistence(5, 30_000, path.clone()).unwrap();
    queue.enqueue(build_item("persistent", 100, None, "Persistent summary"));
    drop(queue);

    let restored = MainSessionSummaryQueue::with_persistence(5, 30_000, path).unwrap();
    let snapshot = restored.snapshot();
    assert_eq!(snapshot.pending_count, 1);
    assert_eq!(snapshot.items[0].title, "Persistent summary");
}
