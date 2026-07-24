# Task Event and Restart Recovery Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep background-task events bound to their origin, retry failed event delivery, notify interrupted tasks after restart, and prevent stale terminal task files from starving recovery.

**Architecture:** Persist immutable origin routing on event-producing tasks and copy it into an origin-scoped `SystemEvent`. Split orchestration from transport acknowledgement so pending events and due summary items remain queued until dispatch succeeds. Mark restored interrupted tasks explicitly, replay their failure lifecycle event after the matching emitter is registered, and delete terminal task records during startup scanning without counting them against the unfinished-task recovery limit.

**Tech Stack:** Rust 2021, Tokio channels, serde JSON persistence, BlockCell runtime and task manager tests.

---

### Task 1: Bind lifecycle events to the task origin

**Files:**
- Modify: `crates/core/src/system_event.rs`
- Modify: `crates/agent/src/task_manager.rs`
- Modify: `crates/agent/src/runtime.rs`
- Modify: `crates/agent/src/runtime/subagent.rs`
- Modify: `crates/agent/src/runtime/tool_exec.rs`
- Modify: `crates/agent/src/runtime/wiring.rs`
- Test: `crates/agent/src/runtime/tests.rs`

- [x] **Step 1: Write the failing regression test**

Create a task whose immutable origin is chat A, rotate `main_session_target` to chat B, complete the task, process a system-event tick, and assert the emitted notification identifies chat A and its account.

- [x] **Step 2: Verify RED**

Run: `cargo test -p blockcell-agent task_lifecycle_event_stays_with_origin_after_target_rotation -- --nocapture`

Expected: fail because lifecycle events use `EventScope::MainSession` and resolve to chat B.

- [x] **Step 3: Implement immutable event routing**

Add optional account identity to session event scope, store `origin_account_id` and `origin_session_key` on routed tasks, add a routed task-creation entry point, and construct lifecycle events with the task's origin scope. Preserve serde compatibility with defaults for existing task files.

- [x] **Step 4: Verify GREEN**

Run: `cargo test -p blockcell-agent task_lifecycle_event_stays_with_origin_after_target_rotation -- --nocapture`

Expected: pass with the notification addressed to chat A.

### Task 2: Acknowledge events and summaries only after successful dispatch

**Files:**
- Modify: `crates/agent/src/system_event_orchestrator.rs`
- Modify: `crates/agent/src/system_event_store.rs`
- Modify: `crates/agent/src/summary_queue.rs`
- Modify: `crates/agent/src/runtime/wiring.rs`
- Test: `crates/agent/src/runtime/tests.rs`
- Test: `crates/agent/tests/system_event_orchestrator.rs`
- Test: `crates/agent/tests/summary_queue.rs`

- [x] **Step 1: Write failing transport tests**

Add one test with a closed/missing immediate-notification transport and one with a due summary whose outbound transport is missing. Assert the event remains pending and the summary items remain queued.

- [x] **Step 2: Verify RED**

Run: `cargo test -p blockcell-agent system_event_delivery_failure_keeps_pending -- --nocapture`

Run: `cargo test -p blockcell-agent summary_delivery_failure_keeps_items -- --nocapture`

Expected: fail because orchestration marks events delivered and clears summary items before dispatch.

- [x] **Step 3: Implement delivery acknowledgement**

Make notification and summary dispatch return success. Have orchestration produce delivery candidates without marking user-visible events delivered. Deduplicate repeated summary enqueueing by source event ID, expose due items without removing them, and remove summary items plus mark source events delivered only after successful dispatch. Silent events may be acknowledged during orchestration because they intentionally have no user transport.

- [x] **Step 4: Verify GREEN**

Run both focused tests and `cargo test -p blockcell-agent --test system_event_orchestrator -- --nocapture`.

Expected: failed delivery remains retryable and orchestrator tests pass with the updated acknowledgement contract.

### Task 3: Notify interrupted tasks after restart

**Files:**
- Modify: `crates/agent/src/task_manager.rs`
- Modify: `crates/agent/src/task_manager/persistence.rs`
- Modify: `bin/blockcell/src/commands/agent.rs`
- Modify: `bin/blockcell/src/commands/gateway.rs`
- Test: `crates/agent/src/task_manager.rs`

- [x] **Step 1: Write the failing recovery-notification test**

Persist a running event-producing task, restore it into a new manager, register a recording emitter, replay restored failures, and assert exactly one origin-scoped `task.failed` event is emitted and the notification state is persisted.

- [x] **Step 2: Verify RED**

Run: `cargo test -p blockcell-agent restored_interrupted_task_emits_failure_once -- --nocapture`

Expected: fail because restored tasks are only rewritten and logged.

- [x] **Step 3: Implement idempotent replay**

Persist a `restored_after_restart` marker, add an async replay method that selects matching unnotified restored failures, emits after the agent emitter is registered, then persists `notified = true`. Invoke it after runtime/emitter wiring in interactive agent and gateway startup.

- [x] **Step 4: Verify GREEN**

Run the focused test twice through its idempotency assertion.

Expected: one event only and persisted notification state.

### Task 4: Remove stale terminal records without starving recovery

**Files:**
- Modify: `crates/agent/src/task_manager/persistence.rs`
- Test: `crates/agent/src/task_manager.rs`

- [x] **Step 1: Write the failing scan-limit test**

Create terminal records before an unfinished record, use a small injectable unfinished-task limit, restore, and assert terminal files are deleted while the unfinished task is still restored.

- [x] **Step 2: Verify RED**

Run: `cargo test -p blockcell-agent restore_deletes_terminal_files_without_consuming_limit -- --nocapture`

Expected: fail because every directory entry consumes the current scan limit and terminal files remain.

- [x] **Step 3: Implement classified scanning**

Refactor restoration through an internal limit-taking helper. Delete valid terminal JSON records immediately, count only unfinished tasks against the recovery limit, and keep memory usage bounded by processing entries one at a time.

- [x] **Step 4: Verify GREEN**

Run the focused test and all TaskManager tests.

Expected: terminal files are gone and unfinished restoration is not starved.

### Task 5: Document, verify, and commit

**Files:**
- Modify: `docs/reviews/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/superpowers/plans/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/superpowers/plans/2026-07-25-task-event-recovery-fixes.md`

- [x] **Step 1: Record R13-R16 resolutions**

Add exact behavior and regression-test names to each finding and check completed plan steps.

- [x] **Step 2: Run verification**

Run: `cargo fmt --all -- --check`

Run: `cargo test -p blockcell-agent task_manager -- --nocapture`

Run: `cargo test -p blockcell-agent --test system_event_orchestrator -- --nocapture`

Run: `cargo test -p blockcell-agent --test summary_queue -- --nocapture`

Run: `cargo test -p blockcell-agent runtime -- --nocapture`

Expected: formatting and all focused suites pass.

- [x] **Step 3: Commit scoped files**

Stage only the production files, regression tests, review documents, and this implementation plan. Run cached diff checks and commit with `fix: route task events and recover delivery`.
