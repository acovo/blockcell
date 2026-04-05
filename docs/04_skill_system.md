# 第04篇：技能（Skill）系统 —— 当前技能调用链路与运行形态

> 系列文章：《blockcell 开源项目深度解析》第 4 篇
---

## 工具 vs 技能，有什么区别？

上一篇我们介绍了工具（Tool）。工具是原子操作，比如"读文件"、"搜网页"。

但实际任务往往是多步骤的：

```
监控茅台股价 = 
  每隔10分钟 → 查询股价 → 判断是否跌破阈值 → 发送告警 → 记录到日志
```

这种**多步骤、有逻辑的复合任务**，就是技能（Skill）要解决的问题。

**技能 = 封装了多个工具调用的可复用流程**

---

## 技能的组成

当前代码里不能把“技能”简单理解成单一文件类型。更准确地说，它包含三层：

1. **运行模式**：Chat、Test、Cron，决定同一套 skill 逻辑从哪个入口进入。
2. **布局类型**：Prompt Tool、Local Script、Hybrid、Rhai orchestration，决定 skill 目录里有哪些文件、由谁来执行。
3. **自进化类型**：Rhai、PromptOnly、Python，只用于进化、审计和补丁生成，不是全局唯一分类。

一个典型技能目录通常长这样：

```
skills/stock_monitor/
├── meta.yaml      # 元数据：名称、描述、工具、依赖、fallback
├── SKILL.md       # 操作手册：给 LLM 看的说明书
├── SKILL.rhai     # 可选：Rhai 编排脚本
├── SKILL.py       # 可选：本地脚本资产或 CLI 脚本
└── scripts/       # 可选：本地脚本资产
```

这些文件的角色应当这样理解：

| 文件/目录 | 作用 | 谁来读/执行 |
|------|------|--------|
| `meta.yaml` | 技能元数据、工具白名单、依赖、fallback | 系统 |
| `SKILL.md` | 技能操作手册、约束、示例、输出约定 | LLM |
| `SKILL.rhai` | 确定性编排逻辑 | Rhai 引擎 |
| `SKILL.py` | CLI 或 Python 本地脚本资产 | `exec_local` / 进化校验 |
| `scripts/` | Shell、Node、Python 等本地脚本资产 | `exec_local` |

注意这几个事实：

- **运行时契约**：当前技能至少要有 `meta.yaml` + `SKILL.md`。
- **Prompt Tool**：默认的对话技能是“SKILL.md 驱动 + 受限工具循环”。
- **Local Script / CLI**：`exec_local` 只负责执行当前技能目录中的相对脚本，不会自动把 `SKILL.py` 当成主入口。
- **Rhai**：仓库保留了 `SkillDispatcher` 和 `SKILL.rhai`，但它更像独立执行路径，不是普通聊天的默认执行器。

---

## 三种布局形态（Prompt Tool / Local Script / Hybrid）

### 1) Prompt Tool

当技能只有 `SKILL.md`，它就是一个 Prompt Tool：
- `SKILL.md` 提供操作手册
- 激活后，系统把 prompt bundle 注入上下文
- 模型在白名单工具里完成任务

这类技能适合：
- 主要依赖工具组合
- 不需要强确定性编排
- 通过 prompt 约束即可稳定工作的场景

### 2) Local Script / CLI

这类技能的关键不是“有没有 `SKILL.py`”，而是“有没有可被 `exec_local` 调起的本地脚本资产”。它可以是：
- `SKILL.py`
- `scripts/*.sh`
- `bin/*.sh`
- `scripts/*.py`
- `scripts/*.js`

代码里对应的约束是：
- `exec_local` 只能访问当前 active skill 目录
- runner 只允许 `python3`、`bash`、`sh`、`node`、`php`
- 本地脚本是否可用，要看 `SkillCard.supports_local_exec`

这类技能更适合：
- CLI 包装
- 现成脚本复用
- 需要调用外部程序但不想把逻辑塞进 prompt 的场景

### 3) Hybrid

Hybrid 不是一个新的执行器，而是“Prompt Tool + Local Script”的组合：
- `SKILL.md` 负责对话编排和约束
- `exec_local` 负责调用本地脚本
- 模型根据任务决定走 prompt 还是走脚本，或者两者结合

这类技能适合：
- 既要自然语言交互
- 又要复用 CLI 或脚本资产
- 还要保留 fallback 和人工调试能力

### 4) Rhai orchestration

`SKILL.rhai` 是独立的确定性编排路径：
- 通过 `SkillDispatcher` 执行
- 通过 `call_tool` 直接调用宿主工具
- 适合做强约束、多步骤、可测试的流程控制

但它不是普通聊天入口的默认选择。

---

## 当前实际调用流程（以代码实现为准）

下面这条链路对应当前 `runtime.rs`、`context.rs`、`manager.rs` 的真实行为。

### 1) 启动时装载技能

`ContextBuilder::new()` 会创建 `SkillManager`，并通过 `load_from_paths()` 扫描两层目录：
- 内置技能目录：先加载，优先级低
- workspace 技能目录：后加载，可覆盖内置技能

每个技能在加载时会完成这些动作：
- 读取 `meta.yaml` / `meta.json`
- 读取 `SKILL.md`
- 编译 `SKILL.md` 中的 shared / prompt / planning / summary bundle
- 构造 `SkillCard`
- 计算 `supports_local_exec`

### 2) 普通聊天先进入 General，再决定是否激活 skill

当前统一入口不是“按 trigger 自动进技能”。普通用户消息默认先进入 **General 模式**，然后：
- 运行时把已启用技能整理成 `SkillCard`
- 把这些 `SkillCard` 注入 system prompt 的 `Installed Skills` 区块
- 暴露 `activate_skill`

更准确的链路是：

```text
用户消息
  -> General 模式
  -> system prompt 注入 SkillCard
  -> 模型决定是否调用 activate_skill(skill_name, goal)
  -> 运行时进入统一 skill executor
```

### 3) `activate_skill` 之后进入统一 skill executor

当模型调用 `activate_skill` 后，运行时会：
- 归一化技能名，避免模型输出近似名时找不到技能
- 用 `resolve_active_skill_by_name()` 拿到 `ActiveSkillContext`
- 根据会话历史判断这次是否需要重新注入完整手册

手册注入有三种模式：
- `Initial`：第一次进入该技能，注入完整 prompt bundle
- `ReuseRecent`：最近上下文里已经有该技能 trace，不再重复注入整份手册
- `ReloadInsufficient`：历史里用过，但最近窗口信息不够，重新注入

### 4) 进入技能后，工具范围会被缩窄

技能执行阶段会调用 `run_prompt_skill_for_session()`，其关键行为是：
- 模式切换为 `InteractionMode::Skill`
- system prompt 中写入 `## Active Skill: <name>`
- 只给模型暴露该技能声明过的工具
- 如果技能支持本地执行，再额外放开 `exec_local`

因此，Prompt/Local Script/Hybrid 这一类技能的运行本质上是：

```text
SKILL.md 指导模型
  + 技能白名单工具
  + 可选 exec_local
  -> 受限工具循环
  -> 生成最终答案
```

### 5) 会话里会写入 skill trace，后续轮次可复用

技能执行完成后，运行时会把以下信息写回 session history / metadata：
- `activate_skill` 的 tool result
- 内部 trace：`skill_enter`
- 技能内部工具调用 trace
- `active_skill_name`

这样下一轮对话就可以：
- 在 system prompt 中提示 Recent active skill
- 在同一技能连续多轮时减少重复注入手册

### 6) `forced_skill_name` 是旁路入口

如果消息 metadata 里已经带了 `forced_skill_name`，运行时会跳过 `activate_skill` 选择阶段，直接进入技能执行。

当前这条旁路主要被这些场景使用：
- subagent：`spawn` 把任务编码成 `__SKILL_EXEC__:<skill>:<query>`
- cron：定时任务把 `skill_name` 写入 `forced_skill_name`
- WebUI test：测试技能时直接指定 `forced_skill_name`

### 7) Cron 和 Test 都是在同一套 runtime 上跑

仓库里仍保留 `SkillDispatcher` 和 Rhai 的脚本执行能力，但当前网关 / 调度器送入 runtime 的主流做法依然是：

```text
构造 InboundMessage(metadata.forced_skill_name = ...)
  -> 进入统一 skill executor
```

所以从“当前实际调用流程”看：
- 主链路是 Prompt Skill Kernel
- `SKILL.py` / `scripts/` 通过 `exec_local` 使用
- `SKILL.rhai` 是保留的脚本编排能力，不是普通聊天的默认入口

---

## meta.yaml：当前推荐字段

```yaml
name: stock_monitor
description: "A股/港股/美股实时行情监控与分析"
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
  message: "当前无法获取行情数据，请稍后重试。"
```

当前推荐字段以源码中的 `SkillMeta` 和默认 `BLOCKCELL.md` 约束为准：
- 必需：`name`、`description`
- 常用：`tools`、`requires`、`permissions`、`fallback`
- 兼容但不建议新增：`capabilities`、`always`、`output_format`
- 旧文档里常写的 `triggers`，并不是当前统一入口的主路由依据

---

## SKILL.md：给 LLM 的操作手册

这是技能系统最有创意的设计之一。

`SKILL.md` 不是给人看的文档，而是**给 LLM 看的操作规范**。它告诉 LLM：
- 这个技能能做什么
- 应该调用哪些工具
- 参数怎么填
- 遇到错误怎么处理

```markdown
# 股票监控技能操作手册

## 数据源速查

| 市场 | 代码格式 | 工具调用 |
|------|---------|---------|
| A股沪市 | 6位数字，如 600519 | finance_api stock_quote source=eastmoney |
| A股深市 | 6位数字，如 000001 | finance_api stock_quote source=eastmoney |
| 港股 | 5位数字，如 00700 | finance_api stock_quote source=eastmoney |
| 美股 | 字母代码，如 AAPL | finance_api stock_quote |

## 常见股票代码

- 贵州茅台: 600519
- 中国平安: 601318
- 腾讯控股: 00700（港股）
- 苹果: AAPL

## 场景一：查询实时股价

步骤：
1. 调用 finance_api，action=stock_quote，symbol=股票代码
2. 返回：价格、涨跌幅、成交量、市盈率

## 场景二：查询历史走势

步骤：
1. 调用 finance_api，action=stock_history，symbol=股票代码，period=1mo
2. 可选：调用 chart_generate 画折线图
```

这种设计的好处是：**LLM 的行为可以通过修改 Markdown 文件来调整，不需要重新训练模型。**

---

## SKILL.rhai：确定性编排脚本

`SKILL.rhai` 在代码里的位置是明确的，但它不是普通聊天的默认执行路径。更准确地说：
- 它属于独立的脚本编排层
- 通过 `SkillDispatcher` 执行
- 通过 `call_tool` 直接调用宿主工具
- 更适合做可测试、可重复、强约束的流程控制

这意味着 Rhai 适合：
- 参数校验
- 多步骤编排
- 错误处理和降级
- 结果格式化

```javascript
// SKILL.rhai 示例：股票监控

let symbol = ctx["symbol"];
if symbol == "" {
  set_output("请提供股票代码，例如：600519（茅台）");
  return;
}

let quote_result = call_tool("finance_api", #{
  "action": "stock_quote",
  "symbol": symbol
});

if is_error(quote_result) {
  log_warn("finance_api 失败，尝试 web_search");
  let search_result = call_tool("web_search", #{
    "query": `${symbol} 股价 今日`
  });
  set_output(search_result);
  return;
}

let price = get_field(quote_result, "price");
let change = get_field(quote_result, "change_pct");
set_output(`${symbol} 当前价格：${price}，涨跌幅：${change}%`);
```

Rhai 的关键价值不是“能不能写脚本”，而是“把关键流程收进确定性边界里”。

---

## 自进化：准确的改进方案

当前代码的自进化实现，最大问题不是“有没有三分法”，而是**分类轴没有统一**：
- runtime 侧看的是 Chat / Test / Cron
- layout 侧看的是 Prompt Tool / Local Script / Hybrid / Rhai
- evolution 侧却只分了 Rhai / PromptOnly / Python

这会带来两个直接问题：
1. CLI 类脚本没有被独立建模，只能借 Python 或 `exec_local` 间接表达
2. `SKILL.md`、`exec_local`、`SKILL.py`、`scripts/`、`SKILL.rhai` 这些东西在进化时的规则不是同一套

### 建议的目标模型

把自进化层改成四种布局类型，而不是只盯着文件后缀：

- **PromptTool**：只有 `SKILL.md`，靠模型 + 白名单工具完成任务
- **LocalScript**：以 CLI / 脚本资产为主，靠 `exec_local` 调起
- **Hybrid**：`SKILL.md` + `exec_local`，对话和脚本共存
- **RhaiOrchestration**：`SKILL.rhai` 驱动的确定性编排

其中 Python 只是 LocalScript 的一个子集，不应再把它当成全局独立大类。

### 改造步骤

1. 在自进化上下文里引入一个更通用的布局枚举，替代目前只按 `SkillType` 分支的做法。
2. 把布局检测改成文件树 + 运行入口联合判断：
   - `SKILL.md` only -> PromptTool
   - `SKILL.md` + `SKILL.py` / `scripts/` / `bin/` -> LocalScript 或 Hybrid
   - `SKILL.rhai` -> RhaiOrchestration
3. 让生成提示词、审计提示词、编译检查、合同检查都按布局分支，而不是按 Python/Rhai/PromptOnly 这种半抽象分类。
4. 对 LocalScript 单独建立校验：
   - Python：`py_compile`
   - Shell/Node/PHP：最小语法或入口存在性检查
   - CLI 资产：确认可通过 `exec_local` 调起
5. 把 `supports_local_exec` 从运行时辅助字段升级成进化层的显式能力标签。
6. 对导入和回滚路径保留 `scripts/`、`bin/`、`SKILL.py`、`SKILL.rhai`、`SKILL.md`，避免 CLI 技能在进化中丢失资产。

### 优先级建议

如果只能先做一件事，先做这两步：
- 先把自进化分类从 `SkillType` 改成“布局类型”
- 再把 CLI/local script 作为一等公民接入审计和编译检查

这样做的收益最大，因为它会直接修正当前“Python 代表本地脚本”的偏差。

---

## 内置了哪些技能

blockcell 内置了 40+ 技能，主要分几类：

### 金融类（16 个）
```
stock_monitor      - A股/港股/美股行情
bond_monitor       - 债券市场监控
futures_monitor    - 期货衍生品
crypto_research    - 加密货币研究
token_security     - 代币安全检测
whale_tracker      - 巨鲸追踪
address_monitor    - 链上地址监控
nft_analysis       - NFT 分析
defi_analysis      - DeFi 分析
contract_audit     - 合约审计
wallet_security    - 钱包安全
crypto_sentiment   - 市场情绪
dao_analysis       - DAO 分析
crypto_tax         - 加密税务
quant_crypto       - 量化策略
treasury_management - 资金管理
```

### 系统控制类（3 个）
```
camera             - 摄像头拍照
app_control        - macOS 应用控制
chrome_control     - Chrome 浏览器控制
```

### 综合类
```
daily_finance_report - 每日金融日报
stock_screener       - 股票筛选
portfolio_advisor    - 投资组合建议
```

---

## 如何创建自己的技能

### 方法一：直接告诉 AI

```
你: 帮我创建一个技能，每天早上 8 点查询茅台和平安的股价，
    如果任何一个跌超 3%，发 Telegram 消息给我
```

更准确地说，当前更推荐先生成 `meta.yaml` + `SKILL.md`，再按需要补 `scripts/`、`SKILL.py` 或 `SKILL.rhai`。

### 方法二：手动创建

```bash
mkdir -p ~/.blockcell/workspace/skills/my_monitor
```

创建 `meta.yaml`：
```yaml
name: my_monitor
description: "我的自定义监控"
tools:
  - finance_api
fallback:
  strategy: degrade
  message: "监控执行失败，请稍后重试。"
```

创建 `SKILL.md`：
```markdown
# 我的监控技能

## 功能
监控指定股票，跌超阈值时发送通知

## 参数
- symbol: 股票代码
- threshold: 跌幅阈值（百分比）
```

可选：如果你确实需要保留独立的 Rhai 编排能力，再创建 `SKILL.rhai`：
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
        "message": `⚠️ ${symbol} 跌幅 ${change}%，超过阈值 ${threshold}%`
    });
}
```

### 方法三：从社区仓库安装

```
你: 帮我从社区仓库搜索并安装一个 DeFi 监控技能
```

AI 会调用 `community_hub` 工具搜索并下载技能。

常用动作：
- `trending` / `search_skills` / `skill_info`
- `install_skill`：下载安装到 `~/.blockcell/workspace/skills/<skill_name>/`
- `uninstall_skill` / `list_installed`

---

## 从社区获取技能：Blockcell Hub / OpenClaw GitHub 导入（WebUI）

blockcell 目前支持两条“社区分发”路径：

### 1) Blockcell Hub（Agent 侧 + WebUI）

Agent 侧通过内置工具 `community_hub` 完成技能发现与安装。

WebUI 的“Community”页签则通过 Gateway 提供的代理接口一键安装：
- `GET /v1/hub/skills`：拉取 Hub 上的 trending 列表
- `POST /v1/hub/skills/:name/install`：下载 zip 并解压到 `~/.blockcell/workspace/skills/<name>/`

### 2) 从 OpenClaw 社区 GitHub/Zip 导入（WebUI External）

WebUI 的“External”页签调用：
- `POST /v1/skills/install-external`，参数：`{ "url": "..." }`

该导入接口支持 3 种 URL 形态：
- **GitHub 目录**：`https://github.com/<owner>/<repo>/tree/<branch>/<path>`（通过 GitHub Contents API 递归抓取文本文件）
- **GitHub 单文件**：`https://github.com/<owner>/<repo>/blob/<branch>/<path>`（自动转 raw）
- **Zip 包**：任意可下载的 `.zip` URL（解压后读取其中文本文件）

导入逻辑概览：
- 先写入“导入暂存目录（staging）”，再触发自进化把 OpenClaw 技能转换为 blockcell 的 `SKILL.rhai` / `SKILL.py` / `SKILL.md`
- 会尝试从 OpenClaw 的 `SKILL.md` YAML frontmatter 解析 `name/description` 并生成最小 `meta.yaml`

安全与限制：
- 仅允许 http/https，禁止 localhost / .local 等内网目标
- 限制最大下载体积（默认 5MB）、最大文件数（默认 200），GitHub 目录递归深度限制

---

## 技能热重载

当你通过 AI 对话创建或修改技能文件时，blockcell 会**自动检测文件变化并热重载**，不需要重启。

```
你: 帮我修改 my_monitor 技能，把阈值改成 5%
AI: 修改 SKILL.rhai 文件...
    [系统自动检测到技能更新，已热重载 my_monitor]
```

这个功能在 `runtime.rs` 中实现：每次 `write_file` 或 `edit_file` 成功后，如果路径在 skills 目录内，就触发重载并通过 WebSocket 通知 Dashboard。

---

## 技能 vs 工具：什么时候用哪个

| 场景 | 用工具 | 用技能 |
|------|--------|--------|
| 一次性操作 | ✅ | |
| 多步骤流程 | | ✅ |
| 需要复用 | | ✅ |
| 需要降级策略 | | ✅ |
| 需要定时执行 | | ✅ |
| 简单查询 | ✅ | |

---

## Rhai 语言简介

如果你没用过 Rhai，这里是一个快速入门：

```javascript
// 变量
let x = 42;
let name = "blockcell";

// 条件
if x > 10 {
    print("大于10");
} else {
    print("不大于10");
}

// 循环
for i in 0..5 {
    print(i);
}

// Map（类似 JSON 对象）
let params = #{
    "action": "stock_quote",
    "symbol": "600519"
};

// 调用工具（blockcell 特有）
let result = call_tool("finance_api", params);

// 错误处理
if is_error(result) {
    log_warn("调用失败");
    return;
}

// 获取字段
let price = get_field(result, "price");
```

Rhai 的语法非常简单，即使没有编程经验也能快速上手。

---

## 小结

技能系统更准确的理解方式是：

- **`meta.yaml`** 定义技能元数据和工具边界
- **`SKILL.md`** 定义对话式操作手册
- **`exec_local`** 让 CLI / 本地脚本资产变成可调用能力
- **`SKILL.rhai`** 提供确定性编排路径
- **自进化** 负责把这些形态分别校验、修复和演进

这套结构的核心不是“某一种文件格式”，而是“按执行形态分层”：对话、脚本、编排、自进化各管一层，边界清晰，才容易演进。

下一篇，我们来看记忆系统——blockcell 如何用 SQLite + FTS5 让 AI 拥有持久记忆。
---

*上一篇：[blockcell 的工具系统 —— 让 AI 真正能干活](./03_tools_system.md)*
*下一篇：[记忆系统 —— 让 AI 记住你说过的话](./05_memory_system.md)*

*项目地址：https://github.com/blockcell-labs/blockcell*
*官网：https://blockcell.dev*
