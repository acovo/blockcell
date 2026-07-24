# Cross-Module Integration Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the eight confirmed cross-module security, identity, concurrency, supervision, and cron-completion defects without staging unrelated workspace changes.

**Architecture:** Keep security-sensitive parsing and configuration persistence helpers close to their owning modules, and expose only small shared primitives where cross-crate acknowledgement is required. Serialize gateway configuration mutations with one shared lock, scope non-WebSocket confirmations by channel account and sender, supervise long-running tasks through the existing critical-failure channel, and make cron completion depend on an AgentRuntime acknowledgement rather than mpsc enqueue success.

**Tech Stack:** Rust, Tokio, Axum, serde/JSON5, Cargo unit tests.

---

### Task 1: Secure and environment-preserving configuration persistence

**Files:**
- Modify: `crates/core/src/config.rs`
- Modify: `bin/blockcell/src/commands/gateway/channels.rs`
- Test: `crates/core/src/config.rs`
- Test: `bin/blockcell/src/commands/gateway/channels.rs`

- [ ] Add Unix tests proving saved configuration files use mode `0600` and `${ENV_VAR}` strings survive a typed load/change/save cycle.
- [ ] Run `cargo test -p blockcell-core config::tests::test_config_save -- --nocapture` and verify the new tests fail for the current implementation.
- [ ] Create temporary configuration files with restrictive permissions, recursively retain unchanged environment placeholders when serializing typed configuration, and parse channel-update JSON5 without environment expansion.
- [ ] Re-run the focused core and gateway channel tests and verify they pass.

### Task 2: Account-scoped session identity

**Files:**
- Modify: `crates/core/src/message.rs`
- Test: `crates/core/src/message.rs`

- [ ] Change the existing account round-trip test to require distinct session keys for two account IDs while retaining the legacy key for messages without an account.
- [ ] Run `cargo test -p blockcell-core message::tests -- --nocapture` and verify the account-scoped assertion fails.
- [ ] Include a collision-safe account component in `InboundMessage::session_key` only when `account_id` is present.
- [ ] Re-run the message tests and verify they pass.

### Task 3: Fail-closed non-WebSocket confirmation handling

**Files:**
- Modify: `crates/agent/src/runtime.rs`
- Modify: `crates/agent/src/runtime/path_security.rs`
- Modify: `bin/blockcell/src/commands/gateway.rs`
- Test: `bin/blockcell/src/commands/gateway.rs`

- [ ] Add tests showing exact affirmative replies are accepted, negated phrases are rejected, account/sender values produce distinct confirmation scopes, and a second outstanding request in one scope cannot overwrite the first.
- [ ] Run the focused `blockcell` gateway confirmation tests and verify they fail.
- [ ] Carry `account_id` and `sender_id` in `ConfirmRequest`, set the outbound account, use a typed scope key, generate a request ID, and reject duplicate pending requests for the same scope.
- [ ] Replace substring matching with an exact normalized allowlist and re-run the focused tests.

### Task 4: Serialize configuration mutations

**Files:**
- Modify: `bin/blockcell/src/commands/gateway.rs`
- Modify: `bin/blockcell/src/commands/gateway/channels.rs`
- Modify: `bin/blockcell/src/commands/gateway/config_api.rs`
- Modify: any gateway test fixture constructing `GatewayState`
- Test: `bin/blockcell/src/commands/gateway/channels.rs`

- [ ] Add a concurrent update regression test demonstrating that unrelated channel/owner changes are both retained.
- [ ] Run the focused gateway test and verify the lost-update assertion fails.
- [ ] Add one shared async configuration-write mutex to `GatewayState` and hold it across every read/modify/write configuration endpoint.
- [ ] Re-run the focused test and verify both updates survive.

### Task 5: Supervise critical background tasks

**Files:**
- Modify: `bin/blockcell/src/commands/gateway.rs`
- Test: `bin/blockcell/src/commands/gateway.rs`

- [ ] Add a test proving an unexpected runtime/channel/cron task exit sends a critical failure while a shutdown-triggered exit does not.
- [ ] Run the focused gateway supervision test and verify it fails before the helper exists.
- [ ] Create the critical-failure channel before spawning services, wrap runtime handles, and report unexpected cron/channel exits through that channel using a shared shutdown flag.
- [ ] Re-run gateway tests and verify the shutdown waiter receives the task failure.

### Task 6: Acknowledge cron execution completion

**Files:**
- Create: `crates/core/src/message_receipt.rs`
- Modify: `crates/core/src/lib.rs`
- Modify: `crates/agent/src/runtime/message_task.rs`
- Modify: `crates/scheduler/src/cron_service.rs`
- Test: `crates/core/src/message_receipt.rs`
- Test: `crates/scheduler/src/cron_service.rs`

- [ ] Add receipt registry tests and scheduler tests proving enqueue alone does not mark a job successful, runtime success does, and runtime failure retains a delete-after-run job with error state.
- [ ] Run focused core/scheduler tests and verify they fail.
- [ ] Register a unique completion receipt in cron metadata, complete it from `run_message_task`, wait with a bounded timeout, and update/delete jobs only after a successful acknowledgement.
- [ ] Re-run focused tests and verify completion, failure, and timeout behavior.

### Task 7: Full verification and atomic commit

**Files:**
- Modify: only files listed above plus this plan.

- [ ] Run `cargo fmt --all -- --check`, the affected package tests, and `cargo check -p blockcell`.
- [ ] Inspect `git diff --check`, `git status --short`, and the staged diff to ensure original `.DS_Store` and documentation files are excluded.
- [ ] Stage only the plan, changed source files, and regression tests.
- [ ] Commit with `fix: harden cross-module runtime integration`.
