# Knowledge System Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Converge BlockCell durable knowledge onto canonical files, index those files in SQLite, merge legacy Layer 5 sources, resolve conflicting metadata deterministically, and provide audited forgetting that prevents deleted knowledge from returning.

**Architecture:** `USER.md` and `memory/MEMORY.md` become the only automatically writable durable-memory sources. A new `KnowledgeIndex` parses canonical entries and maintains disposable SQLite FTS rows; legacy SQLite long-term rows and Layer 5 files migrate into canonical files. Forgetting removes canonical entries, clears index/short-term rows, records tombstones, and makes automatic writers reject matching forgotten content.

**Tech Stack:** Rust, rusqlite/FTS5, serde, chrono, existing atomic file stores, Clap CLI, Cargo tests.

---

### Task 1: Canonical File Knowledge Index

**Files:**
- Create: `crates/storage/src/knowledge_index.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/core/src/paths.rs`
- Modify: `crates/agent/src/memory_file_store.rs`
- Modify: `crates/agent/src/runtime/wiring.rs`

- [ ] **Step 1: Write failing parser and rebuild tests**

Create `USER.md` and `memory/MEMORY.md`, call the desired API, and assert indexed entries carry `file`, `anchor`, `content_hash`, `scope`, `source`, and `updated_at`. Delete one file entry, rebuild, and assert search no longer returns it.

```rust
let index = KnowledgeIndex::open(&paths.knowledge_index_db())?;
assert_eq!(index.rebuild_from_files(&paths)?.indexed, 2);
assert_eq!(index.search("concise", 10)?.len(), 1);
std::fs::write(paths.user_md(), "")?;
assert_eq!(index.rebuild_from_files(&paths)?.removed, 1);
assert!(index.search("concise", 10)?.is_empty());
```

- [ ] **Step 2: Run RED test**

Run: `cargo test -p blockcell-storage knowledge_index -- --nocapture`

Expected: compilation fails because `KnowledgeIndex` and its path/API do not exist.

- [ ] **Step 3: Implement index and incremental rebuild**

Create `knowledge_entries`, external-content `knowledge_fts`, and `knowledge_files` tables. Use SHA-256 file/entry hashes. Parse metadata list entries when present and generate deterministic legacy IDs from `file + anchor + content` otherwise. Rebuild only changed files and remove rows belonging to deleted entries.

- [ ] **Step 4: Wire canonical file writes**

Give `MemoryFileStore` an optional `Arc<KnowledgeIndex>` and rebuild the affected file after successful add/replace/remove/restore. Attach the index in `AgentRuntime::init_memory_file_store`.

- [ ] **Step 5: Verify and commit**

Run `cargo test -p blockcell-storage knowledge_index`, `cargo test -p blockcell-agent memory_file_store`, and `cargo test -p blockcell-tools memory_manage`.

Commit: `重构：建立文件知识唯一事实源与索引`

### Task 2: Stop SQLite Durable Writes and Migrate Existing Rows

**Files:**
- Modify: `crates/tools/src/memory.rs`
- Modify: `crates/storage/src/memory.rs`
- Modify: `crates/storage/src/memory/query.rs`
- Modify: `bin/blockcell/src/commands/memory.rs`
- Modify: `bin/blockcell/src/main.rs`

- [ ] **Step 1: Write failing tests**

Assert `memory_upsert(scope="long_term")` is rejected with guidance to use `memory_manage`. Seed duplicate active SQLite long-term rows, run the desired migration API, and assert one metadata-prefixed entry is appended to `MEMORY.md`, legacy rows are retired, and reruns are idempotent.

- [ ] **Step 2: Run RED tests**

Run `cargo test -p blockcell-tools memory_upsert_rejects_long_term`, `cargo test -p blockcell-storage migrate_long_term`, and `cargo test -p blockcell memory_migrate_canonical`.

Expected: long-term upsert is accepted and the migration command is absent.

- [ ] **Step 3: Implement durable-write rejection**

Reject new `scope=long_term` rows in `MemoryUpsertTool::validate`; preserve short-term TTL behavior.

- [ ] **Step 4: Implement migration**

Export active long-term rows, deduplicate by normalized content hash, append entries in this format, rebuild the index, then soft-delete migrated rows only after the file write succeeds:

```markdown
- [id:migrated-<hash>] [scope:workspace] [source:verified] [updated:2026-08-02] <content> <!-- migrated-from:memory.db:<id> -->
```

- [ ] **Step 5: Add CLI and commit**

Add `blockcell memory migrate-canonical`, print migrated/deduplicated/retired counts, run targeted tests, and commit `迁移：收敛 SQLite 长期记忆到规范文件`.

### Task 3: Merge Legacy Layer 5 Sources and Resolve Conflicts

**Files:**
- Modify: `crates/agent/src/auto_memory/memory_type.rs`
- Modify: `crates/agent/src/auto_memory/injector.rs`
- Modify: `crates/agent/src/runtime/learning.rs`
- Modify: `crates/storage/src/knowledge_index.rs`
- Modify: `crates/agent/src/ghost_recall.rs`

- [ ] **Step 1: Write failing consolidation test**

Create legacy `memory/user.md`, `project.md`, `feedback.md`, and `reference.md`. Assert consolidation writes user entries once to `USER.md`, other categories under `## Project`, `## Feedback`, and `## Reference` in `MEMORY.md`, and canonical reload watches only those two files.

- [ ] **Step 2: Write failing conflict test**

Index:

```markdown
- [id:pref-old] [scope:user] [source:inferred] [updated:2026-07-01] 用户偏好详细回答
- [id:pref-new] [scope:user] [source:user_statement] [updated:2026-08-01] [supersedes:pref-old] 用户偏好简洁回答
```

Assert search returns only `pref-new`.

- [ ] **Step 3: Run RED tests**

Run `cargo test -p blockcell-agent layer5` and `cargo test -p blockcell-storage knowledge_index_conflict`.

- [ ] **Step 4: Implement consolidation and ordering**

Merge non-template legacy contents once with normalized paragraph deduplication. Load canonical files for Layer 5. Parse required metadata (`id`, `scope`, `source`, `updated`) and optional `supersedes`; search removes duplicate hashes and superseded IDs, then orders `user_statement > verified > inferred`, newer timestamps, and relevance.

- [ ] **Step 5: Verify and commit**

Run storage knowledge-index tests plus agent auto-memory and ghost-recall tests. Commit `重构：合并记忆文件并统一冲突排序`.

### Task 4: Unified `knowledge_forget`

**Files:**
- Create: `crates/tools/src/knowledge_forget.rs`
- Modify: `crates/tools/src/registry.rs`
- Modify: `crates/tools/src/lib.rs`
- Modify: `crates/storage/src/knowledge_index.rs`
- Modify: `crates/storage/src/memory.rs`
- Modify: `crates/agent/src/memory_file_store.rs`
- Modify: `crates/agent/src/ghost_background_review.rs`
- Modify: `crates/storage/src/ghost_ledger.rs`

- [ ] **Step 1: Write failing preview/confirm tests**

`knowledge_forget(action="preview", query="简洁回答")` must return canonical and SQLite short-term matches without mutation. `action="confirm"` with the preview token must remove those matches and create a tombstone.

- [ ] **Step 2: Write failing resurrection test**

Forget a preference, replay a Ghost Review write with the same normalized content, and assert the write is rejected and files remain unchanged.

- [ ] **Step 3: Run RED tests**

Run `cargo test -p blockcell-tools knowledge_forget` and `cargo test -p blockcell-agent forgotten_memory_is_not_recreated`.

- [ ] **Step 4: Implement two-phase forgetting**

Add `forgotten(dedup_key TEXT PRIMARY KEY, reason TEXT NOT NULL, forgotten_at TEXT NOT NULL)` to the knowledge-index DB. Preview returns a hash-bound token over exact affected IDs. Confirm validates it, atomically removes canonical entries, deletes index/vector rows, soft-deletes SQLite short-term/session matches, and inserts tombstones.

- [ ] **Step 5: Enforce tombstones and audit**

Before canonical add/replace and Ghost Review persistence, compute the same normalized hash and reject tombstoned content. Add GhostLedger forget-event audit records containing query, reason, affected sources, caller session, and timestamp.

- [ ] **Step 6: Verify and commit**

Run storage/tools/agent tests and commit `功能：统一遗忘知识并阻止后台复活`.

### Task 5: Documentation and Full Verification

**Files:**
- Modify: `docs/05_memory_system.md`
- Modify: `docs/en/05_memory_system.md`
- Modify: `docs/27_ghost_learning_design.md`

- [ ] **Step 1: Update docs**

Document canonical writable files, disposable index semantics, Layer 5 migration, metadata precedence, migration CLI, and `knowledge_forget` preview/confirm workflow.

- [ ] **Step 2: Run full verification**

```bash
cargo fmt --all -- --check
cargo test -p blockcell-core
cargo test -p blockcell-storage
cargo test -p blockcell-tools
cargo test -p blockcell-agent
cargo test -p blockcell
cargo check -p blockcell
git diff --check
```

- [ ] **Step 3: Commit docs**

Commit `文档：说明统一知识事实源与遗忘流程`.
