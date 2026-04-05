# blockcell 技术文章系列：《一个会自我进化的 AI 智能体框架》

> 面向普通开发者和入门开发者的 blockcell 系列索引，当前共 26 篇，全部基于真实代码与当前实现整理。

---

## 系列简介

blockcell 是一个用 **Rust** 编写的开源 AI 智能体框架。它不只是一个聊天机器人，而是一个能真正执行任务、拥有持久记忆、可以自我进化的 AI 工作台。

本系列从零开始，依次覆盖：

- Agent 的基础概念与上手方式
- 工具、技能、记忆、多渠道、浏览器自动化
- Gateway、自我进化、子智能体、Ghost、Hub 社区
- CLI、Provider、MCP、Provider Pool、intentRouter、多智能体配置等更贴近真实产品落地的专题

名字来源见：

*番外：[名字由来](./14_name_origin.md)*

---

## 目录总览

| # | 标题 | 核心内容 | 适合读者 |
|----|------|-----------|---------|
| 01 | [什么是 blockcell？](./01_what_is_blockcell.md) | 项目定位、核心能力、与普通聊天机器人的区别 | 所有人 |
| 02 | [5 分钟上手](./02_quickstart.md) | 安装、初始化、第一次对话、Gateway 启动 | 入门 |
| 03 | [工具系统](./03_tools_system.md) | 内置工具、调用模型、工具执行流程 | 入门 |
| 04 | [技能（Skill）系统](./04_skill_system.md) | Rhai、`SKILL.md`、自定义技能与热更新 | 入门 |
| 05 | [记忆系统](./05_memory_system.md) | SQLite + FTS5、长期记忆、自动注入 | 进阶 |
| 06 | [多渠道接入](./06_channels.md) | Telegram / Slack / Discord / 飞书 / 钉钉 / 企业微信 | 进阶 |
| 07 | [浏览器自动化](./07_browser_automation.md) | CDP、无障碍树、页面操作与自动化执行 | 进阶 |
| 08 | [Gateway 模式](./08_gateway_mode.md) | HTTP API、WebSocket、服务化部署 | 进阶 |
| 09 | [自我进化](./09_self_evolution.md) | 错误触发、自动修复、灰度发布思路 | 深入 |
| 10 | [金融场景实战](./10_finance_use_case.md) | 股票 / 加密货币监控、告警、日报工作流 | 实战 |
| 11 | [子智能体与任务并发](./11_subagents.md) | `spawn`、任务拆分、并发执行 | 深入 |
| 12 | [架构深度解析](./12_architecture.md) | Rust crate 结构、设计边界、核心模块 | 深入 |
| 13 | [消息处理与自进化生命周期](./13_message_processing_and_evolution.md) | 从收到消息到工具调用、记忆写入、进化触发的全流程 | 深入 |
| 14 | [名字由来](./14_name_origin.md) | Block + Cell 的含义与项目命名背景 | 番外 |
| 15 | [幽灵智能体（Ghost Agent）](./15_ghost_agent.md) | 后台维护、记忆整理、社区同步 | 深入 |
| 16 | [Agent2Agent 社区（Blockcell Hub）](./16_hub_community.md) | 节点发现、技能流动、自治社区方向 | 深入 |
| 17 | [CLI 参考手册](./17_cli_reference.md) | 当前命令体系、参数、快捷命令、运维常用入口 | 进阶 |
| 18 | [代理与 LLM Provider 配置](./18_proxy_and_provider_config.md) | `config.json5`、Provider 选择、代理优先级、WebUI 原始配置编辑 | 进阶 |
| 19 | [MCP Server 集成](./19_mcp_servers.md) | 独立 `mcp.json` / `mcp.d`、模板添加、Agent 视角权限绑定 | 进阶 |
| 20 | [Provider Pool — 多模型高可用配置](./20_provider_pool.md) | `modelPool`、优先级、权重、失败降级与 CLI 覆盖 | 深入 |
| 21 | [intentRouter 多智能体配置指南](./21_intent_router_profiles.md) | 多 Agent 归属、intentRouter profiles、工具编排与示例 | 深入 |
| 22 | [目录访问安全策略](./22_path_access_policy.md) | `path_access.json5`、allow/confirm/deny 规则、内置敏感路径保护 | 进阶 |
| 23 | [微信接入使用说明](./23_weixin_integration.md) | 微信扫码登录、owner 绑定、多账号路由 | 进阶 |
| 24 | [Skill 开发初级教程](./24_skill_beginner.md) | Skill 最小组成、执行流程、入门写法与热更新 | 入门 |
| 25 | [Skill 开发中级教程](./25_skill_intermediate.md) | Prompt Skill 到 Hybrid Skill、`exec_local`、CLI 化设计 | 进阶 |
| 26 | [Skill 开发高级教程](./26_skill_advanced.md) | 控制面/执行面分层、测试、发布与 CLI 工作流 | 深入 |

---

## 推荐阅读顺序

**如果你是完全新手：** 01 → 02 → 03 → 04

**如果你想快速部署：** 01 → 02 → 06 → 08 → 17

**如果你想系统学习 skill 开发：** 04 → 24 → 25 → 26

**如果你想做真实产品配置：** 02 → 08 → 17 → 18 → 19 → 20 → 21

**如果你想理解系统内部设计：** 11 → 12 → 13 → 15 → 16

---

## 项目信息

- **GitHub**：https://github.com/blockcell-labs/blockcell
- **官网**：https://blockcell.dev
- **License**：MIT

---

*本系列文章基于 blockcell 当前真实代码整理，示例与配置说明以现有实现为准。*
