# Runtime Error Delivery and Provider Accounting Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver terminal runtime errors to external channels and record successful explicit stream completion in the provider pool.

**Architecture:** Keep WebSocket error events on `event_tx`, while routing one account-aware `OutboundMessage` to non-WebSocket external channels from the message-task failure branch. Treat both explicit `StreamChunk::Done` and clean stream closure as successful provider calls, reporting success exactly once before returning.

**Tech Stack:** Rust 2021, Tokio channels, BlockCell AgentRuntime, ProviderPool.

---

### Task 1: Deliver runtime failures to external channels

**Files:**
- Modify: `crates/agent/src/runtime/message_task.rs:210-253`
- Test: `crates/agent/src/runtime/tests.rs`

- [x] **Step 1: Write failing routing tests**

Add tests that call the message-task terminal failure helper with a Telegram message and assert one outbound `❌` reply preserving `account_id`, then call it with a WebSocket message and assert an error event but no outbound reply.

```rust
#[tokio::test]
async fn message_task_failure_delivers_error_to_external_channel() {
    // Build telegram inbound, outbound/event channels, invoke failure delivery.
    // Assert outbound channel/chat/account and terminal error text.
}

#[tokio::test]
async fn message_task_failure_uses_event_only_for_websocket() {
    // Build ws inbound, invoke failure delivery.
    // Assert one `error` event and no outbound message.
}
```

- [x] **Step 2: Run tests and verify RED**

Run: `cargo test -p blockcell-agent message_task_failure_ -- --nocapture`

Expected: FAIL because the failure-delivery helper/behavior does not exist.

- [x] **Step 3: Implement minimal terminal failure delivery**

Extract one async helper in `message_task.rs` that emits the existing error event and sends an account-aware outbound error only when the channel is not `ws`, `cli`, `http`, or `ghost`. Call it from the `process_message` error branch before task/receipt completion.

```rust
async fn deliver_message_task_failure(
    msg: &InboundMessage,
    task_id: &str,
    agent_id: Option<&str>,
    err_msg: &str,
    event_tx: Option<&broadcast::Sender<String>>,
    outbound_tx: Option<&mpsc::Sender<OutboundMessage>>,
) {
    // Emit filtered runtime event, then external outbound error.
}
```

- [x] **Step 4: Run focused tests and verify GREEN**

Run: `cargo test -p blockcell-agent message_task_failure_ -- --nocapture`

Expected: 2 passed, 0 failed.

### Task 2: Report explicit stream completion as provider success

**Files:**
- Modify: `crates/agent/src/runtime/message_dispatch.rs:138-168`
- Test: `crates/agent/src/runtime/tests.rs`

- [x] **Step 1: Write the failing provider accounting test**

Use a streaming provider that emits `StreamChunk::Done`, call `call_llm_with_retry`, and assert `status_summary()[0]` has `success_count == 1` and `fail_count == 0` after a pre-recorded transient failure.

```rust
#[tokio::test]
async fn explicit_stream_done_reports_provider_success() {
    provider_pool.report(0, CallResult::Transient);
    runtime.call_llm_with_retry(/* ... */).await.expect("stream succeeds");
    let status = provider_pool.status_summary();
    assert_eq!(status[0].success_count, 1);
    assert_eq!(status[0].fail_count, 0);
}
```

- [x] **Step 2: Run test and verify RED**

Run: `cargo test -p blockcell-agent explicit_stream_done_reports_provider_success -- --nocapture`

Expected: FAIL with success count 0 and stale fail count 1.

- [x] **Step 3: Implement minimal success report**

Immediately before the `StreamChunk::Done` success return, call:

```rust
self.provider_pool.report(pool_idx, CallResult::Success);
```

- [x] **Step 4: Run focused test and verify GREEN**

Run: `cargo test -p blockcell-agent explicit_stream_done_reports_provider_success -- --nocapture`

Expected: 1 passed, 0 failed.

### Task 3: Verify, document, and commit

**Files:**
- Modify: `docs/reviews/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/superpowers/plans/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/superpowers/plans/2026-07-24-runtime-error-delivery-provider-accounting-fixes.md`

- [x] **Step 1: Run regression verification**

Run: `cargo test -p blockcell-agent runtime -- --nocapture`

Expected: all runtime tests pass.

Run: `cargo fmt --all -- --check`

Expected: exit 0.

- [x] **Step 2: Update review resolutions and plan checkboxes**

Mark R4 and R5 fixed with their regression test names, update the runtime baseline, and mark this fix plan complete.

- [x] **Step 3: Commit only scoped files**

```bash
git add crates/agent/src/runtime/message_task.rs \
  crates/agent/src/runtime/message_dispatch.rs \
  crates/agent/src/runtime/tests.rs \
  docs/reviews/2026-07-24-agent-runtime-learning-review.md \
  docs/superpowers/plans/2026-07-24-agent-runtime-learning-review.md \
  docs/superpowers/plans/2026-07-24-runtime-error-delivery-provider-accounting-fixes.md
git commit -m "fix: deliver runtime failures and record stream success"
```
