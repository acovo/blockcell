# Structured Memory Contract Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix confirmed structured-memory findings M4-M8, add regression coverage, update the review record, and commit only scoped changes before continuing Module 5 Task 5 review.

**Architecture:** Keep SQLite `memory_items` canonical and make caller ownership explicit at tool-to-adapter mutation boundaries. Enforce bounded query parameters and exact tag semantics in every retrieval path, pass session ownership into vector search before truncation, and turn reindex into a durable queue-backed rebuild that revalidates canonical state around external index writes.

**Tech Stack:** Rust 2021, Tokio, SQLite/rusqlite, RabitQ, serde_json, Cargo tests.

---

### Task 1: Enforce session ownership for tool mutations and statistics

**Files:**
- Modify: `crates/tools/src/lib.rs`
- Modify: `crates/tools/src/memory.rs`
- Modify: `crates/agent/src/memory_adapter.rs`
- Modify: `crates/storage/src/memory/maintenance.rs`
- Modify: `crates/storage/src/memory/brief.rs`
- Test: `crates/tools/src/memory.rs`
- Test: `crates/storage/src/memory/tests.rs`

- [x] Add a failing tool test asserting `memory_forget` forwards `ctx.session_key` in batch mutation parameters and rejects an empty batch filter.
- [x] Run `cargo test -p blockcell-tools memory_forget -- --nocapture`; verify failure because the session key is absent and empty batch deletion is accepted.
- [x] Add session-aware `MemoryStoreOps` methods for delete, restore, batch delete, and stats whose safe defaults reject unsupported scoped access; make ordinary tools call only these methods.
- [x] Implement adapter overrides that call storage methods with `(session_key = caller)` ownership predicates; ordinary callers must not mutate global rows.
- [x] Add regression tests proving session A cannot delete/restore session B or global rows and batch deletion affects only session A.
- [x] Run focused Tools and Agent adapter tests; verify all ownership tests pass.

### Task 2: Bound query limits at every layer

**Files:**
- Modify: `crates/tools/src/memory.rs`
- Modify: `crates/agent/src/memory_adapter.rs`
- Modify: `crates/storage/src/memory.rs`
- Modify: `crates/storage/src/retriever.rs`
- Test: `crates/tools/src/memory.rs`
- Test: `crates/agent/src/memory_adapter.rs`
- Test: `crates/storage/src/memory/tests.rs`

- [x] Add failing tests for `top_k` values `-1`, `0`, and `51`, plus an adapter test proving negative JSON input never becomes `usize::MAX`.
- [x] Run focused Tools and Agent adapter tests; verify boundary failures.
- [x] Validate tool input as `1..=50`, convert with `usize::try_from`, and clamp `QueryParams` inside storage to a shared maximum before SQL/vector use.
- [x] Run focused Tools, Agent, and Storage tests; verify bounded behavior and no unbounded LIMIT.

### Task 3: Filter vectors by ownership before candidate truncation

**Files:**
- Modify: `crates/storage/src/vector.rs`
- Modify: `crates/storage/src/retriever.rs`
- Modify: `crates/storage/src/memory/vector_sync.rs`
- Modify: `crates/storage/src/rabitq_index.rs`
- Test: `crates/storage/src/memory/tests.rs`
- Test: `crates/storage/src/rabitq_index.rs`

- [x] Make the fake vector index honor `top_k`, then add a failing hybrid-retrieval test with 20 higher-ranked session-B hits ahead of one semantic-only session-A hit.
- [x] Run the focused storage test; verify session A's valid result is missing.
- [x] Add `session_key` to `VectorMeta` and a `VectorFilter` carrying session/global ownership plus scope/type/tags; require `VectorIndex::search` to apply it before truncation.
- [x] Implement RabitQ filtered search and canonical iterative over-fetch for legacy metadata; pass `QueryParams` filters from the retriever.
- [x] Run storage vector/retrieval tests; verify session A plus explicit-global semantics pass.

### Task 4: Make vector reindex durable and mutation-aware

**Files:**
- Modify: `crates/storage/src/memory/maintenance.rs`
- Modify: `crates/storage/src/memory/vector_sync.rs`
- Test: `crates/storage/src/memory/tests.rs`

- [x] Add a failing reset-error test asserting every active canonical row has an upsert intent before `reset()` is attempted.
- [x] Add an index-callback test that marks a row deleted during reindex and asserts the final vector operation is delete, not stale upsert.
- [x] Run the focused reindex tests; verify missing durable intents before implementation.
- [x] Seed durable upsert intents transactionally before reset, never clear the whole queue during rebuild, and re-read canonical state before and after each upsert; delete instead whenever the row is no longer active.
- [x] Run focused reindex tests and the full Storage suite.

### Task 5: Use exact tag membership

**Files:**
- Modify: `crates/storage/src/memory/query.rs`
- Modify: `crates/storage/src/memory/maintenance.rs`
- Modify: `crates/storage/src/rabitq_index.rs`
- Test: `crates/storage/src/memory/tests.rs`

- [x] Add failing query and batch-delete tests proving tag `go` does not match `mongodb` and tag `prod` does not match `production`.
- [x] Run focused tag tests; verify substring false positives.
- [x] Replace SQL substring filters with exact comma-delimited token membership, replace Rust `contains` with equality, and apply exact matching in vector metadata filters.
- [x] Run focused tag tests and the full Storage suite.

### Task 6: Verify, document, and commit

**Files:**
- Modify: `docs/reviews/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/superpowers/plans/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/superpowers/plans/2026-07-25-memory-contract-review-fixes.md`

- [x] Run `cargo fmt --all -- --check`.
- [x] Run `cargo test -p blockcell-storage -- --nocapture`.
- [x] Run `cargo test -p blockcell-tools memory -- --nocapture`.
- [x] Run `cargo test -p blockcell-agent memory -- --nocapture`.
- [x] Mark M4-M8 resolved with regression-test names and update plan checkboxes.
- [x] Run `git diff --check`, inspect scoped diff, and stage only implementation, tests, and the three review-plan documents.
- [x] Commit with Chinese message `修复：强化结构化记忆隔离与向量一致性`.

### Task 7: Continue the next large-module review

**Files:**
- Inspect: Module 5 Task 5 files listed in `docs/superpowers/plans/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/reviews/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/superpowers/plans/2026-07-24-agent-runtime-learning-review.md`

- [x] Review Ghost learning decisions, deduplication, throttling, guarded file writes, background scheduling, and failure recovery end-to-end.
- [x] Run focused Ghost/learning tests without modifying production code during review.
- [x] Record confirmed defects, architecture risks, missing tests, and reviewed no-defect areas in the review MD.
- [x] Mark Module 5 Task 5 complete; do not fix or commit newly discovered defects without a new user instruction.
