# Storage Integrity Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the eight reviewed persistence, crash-consistency, concurrency, migration, audit, and referential-integrity defects in `crates/storage` without including unrelated working-tree changes in the commit.

**Architecture:** Persist vector-sync intent in the same SQLite transaction as memory mutations, treat disk corruption and migration failures as errors, and serialize read-modify-write file stores with advisory file locks and atomic replacement. Make RaBitQ rebuild conservatively after restart, enforce deduplication and workflow foreign keys at the database layer, and make audit verification fail closed.

**Tech Stack:** Rust, rusqlite/SQLite, fs2 advisory locks, tempfile-based tests, Cargo test.

---

### Task 1: RaBitQ restart consistency

**Files:**
- Modify: `crates/storage/src/rabitq_index.rs`

- [ ] Add a regression test that builds an index, mutates its SQLite rows, drops the handle, reopens it, and verifies search cannot use the stale on-disk index.
- [ ] Run `cargo test -p blockcell-storage rabitq_rebuilds_after_reopen` and confirm the stale result fails.
- [ ] Mark every non-empty reopened index dirty so the first search validates current rows by rebuilding.
- [ ] Re-run the targeted test and confirm it passes.

### Task 2: Transactional vector outbox

**Files:**
- Modify: `crates/storage/src/memory/crud.rs`
- Modify: `crates/storage/src/memory/maintenance.rs`
- Modify: `crates/storage/src/memory/vector_sync.rs`
- Modify: `crates/storage/src/memory/tests.rs`

- [ ] Add an observing vector-index test double that checks the queue through a second SQLite connection while an external upsert/delete is executing.
- [ ] Run the targeted tests and confirm no pending intent is visible with the current implementation.
- [ ] Add helpers that enqueue vector intent on an existing transaction.
- [ ] Wrap memory upsert, soft-delete, batch-delete, restore, and maintenance mutations with their corresponding queue writes in one transaction.
- [ ] Keep successful vector operations clearing their queued intent and failed operations recording the error.
- [ ] Run all memory vector-consistency tests.

### Task 3: Fail-safe file migration

**Files:**
- Modify: `crates/storage/src/memory/import_migrate.rs`
- Modify: `crates/storage/src/memory/tests.rs`

- [ ] Add a migration test containing an unreadable/invalid UTF-8 daily file and assert migration returns an error without setting `migrated_from_md`.
- [ ] Run the test and confirm the current unconditional completion marker fails it.
- [ ] Propagate directory-entry, file-read, and import failures; only mark migration complete after all inputs succeed.
- [ ] Re-run migration tests.

### Task 4: Session write serialization

**Files:**
- Modify: `crates/storage/Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/storage/src/file_lock.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/storage/src/session.rs`

- [ ] Add a test that externally holds a session lock and verifies `append` cannot pass it until released.
- [ ] Run the test and confirm the unlocked implementation fails.
- [ ] Implement an RAII advisory file lock and hold it across session save, append, clear, and metadata read-modify-write operations.
- [ ] Avoid recursively reacquiring the same lock by using unlocked internal helpers.
- [ ] Re-run session tests.

### Task 5: Contact-store error handling and atomicity

**Files:**
- Modify: `crates/storage/src/contacts.rs`
- Modify: `crates/agent/src/runtime/process_message_inner.rs`
- Modify: `crates/tools/src/message.rs`

- [ ] Add a regression test proving invalid JSON is preserved rather than replaced by a subsequent upsert.
- [ ] Run the test and confirm the current implementation overwrites the damaged file.
- [ ] Add fallible load/save paths, advisory locking, and atomic replacement; return `Result` from upsert.
- [ ] Update callers to log or propagate contact persistence failures.
- [ ] Re-run contact and affected crate checks.

### Task 6: Database-enforced memory deduplication

**Files:**
- Modify: `crates/storage/src/memory/schema.rs`
- Modify: `crates/storage/src/memory/crud.rs`
- Modify: `crates/storage/src/memory/tests.rs`

- [ ] Add a two-connection test that attempts duplicate active `dedup_key` values and asserts only one active row remains.
- [ ] Run it and confirm duplicates are currently possible.
- [ ] Normalize legacy duplicates, create a partial unique index for active non-empty keys, and handle a raced insert by updating the winner.
- [ ] Enable a SQLite busy timeout for independent store handles.
- [ ] Re-run dedup and memory tests.

### Task 7: Fail-closed audit verification

**Files:**
- Modify: `crates/storage/src/audit.rs`

- [ ] Add a test appending a malformed non-empty line and assert chain verification is invalid.
- [ ] Run it and confirm verification currently reports valid.
- [ ] Record malformed records as verification errors while retaining `skipped_records` diagnostics.
- [ ] Re-run audit tests.

### Task 8: Workflow referential integrity

**Files:**
- Modify: `crates/storage/src/evolution_workflow.rs`

- [ ] Add tests showing steps and events cannot reference a missing workflow.
- [ ] Run them and confirm the current connection accepts orphan rows.
- [ ] Enable `PRAGMA foreign_keys=ON` when opening the workflow database.
- [ ] Replace manual recovery transaction control with an RAII transaction while touching the code, so intermediate errors roll back safely.
- [ ] Re-run workflow tests.

### Task 9: Full verification and exact commit

**Files:**
- Verify only files listed above.

- [ ] Run `cargo fmt --check` and format only touched Rust files if required.
- [ ] Run `cargo test -p blockcell-storage`.
- [ ] Run checks/tests for any caller crates changed by the contact API.
- [ ] Inspect `git diff --check`, `git status --short`, and the exact staged diff.
- [ ] Stage only files created or modified by this plan; do not stage pre-existing `.DS_Store`, demo, design, plan-directory, or other unrelated changes.
- [ ] Commit with a storage-integrity-focused message and report the commit hash.
