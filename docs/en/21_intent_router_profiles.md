# Article 21: Multi-Agent Configuration Guide for `intentRouter`

> Series: *In-Depth Analysis of the Open Source Project “blockcell”* — Article 21

`intentRouter` is now the **single configuration entry point** for intent-to-tool mapping in blockcell. If you want different agents to expose different capabilities, keep “which agent gets which profile” separate from “which tools each profile can expose”.

## What to remember first

- Bind each agent with `agents.list[].intentProfile`
- Define reusable tool sets under `intentRouter.profiles`
- Put shared baseline tools in `coreTools`
- Add intent-specific tools in `intentTools`
- Remove disallowed tools with `denyTools`
- Explicitly configure `Chat` as `{ "inheritBase": false, "tools": [] }`
- Always configure `Unknown`

> `allowedMcpServers` and `allowedMcpTools` are agent-level MCP visibility allowlists. The JSON field names are camelCase.

## Key rules

1. `agents.list[].intentProfile` has the highest priority.
2. If an agent does not set `intentProfile`, blockcell falls back to `intentRouter.agentProfiles`.
3. If that still does not resolve, blockcell falls back to `intentRouter.defaultProfile`.
4. If `agents.list` is empty, runtime falls back to an implicit `default` agent.
5. If `intentRouter` is missing, blockcell injects the built-in default router automatically.
6. If `intentRouter.enabled = false`, runtime still resolves profiles, but it ultimately keeps only that profile's `Unknown` tool set.
7. Every profile must configure `Unknown`, otherwise validation fails.

## Example 1: default assistant + ops assistant

This is the most common two-role split:

- `default` handles daily chat, file operations, and web search
- `ops` handles ops work, debugging, and internal maintenance

```json
{
  "agents": {
    "list": [
      {
        "id": "default",
        "enabled": true,
        "name": "Daily Assistant",
        "intentProfile": "default",
        "allowedMcpServers": ["github", "sqlite"],
        "allowedMcpTools": ["github__list_issues", "sqlite__query"]
      },
      {
        "id": "ops",
        "enabled": true,
        "name": "Ops Assistant",
        "intentProfile": "ops",
        "allowedMcpServers": ["github"],
        "allowedMcpTools": ["github__list_issues", "github__create_issue"]
      }
    ]
  },
  "intentRouter": {
    "enabled": true,
    "defaultProfile": "default",
    "profiles": {
      "default": {
        "coreTools": [
          "read_file",
          "write_file",
          "list_dir",
          "exec",
          "web_search",
          "web_fetch",
          "memory_query",
          "memory_upsert",
          "toggle_manage",
          "message",
          "agent_status"
        ],
        "intentTools": {
          "Chat": { "inheritBase": false, "tools": [] },
          "FileOps": ["edit_file", "file_ops", "data_process", "office_write"],
          "WebSearch": ["browse", "http_request"],
          "Unknown": ["edit_file", "file_ops", "office_write", "http_request", "browse"]
        }
      },
      "ops": {
        "coreTools": [
          "read_file",
          "list_dir",
          "exec",
          "web_search",
          "web_fetch",
          "message",
          "agent_status"
        ],
        "intentTools": {
          "DevOps": ["network_monitor", "encrypt", "http_request", "edit_file", "file_ops"],
          "Organization": ["cron", "memory_maintenance", "list_skills"],
          "Unknown": ["http_request", "browse"]
        },
        "denyTools": ["email"]
      }
    }
  }
}
```

### How to read this example

- `default` keeps `Chat` tool-free, so plain conversation does not accidentally expose tools.
- `default`'s `FileOps` and `WebSearch` cover file work and web search.
- `ops`'s `DevOps` profile allows debugging, encryption, file editing, and HTTP inspection.
- `ops` also sets `denyTools: ["email"]`, so `email` is removed even if it is added elsewhere.
- `allowedMcpServers` and `allowedMcpTools` only control what MCP resources the agent can see; they do not change the profile's tool mapping.

## Example 2: support + finance + admin

This example is closer to a “front desk / finance / platform ops” split:

- `support` handles day-to-day support and message replies
- `finance` handles market data, charts, alerts, and daily reports
- `admin` handles system control and platform maintenance

```json
{
  "agents": {
    "list": [
      {
        "id": "support",
        "enabled": true,
        "name": "Support",
        "intentProfile": "support",
        "allowedMcpServers": ["sqlite"],
        "allowedMcpTools": ["sqlite__query"]
      },
      {
        "id": "finance",
        "enabled": true,
        "name": "Finance",
        "intentProfile": "finance",
        "allowedMcpServers": ["market-data", "news"],
        "allowedMcpTools": ["market-data__quote", "news__search"]
      },
      {
        "id": "admin",
        "enabled": true,
        "name": "Admin",
        "intentProfile": "admin",
        "allowedMcpServers": ["github"],
        "allowedMcpTools": ["github__list_issues", "github__create_issue"]
      }
    ]
  },
  "intentRouter": {
    "enabled": true,
    "defaultProfile": "support",
    "profiles": {
      "support": {
        "coreTools": [
          "read_file",
          "list_dir",
          "message",
          "web_search",
          "web_fetch",
          "memory_query",
          "memory_upsert"
        ],
        "intentTools": {
          "Chat": { "inheritBase": false, "tools": [] },
          "Communication": ["email", "message", "http_request"],
          "Organization": ["list_tasks", "cron", "community_hub", "memory_maintenance"],
          "Unknown": ["message", "http_request"]
        }
      },
      "finance": {
        "coreTools": ["read_file", "web_search", "web_fetch", "message", "agent_status"],
        "intentTools": {
          "Finance": [
            "http_request",
            "data_process",
            "chart_generate",
            "alert_rule",
            "stream_subscribe",
            "knowledge_graph",
            "cron",
            "office_write",
            "browse"
          ],
          "WebSearch": ["browse", "http_request"],
          "Unknown": ["http_request", "browse"]
        },
        "denyTools": ["email"]
      },
      "admin": {
        "coreTools": ["read_file", "write_file", "list_dir", "exec", "toggle_manage", "message", "agent_status"],
        "intentTools": {
          "SystemControl": ["system_info", "app_control", "camera_capture", "browse", "image_understand", "termux_api"],
          "DevOps": ["network_monitor", "encrypt", "http_request", "edit_file", "file_ops"],
          "Unknown": ["edit_file", "file_ops", "http_request"]
        }
      }
    }
  }
}
```

### How to read this example

- `support` is used for high-frequency chat and ticket replies, so `Chat` still exposes no tools.
- `finance` groups market data, alerts, charts, and reports together, which makes it a good finance agent.
- `admin` can use system-control and ops tools, but it does not get finance-specific capabilities.
- If you bind `finance` as the owner for a channel, messages from that channel will land on the finance profile first.
- If `intentRouter.enabled = false`, these profiles still exist, but each agent will end up with only its profile's `Unknown` tool set.

> The MCP server / tool names above are examples only. Replace them with real names from your merged `mcp.json` + `mcp.d` view.

## Resolution order

The final tool set is computed in this order:

1. Choose the profile for the current agent: first `agents.list[].intentProfile`, then `intentRouter.agentProfiles`, then `defaultProfile`
2. Merge `coreTools` and `intentTools` for the classified intents
3. Add runtime-required tools such as ghost-required tools
4. Apply `denyTools`
5. Apply runtime tool switches or visibility filters

## Compatibility behavior

- If `intentRouter` is missing, blockcell injects the built-in default router automatically
- `agents.list[].intentProfile` takes precedence over `intentRouter.agentProfiles`; the latter mainly exists for backward compatibility
- If `agents.list` is empty, runtime falls back to an implicit `default` agent
- `allowedMcpServers` and `allowedMcpTools` are agent-level MCP visibility allowlists, and their field names are camelCase

## Troubleshooting

Start with:

```bash
blockcell status
blockcell doctor
```

Check:

- which profile each agent resolves to
- whether `intentRouter` validation passes
- whether any tool names are invalid or unregistered
- whether `Unknown` is configured
- whether MCP-related names referenced by the profile actually exist in the merged `mcp.json` + `mcp.d/*.json` view

---

*Previous: [Provider Pool](./20_provider_pool.md)*

*Index: [Series directory](./00_index.md)*
