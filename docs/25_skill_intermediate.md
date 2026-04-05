# 第25篇：Skill 开发中级教程 —— 从 Prompt Skill 走向 CLI 化 Skill

> 系列文章：《blockcell 开源项目深度解析》—— 技能开发进阶篇

---

## 这一篇要解决什么问题

初级 Skill 解决的是“能跑起来”。

中级 Skill 解决的是：**如何让 Skill 更像一个稳定的 CLI 工作流**。

现在 blockcell 的 Skill 正在明显朝这个方向发展：

- `SKILL.md` 负责控制面，决定什么时候该做什么
- `exec_local` 负责执行本地脚本资产
- `exec_skill_script` 负责统一执行 `.rhai` 或进程型脚本资产
- `SKILL.py`、`scripts/`、`bin/` 都可以成为可复用的脚本入口

所以中级阶段你要建立的新认知是：

> Skill 不只是“让 LLM 说话”，而是“让 LLM 调度一组可重复执行的本地能力”。

---

## 从 PromptTool 到 Hybrid

初级 Skill 通常只靠 `SKILL.md` + 工具白名单完成任务。

但当任务变复杂时，你会发现纯 Prompt 有几个问题：

- 规则越来越多，`SKILL.md` 会变长
- 输出越来越依赖模型稳定性
- 复杂步骤容易被模型跳过
- 外部程序、脚本、批处理能力不好表达

这时最自然的演进方式就是 **Hybrid Skill**：

```text
SKILL.md 负责告诉模型“何时调用脚本、何时直接回答”
  +
本地脚本资产负责做确定性执行
```

这类 skill 特别适合：

- 文件处理
- 批量转换
- 代码生成/检查
- 数据清洗
- 包装现有 CLI

---

## 一个更像 CLI 的 skill 目录

中级 Skill 建议开始引入脚本资产：

```text
skills/report_builder/
├── meta.yaml
├── SKILL.md
├── SKILL.py
├── scripts/
│   ├── build_report.sh
│   └── parse_input.py
└── bin/
    └── report_builder
```

这里最重要的不是文件后缀，而是**脚本是否能被稳定调用**。

当前实现里，`exec_local` 支持的 runner 主要是：

- `python3`
- `bash`
- `sh`
- `node`
- `php`

而且只能访问当前 active skill 目录下的相对路径。

这意味着：

- 你可以把 skill 当成一个“小型 CLI 项目”来设计
- 但不要把它当成任意系统命令执行器
- 所有脚本都应该是 skill 内的受控资产

---

## `SKILL.md` 在中级阶段的职责

当 skill 开始 CLI 化，`SKILL.md` 不再只是说明书，它更像**调度协议**。

建议你写清楚四件事：

### 1. 入口判断

告诉模型什么时候应该走脚本，什么时候应该直接回答。

例如：

- 只需要总结时，直接回答
- 需要格式化产物时，调用脚本
- 需要批量处理时，调用 `exec_local`
- 需要确定性逻辑时，调用 `exec_skill_script`

### 2. 资产说明

明确告诉模型有哪些本地资产可用：

- `SKILL.py`
- `scripts/*.sh`
- `scripts/*.py`
- `bin/*`

### 3. 调用规则

例如：

- 路径必须是相对路径
- 参数必须先校验
- 结果先落盘再总结
- 不要把大段内容直接塞给模型反复改写

### 4. 输出约定

CLI 化 skill 最常见的问题不是“不会执行”，而是“执行完怎么汇报结果”。

所以 `SKILL.md` 最后一定要规定：

- 产物存在哪里
- 输出给用户什么摘要
- 失败时怎么降级

---

## 推荐的调用方式

当前 blockcell 有两个相关工具：

- `exec_local`
- `exec_skill_script`

### `exec_local`

适合调用 skill 目录里的脚本和可执行文件。

你可以把它理解成：

> 在当前 skill 目录内执行一个受控的本地命令。

适合场景：

- shell 脚本
- Python 脚本
- Node 脚本
- PHP 脚本
- 可执行二进制

### `exec_skill_script`

适合统一执行 skill 内脚本资产。

当前实现里：

- `.rhai` 会走 in-process 执行
- 其他路径会按 process 方式执行

这类统一入口很适合 CLI 化 skill，因为它把“脚本是什么”从模型心里挪到了 runtime 层。

---

## 真实案例：`ppt-generator` 已经很接近一个“CLI 型 Skill”

如果你想看一个更接近真实产品的例子，可以直接看仓库里的 `ppt-generator`：

```text
blockcell skills show ppt-generator

🧠 Skill: ppt-generator
  Path: /Users/apple/.blockcell/workspace/skills/ppt-generator
  Status: ✅ enabled

  meta.yaml:
    name: ppt-generator
    description: 把讲稿整理成极简科技风 HTML 演示稿
    tools:
      - read_file
      - write_file
  Control Plane: SKILL.md ✓
```

这个 skill 很适合拿来做中级教程，因为它已经把“控制面”写得很完整了：

```markdown
# PPT Generator

## Shared {#shared}

- 这个 skill 用于将用户讲稿转换为极简科技风的竖屏 HTML 演示稿。
- 适用场景包括：
  - 生成 PPT
  - 生成演示文稿
  - 生成 Slides / 幻灯片
  - 将现有讲稿改写成科技风或乔布斯风演示稿
- 最终产物是单个可直接运行的 HTML 文件。
- 设计目标：
  - 极简主义：一屏只讲一件事
  - 强视觉对比：深色背景 + 白色文字
  - 高留白：禁止密集排版
  - 强节奏感：让观众想继续看

## Prompt {#prompt}

- 先判断用户是否已经提供可用的原始内容。
- 只有在以下情况才先澄清：
  - 没有任何可用于生成演示稿的正文、提纲或可读取文件
  - 用户目标明显矛盾，无法判断主题或用途
  - 用户明确要求的风格与当前 skill 的极简科技风严重冲突
- 不要因为缺少页数、受众、输出路径这类可默认推断的信息而阻塞执行。
- 如果用户已经直接给出讲稿正文、文章、提纲或聊天内已有足够内容，不要再追问“请提供文件路径”。
- 生成流程必须严格遵循：
  1. 保留原始内容的事实含义，不杜撰数据和案例
  2. 生成提炼版讲稿，增强演示冲击力，输出 Markdown
  3. 设计幻灯片结构，并为每一页标注页面类型
  4. 为每个章节生成标题，标题必须满足：
     - 不超过 12 字
     - 优先采用对比式、问题式、断言式、数字式、比喻式之一
  5. 基于视觉规范和基础模板生成最终 HTML
- 如果用户要求“直接生成文件”或“保存到本地”，在 HTML 完成后调用 `write_file`
```

这个例子很重要，因为它展示了一个中级 skill 的关键特征：

- 它不是简单回答问题，而是明确约束了**输入、产物、风格、输出路径**
- 它仍然是以 `SKILL.md` 为核心，而不是先把逻辑塞进脚本
- 它已经天然有 CLI 思维：讲稿输入 → 页面结构 → HTML 产物

### 你可以把它理解成一个“半 CLI”工作流

`ppt-generator` 还没有显式脚本目录，但它的行为已经像一个命令式程序：

```text
输入讲稿
  -> 提炼成 Markdown
  -> 组织页面结构
  -> 生成 HTML
  -> 如需落盘则 write_file
```

如果后面要把它继续演进成更强的 CLI Skill，最自然的方向就是：

- 把页面生成拆成脚本资产
- 把排版规则拆成可复用的本地模块
- 把最终 HTML 产物和中间 Markdown 产物都保留下来

### 真实的本地执行约束

当 skill 进入本地脚本阶段，runtime 不是“随便执行命令”，而是类似这样：

```rust
let mut command = if let Some(runner) = runner {
    let mut command = Command::new(runner);
    command.arg(&resolved_path);
    command
} else {
    Command::new(&resolved_path)
};
command
    .args(&args)
    .current_dir(&skill_dir)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
```

这段代码说明了一件事：

- 脚本始终运行在 active skill 目录内
- runner 只有白名单中的解释器
- 输出会被收集成结构化结果，而不是直接散落在终端里

所以，中级 Skill 真正应该学会的是：**把 skill 设计成受控的工作流，而不是一堆自由脚本**。

---

## 为什么 Skill 会越来越像 CLI

因为很多真实业务任务，最后都要落到“可执行产物”上。

比如：

- 生成日报
- 产出代码补丁
- 批量整理文件
- 运行检查脚本
- 导出结构化 JSON

这类任务有一个共同特征：

> 结果不是一段漂亮回复，而是一个可以继续被系统消费的产物。

CLI 化 Skill 的优势就在这里：

- 可重复
- 可调试
- 可审计
- 易于自动化
- 容易跟 CI / cron / 本地脚本结合

---

## 如何设计一个好的 Hybrid Skill

### 1. 先分层

把职责拆成三层：

- `SKILL.md`：控制面
- 脚本资产：执行面
- 输出摘要：用户界面

### 2. 先定入口，再定实现

先写“什么时候调用脚本”，再写脚本本身。

不要倒过来。

### 3. 保持脚本幂等

如果脚本会多次执行，尽量让它：

- 输入相同，输出相同
- 不依赖不稳定的隐式状态
- 能单独跑通

### 4. 尽量让脚本返回结构化结果

比如 JSON、表格、明确的退出码。

这样模型更容易总结，测试也更容易。

---

## 中级阶段最常见的坑

- **脚本太多，入口不清楚**

  模型不知道该调哪个脚本，Skill 就会变得脆弱。

- **`SKILL.md` 写得像 README**

  它应该像执行协议，不是产品介绍。

- **本地脚本里塞了过多业务判断**

  逻辑一旦复杂，就应该回到 `SKILL.md` 分层。

- **不写失败路径**

  失败时用户看到的往往不是技术错误，而是“为什么没结果”。

---

## 中级阶段的验收标准

如果一个 skill 已经算中级，至少应该满足：

- `SKILL.md` 能清楚地决定是否调用脚本
- 本地脚本资产有明确职责
- 脚本可以通过 `exec_local` 或 `exec_skill_script` 调起
- 输出结果可复用、可追踪
- `blockcell skills test` 能帮助你发现基础问题

---

## 从这里往下走

当你把 skill 做到 Hybrid 之后，下一步就是高级阶段：

- 如何把 skill 设计成“CLI + 控制面”的成熟形态
- 如何保持脚本、Rhai、Prompt 三种资产一致
- 如何设计更稳定的测试、进化和发布流程

下一篇会继续讲这个。
