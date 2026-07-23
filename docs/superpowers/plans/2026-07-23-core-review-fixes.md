# Core Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 `crates/core` Review 中发现的 8 项会话隔离、路径安全、工具策略、取消清理、文件锁和 MCP 持久化缺陷。

**Architecture:** 在路径边界将不安全 agent ID 映射为稳定目录名；为存在碰撞风险的 session stem 使用可逆安全编码并兼容读取旧文件；安全策略加载与编译采用 fail-closed。并发清理在锁外执行，取消链改为无深度限制的迭代检查，文件锁使用 OS advisory lock，MCP 保存复用原子写入工具。

**Tech Stack:** Rust、serde、glob、regex、SHA-256、标准文件系统 API、Cargo test。

---

### Task 1: Agent 路径和 Session 文件隔离

**Files:**
- Modify: `crates/core/src/paths.rs`
- Modify: `crates/core/src/session_key.rs`
- Modify: `crates/storage/src/session.rs`

- [x] **Step 1: Write the failing tests**

在 `paths.rs` 添加非法 agent ID 不得逃出 `agents` 根目录的测试；在 `session_key.rs` 添加 `ws:a/b`、`ws:a_b`、`ws:a\\b` stem 必须互不相同且只能包含安全字符的测试；在 storage session 测试中验证旧 stem 文件仍可读取。

- [x] **Step 2: Run tests to verify they fail**

Run: `cargo test -p blockcell-core paths::tests session_key::tests && cargo test -p blockcell-storage session`

Expected: 新增碰撞和路径校验测试失败。

- [x] **Step 3: Write minimal implementation**

让 `Paths::for_agent` 将不安全 ID 映射为稳定哈希目录；session stem 对风险字符使用带安全前缀的可逆字节转义，读取和删除路径兼容旧 stem。

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p blockcell-core paths::tests session_key::tests && cargo test -p blockcell-storage session`

Expected: PASS。

### Task 2: Tool policy fail-closed

**Files:**
- Modify: `crates/core/src/tool_policy.rs`
- Modify: `crates/core/tests/tool_policy.rs`

- [x] **Step 1: Write the failing tests**

添加策略文件不可读/不可解析时拒绝调用、无效 deny glob/regex/path glob 和未知继承组导致策略拒绝调用的测试。

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p blockcell-core --test tool_policy`

Expected: 失败，因为当前实现回退 Allow 或跳过无效规则。

- [x] **Step 3: Write minimal implementation**

增加 fail-closed policy；让编译过程返回错误，任何规则编译失败时整份文件策略进入 Deny，缺少策略文件仍保持兼容的 permissive 行为。

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test -p blockcell-core --test tool_policy`

Expected: PASS。

### Task 3: Abort 与 Cleanup 并发语义

**Files:**
- Modify: `crates/core/src/abort_token.rs`

- [x] **Step 1: Write the failing tests**

添加超过 16 层子 token 仍继承根取消的测试；添加 cleanup handler 可重入注册且不会阻塞的测试。

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p blockcell-core abort_token::tests`

Expected: 深链测试断言失败；可重入测试超时或无法完成。

- [x] **Step 3: Write minimal implementation**

让取消检查沿父链迭代遍历而不设深度上限；从 mutex 中先取出 cleanup handler，再在锁外执行。

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test -p blockcell-core abort_token::tests`

Expected: PASS。

### Task 4: 文件锁和 MCP 原子保存

**Files:**
- Modify: `crates/core/src/file_store.rs`
- Modify: `crates/core/src/mcp_config.rs`
- Modify: `crates/core/tests/standalone_mcp.rs`

- [x] **Step 1: Write the failing tests**

添加陈旧锁替换不会删除竞争者新锁的确定性测试钩子/状态测试；添加 MCP 保存复用原子替换且写入后得到完整 JSON 的测试。

- [x] **Step 2: Run tests to verify they fail**

Run: `cargo test -p blockcell-core file_store::tests && cargo test -p blockcell-core --test standalone_mcp`

Expected: 新增测试失败。

- [x] **Step 3: Write minimal implementation**

以 OS advisory lock 取代 PID 文件删除式抢锁，进程退出后由操作系统自动释放；MCP `save` 调用 `atomic_write`。

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p blockcell-core file_store::tests && cargo test -p blockcell-core --test standalone_mcp`

Expected: PASS。

### Task 5: 全量验证和提交

**Files:**
- Verify only all modified source and test files.

- [x] **Step 1: Format and run focused tests**

Run: `cargo fmt --check && cargo test -p blockcell-core && cargo test -p blockcell-storage`

Expected: 全部 PASS。

- [x] **Step 2: Inspect the exact diff**

Run: `git diff --check && git diff -- crates/core crates/storage docs/superpowers/plans/2026-07-23-core-review-fixes.md`

Expected: 无格式错误、无无关文件。

- [x] **Step 3: Commit only this fix set**

Run: `git add <本计划列出的精确文件> && git commit -m "fix(core): harden isolation policies and persistence"`

Expected: 提交成功，原有 `.DS_Store` 和其他未跟踪文档保持未提交。
