use blockcell_core::{BudgetConfig, BudgetTracker};

#[test]
fn record_usage_returns_snapshot_with_remaining_budget() {
    let tracker = BudgetTracker::new(&BudgetConfig {
        max_tokens_per_session: 1_000,
        max_cost_cents_per_session: 250,
        warning_threshold: 0.8,
    });

    let snapshot = tracker.record_usage(300, 100, 750_000);

    assert_eq!(snapshot.input_tokens_used, 300);
    assert_eq!(snapshot.output_tokens_used, 100);
    assert_eq!(snapshot.total_tokens_used, 400);
    assert_eq!(snapshot.cost_used_micro_usd, 750_000);
    assert_eq!(snapshot.tokens_remaining, 600);
    assert_eq!(snapshot.cost_remaining_micro_usd, 1_750_000);
    assert!((snapshot.usage_ratio - 0.4).abs() < f64::EPSILON);
    assert!(!tracker.is_exhausted());
}

#[test]
fn zero_limits_are_unlimited() {
    let tracker = BudgetTracker::new(&BudgetConfig::default());

    let snapshot = tracker.record_usage(10_000, 20_000, 9_999);

    assert_eq!(snapshot.total_tokens_used, 30_000);
    assert_eq!(snapshot.tokens_remaining, 0);
    assert_eq!(snapshot.cost_remaining_micro_usd, 0);
    assert_eq!(snapshot.usage_ratio, 0.0);
    assert!(!tracker.is_exhausted());
    assert!(tracker.check_budget().is_ok());
}

#[test]
fn check_budget_errors_when_token_limit_is_reached() {
    let tracker = BudgetTracker::new(&BudgetConfig {
        max_tokens_per_session: 150,
        max_cost_cents_per_session: 0,
        warning_threshold: 0.8,
    });

    tracker.record_usage(100, 50, 0);
    let error = tracker
        .check_budget()
        .expect_err("budget should be exhausted");

    assert!(error.message.contains("tokens: 150/150"));
    assert_eq!(error.snapshot.total_tokens_used, 150);
    assert_eq!(error.snapshot.tokens_remaining, 0);
}

#[test]
fn check_budget_errors_when_cost_limit_is_reached() {
    let tracker = BudgetTracker::new(&BudgetConfig {
        max_tokens_per_session: 0,
        max_cost_cents_per_session: 25,
        warning_threshold: 0.8,
    });

    tracker.record_usage(10, 20, 250_000);
    let error = tracker
        .check_budget()
        .expect_err("cost budget should be exhausted");

    assert!(error.message.contains("cost: $0.25/$0.25"));
    assert_eq!(error.snapshot.cost_used_micro_usd, 250_000);
    assert_eq!(error.snapshot.cost_remaining_micro_usd, 0);
}

#[test]
fn sub_cent_costs_accumulate_against_cent_budget() {
    let tracker = BudgetTracker::new(&BudgetConfig {
        max_tokens_per_session: 0,
        max_cost_cents_per_session: 1,
        warning_threshold: 0.8,
    });

    tracker.record_usage(0, 0, 3_000);
    tracker.record_usage(0, 0, 3_000);
    let snapshot = tracker.record_usage(0, 0, 3_000);

    assert_eq!(snapshot.cost_used_micro_usd, 9_000);
    assert_eq!(snapshot.cost_remaining_micro_usd, 1_000);
    assert!(!tracker.is_exhausted());

    tracker.record_usage(0, 0, 3_000);
    let error = tracker
        .check_budget()
        .expect_err("sub-cent costs should cumulatively exhaust budget");
    assert_eq!(error.snapshot.cost_used_micro_usd, 12_000);
    assert_eq!(error.snapshot.cost_remaining_micro_usd, 0);
}

#[test]
fn warning_triggers_above_threshold_before_exhaustion() {
    let tracker = BudgetTracker::new(&BudgetConfig {
        max_tokens_per_session: 100,
        max_cost_cents_per_session: 0,
        warning_threshold: 0.75,
    });

    tracker.record_usage(60, 15, 0);
    assert!(tracker.should_warn());

    tracker.record_usage(25, 0, 0);
    assert!(tracker.is_exhausted());
    assert!(!tracker.should_warn());
}

#[test]
fn reset_clears_accumulated_usage() {
    let tracker = BudgetTracker::new(&BudgetConfig {
        max_tokens_per_session: 100,
        max_cost_cents_per_session: 100,
        warning_threshold: 0.8,
    });

    tracker.record_usage(40, 30, 900_000);
    tracker.reset();

    let snapshot = tracker.snapshot();
    assert_eq!(snapshot.total_tokens_used, 0);
    assert_eq!(snapshot.cost_used_micro_usd, 0);
    assert_eq!(snapshot.tokens_remaining, 100);
    assert_eq!(snapshot.cost_remaining_micro_usd, 1_000_000);
}

#[test]
fn current_context_window_is_separate_from_cumulative_usage() {
    let tracker = BudgetTracker::new(&BudgetConfig::default());
    tracker.record_usage(1_000, 200, 0);
    tracker.record_context_window(750, 1_000, 800);

    let context = tracker.context_window_snapshot();
    assert_eq!(context.tokens_used, 750);
    assert_eq!(context.token_limit, 1_000);
    assert_eq!(context.tokens_remaining, 250);
    assert_eq!(context.compact_threshold_tokens, 800);
    assert_eq!(tracker.snapshot().total_tokens_used, 1_200);
}
