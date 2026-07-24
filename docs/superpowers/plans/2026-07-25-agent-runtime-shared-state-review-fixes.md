# Agent Runtime Shared-State Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 review 中的 R17–R19，使父级取消能终结普通 subagent、后台记忆提取 panic 后能清理占用标记，并让零容量 ResponseCache 真正禁用缓存。

**Architecture:** TaskManager 提供带原因的统一取消终态入口，普通 subagent 的父级取消分支复用该入口。记忆提取 marker/journal 使用拥有路径的 RAII guard，在正常返回和 panic unwind 时清理，Auto Memory 游标保存失败时显式保留。ResponseCache 在生成引用前短路零容量配置。

**Tech Stack:** Rust 2021、Tokio、TaskManager、filesystem marker/journal、cargo test。

---

### Task 1: 普通 subagent 父级取消终态

**Files:**
- Modify: `crates/agent/src/task_manager.rs`
- Modify: `crates/agent/src/runtime/subagent.rs`
- Test: `crates/agent/src/task_manager.rs`

- [x] **Step 1: 写失败测试**

新增测试，创建 Running 任务后调用 `set_cancelled(task_id, "父任务已取消")`，断言状态为 `Cancelled`、错误原因被保留、AbortToken 已取消。

- [x] **Step 2: 验证测试失败**

Run: `cargo test -p blockcell-agent set_cancelled_records_reason_and_cancels_token -- --nocapture`

Expected: FAIL，因为 `TaskManager::set_cancelled` 尚不存在。

- [x] **Step 3: 最小实现**

在 TaskManager 中新增统一入口：

```rust
pub async fn set_cancelled(&self, task_id: &str, reason: &str) -> Result<(), Error> {
    // Running/Queued -> Cancelled，记录 reason，取消并注销 token，发送生命周期事件并持久化。
}
```

让 `cancel_task` 委托给该方法；在 `run_subagent_task` 的 `token.cancelled()` 分支调用：

```rust
let _ = task_manager.set_cancelled(&task_id, "父任务已取消").await;
return;
```

- [x] **Step 4: 验证通过**

Run: `cargo test -p blockcell-agent set_cancelled_records_reason_and_cancels_token -- --nocapture`

Expected: PASS。

### Task 2: detached memory extraction panic 清理

**Files:**
- Modify: `crates/agent/src/memory_system/mod.rs`
- Modify: `crates/agent/src/runtime/process_message_inner.rs`
- Test: `crates/agent/src/memory_system/mod.rs`

- [x] **Step 1: 写失败测试**

新增测试，在临时目录创建 marker/journal，将 `ExtractionMarkerGuard` 放入 `catch_unwind` 的 panic 闭包，断言两个文件都被删除；再测试 `preserve()` 后文件保留。

- [x] **Step 2: 验证测试失败**

Run: `cargo test -p blockcell-agent extraction_marker_guard -- --nocapture`

Expected: FAIL，因为 guard 尚不存在。

- [x] **Step 3: 最小实现**

实现拥有 marker/journal 路径的 guard：

```rust
pub(crate) struct ExtractionMarkerGuard {
    marker_path: PathBuf,
    journal_path: PathBuf,
    cleanup_on_drop: bool,
}
```

`Drop` 在 `cleanup_on_drop` 时尽力删除两文件，`preserve()` 关闭清理。Session Memory 和 Auto Memory spawn 闭包开头创建 guard，删除分散的尾部清理；Auto Memory 仅在 `result.success && result.cursor_save_failed` 时调用 `preserve()`。

- [x] **Step 4: 验证通过**

Run: `cargo test -p blockcell-agent extraction_marker_guard -- --nocapture`

Expected: PASS。

### Task 3: ResponseCache 零容量契约

**Files:**
- Modify: `crates/agent/src/response_cache.rs`
- Test: `crates/agent/src/response_cache.rs`

- [x] **Step 1: 写失败测试**

构造 `cache_max_per_session: 0` 的缓存，传入满足缓存条件的列表，断言 `maybe_cache_and_stub` 返回 `None` 且 session 中没有记录。

- [x] **Step 2: 验证测试失败**

Run: `cargo test -p blockcell-agent zero_capacity_disables_response_cache -- --nocapture`

Expected: FAIL，当前实现仍返回缓存 stub。

- [x] **Step 3: 最小实现**

在 `maybe_cache_and_stub` 获取配置锁后、生成最终缓存行为前加入零容量短路，保证不插入条目且不返回引用。

- [x] **Step 4: 验证通过**

Run: `cargo test -p blockcell-agent zero_capacity_disables_response_cache -- --nocapture`

Expected: PASS。

### Task 4: 回归验证、文档和提交

**Files:**
- Modify: `docs/reviews/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/superpowers/plans/2026-07-24-agent-runtime-learning-review.md`
- Modify: `docs/superpowers/plans/2026-07-25-agent-runtime-shared-state-review-fixes.md`

- [x] **Step 1: 运行聚焦测试**

Run: `cargo test -p blockcell-agent subagent -- --nocapture`

Run: `cargo test -p blockcell-agent memory_system -- --nocapture`

Run: `cargo test -p blockcell-agent response_cache -- --nocapture`

- [x] **Step 2: 运行完整 Agent 测试和格式检查**

Run: `cargo test -p blockcell-agent --all-targets`

Run: `cargo fmt --all -- --check`

- [x] **Step 3: 更新 review resolution 和计划勾选状态**

将 R17–R19 标记为 Fixed，记录回归测试名称和验证结果。

- [x] **Step 4: 仅暂存本次相关文件并提交**

```bash
git add crates/agent/src/task_manager.rs \
  crates/agent/src/runtime/subagent.rs \
  crates/agent/src/memory_system/mod.rs \
  crates/agent/src/runtime/process_message_inner.rs \
  crates/agent/src/response_cache.rs \
  docs/reviews/2026-07-24-agent-runtime-learning-review.md \
  docs/superpowers/plans/2026-07-24-agent-runtime-learning-review.md \
  docs/superpowers/plans/2026-07-25-agent-runtime-shared-state-review-fixes.md
git commit -m "修复：完善子任务取消与后台清理"
```
