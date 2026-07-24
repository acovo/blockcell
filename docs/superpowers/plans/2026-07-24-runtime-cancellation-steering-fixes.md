# Runtime Cancellation and Steering Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make background-agent provider/tool work stop promptly on cancellation and keep the global inbound loop responsive when a steering queue is full.

**Architecture:** Add an async cancellation wait primitive to `AbortToken`, then select it against ordinary-subagent, fork-provider, and fork-tool futures. Ensure forked shell children die when their future is dropped. Replace inline steering backpressure with a synchronous bounded enqueue policy that rejects the newest message without blocking the dispatcher.

**Tech Stack:** Rust 2021, Tokio, async-trait, Tokio process management, Cargo tests.

---

### Task 1: Awaitable cancellation primitive

**Files:**
- Modify: `crates/core/src/abort_token.rs`

- [x] **Step 1: Write failing async tests**

Add tests that create a token and child token, await `cancelled()` in spawned tasks, cancel the token or its parent, and require both waits to finish within 200 ms.

- [x] **Step 2: Run the tests and verify RED**

Run: `cargo test -p blockcell-core abort_token -- --nocapture`

Expected: compilation fails because `AbortToken::cancelled` does not exist.

- [x] **Step 3: Implement the minimal wait API**

Add an `Arc<tokio::sync::Notify>` to each token. Implement `pub async fn cancelled(&self)` so it registers notification before checking state and waits on either the local token or the boxed parent wait. Notify waiters from `cancel()` and `cancel_with_reason()`.

- [x] **Step 4: Run the focused tests and verify GREEN**

Run: `cargo test -p blockcell-core abort_token -- --nocapture`

Expected: all abort-token tests pass.

### Task 2: Cancel ordinary and forked background work

**Files:**
- Modify: `crates/agent/src/runtime/subagent.rs`
- Modify: `crates/agent/src/forked/agent/run.rs`
- Modify: `crates/agent/src/forked/agent/tool_exec.rs`
- Modify: `crates/agent/src/forked/agent/tests.rs`

- [x] **Step 1: Write failing fork-provider cancellation test**

Add a provider whose `chat` future remains pending. Run `run_forked_agent` inside `scope_abort_token`, cancel the parent token, and assert the result becomes `ForkedAgentError::Aborted` within 200 ms.

- [x] **Step 2: Run the test and verify RED**

Run: `cargo test -p blockcell-agent forked_agent_cancels_pending_provider -- --nocapture`

Expected: timeout because `provider.chat(...).await` does not observe cancellation.

- [x] **Step 3: Implement cancellation-aware waits**

Use `tokio::select!` to race `context.abort_token.cancelled()` against `provider.chat(...)` and each `execute_forked_tool(...)`. In `run_subagent_task`, race the registered token against `sub_runtime.process_message(...)` and return without learning, persistence, or delivery after cancellation. Set `Command::kill_on_drop(true)` before awaiting forked shell output.

- [x] **Step 4: Run focused cancellation tests and verify GREEN**

Run: `cargo test -p blockcell-agent cancel -- --nocapture`

Expected: all cancellation tests pass, including the pending-provider regression.

### Task 3: Non-blocking steering overflow

**Files:**
- Modify: `crates/agent/src/steering.rs`
- Modify: `crates/agent/src/runtime/run_loop.rs`

- [x] **Step 1: Write failing overflow-policy test**

Add `try_route` behavior that returns a `Full` outcome immediately when a capacity-one channel already contains a message, and assert the original queued message remains available.

- [x] **Step 2: Run the test and verify RED**

Run: `cargo test -p blockcell-agent steering_queue_full_rejects_newest_without_waiting -- --nocapture`

Expected: compilation fails because the routing outcome/API does not exist.

- [x] **Step 3: Implement and wire the bounded policy**

Introduce a small `SteeringRouteOutcome` enum and synchronous sender method that maps Tokio `try_send` to `Enqueued`, `Full`, or `Closed`. Update the runtime loop to log and drop the newest steering message on `Full`, never awaiting channel capacity.

- [x] **Step 4: Run steering tests and verify GREEN**

Run: `cargo test -p blockcell-agent steering -- --nocapture`

Expected: all steering tests pass.

### Task 4: Documentation, full verification, and commit

**Files:**
- Modify: `docs/reviews/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/superpowers/plans/2026-07-24-runtime-cancellation-steering-fixes.md`

- [x] **Step 1: Record resolutions and test names**

Mark R8 and R9 fixed, describe the implemented cancellation/overflow contracts, and check every completed plan step.

- [x] **Step 2: Run verification**

Run: `cargo fmt --all -- --check`

Run: `cargo test -p blockcell-core abort_token -- --nocapture`

Run: `cargo test -p blockcell-agent runtime -- --nocapture`

Run: `cargo test -p blockcell-agent forked -- --nocapture`

Expected: formatting and all tests pass.

- [x] **Step 3: Commit only scoped files**

Stage the files listed in Tasks 1-4 plus the existing review-plan progress document, inspect `git diff --cached`, and commit with `fix: make agent cancellation and steering responsive`.
