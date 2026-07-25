# Ghost Learning and Guarded Write Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix confirmed findings M9-M15 by isolating automatic Ghost memory, sharing learning coordination across message runtimes, cancelling reviews on lease loss, and making learned-skill mutations path-safe, scan-complete, rollback-correct, and retry-safe.

**Architecture:** Automatic Ghost writes use session-scoped file-memory directories derived from the stable session hash, while explicit global `USER.md`/`MEMORY.md` remain available and recall merges current-session plus global content. The long-lived runtime shares one `LearningCoordinator` with per-message runtimes. Ghost lease heartbeat owns an abort token consumed by the provider/tool loop. `SkillFileStore` canonicalizes its root, rejects symlinks in every target chain, snapshots whole skill directories, scans final candidates, stages creates/restores, and treats post-commit cache/toggle maintenance as warnings rather than failed mutations.

**Tech Stack:** Rust, Tokio, SQLite Ghost ledger, filesystem atomic rename/fsync, Cargo tests.

---

### Task 1: Isolate automatic Ghost file memory by session

**Files:**
- Modify: `crates/agent/src/memory_file_store.rs`
- Modify: `crates/agent/src/ghost_background_review.rs`
- Modify: `crates/agent/src/ghost_recall.rs`
- Modify: `crates/agent/src/ghost_memory_provider.rs`
- Test: inline unit tests in the same modules

- [x] **Step 1: Write failing session-isolation tests**

Add tests proving `MemoryFileStore::open_for_session(&paths, "cli:a")` and `cli:b` write different files, Ghost background review for A does not update global files, and recall for B cannot see A while A can still see explicit global memory.

```rust
let a = MemoryFileStore::open_for_session(&paths, "cli:a").unwrap();
let b = MemoryFileStore::open_for_session(&paths, "cli:b").unwrap();
a.add(MemoryFileTarget::Memory, "private-a").unwrap();
assert!(a.load_snapshot().unwrap().memory_block.unwrap().contains("private-a"));
assert!(b.load_snapshot().unwrap().memory_block.is_none());
```

- [x] **Step 2: Run tests and verify RED**

Run: `cargo test -p blockcell-agent ghost_session -- --nocapture`
Expected: FAIL because scoped store/recall APIs do not exist and Ghost review writes global files.

- [x] **Step 3: Implement scoped store and merged recall**

Add a stable hashed directory below `memory/sessions/<hash>/`, use it in Ghost review when `snapshot.session_key` is present, preserve the original session key in `ToolContext`, and make recall search current-session files plus explicit global files.

```rust
pub fn open_for_session(paths: &Paths, session_key: &str) -> Result<Self> {
    let root = paths.memory_dir().join("sessions")
        .join(blockcell_core::stable_hash_session_key(session_key));
    Self::open_at(root.join("USER.md"), root.join("MEMORY.md"), root)
}
```

- [x] **Step 4: Run focused tests and verify GREEN**

Run: `cargo test -p blockcell-agent ghost_session -- --nocapture`
Expected: PASS.

### Task 2: Share learning coordinator across per-message runtimes

**Files:**
- Modify: `crates/agent/src/runtime/run_loop.rs`
- Modify: `crates/agent/src/runtime/message_task.rs`
- Modify: `crates/agent/src/runtime.rs`
- Test: `crates/agent/src/runtime/tests.rs`

- [x] **Step 1: Write failing lifecycle tests**

Construct separate runtimes that receive the same `Arc<LearningCoordinator>`, record one turn each, and prove the third runtime observes the configured memory threshold. Add an `Arc::ptr_eq` assertion for the run-loop handoff helper.

- [x] **Step 2: Run tests and verify RED**

Run: `cargo test -p blockcell-agent shared_learning_coordinator -- --nocapture`
Expected: FAIL because each Runtime owns a fresh coordinator.

- [x] **Step 3: Pass the shared coordinator into message tasks**

Clone `self.learning_coordinator` in `run_loop`, add it to `run_message_task`, and replace the newly constructed Runtime's coordinator before processing the message.

```rust
let learning_coordinator = Arc::clone(&self.learning_coordinator);
// run_message_task(..., learning_coordinator, ...)
runtime.learning_coordinator = learning_coordinator;
```

- [x] **Step 4: Run focused tests and verify GREEN**

Run: `cargo test -p blockcell-agent shared_learning_coordinator -- --nocapture`
Expected: PASS.

### Task 3: Cancel Ghost review work when its lease is lost

**Files:**
- Modify: `crates/agent/src/ghost_background_review.rs`
- Test: inline tests in `crates/agent/src/ghost_background_review.rs`

- [x] **Step 1: Write failing cancellation tests**

Use a provider that waits or returns a `memory_manage` tool call and a pre-cancelled lease token. Assert the review returns a lease-loss error and no file-memory mutation occurs.

- [x] **Step 2: Run tests and verify RED**

Run: `cargo test -p blockcell-agent ghost_review_stops_after_lease_loss -- --nocapture`
Expected: FAIL because the tool loop ignores lease ownership.

- [x] **Step 3: Wire lease cancellation through the review stack**

Create an `AbortToken` per claimed episode. Cancel it when heartbeat returns `false` or reaches the failure threshold. Use `tokio::select!` around provider calls and check cancellation immediately before every side-effecting tool execution.

```rust
let response = tokio::select! {
    _ = lease_abort.cancelled() => return Err(Error::Storage("Lost review lease".into())),
    result = provider.chat(&messages, &tools) => result?,
};
```

- [x] **Step 4: Run focused tests and verify GREEN**

Run: `cargo test -p blockcell-agent ghost_review_stops_after_lease_loss -- --nocapture`
Expected: PASS.

### Task 4: Reject symlinked skill target chains

**Files:**
- Modify: `crates/agent/src/skill_file_store.rs`
- Test: inline Unix tests in `crates/agent/src/skill_file_store.rs`

- [x] **Step 1: Write failing symlink tests**

On Unix, create a symlinked skill directory and a symlinked auxiliary parent pointing outside the skills root. Assert `view`, `patch`, and `write_file` return validation errors and the external file is unchanged.

- [x] **Step 2: Run tests and verify RED**

Run: `cargo test -p blockcell-agent skill_file_store_rejects_symlink -- --nocapture`
Expected: FAIL because current resolution follows symlinks.

- [x] **Step 3: Canonicalize root and validate every component**

Store a canonical `skills_dir`, reject `symlink_metadata(...).file_type().is_symlink()` for every existing component between root and target, verify canonical existing targets remain below root, and apply the same validation to auxiliary paths before reads/writes/removes.

- [x] **Step 4: Run focused tests and verify GREEN**

Run: `cargo test -p blockcell-agent skill_file_store_rejects_symlink -- --nocapture`
Expected: PASS.

### Task 5: Scan composed skill patches

**Files:**
- Modify: `crates/agent/src/skill_file_store.rs`
- Test: inline tests in `crates/agent/src/skill_file_store.rs`

- [x] **Step 1: Write the failing split-payload test**

Create a safe skill containing `Ignore previous PLACEHOLDER`, patch `PLACEHOLDER` to `instructions`, and assert the patch is rejected while the file remains unchanged.

- [x] **Step 2: Run test and verify RED**

Run: `cargo test -p blockcell-agent skill_file_store_patch_scans_composed_content -- --nocapture`
Expected: FAIL because only the replacement fragment is scanned.

- [x] **Step 3: Scan the final candidate**

Move scanning after `patch_skill_content` and call `scan_learned_skill_content(&next)` before snapshot/write.

- [x] **Step 4: Run test and verify GREEN**

Run: `cargo test -p blockcell-agent skill_file_store_patch_scans_composed_content -- --nocapture`
Expected: PASS.

### Task 6: Make skill snapshots/restores exact and secure

**Files:**
- Modify: `crates/agent/src/skill_file_store.rs`
- Test: inline tests in `crates/agent/src/skill_file_store.rs`

- [x] **Step 1: Write failing exact-restore tests**

Snapshot a skill, add `scripts/new.py`, restore, and assert the added file is absent. Tamper a snapshot with unsafe content and assert restore is rejected without changing the live skill.

- [x] **Step 2: Run tests and verify RED**

Run: `cargo test -p blockcell-agent skill_file_store_restore_is_exact -- --nocapture`
Expected: FAIL because restore overlays and does not scan.

- [x] **Step 3: Snapshot full directories and atomically replace on restore**

Make `snapshot_before_write` copy the complete skill directory. Restore into a fresh sibling staging directory, scan it with `scan_learned_skill_dir`, rename the live directory aside, rename staging into place, restore the old directory if commit fails, then remove the temporary old directory after success.

- [x] **Step 4: Run focused tests and verify GREEN**

Run: `cargo test -p blockcell-agent skill_file_store_restore_is_exact -- --nocapture`
Expected: PASS.

### Task 7: Eliminate ambiguous partial skill commits

**Files:**
- Modify: `crates/agent/src/skill_file_store.rs`
- Test: inline tests in `crates/agent/src/skill_file_store.rs`

- [x] **Step 1: Write failing post-commit failure tests**

Create an undeletable/non-file prompt snapshot path and assert create/edit still return a committed success when only cache invalidation fails. Assert create never exposes a directory containing only `SKILL.md` by staging the complete directory before rename.

- [x] **Step 2: Run tests and verify RED**

Run: `cargo test -p blockcell-agent skill_file_store_post_commit -- --nocapture`
Expected: FAIL because current methods return `Err` after the primary mutation is visible.

- [x] **Step 3: Stage create and make maintenance best-effort**

Write `SKILL.md` and metadata into a sibling staging directory, fsync, then rename once. After any committed mutation, log toggle/cache maintenance failures instead of converting the committed mutation into `Err`.

- [x] **Step 4: Run focused tests and verify GREEN**

Run: `cargo test -p blockcell-agent skill_file_store_post_commit -- --nocapture`
Expected: PASS.

### Task 8: Full verification, documentation, and commit

**Files:**
- Modify: `docs/reviews/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/superpowers/plans/2026-07-24-agent-runtime-learning-review.md`
- Modify: this plan

- [x] Run `cargo fmt --all -- --check`.
- [x] Run `cargo test -p blockcell-agent ghost -- --nocapture`.
- [x] Run `cargo test -p blockcell-agent learning -- --nocapture`.
- [x] Run `cargo test -p blockcell-agent skill_file_store -- --nocapture`.
- [x] Run `cargo test -p blockcell-storage ghost_ledger -- --nocapture`.
- [x] Run `cargo test -p blockcell-agent runtime -- --nocapture`.
- [x] Run `git diff --check` and inspect the scoped diff.
- [x] Mark M9-M15 resolved with regression-test names.
- [x] Stage only scoped implementation, tests, and review-plan documents.
- [x] Commit with Chinese message `修复：隔离 Ghost 学习并强化技能写入安全`.

### Task 9: Continue Module 5 Task 6 review

**Files:**
- Inspect the Task 6 files in `docs/superpowers/plans/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/reviews/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/superpowers/plans/2026-07-24-agent-runtime-learning-review.md`

- [ ] Review the complete skill-evolution lifecycle from observation through candidate generation, audit, compilation, tests, versioning, deployment, canary promotion, and rollback.
- [ ] Check state-machine transitions, concurrent evolution, trust/signature policy, restart recovery, and generated-code tool/path escape.
- [ ] Run focused skills and scheduler tests.
- [ ] Record all confirmed defects in the review MD without fixing them in the review phase.
