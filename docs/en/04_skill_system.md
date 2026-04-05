# Article 04: The Skill System — Current Invocation Flow and Runtime Shapes

> Series: *In-Depth Analysis of the Open Source Project “blockcell”* — Article 4
---

## Tools vs skills — what’s the difference?

In the previous article we covered tools. Tools are atomic actions like “read a file” or “search the web”.

But real tasks are often multi-step:

```
Monitor Moutai stock price =
  every 10 minutes → query price → check threshold → send alert → write logs
```

These **multi-step tasks with logic and branching** are what skills are meant to handle.

**A skill = a reusable workflow that encapsulates multiple tool calls**

---

## What a skill contains

Each skill is a directory, and it currently falls into three runtime shapes (**Prompt-only / Local Script / Rhai**).

In practice, a skill directory may contain the following files (optional combinations):

```
skills/stock_monitor/
├── meta.yaml      # metadata: name, description, tools, dependencies, fallback
├── SKILL.md       # playbook: instructions for the LLM
├── SKILL.rhai     # Rhai script support retained in the repository
└── SKILL.py       # local script asset, usually invoked via exec_local
```

These files have different responsibilities:

| File | Purpose | Read by |
|------|---------|---------|
| `meta.yaml` | skill metadata, tool allowlist, dependencies, fallback | system |
| `SKILL.md` | operating rules, parameters, examples | LLM |
| `SKILL.rhai` | deterministic orchestration logic | Rhai engine |
| `SKILL.py` | local script asset | `exec_local` / testing tools |

Notes:
- **Current runtime contract**: a practical skill should have `meta.yaml` + `SKILL.md`, and the chat runtime primarily relies on `SKILL.md`.
- **Local Script skills**: if the directory contains `SKILL.py`, `scripts/`, or `bin/`, the current chat path exposes `exec_local` inside the skill scope instead of auto-running `SKILL.py`.
- **Rhai support still exists**: the repo still includes `SkillDispatcher` and `SKILL.rhai`, but normal user conversations primarily go through the prompt-skill executor.

---

## Three shapes (Prompt / Local Script / Rhai)

### 1) Prompt-only (MD)

When a skill directory only has `SKILL.md`, it works as an **operating playbook**:
- It describes goals, steps, parameters and fallbacks
- Once the skill is activated, blockcell injects the prompt bundle compiled from `SKILL.md` and lets the model operate within the skill’s scoped tools

### 2) Local Script (`SKILL.py` / `scripts/` / `bin/`)

These skills still enter through `SKILL.md`, but they carry local script assets that the model can invoke with `exec_local`.

In the current implementation:
- `SkillManager::build_skill_card()` infers local-exec support from `SKILL.py`, `SKILL.rhai`, nested script files, or `exec_local` hints in the manual
- Once activated, the runtime automatically adds `exec_local` to that skill’s allowed tools
- `exec_local` only accepts relative paths inside the active skill directory, and the runner is limited to `python3`, `bash`, `sh`, `node`, or `php`

So:
- Chat activation does not auto-run `SKILL.py`
- Whether a local script runs is still decided by the model following `SKILL.md`

### 3) Rhai (SKILL.rhai)

When `SKILL.rhai` exists, it carries **deterministic orchestration**.

Implementation-wise, blockcell executes Rhai via `SkillDispatcher` and injects host functions into the script, including:
- `call_tool(name, params)` / `call_tool_json(name, json)`
- `set_output(value)` / `set_output_json(json)`
- `log(msg)` / `log_warn(msg)`
- `is_error(result)` / `get_field(map, key)`

---

## Current Invocation Flow (based on the code)

This is the actual flow implemented today in `runtime.rs`, `context.rs`, and `manager.rs`.

### 1) Skills are loaded at startup

`ContextBuilder::new()` creates a `SkillManager`, then `load_from_paths()` scans two roots:
- built-in skills first, lower priority
- workspace skills second, higher priority and allowed to override built-ins

Each skill load performs:
- reading `meta.yaml` / `meta.json`
- validating `requires.bins`, `requires.env`, and declared `tools`
- reading `SKILL.md`
- compiling `shared` / `prompt` / `planning` / `summary` bundles from `SKILL.md`
- building a runtime `SkillCard`

### 2) Normal chat does not auto-route by `triggers`

In the unified entry, ordinary user messages first enter **General mode**. Then the runtime:
- turns enabled skills into `SkillCard`s
- injects those cards into the system prompt under `Installed Skills`
- exposes a dedicated function tool: `activate_skill`

So the main path is:

```text
user message
  -> General mode
  -> system prompt receives SkillCards
  -> model decides whether to call activate_skill(skill_name, goal)
  -> runtime enters the unified skill executor
```

That is different from the older “match `meta.yaml.triggers` and inject the skill immediately” model.

### 3) `activate_skill` enters the unified skill executor

After the model calls `activate_skill`, the runtime:
- normalizes the selected skill name
- resolves `ActiveSkillContext` with `resolve_active_skill_by_name()`
- checks history to decide whether the full manual should be re-injected

Manual loading has three modes:
- `Initial`: first time entering the skill, inject the full prompt bundle
- `ReuseRecent`: a recent trace already exists, skip re-injecting the full manual
- `ReloadInsufficient`: the skill was used before, but recent context is insufficient, so reload it

### 4) Tool scope becomes narrow inside the skill

Skill execution uses `run_prompt_skill_for_session()`. Its key behavior is:
- switch mode to `InteractionMode::Skill`
- write `## Active Skill: <name>` into the system prompt
- expose only the tools declared by that skill
- add `exec_local` if the skill supports local execution

So the effective runtime model is:

```text
SKILL.md guides the model
  + skill-scoped tool allowlist
  + optional exec_local
  -> constrained tool loop
  -> final answer
```

### 5) Skill traces are persisted for follow-up turns

After execution, the runtime writes these back into session history / metadata:
- the `activate_skill` tool result
- the internal trace `skill_enter`
- skill-internal tool call traces
- `active_skill_name`

That lets later turns:
- see a `Recent active skill` hint in the system prompt
- avoid re-injecting the full manual on short follow-ups

### 6) `forced_skill_name` is the bypass entry

If the inbound message metadata already contains `forced_skill_name`, the runtime skips skill selection and enters the skill directly.

Today that path is mainly used by:
- subagents: `spawn` encodes tasks as `__SKILL_EXEC__:<skill>:<query>`
- cron jobs: schedulers populate `forced_skill_name`
- WebUI skill tests: tests specify `forced_skill_name` directly

### 7) Cron and tests still mostly reuse the unified skill kernel

The repository still contains direct script helpers such as `SkillDispatcher` and `run_rhai_script_with_context()`, but the mainstream gateway / scheduler path still does:

```text
construct InboundMessage(metadata.forced_skill_name = ...)
  -> enter the unified skill executor
```

So in the current implementation:
- the mainline is the **Prompt Skill Kernel**
- `SKILL.py` / `scripts/` are used through `exec_local`
- `SKILL.rhai` is retained as script-orchestration capability, but not the default entry for ordinary chat

---

## meta.yaml: recommended fields today

```yaml
name: stock_monitor
description: "Real-time quote monitoring and analysis for CN/HK/US stocks"
tools:
  - finance_api
  - chart_generate
requires:
  bins:
    - python3
  env:
    - EASTMONEY_API_KEY
permissions:
  - network
fallback:
  strategy: degrade
  message: "Market data is temporarily unavailable. Please try again later."
```

The recommended fields are defined by `SkillMeta` and the default `BLOCKCELL.md` contract:
- required: `name`, `description`
- common: `tools`, `requires`, `permissions`, `fallback`
- still supported for compatibility but not recommended for new skills: `capabilities`, `always`, `output_format`
- older docs often showed `triggers`, but that is not the main router in the current unified entry

---

## SKILL.md: an operating manual for the LLM

This is one of the most creative parts of the design.

`SKILL.md` is not documentation for humans — it’s an **operating playbook for the LLM**. It tells the model:
- What the skill can do
- Which tools to call
- How to fill parameters
- How to handle errors

```markdown
# Stock monitoring skill playbook

## Quick data source guide

| Market | Code format | Tool calls |
|------|---------|---------|
| CN A-share (Shanghai) | 6 digits, e.g. 600519 | finance_api stock_quote source=eastmoney |
| CN A-share (Shenzhen) | 6 digits, e.g. 000001 | finance_api stock_quote source=eastmoney |
| HK stocks | 5 digits, e.g. 00700 | finance_api stock_quote source=eastmoney |
| US stocks | symbols, e.g. AAPL | finance_api stock_quote |

## Common symbols

- Kweichow Moutai: 600519
- Ping An: 601318
- Tencent: 00700 (HK)
- Apple: AAPL

## Scenario 1: real-time quote

Steps:
1. Call finance_api, action=stock_quote, symbol=code
2. Return: price, change %, volume, PE

## Scenario 2: historical trend

Steps:
1. Call finance_api, action=stock_history, symbol=code, period=1mo
2. Optional: call chart_generate to draw a line chart
```

The advantage: **you can shape LLM behavior by editing a Markdown file — without retraining the model.**

---

## SKILL.rhai: deterministic orchestration scripts

Current status:
- standalone `SKILL.rhai` execution support still exists in the codebase, but the main chat runtime does not auto-run it in the normal user flow
- `AgentRuntime::run_rhai_script_with_context()` is currently a retained helper without a mainline call site
- `blockcell skills test` does “compile + mock run” for `SKILL.rhai`, while `SKILL.py` only gets a `py_compile` syntax check, so the current validation paths are not symmetrical

Rhai is an embedded scripting language with a JavaScript/Rust-like syntax, designed for embedding into Rust programs.

`SKILL.rhai` handles **deterministic logic**, such as:
- Parameter validation
- Multi-step orchestration
- Error handling and graceful degradation
- Result formatting

```javascript
// Example SKILL.rhai: stock monitoring

// Get the stock symbol from user context
let symbol = ctx["symbol"];
if symbol == "" {
    set_output("Please provide a stock symbol, e.g. 600519 (Moutai)");
    return;
}

// Fetch real-time quote
let quote_result = call_tool("finance_api", #{
    "action": "stock_quote",
    "symbol": symbol
});

if is_error(quote_result) {
    // Degrade: try searching the web
    log_warn("finance_api failed, trying web_search");
    let search_result = call_tool("web_search", #{
        "query": `${symbol} stock price today`
    });
    set_output(search_result);
    return;
}

// Format output
let price = get_field(quote_result, "price");
let change = get_field(quote_result, "change_pct");
set_output(`${symbol} price: ${price}, change: ${change}%`);
```

In Rhai scripts, you can call any built-in tool (via `call_tool`), and you can implement branching, loops, and error handling.

---

## What skills are built in?

blockcell includes 40+ skills, broadly grouped as:

### Finance (16)
```
stock_monitor       - CN/HK/US stock quotes
bond_monitor        - bond market monitoring
futures_monitor     - futures & derivatives
crypto_research     - crypto research
token_security      - token security checks
whale_tracker       - whale tracking
address_monitor     - on-chain address monitoring
nft_analysis        - NFT analysis
defi_analysis       - DeFi analysis
contract_audit      - smart contract auditing
wallet_security     - wallet security
crypto_sentiment    - market sentiment
dao_analysis        - DAO analysis
crypto_tax          - crypto taxation
quant_crypto        - quantitative strategies
treasury_management - treasury management
```

### System control (3)
```
camera              - take a photo via camera
app_control          - macOS application control
chrome_control       - Chrome browser control
```

### General
```
daily_finance_report - daily finance report
stock_screener       - stock screening
portfolio_advisor    - portfolio advice
```

---

## How to create your own skill

### Method 1: just tell the AI

```
You: Create a skill that checks Moutai and Ping An every day at 8am.
    If either drops more than 3%, send me a Telegram message.
```

More accurately, the recommended starting point today is `meta.yaml` + `SKILL.md`, then add `scripts/`, `SKILL.py`, or `SKILL.rhai` only when needed.

### Method 2: create manually

```bash
mkdir -p ~/.blockcell/workspace/skills/my_monitor
```

Create `meta.yaml`:
```yaml
name: my_monitor
description: "My custom monitor"
tools:
  - finance_api
fallback:
  strategy: degrade
  message: "Monitoring failed. Please try again later."
```

Create `SKILL.md`:
```markdown
# My monitoring skill

## Function
Monitor a specified stock and send a notification when it drops beyond a threshold.

## Parameters
- symbol: stock symbol
- threshold: drop threshold (percentage)
```

Optional: only create `SKILL.rhai` if you explicitly want to keep a standalone Rhai orchestration path:
```javascript
let symbol = ctx["symbol"] ?? "600519";
let threshold = ctx["threshold"] ?? 3.0;

let quote = call_tool("finance_api", #{
    "action": "stock_quote",
    "symbol": symbol
});

let change = get_field(quote, "change_pct");
if change < -threshold {
    call_tool("notification", #{
        "channel": "telegram",
        "message": `⚠️ ${symbol} dropped ${change}%, beyond threshold ${threshold}%`
    });
}
```

### Method 3: install from the community hub

```
You: Search and install a DeFi monitoring skill from the community hub
```

The AI will call the `community_hub` tool to search and download the skill.

Common actions:
- `trending` / `search_skills` / `skill_info`
- `install_skill`: installs into `~/.blockcell/workspace/skills/<skill_name>/`
- `uninstall_skill` / `list_installed`

---

## Getting skills from communities: Blockcell Hub / OpenClaw GitHub import (WebUI)

blockcell currently supports two “community distribution” paths.

### 1) Blockcell Hub (Agent side + WebUI)

On the agent side, the built-in `community_hub` tool is used for discovery and installation.

In the WebUI, the “Community” tab is backed by Gateway proxy APIs:
- `GET /v1/hub/skills`: fetch trending list from Hub
- `POST /v1/hub/skills/:name/install`: download zip and extract into `~/.blockcell/workspace/skills/<name>/`

### 2) Import from OpenClaw community (WebUI External)

The WebUI “External” tab calls:
- `POST /v1/skills/install-external` with body `{ "url": "..." }`

Supported URL formats:
- **GitHub directory**: `https://github.com/<owner>/<repo>/tree/<branch>/<path>` (recursively fetched via GitHub Contents API)
- **GitHub single file**: `https://github.com/<owner>/<repo>/blob/<branch>/<path>` (auto-converted to raw)
- **Zip bundle**: any downloadable `.zip` URL

Import behavior (high-level):
- Downloads into an import **staging** directory first
- Tries to parse OpenClaw `SKILL.md` YAML frontmatter (`name`/`description`) and generates a minimal `meta.yaml`
- Triggers the self-evolution pipeline to convert the imported OpenClaw skill into blockcell format:
  - `.rhai` → `SKILL.rhai`
  - `.py` → `SKILL.py`
  - docs-only → improved `SKILL.md` + `meta.yaml`

Security and limits:
- Only http/https are allowed; localhost and `.local` are blocked
- Max download size (default 5MB), max file count (default 200), and a GitHub directory recursion depth limit

---

## Skill hot reload

When you create or modify skill files via chat, blockcell will **auto-detect changes and hot-reload** — no restart required.

```
You: Modify my_monitor so the threshold becomes 5%
AI: edits SKILL.rhai...
    [system detected skill updates and hot-reloaded my_monitor]
```

This is implemented in `runtime.rs`: after `write_file` or `edit_file` succeeds, if the path is under the skills directory, it triggers a reload and notifies the Dashboard via WebSocket.

---

## Skills vs tools: when to use which

| Scenario | Use tools | Use skills |
|------|--------|--------|
| One-off operations | ✅ | |
| Multi-step workflows | | ✅ |
| Reusability needed | | ✅ |
| Degradation/fallback strategy | | ✅ |
| Scheduled execution | | ✅ |
| Simple queries | ✅ | |

---

## A quick Rhai language primer

If you haven’t used Rhai before, here’s a quick intro:

```javascript
// Variables
let x = 42;
let name = "blockcell";

// Conditions
if x > 10 {
    print("greater than 10");
} else {
    print("not greater than 10");
}

// Loops
for i in 0..5 {
    print(i);
}

// Map (like a JSON object)
let params = #{
    "action": "stock_quote",
    "symbol": "600519"
};

// Call a tool (blockcell-specific)
let result = call_tool("finance_api", params);

// Error handling
if is_error(result) {
    log_warn("tool call failed");
    return;
}

// Get fields
let price = get_field(result, "price");
```

Rhai syntax is simple — even without prior programming experience, you can pick it up quickly.

---

## Summary

The Skill system is blockcell’s “software layer”:

- **`meta.yaml`** defines the trigger conditions
- **`SKILL.md`** provides operating guidance for the LLM
- **`SKILL.rhai`** implements deterministic orchestration logic

This three-layer design keeps skills flexible (LLMs can improvise) yet controlled (critical logic is enforced by scripts).

Next, we’ll look at the memory system — how blockcell uses SQLite + FTS5 to give AI persistent memory.

---

*Previous: [blockcell’s tool system — enabling AI to really execute tasks](./03_tools_system.md)*
*Next: [The memory system — letting AI remember what you said](./05_memory_system.md)*

*Repo: https://github.com/blockcell-labs/blockcell*
*Website: https://blockcell.dev*
