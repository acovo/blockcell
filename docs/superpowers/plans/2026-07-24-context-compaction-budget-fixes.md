# Context and Compaction Budget Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve complete current-user input, retain valid tool-call groups across compaction, and enforce configured recovery token budgets.

**Architecture:** Remove the fixed character-level transformation from current-turn context construction. Add a compaction suffix selector that expands a requested message count to complete assistant/tool groups. Build each recovery section inside its own exact estimated-token allowance, admitting recent records until the configured total is exhausted.

**Tech Stack:** Rust 2021, Tokio, BlockCell token estimator, Cargo tests.

---

### Task 1: Preserve current user input

**Files:**
- Modify: `crates/agent/src/context.rs`

- [x] **Step 1: Add a failing regression test**

Build a request longer than 4,000 characters with a unique middle marker, call `build_messages_for_session_mode_with_channel`, and assert the final user message equals the original request exactly.

- [x] **Step 2: Verify RED**

Run: `cargo test -p blockcell-agent preserves_long_current_user_input -- --nocapture`

Expected: failure because the middle is replaced by the trim marker.

- [x] **Step 3: Implement the minimal fix**

Construct text-only and multimodal current user messages from `user_content` without `trim_text_head_tail`; remove the now-unused helper.

- [x] **Step 4: Verify GREEN**

Run: `cargo test -p blockcell-agent preserves_long_current_user_input -- --nocapture`

Expected: pass.

### Task 2: Preserve complete tool groups after compact

**Files:**
- Modify: `crates/agent/src/runtime/compaction.rs`

- [x] **Step 1: Add a failing regression test**

Create an assistant message containing three tool calls followed by three tool results. Request retention of two messages and assert the selected suffix includes the assistant plus all three tool results.

- [x] **Step 2: Verify RED**

Run: `cargo test -p blockcell-agent compact_recent_messages_preserve_complete_tool_group -- --nocapture`

Expected: compilation failure because the selector does not exist.

- [x] **Step 3: Implement the selector**

Add `select_recent_messages(messages, keep_recent_messages)`. Start at `len - keep`, walk backward across leading tool results to their assistant tool-call message, and skip any incomplete assistant/tool prefix rather than returning invalid protocol. Use this helper in `execute_layer4_compact`.

- [x] **Step 4: Verify GREEN**

Run: `cargo test -p blockcell-agent compact_recent_messages_preserve_complete_tool_group -- --nocapture`

Expected: pass.

### Task 3: Enforce total recovery budgets

**Files:**
- Modify: `crates/agent/src/compact/mod.rs`

- [x] **Step 1: Add a failing regression test**

Create multiple file and skill records, configure small file and skill total budgets, build recovery, and assert the emitted recovery token estimate does not exceed the sum of those budgets.

- [x] **Step 2: Verify RED**

Run: `cargo test -p blockcell-agent compact_recovery_enforces_total_budgets -- --nocapture`

Expected: failure because all skills and count-limited files are appended regardless of total budgets.

- [x] **Step 3: Implement budgeted section assembly**

Add an estimator-backed prefix truncator and a helper that builds a section within an exact allowance including headings and fences. Apply independent remaining totals for files, skills, and session memory; stop admitting entries once an allocation is exhausted.

- [x] **Step 4: Verify GREEN**

Run: `cargo test -p blockcell-agent compact_recovery_enforces_total_budgets -- --nocapture`

Expected: pass.

### Task 4: Document, verify, and commit

**Files:**
- Modify: `docs/reviews/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/superpowers/plans/2026-07-24-context-compaction-budget-fixes.md`

- [x] **Step 1: Record R10-R12 resolutions**

Document the exact behavior and regression-test names, then check completed plan steps.

- [x] **Step 2: Run verification**

Run: `cargo fmt --all -- --check`

Run: `cargo test -p blockcell-agent context -- --nocapture`

Run: `cargo test -p blockcell-agent compact -- --nocapture`

Run: `cargo test -p blockcell-agent runtime -- --nocapture`

Expected: formatting and all tests pass.

- [x] **Step 3: Commit scoped files**

Stage only the three production files, their tests, the review documents, and this plan. Inspect the cached diff and commit with `fix: preserve context and enforce compact budgets`.
