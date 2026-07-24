# Agent Runtime and Learning Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the six confirmed Agent Runtime and learning-system defects documented in `docs/reviews/2026-07-24-agent-runtime-learning-review.md`, add regression coverage, and commit only the scoped changes.

**Architecture:** Use canonical conversation identity for active runtime state, supervise message-task termination, and make learning-review reservations RAII-safe. Scope structured memory to the current session unless a record is explicitly global, filter invalid FTS rows before limiting candidates, and replace crash-fragile lock directories with owner-aware stale-lock recovery.

**Tech Stack:** Rust 2021, Tokio, SQLite/rusqlite, filesystem locks, Cargo tests.

---

### Task 1: Canonicalize active conversation identity

**Files:**
- Modify: `crates/agent/src/steering.rs`
- Modify: `crates/agent/src/runtime/run_loop.rs`
- Modify: `bin/blockcell/src/commands/gateway/websocket.rs`
- Test: `crates/agent/src/runtime/tests.rs`
- Test: `bin/blockcell/src/commands/gateway/websocket.rs`

- [x] Add a failing test proving two messages with the same raw `chat_id` but different channel/account identity produce different active conversation keys.
- [x] Run `cargo test -p blockcell-agent active_conversation -- --nocapture` and verify the new test fails because active routing has no canonical key.
- [x] Add `ActiveConversationKey::from_message(agent_id, msg)` using `msg.session_key()`, change active maps and `SteeringSessionKey` to use the canonical session key, and carry that key through task completion and cancellation cleanup.
- [x] Update WebSocket steering construction to use `ws` session identity.
- [x] Run the focused Agent and Gateway WebSocket tests and verify they pass.

### Task 2: Supervise message-task termination

**Files:**
- Modify: `crates/agent/src/runtime/run_loop.rs`
- Modify: `crates/agent/src/runtime/message_task.rs`
- Test: `crates/agent/src/runtime/tests.rs`

- [x] Add a failing async test that spawns a panicking message-task future and asserts its TaskManager record becomes `Failed` and its cleanup notification is emitted.
- [x] Run the focused test and verify the task remains `Running` before the fix.
- [x] Store `tokio::task::AbortHandle` in the active-task map and spawn a supervisor that awaits the JoinHandle, marks panic as failed, resolves/cancels the completion receipt, and always sends task completion cleanup.
- [x] Keep normal success/error transitions owned by `run_message_task`; make supervisor cleanup idempotent.
- [x] Run focused runtime tests and verify success.

### Task 3: Make deferred learning-review reservations RAII-safe

**Files:**
- Modify: `crates/agent/src/learning_coordinator.rs`
- Modify: `crates/agent/src/runtime.rs`
- Modify: `crates/agent/src/runtime/process_message_inner.rs`
- Test: `crates/agent/src/runtime/tests.rs`

- [x] Add a failing test that reserves a nudge review, drops the pending runtime reservation without spawning, and asserts a new reservation can be acquired immediately.
- [x] Run the focused test and verify the active review slot remains consumed.
- [x] Add `LearningReviewReservationGuard`, whose `Drop` calls a coordinator cancellation method unless ownership is transferred to `spawn_review`.
- [x] Create the guard whenever memory/skill nudge acquisition succeeds and disarm it only when the review task is spawned.
- [x] Run focused learning/runtime tests and verify empty-response and early-return cleanup.

### Task 4: Scope structured memory and deduplication

**Files:**
- Modify: `crates/storage/src/memory.rs`
- Modify: `crates/storage/src/memory/schema.rs`
- Modify: `crates/storage/src/memory/crud.rs`
- Modify: `crates/storage/src/memory/query.rs`
- Modify: `crates/storage/src/memory/brief.rs`
- Modify: `crates/storage/src/retriever.rs`
- Modify: `crates/agent/src/memory_adapter.rs`
- Modify: `crates/agent/src/context.rs`
- Modify: `crates/tools/src/lib.rs`
- Modify: `crates/tools/src/memory.rs`
- Test: `crates/storage/src/memory/tests.rs`
- Test: `crates/agent/src/context.rs`

- [x] Add failing storage tests proving session A cannot query session B data and equal dedup keys in two sessions create two rows.
- [x] Add a failing context test proving automatic memory brief generation receives the current session key.
- [x] Run focused tests and verify cross-session rows are currently returned/overwritten.
- [x] Add `session_key` filtering to `QueryParams`; treat `session_key IS NULL` as explicitly global and visible to all sessions.
- [x] Replace the global active-dedup unique index with a session-aware unique expression and make dedup updates match the same session/global owner.
- [x] Add session-aware brief methods to `MemoryStoreOps` with backwards-compatible defaults, override them in `MemoryStoreAdapter`, and use them from session context construction and memory tools.
- [x] Run storage, tools, and Agent context tests.

### Task 5: Filter invalid FTS candidates before limiting

**Files:**
- Modify: `crates/storage/src/memory/query.rs`
- Modify: `crates/storage/src/retriever.rs`
- Test: `crates/storage/src/memory/tests.rs`

- [x] Add a failing test with more than the candidate window of deleted/expired high-relevance rows ahead of one active matching row.
- [x] Run the test and verify the active row is missing.
- [x] Pass `QueryParams` into FTS candidate selection and apply deletion, expiry, session, scope, type, tags, and time filters before `LIMIT`.
- [x] Run the focused test and full storage test suite.

### Task 6: Recover stale learning file locks

**Files:**
- Create: `crates/agent/src/learning_file_lock.rs`
- Modify: `crates/agent/src/lib.rs`
- Modify: `crates/agent/src/memory_file_store.rs`
- Modify: `crates/agent/src/skill_file_store.rs`
- Test: `crates/agent/src/learning_file_lock.rs`

- [x] Add failing tests for recovering a lock owned by a dead PID and refusing to steal a lock owned by the current live PID.
- [x] Run the focused tests and verify no shared owner-aware lock implementation exists.
- [x] Implement one shared RAII lock-directory guard that writes an owner PID, removes dead-owner locks, waits for live owners, and deletes only a lock it acquired.
- [x] Replace both duplicated `FileWriteGuard` implementations with the shared guard.
- [x] Run memory-file-store, skill-file-store, and full Agent tests.

### Task 7: Verify, update review status, and commit

**Files:**
- Modify: `docs/reviews/2026-07-24-agent-runtime-learning-review.md`
- Include: `docs/project-core-modules.md`
- Include: `docs/superpowers/plans/2026-07-24-agent-runtime-learning-review.md`
- Include: `docs/superpowers/plans/2026-07-24-agent-runtime-learning-review-fixes.md`

- [x] Run `cargo fmt --all -- --check`.
- [x] Run `cargo test -p blockcell-storage -p blockcell-tools -p blockcell-agent --all-targets`.
- [x] Run focused Gateway WebSocket tests through `cargo test -p blockcell --bin blockcell websocket -- --nocapture`.
- [x] Update each finding with resolution and regression-test references.
- [x] Inspect `git diff --check` and `git status --short`; stage only files from Tasks 1-7.
- [x] Commit with `git commit -m "fix: isolate runtime sessions and harden learning state"`.
