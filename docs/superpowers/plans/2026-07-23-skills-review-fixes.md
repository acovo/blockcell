# Skills Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 `crates/skills` Review 中发现的 8 项执行边界、注册表并发、技能重载和版本导入缺陷。

**Architecture:** 将 Rhai 和外部进程的资源限制放在实际执行入口；将能力执行拆成“锁内准备、锁外 await、锁内记录”；技能重载使用临时状态/失败回滚；所有递归目录及归档导入拒绝软链接并限制资源规模。

**Tech Stack:** Rust、Tokio、Rhai、tar/flate2、Cargo tests。

---

### Task 1: 限制 Rhai 和外部能力执行

**Files:**
- Modify: `crates/skills/src/dispatcher.rs`
- Modify: `crates/skills/src/capability_provider.rs`

- [ ] **Step 1: Write failing Rhai operation-budget and provider-timeout tests**

在现有测试模块加入：一个超过操作预算的有限 Rhai 循环应返回失败；ProcessProvider 和 ScriptProvider 运行超时脚本应在配置时限附近返回错误。

- [ ] **Step 2: Run focused tests and verify RED**

Run: `cargo test -p blockcell-skills dispatcher::tests::test_dispatcher_enforces_operation_budget capability_provider::tests::test_process_provider_times_out capability_provider::tests::test_script_provider_times_out`

Expected: tests fail because dispatcher has no budget and providers have no working timeout configuration.

- [ ] **Step 3: Implement bounded execution**

在 dispatcher 的 Engine 上注册 `on_progress`，同时检查最大操作数和 elapsed timeout。为两个 provider 增加可配置超时、`kill_on_drop(true)`，以限长 reader 持续排空 stdout/stderr，仅保留固定上限，并在超时后终止子进程。

- [ ] **Step 4: Run focused tests and verify GREEN**

Run the focused tests above; expected PASS.

### Task 2: 修复能力状态门禁和全局锁竞争

**Files:**
- Modify: `crates/skills/src/capability_provider.rs`
- Modify: `crates/agent/src/capability_adapter.rs`

- [ ] **Step 1: Write failing unavailable-status and concurrent-execution tests**

加入测试验证 Unavailable/Deprecated descriptor 即使仍有 executor 也不能执行；并验证通过共享 handle 执行慢能力时，注册表 stats/list 锁仍可及时获取。

- [ ] **Step 2: Run focused tests and verify RED**

Run: `cargo test -p blockcell-skills capability_provider::tests::test_unavailable_capability_is_not_executable` and the adapter test target.

Expected: unavailable capability currently executes; adapter currently holds mutex across await.

- [ ] **Step 3: Implement prepare/execute/record flow**

增加锁内 executor/status 检查和锁内 canary 结果记录接口；共享 handle 的执行函数先 clone executor 后释放锁，执行完成后重新加锁记录结果。adapter 改用该函数。

- [ ] **Step 4: Run focused tests and verify GREEN**

Run focused tests; expected PASS.

### Task 3: 修复技能扫描和重载一致性

**Files:**
- Modify: `crates/skills/src/manager.rs`

- [ ] **Step 1: Write failing reload, malformed-skill and symlink tests**

加入测试验证：删除 workspace skill 后 reload 会移除它；损坏 meta 的技能不会阻断同目录其他技能；脚本检测和 pack 扫描不会跟随目录 symlink。

- [ ] **Step 2: Run focused tests and verify RED**

Run the three new manager tests; expected failures for stale skills, scan abortion, or symlink traversal.

- [ ] **Step 3: Implement reconciled and symlink-safe scanning**

reload 前临时清空技能集合，失败时恢复旧集合；单个技能加载错误记录 warning 后继续；使用 `DirEntry::file_type` 拒绝 symlink，递归脚本检测同样跳过 symlink。

- [ ] **Step 4: Run focused tests and verify GREEN**

Run manager tests; expected PASS.

### Task 4: 加固版本归档导入和递归复制

**Files:**
- Modify: `crates/skills/src/versioning.rs`

- [ ] **Step 1: Write failing malicious-archive and resource-limit tests**

构造包含 symlink entry、过多 entry 或声明超限文件的 tar.gz，验证 import 拒绝且不创建版本快照。

- [ ] **Step 2: Run focused tests and verify RED**

Run new versioning import security tests; expected current implementation to accept at least the symlink archive.

- [ ] **Step 3: Implement validated extraction**

逐 entry 校验相对路径必须位于指定 skill 根下，拒绝 symlink/hardlink/特殊文件，限制 entry 数、单文件大小和累计大小；使用 `unpack_in`。递归复制通过 `symlink_metadata` 明确拒绝链接。

- [ ] **Step 4: Run focused tests and verify GREEN**

Run versioning tests; expected PASS.

### Task 5: 全量验证和提交

**Files:**
- Verify only the files changed above and this plan.

- [ ] **Step 1: Format and test**

Run: `cargo fmt --check`, `cargo test -p blockcell-skills`, and relevant `blockcell-agent` tests/checks.

- [ ] **Step 2: Inspect diff and worktree scope**

Run: `git diff --check`, `git status --short`, and `git diff -- crates/skills/src crates/agent/src/capability_adapter.rs`.

- [ ] **Step 3: Commit only this task's files**

Stage explicit paths only and commit with message `fix(skills): harden execution and reload boundaries`.
