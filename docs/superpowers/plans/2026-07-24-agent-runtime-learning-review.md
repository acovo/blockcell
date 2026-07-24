# Agent Runtime and Learning Systems Review Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to execute this review task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Review BlockCell's module 2 (Agent core runtime) and module 5 (memory, self-learning, and self-evolution), producing source-backed correctness, security, reliability, and maintainability findings without modifying implementation code.

**Architecture:** Review the two modules as separate parts, then inspect their shared boundaries. Part 1 follows one interactive message through context construction, provider/tool loops, subagents, cancellation, compaction, and task completion. Part 2 follows information from session persistence and retrieval through memory extraction, Ghost learning decisions, guarded writes, skill evolution, deployment, rollback, and background scheduling.

**Tech Stack:** Rust 2021, Tokio, SQLite/rusqlite, RabitQ, Rhai, serde, filesystem-backed Markdown memory and skills.

---

## Review outputs

- Create: `docs/reviews/2026-07-24-agent-runtime-learning-review.md`
- Do not modify production code while reviewing.
- Every confirmed finding must include severity, exact source location, trigger/control flow, impact, evidence, and repair direction.
- Keep architectural risks and missing test coverage separate from confirmed defects.

## Part 1: Module 2 — Agent core runtime

### Task 1: Map runtime construction and message lifecycle

**Files:**
- Inspect: `crates/agent/src/runtime.rs`
- Inspect: `crates/agent/src/runtime/wiring.rs`
- Inspect: `crates/agent/src/runtime/run_loop.rs`
- Inspect: `crates/agent/src/runtime/message_dispatch.rs`
- Inspect: `crates/agent/src/runtime/message_task.rs`
- Inspect: `crates/agent/src/runtime/process_message_inner.rs`
- Inspect: `crates/agent/src/runtime/process_message_phases.rs`
- Inspect: `crates/agent/src/runtime/turn_flow.rs`

- [x] Trace runtime construction and ownership of providers, registries, stores, channels, cancellation handles, and background services.
- [x] Trace one inbound message through dispatch, context building, LLM/tool iterations, persistence, outbound delivery, and cleanup.
- [x] Check every early return and error branch for missing cleanup, stale state, lost replies, or duplicate persistence.
- [x] Record the lifecycle map and supported findings in the review output.

### Task 2: Review tool execution, permissions, and cancellation

**Files:**
- Inspect: `crates/agent/src/runtime/tool_exec.rs`
- Inspect: `crates/agent/src/runtime/path_security.rs`
- Inspect: `crates/agent/src/runtime/subagent.rs`
- Inspect: `crates/agent/src/runtime/fork_spawn.rs`
- Inspect: `crates/agent/src/forked/**/*.rs`
- Inspect: `crates/agent/src/checkpoint.rs`
- Inspect: `crates/agent/src/steering.rs`
- Inspect: `crates/core/src/abort_token*.rs`
- Inspect: `crates/core/src/tool_policy.rs`

- [ ] Trace authorization from model-selected tool name and arguments to the final tool invocation.
- [ ] Verify path checks, user confirmation, permission inheritance, subagent restrictions, and policy reload behavior.
- [ ] Trace cancellation and steering during provider streaming, tool execution, forked-agent work, and cleanup.
- [ ] Run `cargo test -p blockcell-agent runtime -- --nocapture` and focused tests for any candidate defect.

### Task 3: Review context, compaction, tasks, and shared state

**Files:**
- Inspect: `crates/agent/src/context.rs`
- Inspect: `crates/agent/src/compact/**/*.rs`
- Inspect: `crates/agent/src/session_memory/**/*.rs`
- Inspect: `crates/agent/src/task_manager/**/*.rs`
- Inspect: `crates/agent/src/system_event*.rs`
- Inspect: `crates/agent/src/summary_queue.rs`
- Inspect: `crates/agent/src/response_cache/**/*.rs`

- [ ] Check context ordering, truncation, recovery budgets, compact summaries, and file/skill tracking for information loss or prompt corruption.
- [ ] Check task state transitions, restart recovery, event emission, and notification routing for races or inconsistent state.
- [ ] Check mutex/RwLock usage, spawned-task ownership, shutdown handling, and cache keys for deadlocks, leaks, or cross-session contamination.
- [ ] Record confirmed defects, architectural risks, and missing tests separately.

## Part 2: Module 5 — Memory, self-learning, and self-evolution

### Task 4: Map persistence, retrieval, and memory contracts

**Files:**
- Inspect: `crates/storage/src/memory/**/*.rs`
- Inspect: `crates/storage/src/memory_contract.rs`
- Inspect: `crates/storage/src/memory_service.rs`
- Inspect: `crates/storage/src/retriever.rs`
- Inspect: `crates/storage/src/vector.rs`
- Inspect: `crates/storage/src/rabitq_index.rs`
- Inspect: `crates/storage/src/session.rs`
- Inspect: `crates/agent/src/memory_adapter.rs`
- Inspect: `crates/agent/src/memory_system/**/*.rs`
- Inspect: `crates/agent/src/auto_memory/**/*.rs`

- [ ] Map schemas, migrations, identifiers, transaction boundaries, file locks, vector synchronization, and deletion/update behavior.
- [ ] Trace memory writes and retrieval into prompt context, including scoring, limits, subject/session isolation, and fallback behavior.
- [ ] Check crash consistency, partial failures, concurrent writers, duplicate records, stale vector entries, and accidental cross-user recall.
- [ ] Run `cargo test -p blockcell-storage -- --nocapture` and focused agent memory tests; record results without changing code.

### Task 5: Review Ghost learning decisions and guarded writes

**Files:**
- Inspect: `crates/agent/src/ghost_learning.rs`
- Inspect: `crates/agent/src/ghost_background_review.rs`
- Inspect: `crates/agent/src/ghost_memory_provider.rs`
- Inspect: `crates/agent/src/ghost_recall.rs`
- Inspect: `crates/agent/src/learning_coordinator.rs`
- Inspect: `crates/agent/src/learning_dedup.rs`
- Inspect: `crates/agent/src/learning_throttle.rs`
- Inspect: `crates/agent/src/memory_file_store.rs`
- Inspect: `crates/agent/src/skill_file_store/**/*.rs`
- Inspect: `crates/agent/src/write_guard.rs`
- Inspect: `crates/agent/src/unified_security_scanner.rs`
- Inspect: `crates/storage/src/ghost_ledger.rs`

- [ ] Trace all learning boundaries and decisions through deduplication, throttling, background review, ledger state, security scanning, snapshot creation, write, rollback, and audit.
- [ ] Verify that failures release throttle slots and locks, retries cannot double-write, configuration reload reaches live components, and session/user boundaries remain intact.
- [ ] Check generated Markdown and skill patches for traversal, symlink, prompt-injection, unsafe-content, and lost-update risks.
- [ ] Run focused tests using `cargo test -p blockcell-agent ghost -- --nocapture` and `cargo test -p blockcell-agent learning -- --nocapture`.

### Task 6: Review skill evolution lifecycle

**Files:**
- Inspect: `crates/skills/src/evolution/**/*.rs`
- Inspect: `crates/skills/src/core_evolution/**/*.rs`
- Inspect: `crates/skills/src/versioning.rs`
- Inspect: `crates/skills/src/capability_versioning.rs`
- Inspect: `crates/skills/src/audit.rs`
- Inspect: `crates/scheduler/src/evolution_worker.rs`
- Inspect: `crates/scheduler/src/skill_evolution_worker.rs`
- Inspect: `crates/scheduler/src/ghost.rs`

- [ ] Trace error observation to candidate generation, audit, compilation, testing, versioning, deployment, canary promotion, and rollback.
- [ ] Check state-machine transitions, atomic replacement, signature/trust decisions, concurrent evolution, recovery after restart, and audit completeness.
- [ ] Confirm that generated code cannot bypass tool/path policy through Rhai, Python, shell, or deployment hooks.
- [ ] Run `cargo test -p blockcell-skills -- --nocapture` and focused scheduler tests.

### Task 7: Cross-boundary synthesis and report completion

**Files:**
- Update: `docs/reviews/2026-07-24-agent-runtime-learning-review.md`
- Re-inspect: call sites identified in Tasks 1-6

- [ ] Reproduce or prove each high/medium finding with a focused existing test, a read-only command, or explicit control-flow analysis.
- [ ] Remove speculative findings that lack a concrete trigger and consequence.
- [ ] Rank findings by severity and include repair direction without implementing the repair.
- [ ] Add separate sections for architecture risks, test gaps, and reviewed areas with no confirmed defect.
- [ ] Run `cargo test -p blockcell-storage -p blockcell-agent -p blockcell-skills -p blockcell-scheduler --all-targets` and record the final baseline.
