# Tool Path and Fork Authorization Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve PathPolicy hard-deny precedence and make default fork-mode execution structurally read-only.

**Architecture:** Split path authorization into hard-deny classification and confirm/allow handling, passing tool-policy approval only as satisfaction for `Confirm` paths. Define one canonical default-fork disallow list and use it for both advertised schemas and runtime execution enforcement.

**Tech Stack:** Rust 2021, Tokio, BlockCell AgentRuntime, forked-agent tool dispatcher.

---

### Task 1: Keep PathPolicy deny stronger than tool confirmation

**Files:**
- Modify: `crates/agent/src/runtime/path_security.rs:190-330`
- Modify: `crates/agent/src/runtime/tool_exec.rs:214-308`
- Test: `crates/agent/src/runtime/tests.rs`

- [x] **Step 1: Write the failing regression test**

Add `tool_policy_ask_cannot_override_builtin_path_deny`. Configure `write_file` as ToolPolicy `Ask`, target `~/.ssh/authorized_keys`, approve the tool confirmation, and assert the result is path-policy denial rather than registry execution.

```rust
assert!(result.contains("Path access denied"));
assert!(!result.contains("Unknown tool"));
```

- [x] **Step 2: Run the test and verify RED**

Run: `cargo test -p blockcell-agent --lib tool_policy_ask_cannot_override_builtin_path_deny -- --nocapture`

Expected: FAIL because approval currently caches the sensitive directory before PathPolicy evaluation.

- [x] **Step 3: Implement deny-first path evaluation**

Add an internal `check_path_permission_with_confirmation(..., policy_confirmed: bool)` method. Evaluate `PathPolicy::Deny` before workspace and cached authorization; use `policy_confirmed` only to approve and optionally cache paths whose policy result is `Confirm`. Keep `check_path_permission` as a wrapper passing `false`, and stop pre-authorizing paths in the ToolPolicy `ProceedConfirmed` branch.

```rust
let policy_confirmed = matches!(outcome, PolicyOutcome::ProceedConfirmed);
if !self
    .check_path_permission_with_confirmation(name, args, msg, policy_confirmed)
    .await
{
    return path_access_denied(name, "outside workspace");
}
```

- [x] **Step 4: Run focused path/tool-policy tests**

Run: `cargo test -p blockcell-agent --lib tool_policy_ -- --nocapture`

Expected: all matching tests pass, including single-confirm behavior.

### Task 2: Make default fork mode read-only by enforcement

**Files:**
- Modify: `crates/agent/src/forked/agent/event.rs:110-282`
- Modify: `crates/agent/src/forked/mod.rs:33-38`
- Modify: `crates/agent/src/runtime/lightweight_handle.rs:185-292`
- Modify: `crates/agent/src/runtime/fork_spawn.rs:34-147`
- Test: `crates/agent/src/forked/agent/tests.rs`

- [x] **Step 1: Write the failing capability test**

Add a test for `default_read_only_fork_disallowed_tools()` asserting it contains `exec`, `edit_file`, `file_edit`, `write_file`, and `file_write`; build schemas with that list and assert only `read_file`, `list_dir`, `grep`, and `glob` remain.

```rust
assert_eq!(
    schema_names(&schemas),
    vec!["read_file", "list_dir", "grep", "glob"]
);
```

- [x] **Step 2: Run the test and verify RED**

Run: `cargo test -p blockcell-agent --lib default_fork_capabilities_are_structurally_read_only -- --nocapture`

Expected: FAIL because the canonical read-only disallow helper does not exist and default schemas include mutation tools.

- [x] **Step 3: Implement and wire the canonical disallow list**

Create `default_read_only_fork_disallowed_tools()` in forked agent event support and use the same vector for `.disallowed_tools(...)` and `build_forked_tool_schemas(...)` in both runtime fork entry points.

```rust
pub fn default_read_only_fork_disallowed_tools() -> Vec<String> {
    ["agent", "spawn", "exec", "edit_file", "file_edit", "write_file", "file_write"]
        .into_iter()
        .map(str::to_string)
        .collect()
}
```

- [x] **Step 4: Run focused fork tests**

Run: `cargo test -p blockcell-agent --lib fork -- --nocapture`

Expected: all fork tests pass.

### Task 3: Verify, document, and commit

**Files:**
- Modify: `docs/reviews/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/superpowers/plans/2026-07-24-tool-path-and-fork-authorization-fixes.md`

- [x] **Step 1: Run regression verification**

Run: `cargo test -p blockcell-agent runtime -- --nocapture`

Expected: all runtime tests pass.

Run: `cargo test -p blockcell-agent --lib fork -- --nocapture`

Expected: all fork tests pass.

Run: `cargo fmt --all -- --check`

Expected: exit 0.

- [x] **Step 2: Mark R6/R7 fixed and record regression tests**

Update both resolutions and the review progress without changing the unfinished cancellation checklist.

- [x] **Step 3: Commit only scoped files**

```bash
git add crates/agent/src/runtime/path_security.rs \
  crates/agent/src/runtime/tool_exec.rs \
  crates/agent/src/runtime/tests.rs \
  crates/agent/src/forked/agent/event.rs \
  crates/agent/src/forked/agent/tests.rs \
  crates/agent/src/forked/mod.rs \
  crates/agent/src/runtime/lightweight_handle.rs \
  crates/agent/src/runtime/fork_spawn.rs \
  docs/reviews/2026-07-24-agent-runtime-learning-review.md \
  docs/superpowers/plans/2026-07-24-agent-runtime-learning-review.md \
  docs/superpowers/plans/2026-07-24-tool-path-and-fork-authorization-fixes.md
git commit -m "fix: enforce path denies and read-only forks"
```
