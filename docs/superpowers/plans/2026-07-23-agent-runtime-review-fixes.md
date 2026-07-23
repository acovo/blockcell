# Agent Runtime Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 `crates/agent` review 中确认的八项会话隔离、任务生命周期、checkpoint 与持久化缺陷。

**Architecture:** 在 `TaskManager` 和 `CheckpointManager` 中加入显式 origin scope，并让斜杠命令、子代理汇总和恢复路径只操作当前来源的数据。消息任务在工具轮次保存 checkpoint、成功后标记完成；所有持久化统一复用唯一临时文件的原子写入实现。

**Tech Stack:** Rust, Tokio, Serde, Cargo test

**Status:** Completed on local `main`; all planned verification passed.

---

### Task 1: Task origin scope

**Files:**
- Modify: `crates/agent/src/task_manager.rs`
- Modify: `bin/blockcell/src/commands/slash_commands/handlers/tasks.rs`

- [ ] **Step 1: Write failing tests**

```rust
assert_eq!(manager.list_tasks_for_origin("ws", "chat-a", None).await.len(), 1);
assert!(manager.find_task_by_prefix_for_origin("foreign", "ws", "chat-a").await.is_empty());
```

- [ ] **Step 2: Verify RED**

Run: `cargo test -p blockcell-agent task_manager_origin -- --nocapture`
Expected: FAIL because scoped APIs do not exist.

- [ ] **Step 3: Implement scoped APIs and route `/tasks` through them**

```rust
fn belongs_to_origin(task: &TaskInfo, channel: &str, chat_id: &str) -> bool {
    task.origin_channel == channel && task.origin_chat_id == chat_id
}
```

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p blockcell-agent task_manager_origin`
Expected: PASS.

### Task 2: Scope typed-agent aggregation

**Files:**
- Modify: `crates/agent/src/runtime/process_message_inner.rs`
- Test: `crates/agent/src/runtime/tests.rs`

- [ ] **Step 1: Add a failing test proving another chat's completed typed task is excluded.**
- [ ] **Step 2: Run the focused test and observe the foreign task is returned.**
- [ ] **Step 3: Filter running/completed tasks by `msg.channel`, `msg.chat_id`, and routed agent identity before waiting or injecting.**
- [ ] **Step 4: Re-run the focused test and verify PASS.**

### Task 3: Checkpoint ownership and lifecycle

**Files:**
- Modify: `crates/agent/src/checkpoint.rs`
- Modify: `crates/agent/src/runtime/process_message_inner.rs`
- Modify: `crates/agent/src/runtime/message_task.rs`
- Modify: `crates/agent/src/runtime/run_loop.rs`
- Modify: `bin/blockcell/src/commands/slash_commands/handlers/tasks.rs`

- [ ] **Step 1: Add failing tests for owner filtering, owner mismatch, checkpoint progress saves, and completion marking.**
- [ ] **Step 2: Verify the new tests fail.**
- [ ] **Step 3: Add origin fields with serde defaults and ownership helpers.**

```rust
#[serde(default)] pub origin_channel: String,
#[serde(default)] pub origin_chat_id: String,
#[serde(default)] pub session_key: String,
```

- [ ] **Step 4: Save after committed tool results and mark the task completed after successful final delivery.**
- [ ] **Step 5: Reject resume when checkpoint origin does not equal the command/message origin.**
- [ ] **Step 6: Verify focused checkpoint and runtime tests pass.**

### Task 4: Runtime shutdown cleanup

**Files:**
- Modify: `crates/agent/src/runtime/run_loop.rs`

- [ ] **Step 1: Add a failing run-loop test where dropping inbound sender must cancel an active message task.**
- [ ] **Step 2: Verify RED.**
- [ ] **Step 3: Invoke the same active-task cleanup used by explicit shutdown in the inbound-closed branch.**
- [ ] **Step 4: Verify GREEN.**

### Task 5: Persist subagent delivery

**Files:**
- Modify: `crates/agent/src/runtime.rs`
- Modify: `crates/agent/src/runtime/subagent.rs`
- Test: `crates/agent/src/runtime/tests.rs`

- [ ] **Step 1: Add a failing test that delivers a result and reloads it from the origin session.**
- [ ] **Step 2: Verify RED.**
- [ ] **Step 3: Pass `origin_session_key` into `run_subagent_task`, log seed-save errors, and supply that key to delivery.**
- [ ] **Step 4: Verify GREEN.**

### Task 6: Atomic task/checkpoint persistence

**Files:**
- Modify: `crates/agent/src/task_manager/persistence.rs`
- Modify: `crates/agent/src/checkpoint.rs`
- Test: `crates/agent/src/task_manager.rs`
- Test: `crates/agent/src/checkpoint.rs`

- [ ] **Step 1: Add concurrent persistence tests that require valid final JSON and no shared `.json.tmp` path failures.**
- [ ] **Step 2: Verify at least the checkpoint concurrency test fails on the fixed temp path.**
- [ ] **Step 3: Use `crate::fs_util::atomic_write` via `spawn_blocking` for tasks and directly for checkpoints; log all errors with target paths.**
- [ ] **Step 4: Verify focused persistence tests pass.**

### Task 7: Full verification and commit

**Files:**
- Modify only files listed above and this plan.

- [ ] **Step 1: Run `cargo fmt --all -- --check`, then format if needed.**
- [ ] **Step 2: Run `cargo test -p blockcell-agent`.**
- [ ] **Step 3: Run slash-command package tests or `cargo test -p blockcell` for changed command routing.**
- [ ] **Step 4: Inspect `git diff` and `git status`; stage only this plan and changed agent/command files.**
- [ ] **Step 5: Commit with `fix(agent): isolate tasks and harden recovery`.**
