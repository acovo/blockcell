use blockcell_core::system_event::{
    DeliveryPolicy, EventPriority, EventScope, SessionSummary, SummaryCategory, SummaryItem,
    SummaryScope, SystemEvent,
};

#[test]
fn system_event_delivery_policy_helpers_have_expected_defaults() {
    let default_policy = DeliveryPolicy::default();
    assert!(!default_policy.immediate);
    assert!(default_policy.include_in_summary);
    assert!(default_policy.notify_user);
    assert!(default_policy.persist);
    assert_eq!(default_policy.max_delay_seconds, None);

    let critical = DeliveryPolicy::critical();
    assert!(critical.immediate);
    assert!(critical.include_in_summary);

    let silent = DeliveryPolicy::silent();
    assert!(!silent.immediate);
    assert!(!silent.include_in_summary);
    assert!(!silent.notify_user);
}

#[test]
fn system_event_new_main_session_helper_uses_main_session_defaults() {
    let event = SystemEvent::new_main_session(
        "task.completed",
        "task_manager",
        EventPriority::Normal,
        "Task finished",
        "Background task finished successfully",
    );

    assert_eq!(event.kind, "task.completed");
    assert_eq!(event.source, "task_manager");
    assert_eq!(event.scope, EventScope::MainSession);
    assert_eq!(event.priority, EventPriority::Normal);
    assert_eq!(event.title, "Task finished");
    assert_eq!(event.summary, "Background task finished successfully");
    assert!(event.delivery.include_in_summary);
    assert!(!event.delivered);
    assert!(!event.acked);
}

#[test]
fn event_priority_orders_and_roundtrips_over_json() {
    assert!(EventPriority::Critical > EventPriority::High);
    assert!(EventPriority::High > EventPriority::Normal);
    assert!(EventPriority::Normal > EventPriority::Low);

    let json = serde_json::to_string(&EventPriority::Critical).expect("serialize priority");
    let restored: EventPriority = serde_json::from_str(&json).expect("deserialize priority");
    assert_eq!(restored, EventPriority::Critical);
}

#[test]
fn summary_models_serialize_for_main_session() {
    let item = SummaryItem {
        id: "sum-1".to_string(),
        scope: SummaryScope::MainSession,
        category: SummaryCategory::Task,
        title: "1 task completed".to_string(),
        body: "The nightly report task finished.".to_string(),
        source_event_ids: vec!["evt-1".to_string()],
        created_at_ms: 1_700_000_000_000,
        priority: EventPriority::Normal,
        merge_key: Some("task:report".to_string()),
        persist: true,
    };
    let summary = SessionSummary {
        title: "System updates".to_string(),
        items: vec![item],
        compact_text: "后台任务更新：1 个任务已完成".to_string(),
    };

    let json = serde_json::to_value(&summary).expect("serialize summary");
    assert_eq!(json["items"][0]["scope"], "main_session");
    assert_eq!(json["items"][0]["category"], "task");
}
