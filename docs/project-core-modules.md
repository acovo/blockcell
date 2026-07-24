# BlockCell 项目核心模块总结

BlockCell 从项目能力和业务职责角度，可以归纳为以下 7 个核心大模块，而不必机械地按 Cargo crate 划分。

## 1. 多入口交互与接入层

负责用户如何使用 BlockCell，以及消息如何进入系统。

- CLI：Agent 对话、配置、诊断、技能、记忆等命令
- Gateway：HTTP API、WebSocket、Webhook、文件服务
- WebUI：聊天、Agent 管理、系统事件和配置界面
- 消息渠道：Telegram、微信、飞书、Slack、Discord、企业微信、QQ 等
- 多账号与渠道路由

主要代码：`bin/blockcell`、`webui/src`、`crates/channels`。

## 2. Agent 核心运行时

这是整个项目的中枢，负责把用户请求真正变成一轮智能体执行。

核心职责包括：

- 构建上下文和 System Prompt
- 意图识别与工具选择
- 驱动 LLM 多轮 Tool Calling
- 工具执行、结果回填和最终回复
- 上下文压缩与恢复
- Token、成本和循环次数控制
- 多 Agent RuntimePool
- Forked Subagent、任务取消和 Checkpoint
- 用户中途追加指令 Steering

```text
用户消息
  → 上下文构建
  → 模型推理
  → 工具/技能调用
  → 结果返回模型
  → 最终回复
```

主要代码在 `crates/agent`。

## 3. 模型与 Provider 路由模块

负责对接各种大模型，并为 Agent Runtime 提供统一的推理接口。

- OpenAI、Anthropic、Gemini、Ollama 等 Provider
- OpenAI Responses API 和 Embedding 模型
- Provider Pool 及多 Provider 优先级、权重选择
- 成本优先、质量优先等 ModelRouter 策略
- 建连阶段故障降级
- 流式响应与 Tool Calling 格式适配

主要代码在 `crates/providers`。

## 4. 能力执行层：Tools、Skills 与 MCP

这是 BlockCell 真正“能干活”的部分。

| 类型 | 定位 | 典型能力 |
| --- | --- | --- |
| Tools | Rust 内置、稳定可信 | 文件、Shell、浏览器、邮件、Office、OCR、网络请求 |
| Skills | 可扩展的任务流程 | Markdown、Rhai、Python 技能 |
| MCP | 外部能力接入协议 | 动态连接第三方工具服务器 |

这一层还包括 Tool Registry、JSON Schema 参数验证、权限检查、技能扫描和加载、浏览器 CDP 自动化以及技能安装与版本管理。

主要代码：`crates/tools`、`crates/skills`、`skills`。

## 5. 记忆、自学习与自我进化模块

这是 BlockCell 区别于普通聊天框架的核心模块，包含三个层次：

- 会话记忆：保存和检索历史对话、压缩长上下文
- 长期学习：Ghost Learning 将偏好、项目事实和踩坑经验写入记忆或技能
- 能力进化：发现重复失败后，生成、审计、测试并部署新版技能

具体能力包括：

- SQLite 结构化记忆
- 向量检索和 RabitQ 索引
- Session Memory 和 Auto Memory
- `USER.md`、`MEMORY.md` 文件记忆
- Ghost Recall 和后台 Review
- 学习去重、节流和安全扫描
- 技能版本管理、金丝雀发布和回滚

主要代码：`crates/storage`、`crates/agent/src/ghost_*`、`crates/agent/src/memory_*`、`crates/skills/src/evolution*`。

## 6. 自动化调度与主动任务模块

负责让 Agent 不只在用户发消息时运行，还可以持续在后台工作。

- Cron 定时任务和 Heartbeat 周期性唤醒
- 后台 TaskManager 和任务状态持久化
- Dream/记忆整理任务
- Ghost Learning 后台 Review
- 技能进化 Worker
- 系统事件聚合、进度推送和主动通知

主要代码：`crates/scheduler`、`crates/agent/src/task_manager*`、`crates/agent/src/system_event_orchestrator.rs`。

## 7. 平台基础设施与安全治理

这是支撑所有模块稳定、安全运行的底座。

- 全局配置、热加载、工作目录和路径管理
- Tool Policy、路径访问策略和用户确认
- Token/成本预算和审计日志
- Hook 生命周期
- SSRF、命令和内容安全扫描
- 文件写入锁与并发保护
- 健康检查、日志、升级和签名校验

主要代码：`crates/core`、`crates/updater`。

## 整体主链路

```text
CLI / WebUI / 外部渠道
          ↓
   Agent Runtime
          ↓
  Model / Provider Router
          ↓
Tools / Skills / MCP
          ↓
记忆、学习、任务与存储
          ↓
安全策略、审计和配置底座
```

如果用 BlockCell 自己的理念概括：

- **Block**：Rust Runtime、存储、安全、调度、渠道等稳定可信的宿主。
- **Cell**：Skills、记忆和自我进化等可以持续变化、组合和生长的能力层。
