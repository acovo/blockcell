# 第 27 篇：Ghost Native 学习闭环技术设计

> 本文档描述 BlockCell 当前 Ghost 学习系统的工程设计。
> 重点不是旧式“自进化资产流水线”，而是 runtime 内嵌的学习闭环：在对话边界捕获经验，通过受限工具写入文件化长期知识，并在后续 session 中稳定召回。

---

## 0. 背景与边界

Ghost 学习的产品目标是让助手在真实使用中持续变聪明：记住用户偏好、项目事实、环境坑点和可复用工作流，并在下一次相似任务中自动受益。

当前实现不再走“生成中间资产、等待审批、再发布”的旧式流水线。Ghost 学习不是独立 daemon 主导的训练系统，而是 agent runtime 的嵌入式工作流，触发点来自 turn end、pre-compress、session rotate、session end、delegation end 等生命周期边界。

pre-compress、session rotate、session end 与 delegation end 使用统一的边界学习任务：
一次读取并冻结历史快照，合并 session summary、durable memory、user preferences 和
skill candidate 请求。后台提供方调用总预算最多为 2 次（一次分类，最多一次深化），Dream
仍保持独立的低频维护节奏。审计记录包含请求输出、调用次数、成功分发目标、部分失败与停止原因。

---

## 1. 目标与非目标

### 1.1 目标

- 从成功任务和用户纠正中捕获长期有效的经验。
- 将事实类知识写入 `USER.md` 或 `memory/MEMORY.md`。
- 将声明性知识交给 Ghost Background Review；方法类知识由主 Agent 或独立 Skill Learning 通道沉淀为 `skills/<name>/SKILL.md`。
- 在后续对话中通过 prompt snapshot、recall 和 learned skill 自动使用已学知识。
- 后台学习失败不影响主响应。
- 所有自动写入都可审计、可撤销、有安全扫描。

### 1.2 非目标

- 不做默认人工审批队列。
- 不把学习结果先生成中间草稿再发布。
- 不把 SQLite 作为长期知识源头。
- 不让 WebUI 的 Ghost Maintenance 开关控制嵌入式学习。
- 不把普通任务进度、日志流水、临时 TODO 写入长期知识。

---

## 2. 核心组件

### 2.1 `AgentRuntime`

主要职责：

- 在 runtime 边界构建 `GhostEpisodeSnapshot`。
- 触发 background review。
- 在 pre-compress/session 边界排入一个合并学习任务，不再额外运行独立 memory flush。
- 为普通工具调用和后台 review 注入 `memory_file_store`、`skill_file_store`、`session_search`。

关键文件：

- `crates/agent/src/runtime.rs`
- `crates/agent/src/ghost_learning.rs`
- `crates/agent/src/ghost_background_review.rs`

### 2.2 `ContextBuilder`

主要职责：

- 在 session start 构建稳定 system prompt。
- 读取 `USER.md` 和 `memory/MEMORY.md`。
- 对 session 使用 frozen file memory snapshot，避免同一 session 内 prompt 随后台写入漂移。
- 注入 Ghost Learning 规则，告诉模型什么应该学、什么不应该学。

关键文件：

- `crates/agent/src/context.rs`

### 2.3 `MemoryFileStore`

主要职责：

- 管理 `USER.md` 和 `memory/MEMORY.md`。
- 提供 add、replace、remove、restore_latest。
- 写入前做内容规范化和安全扫描。
- 写入前保存 snapshot。
- 使用锁和原子写降低并发写损坏风险。

关键文件：

- `crates/agent/src/memory_file_store.rs`

### 2.4 `SkillFileStore`

主要职责：

- 管理 workspace learned skills。
- 支持 create、edit、patch、delete、write_file、remove_file、undo_latest。
- patch 支持 fuzzy matching。
- patch 失败时返回 preview、hint、possible matches，便于模型自修正。
- 歧义匹配不盲写。

关键文件：

- `crates/agent/src/skill_file_store.rs`
- `crates/tools/src/skills.rs`

### 2.5 `GhostLedger`

主要职责：

- 记录 episode。
- 记录 review run。
- 记录工具动作、状态、失败原因、轮数、stop reason。
- 支持审计、诊断和后续补偿重跑。

Ledger 不承载最终知识。最终知识源头是文件。

关键文件：

- `crates/storage/src/ghost_ledger.rs`

### 2.6 Tool Layer

Ghost Background Review 固定使用受限的声明性知识工具：

- `memory_manage`
- `skill_view`
- `session_search`

`skill_manage` 不在 Ghost Background Review 的允许列表中。技能创建与修改由主 Agent 或独立 Skill Learning 通道完成。

关键文件：

- `crates/tools/src/memory.rs`
- `crates/tools/src/skills.rs`
- `crates/tools/src/session_search.rs`
- `crates/tools/src/registry.rs`

---

## 3. 文件与数据源

### 3.1 长期知识文件

```text
workspace/
  USER.md
  memory/
    MEMORY.md
    knowledge_index.db
    memory.db
    .snapshots/
  ghost/
    ghost_ledger.db
  skills/
    <skill-name>/
      SKILL.md
      meta.yaml
      references/
      templates/
      scripts/
      assets/
```

其中：

- `USER.md` 与 `memory/MEMORY.md` 是唯一可写的长期知识事实源。
- `knowledge_index.db` 是可删除、可重建的 FTS/元数据/向量索引，不是事实源。
- `memory.db` 只承载短期 TTL 记忆和待迁移的遗留长期行。
- `ghost_ledger.db` 只承载学习与遗忘审计。

### 3.2 `USER.md`

保存用户维度的稳定信息：

- 用户偏好。
- 沟通风格。
- 长期约束。
- 反复强调的工作习惯。

示例：

```markdown
- [id:pref-summary] [scope:user] [source:user_statement] [updated:2026-08-01] User prefers concise Chinese summaries after code changes.
- [id:pref-no-push] [scope:user] [source:user_statement] [updated:2026-08-01] User does not want git push to be performed automatically.
```

### 3.3 `memory/MEMORY.md`

保存项目和环境维度的长期知识：

- 项目结构事实。
- 部署和验证约定。
- 工具坑点。
- 已验证的稳定经验。

示例：

```markdown
## Project

- [id:ghost-source] [scope:workspace] [source:verified] [updated:2026-08-01] BlockCell Ghost learning uses USER.md and memory/MEMORY.md as the durable knowledge source.

## Feedback

- [id:release-check] [scope:workspace] [source:user_statement] [updated:2026-08-01] Before release verification, confirm rollback planning and run targeted Ghost learning tests.

## Reference
```

长期条目元数据支持 `id`、`scope`、`source`、`updated` 和可选的 `supersedes`。`source` 的冲突优先级是 `user_statement > verified > inferred`；`supersedes` 用于明确淘汰旧条目。

### 3.4 Layer 5 收敛

旧版 Layer 5 文件会自动、幂等地迁移：

```text
base/memory/user.md       → workspace/USER.md
base/memory/project.md    → workspace/memory/MEMORY.md / Project
base/memory/feedback.md   → workspace/memory/MEMORY.md / Feedback
base/memory/reference.md  → workspace/memory/MEMORY.md / Reference
```

迁移按规范化内容去重，并给条目增加来源元数据；迁移来源 HTML 注释不参与内容 hash 和召回正文。成功后旧文件重置为兼容模板，runtime injector 只读取两个 canonical 文件。

### 3.5 Learned Skill

保存方法类知识，适合重复执行的流程：

```markdown
---
name: release-verification
description: Verify BlockCell release changes before local merge.
---

# Release Verification

Run formatting, cargo check, targeted Ghost tests, and git diff checks before reporting completion.
```

---

## 4. SQLite 数据语义

SQLite 不作为长期 knowledge source of truth，但不同数据库有明确分工：

- `memory.db`：短期、结构化、可过期记忆；遗留 `long_term` 行通过 `blockcell memory migrate-canonical` 迁移到规范文件。
- `knowledge_index.db`：从规范文件生成的可重建全文、元数据与可选向量索引。
- `ghost_ledger.db`：episode、review run、工具动作和统一遗忘事件的审计账本。

### 4.1 Episode

Episode 表示一次可复盘的学习边界。

典型字段语义：

- `id`：episode id。
- `boundary`：`turn_end`、`pre_compress`、`session_rotate`、`session_end`、`delegation_end` 等。
- `status`：`pending_review`、`reviewed`、`failed` 等。
- `metadata`：序列化后的 `GhostEpisodeSnapshot`。

### 4.2 Review Run

Review run 表示一次后台复盘执行。

典型字段语义：

- `episode_id`：关联 episode。
- `reviewer`：例如 `embedded_ghost_background_review_v1`。
- `status`：`completed` 或 `failed`。
- `result`：工具动作、轮数、stop reason、失败原因等 JSON 元数据。

### 4.3 为什么不把长期知识放进 SQLite

文件化知识有 4 个优势：

- 可读：用户和开发者可以直接审查。
- 可版本化：天然适合 git diff 和 snapshot。
- 可编辑：用户可以手工修正错误学习。
- 更符合当前产品目标：memory 和 skill 都是可读、可迁移的资产。

SQLite 仍用于高效检索、短期 TTL 和审计，但这些数据都不能反向覆盖规范文件。

---

## 5. Runtime 时序

### 5.1 Session Start

```text
load config
open MemoryFileStore
open SkillFileStore
scan skills
load USER.md
load memory/MEMORY.md
create frozen file memory snapshot for session
build system prompt
```

关键点：

- frozen snapshot 按 session key 缓存。
- 后台 review 写入新 memory 后，当前 session 不强制重建 prompt。
- 下一 session 会自然读到新知识。

### 5.2 Turn Start

```text
receive inbound message
check ghost learning config
query file memory recall items from USER.md and MEMORY.md
build <memory-context> block if relevant
append block as ephemeral context
```

Recall 特性：

- 只在允许的 channel 注入。
- 对 `ghost`、`cron`、`system`、`subagent` 等内部 channel 默认禁用。
- recall block 不写回 transcript。
- 当前用户指令优先级高于 recall。

### 5.3 Main Turn

主 turn 中模型可按普通工具规则调用知识工具。

`ToolContext` 会携带：

- `memory_file_store`
- `skill_file_store`
- `session_search`
- `ghost_memory_lifecycle`
- 当前 channel、session、权限信息

### 5.4 Turn End

```text
assistant response delivered
evaluate GhostLearningPolicy
if reviewable:
  persist episode to GhostLedger
  spawn background review task
```

策略倾向捕获：

- 用户纠正。
- 复杂工具链成功。
- 可复用流程出现。
- delegation 完成。
- 周期性学习检查点。

策略倾向忽略：

- 简单闲聊。
- 明显一次性内容。
- 没有稳定知识价值的任务状态。

### 5.5 Pre-Compress / Session End

```text
collect recent messages
append temporary sentinel user message
expose only memory_manage
run at most small bounded loop
strip artifacts
continue compression/session close
```

Flush 重点：

- 只允许写 memory，不允许写 skill。
- sentinel 只存在于 flush tool loop 中，不污染正式 transcript。
- 如果没有值得保存的长期事实，模型应不调用工具。
- provider 失败时返回已有 writes 数，不阻塞压缩。

---

## 6. Background Review Tool Loop

### 6.1 允许工具

后台 review 的允许工具固定为：

```text
memory_manage
session_search
skill_view
```

不允许执行 shell、文件任意写、网络工具或普通业务工具。

### 6.2 循环协议

```text
for round in 1..=max_rounds:
  provider.chat(messages, allowed_tool_schemas)
  if no tool calls:
    stop
  append assistant tool call message
  for each tool call:
    reject if tool not allowed
    execute with restricted ToolContext
    append tool result
record actions and status in GhostLedger
```

### 6.3 失败语义

- 工具失败：review run 标记失败，记录工具结果。
- 超过最大轮数：review run 标记失败。
- provider 不可用：记录失败。
- 模型不行动：不 fallback 到旧 JSON review。

这个选择是刻意的：宁可不学习，也不恢复旧式中间资产审批路径。

---

## 7. `memory_manage` 技术协议

### 7.1 Schema

核心参数：

```json
{
  "action": "add | replace | remove | undo_latest",
  "target": "user | memory",
  "scope": "session | workspace | user",
  "content": "entry content",
  "old_text": "unique text for replace/remove"
}
```

### 7.2 Scope 与 Target 映射

- `scope=session`：缺省值，写当前 Session 的 `USER.md` 或 `MEMORY.md`，保持原行为。
- `scope=user` + `target=user`：写全局 `workspace/USER.md`，供后续 Session 召回。
- `scope=workspace` + `target=memory`：写全局 `workspace/memory/MEMORY.md`，供项目内后续 Session 召回。
- Shadow 写入关闭时，相同 scope 路由到 `memory/shadow/` 对应命名空间。

### 7.3 写入规则

- `add` 会去重。
- `replace` 和 `remove` 要求 `old_text` 命中唯一 entry。
- 写入前会 normalize entry。
- 写入前会 safety scan。
- 写入前会检查统一遗忘 tombstone，拒绝复活已遗忘内容。
- 写入前会 snapshot 原文件。
- 写入使用 atomic write。
- 写入后同步重建 `KnowledgeIndex` 中受影响的文件。

### 7.4 Undo

`undo_latest` 会从 `.snapshots/` 恢复目标文件的最近版本。

Undo 是文件级恢复，不是逐字段事务回滚。

---

## 8. `skill_manage` 技术协议

本节描述主 Agent / 独立 Skill Learning 通道使用的技能写入协议。Ghost Background Review 不允许调用 `skill_manage`。

### 8.1 Schema

核心 action：

```text
create
edit
patch
delete
write_file
remove_file
undo_latest
```

### 8.2 Skill 写入原则

- 默认生成 prompt-only learned skill。
- `create` 必须有 name、description、content。
- `patch` 必须有 old_text 和 content。
- `write_file` 只能写入受控子目录，例如 references、templates、scripts、assets。
- 对带脚本或外部副作用的 skill，仍然遵守 BlockCell 原有执行权限。

### 8.3 Patch 失败返回

patch 失败时应返回足够信息给模型修复：

- `file_preview`
- `hint`
- `possible_matches`
- 歧义原因

这样 review loop 可以在下一轮工具调用中修正 patch，而不是盲目全量覆盖。

---

## 9. Recall 注入

Recall 使用 `KnowledgeIndex` 对 `USER.md` 和 `MEMORY.md` 做 FTS 检索，并单独合并当前 Session 文件中的相关知识。规范文件发生变化时索引增量重建；索引本身可随时从文件恢复。

长期候选按固定语义收敛：

```text
内容去重
→ 丢弃被 supersedes 指向的旧条目
→ user_statement > verified > inferred
→ updated 越新越优先
→ FTS relevance
```

已写入 tombstone 的内容即使因异常仍残留在文件或索引中，也会在召回时被过滤。

输出格式是 fenced context：

```xml
<memory-context>
Relevant durable file memory from USER.md and MEMORY.md.
Use only when directly relevant. Current user instructions override this context.
- [MEMORY.md] ...
</memory-context>
```

设计约束：

- recall 是 ephemeral context。
- recall 不落 transcript。
- recall 不提升为系统指令。
- recall 有 token budget 和 item limit。
- recall 对内部 channel 有 denylist。

---

## 10. Safety Model

### 10.1 内容安全

自动学习内容必须拒绝：

- prompt injection。
- 伪 system/developer 指令。
- 隐藏 Unicode 控制字符。
- 明显 secret、token、凭据。
- 一次性任务日志。
- 未验证猜测。

### 10.2 写入安全

- 文件写入前 snapshot。
- 文件写入使用临时文件和 rename。
- 同进程写入使用 mutex。
- 跨进程写入使用 lockdir guard。
- 写入后 sync file 和 parent dir。
- `MemoryFileStore::add`、`replace`、`restore_latest` 和 Ghost durable router 共用 tombstone 检查，阻止后台学习或快照恢复重新写回已遗忘知识。

### 10.3 工具安全

- background review 只暴露知识工具。
- memory flush 只暴露 `memory_manage`。
- 不允许 background review 执行 shell、网络或任意文件写。
- 工具循环有轮数上限。

### 10.4 行为安全

- 当前用户指令优先于 learned memory。
- 学习失败不阻塞主任务。
- 学错可 undo。
- 跨来源遗忘必须先 `knowledge_forget(action="preview")`，再使用精确 `preview_token` 调用 `confirm`。
- Ledger 保留审计信息。

---

## 11. 配置与开关

Ghost learning 受 `config.agents.ghost.learning` 控制。

关键语义：

- `enabled`：旧版总开关；未配置 `captureEnabled` 时仍作为兼容来源。
- `captureEnabled`：是否捕获学习事件。
- `writeEnabled`：是否写入正式知识文件；关闭时写入 shadow 命名空间。
- `recallEnabled`：是否在运行时注入 recall。
- `shadowMode`：已弃用，仅用于旧配置兼容；请改用上述三个显式开关。
- `recall_max_items`：recall 最大条数。
- `recall_token_budget`：recall token 预算。
- review interval / policy 参数：控制周期性复盘。

WebUI 的 Ghost 页面当前表示 scheduled maintenance，不是嵌入式学习开关。也就是说，用户关闭定时维护，不应影响正常对话中的 Ghost-native 学习闭环。

---

## 12. 测试矩阵

当前核心测试覆盖以下能力。

### 12.1 Memory File Store

- add user memory 并加载 snapshot。
- replace 唯一 entry 并生成 snapshot。
- restore latest 恢复旧内容。
- 并发 add 串行化。
- 拒绝 prompt injection 内容。
- replace 多匹配或零匹配时失败。
- Layer 5 四类旧文件只迁移一次并收敛到两个 canonical 文件。
- add、replace、restore_latest 不得复活 tombstoned 内容。

### 12.2 Background Review

- 只使用 restricted tool loop。
- 不 fallback 到 JSON review。
- 不执行未允许工具。
- 工具失败时 review 标记失败。
- 超过最大轮数时标记失败。
- 可先 session_search 再写 memory。
- 用户响应后再运行 background review。

### 12.3 Runtime Learning

- 用户纠正触发 review。
- 高复杂度工具 turn 触发 review。
- trivial success turn 被忽略。
- pre-compress 强制 boundary flush。
- 从 episode 到 USER.md、MEMORY.md、learned skill 的闭环成立。
- recall 是 fenced 且不持久化。
- recall 通过 KnowledgeIndex 去重、淘汰 superseded 条目并按来源可信度和更新时间排序。
- runtime injector 只读取 `USER.md` 和 `memory/MEMORY.md`。

### 12.4 Unified Forget

- preview 返回完整影响范围和精确确认 token。
- confirm 拒绝过期或不匹配的 preview token。
- 同时清理 canonical 文件、当前 Session 文件和 SQLite short-term。
- 重建索引并写入 tombstone 与 GhostLedger 审计。
- Ghost background review、普通写入和 snapshot restore 均不能复活遗忘内容。

### 12.5 Skill Tools

- create/edit/patch/delete/write_file/remove_file/undo 路由到 file store。
- patch 支持 fuzzy matching 和失败 preview。
- list_skills 只展示 learned skills，不展示旧 evolution 状态。

推荐回归命令：

```bash
cargo fmt --check
cargo check -p blockcell
cargo test -p blockcell-agent ghost_learning -- --nocapture
cargo test -p blockcell-agent ghost_background_review -- --nocapture
cargo test -p blockcell-tools skill_manage -- --nocapture
cargo test -p blockcell-tools test_list_skills -- --nocapture
git diff --check
```

---

## 13. 运维与诊断

### 13.1 用户反馈“没记住”

排查顺序：

1. 检查 `config.agents.ghost.learning.captureEnabled`（旧配置回退到 `enabled`）。
2. 检查 `writeEnabled` 和 `recallEnabled`；旧配置中的 `shadowMode` 已弃用，仅保留兼容语义。
3. 检查 `USER.md` 和 `memory/MEMORY.md` 是否有写入。
4. 检查 `GhostLedger` 是否有 episode。
5. 检查 review run 是否失败。
6. 检查 `knowledge_index.db` 是否已从规范文件重建，以及查询是否命中 FTS。
7. 检查当前 channel 是否在 recall denylist。

### 13.2 用户反馈“记错了”

处理方式：

1. 如果用户要求“忘记”或需要跨来源彻底清除，先调用 `knowledge_forget(action="preview", query="...")`。
2. 用户确认影响范围后，携带返回的 `preview_token` 调用 `knowledge_forget(action="confirm", reason="用户要求")`。
3. 如果只是修正单条规范知识，用 `memory_manage(action="replace")`；如果刚写入，也可使用 `undo_latest`。
4. 如果错误来自 skill，由主 Agent 或独立 Skill Learning 通道先用 `skill_view` 查看，再调用 `skill_manage(action="patch")`。

### 13.3 Background Review 没动作

可能原因：

- policy 判断本轮不值得学习。
- provider 不可用。
- 模型认为没有 durable learning。
- 工具调用失败。
- review 超过最大轮数。

这不是致命错误。Ghost 学习是 best-effort，主任务不应因此失败。

---

## 14. 和旧 Ghost Maintenance 的关系

旧 Ghost Maintenance 仍可负责：

- memory gardening。
- file cleanup。
- Community Hub sync。
- scheduled routine。

但它不是 Ghost-native 学习闭环的主引擎。

新的学习闭环发生在 agent runtime：

- 用户正常对话。
- runtime 捕获 episode。
- background review 调用受限知识工具。
- Ghost Background Review 更新文件化 memory；主 Agent 或独立 Skill Learning 通道更新 skill。
- 下一轮自然生效。

WebUI 中 Ghost Maintenance 的命名调整，就是为了避免把 scheduled maintenance 和 embedded learning 混为一谈。

---

## 15. 成功标准

这个设计成立需要满足以下标准：

- 用户纠正一次后，稳定偏好能写入 `USER.md`。
- 项目事实和环境坑点能写入 `memory/MEMORY.md`。
- 成功复杂流程能由主 Agent 或独立 Skill Learning 通道沉淀为 `skills/<name>/SKILL.md`，而不是由 Ghost Background Review 直接写入。
- 下一次 session 能通过 frozen snapshot 和 recall 使用已学知识。
- 当前 session 不因后台学习而 prompt 漂移。
- background review 不输出旧式中间草稿。
- 用户不用管理审批或发布状态。
- 学错能 undo 或 patch。
- 所有自动学习动作在 ledger 中可审计。

最终目标是：

> BlockCell 在用户正常使用过程中持续学习；Ghost 静默捕获、谨慎沉淀、可控写入，并在后续任务里自然表现得更懂用户和项目。
