# blockcell Technical Article Series: “A Self-Evolving AI Agent Framework”

> The complete series index for blockcell. It currently contains 24 articles, all aligned to the real codebase and current product behavior.

---

## Series overview

blockcell is an open-source AI agent framework written in **Rust**. It is not just a chat interface — it is an AI workbench that can execute tasks, keep memory, integrate tools, run channels, and evolve itself over time.

This series covers the product from first use to advanced architecture topics, including:

- what blockcell is and how to get started
- tools, skills, memory, channels, browser automation, and gateway mode
- self-evolution, subagents, Ghost, and the Hub community
- production-facing topics such as the CLI, provider config, MCP, provider pools, multi-profile intent routing, and multi-agent configuration

For the naming story, see:

*Appendix: [Name origin](./14_name_origin.md)*

---

## Table of contents

| # | Title | Key topics | Best for |
|----|------|-----------|---------|
| 01 | [What is blockcell?](./01_what_is_blockcell.md) | project positioning, core capabilities, and how it differs from a simple chatbot | Everyone |
| 02 | [5-minute quickstart](./02_quickstart.md) | installation, initialization, first chat, and gateway startup | Beginners |
| 03 | [Tool system](./03_tools_system.md) | built-in tools, invocation model, and tool execution flow | Beginners |
| 04 | [Skill system](./04_skill_system.md) | Rhai, `SKILL.md`, custom skills, and hot reload | Beginners |
| 05 | [Memory system](./05_memory_system.md) | SQLite + FTS5, persistent memory, and automatic injection | Intermediate |
| 06 | [Multi-channel access](./06_channels.md) | Telegram, Slack, Discord, Feishu, DingTalk, and WeCom | Intermediate |
| 07 | [Browser automation](./07_browser_automation.md) | CDP, accessibility tree, and web automation workflows | Intermediate |
| 08 | [Gateway mode](./08_gateway_mode.md) | HTTP API, WebSocket, and server deployment | Intermediate |
| 09 | [Self-evolution](./09_self_evolution.md) | error-triggered upgrades, repair loops, and rollout ideas | Advanced |
| 10 | [Finance in practice](./10_finance_use_case.md) | stock and crypto monitoring, alerts, and report workflows | Hands-on |
| 11 | [Subagents and task concurrency](./11_subagents.md) | `spawn`, task decomposition, and concurrent execution | Advanced |
| 12 | [Architecture deep dive](./12_architecture.md) | Rust crate layout, system boundaries, and core modules | Advanced |
| 13 | [Message processing & evolution lifecycle](./13_message_processing_and_evolution.md) | the full path from input message to tools, memory, and evolution | Advanced |
| 14 | [Name origin](./14_name_origin.md) | why the project is called blockcell | Appendix |
| 15 | [Ghost Agent](./15_ghost_agent.md) | background maintenance, memory gardening, and community sync | Advanced |
| 16 | [Agent2Agent Community (Blockcell Hub)](./16_hub_community.md) | node discovery, skill distribution, and the autonomous-network direction | Advanced |
| 17 | [CLI Reference](./17_cli_reference.md) | current command surface, options, shortcuts, and operator workflows | Intermediate |
| 18 | [Proxy and Provider Configuration](./18_proxy_and_provider_config.md) | `config.json5`, provider resolution, proxy precedence, and the WebUI raw editor | Intermediate |
| 19 | [MCP Server Integration](./19_mcp_servers.md) | standalone `mcp.json` / `mcp.d`, template-based setup, and agent visibility binding | Intermediate |
| 20 | [Provider Pool — Multi-Model High Availability](./20_provider_pool.md) | `modelPool`, priority, weight, failover, and CLI overrides | Advanced |
| 21 | [intentRouter Multi-Agent Configuration Guide](./21_intent_router_profiles.md) | agent profile binding, intentRouter profiles, tool composition, and examples | Advanced |
| 22 | [Path Access Policy](./22_path_access_policy.md) | `path_access.json5`, allow/confirm/deny rules, built-in sensitive path protection | Intermediate |
| 23 | [Weixin Integration Guide](./23_weixin_integration.md) | Weixin QR-code login, owner binding, and multi-account routing | Intermediate |

---

## Suggested reading paths

**Brand new to blockcell:** 01 → 02 → 03 → 04

**Want to deploy quickly:** 01 → 02 → 06 → 08 → 17

**Need production configuration guidance:** 02 → 08 → 17 → 18 → 19 → 20 → 21

**Want the internal architecture:** 11 → 12 → 13 → 15 → 16

---

## Project info

- **GitHub**: https://github.com/blockcell-labs/blockcell
- **Website**: https://blockcell.dev
- **License**: MIT

---

*This series is maintained against the current blockcell codebase, and behavior descriptions are written to match the implementation as it exists today.*
