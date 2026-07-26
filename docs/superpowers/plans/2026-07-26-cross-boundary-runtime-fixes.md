# Cross-Boundary Runtime Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix X1-X2 by making persistent system-event delivery survive restart, enforcing event deadlines, and removing the outer capability-registry mutex; also restore the Scheduler baseline by aligning advisory-lock tests with the current lock contract.

**Architecture:** Keep event and summary state in their existing in-memory structures, but optionally back each structure with an atomically replaced JSON file loaded at Runtime startup. Make the tools capability handle an `Arc<dyn CapabilityRegistryOps>` so the adapter's existing inner registry synchronization is the only lock, allowing executor awaits to run without a global outer guard. Advisory lock files remain reusable coordination inodes, so tests verify reacquisition rather than pathname deletion.

**Tech Stack:** Rust 2021, Tokio, serde JSON, atomic filesystem replacement, OS advisory locks, Cargo tests.

---

### Task 1: Persist system events and summary items, and enforce delivery deadlines (X1)

**Files:**
- Modify: `crates/agent/src/system_event_store.rs`
- Modify: `crates/agent/src/summary_queue.rs`
- Modify: `crates/agent/src/system_event_orchestrator.rs`
- Modify: `crates/agent/src/runtime.rs`
- Test: `crates/agent/tests/system_event_store.rs`
- Test: `crates/agent/tests/summary_queue.rs`
- Test: `crates/agent/tests/system_event_orchestrator.rs`

- [x] **Step 1: Write failing persistence and deadline tests**

Add tests that create a persistent event store, emit one persistent and one non-persistent event, reconstruct the store, and assert only the persistent event remains. Add a summary-queue restart test and a deadline test proving a normal event with `max_delay_seconds = Some(1)` becomes an immediate notification after one second.

- [x] **Step 2: Run focused tests and verify RED**

Run: `cargo test -p blockcell-agent --test system_event_store --test summary_queue --test system_event_orchestrator`

Expected: compilation or assertion failure because persistent constructors and deadline enforcement do not exist.

- [x] **Step 3: Implement durable optional backing stores**

Add `with_persistence(...) -> std::io::Result<Self>` constructors that load JSON arrays, retain only `delivery.persist` events in the event file, and atomically rewrite state after enqueue/dedup/delivered/acked/cleanup and summary enqueue/merge/acknowledge. Keep `Default` and `with_policy` memory-only for isolated callers and tests. Runtime must use files below `paths.workspace()`.

- [x] **Step 4: Enforce event-specific maximum delay**

In `SystemEventOrchestrator::process_tick`, treat a notifying event as immediate when it is Critical, explicitly immediate, or `now_ms >= created_at_ms + max_delay_seconds * 1000`, while preserving summary enqueue behavior.

- [x] **Step 5: Run focused tests and verify GREEN**

Run: `cargo test -p blockcell-agent --test system_event_store --test summary_queue --test system_event_orchestrator`

Expected: all focused tests pass.

### Task 2: Remove the outer capability registry mutex (X2)

**Files:**
- Modify: `crates/tools/src/lib.rs`
- Modify: `crates/tools/src/system_info.rs`
- Modify: `crates/agent/src/capability_adapter.rs`
- Modify: `bin/blockcell/src/commands/agent.rs`
- Modify: `bin/blockcell/src/commands/gateway.rs`
- Test: inline tests in `crates/agent/src/capability_adapter.rs`

- [x] **Step 1: Write a failing adapter concurrency test**

Register a slow capability, start its execution through the opaque tools handle, and assert `list_all_json` completes within 200 ms while execution is still pending. Construct the desired handle as `Arc<dyn CapabilityRegistryOps + Send + Sync>` so the current outer-mutex alias fails before implementation.

- [x] **Step 2: Run the focused test and verify RED**

Run: `cargo test -p blockcell-agent capability_adapter_does_not_block_registry_reads_during_execution -- --nocapture`

Expected: compilation failure or timeout caused by the current `Arc<Mutex<dyn CapabilityRegistryOps>>` handle.

- [x] **Step 3: Make synchronization implementation-owned**

Change `blockcell_tools::CapabilityRegistryHandle` to `Arc<dyn CapabilityRegistryOps + Send + Sync>`, call trait methods directly in `system_info`, and construct adapters with `Arc::new(adapter)` in Agent and Gateway startup. Keep the adapter's concrete `Arc<Mutex<CapabilityRegistry>>`, whose execution helper already releases that inner lock before awaiting the executor.

- [x] **Step 4: Run focused tests and verify GREEN**

Run: `cargo test -p blockcell-agent capability_adapter_does_not_block_registry_reads_during_execution -- --nocapture`

Expected: PASS.

### Task 3: Align Scheduler advisory-lock tests with the lock contract

**Files:**
- Modify: `crates/scheduler/src/consolidator/tests.rs`

- [x] **Step 1: Replace stale pathname assertions**

In both failing Dream cleanup tests, replace `assert!(!lock_path.exists())` with successful `ExclusiveFileLock::try_acquire(&lock_path)`, proving the previous owner released the OS lock while allowing the reusable lock inode to remain.

- [x] **Step 2: Run focused Scheduler tests**

Run: `cargo test -p blockcell-scheduler test_dream_releases_lock_when_commit_backup_recovery_fails -- --nocapture`

Run: `cargo test -p blockcell-scheduler test_dream_cleans_state_and_lock_when_staging_prepare_fails -- --nocapture`

Expected: both pass.

### Task 4: Full verification, documentation, and commit

**Files:**
- Modify: `docs/reviews/2026-07-24-agent-runtime-learning-review.md`
- Modify: this plan

- [x] Run `cargo fmt --all -- --check`.
- [x] Run `cargo test -p blockcell-agent --all-targets`.
- [x] Run `cargo test -p blockcell-tools --all-targets`.
- [x] Run `cargo test -p blockcell-scheduler --all-targets`.
- [x] Run `cargo test -p blockcell --bin blockcell --no-run` to verify Agent/Gateway handle construction.
- [x] Run `git diff --check` and inspect only scoped changes.
- [x] Mark X1-X2 resolved with regression-test names and record the clean Scheduler baseline.
- [x] Stage only the files listed in this plan plus the review/plan documents.
- [x] Commit with Chinese message `修复：持久化系统事件并解除能力执行阻塞`.
