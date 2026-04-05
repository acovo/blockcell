# 第24篇：Skill 开发初级教程 —— 从 0 写出第一个能跑的 Skill

> 系列文章：《blockcell 开源项目深度解析》—— 技能开发入门篇

---

## 先建立正确的认知

blockcell 里的 skill，不是“一个文件”，而是“一套可复用的工作流”。

在当前实现里，普通对话不会直接跳进 skill，而是先走统一入口：

```text
用户消息
  -> General 模式
  -> system prompt 注入 SkillCard
  -> 模型决定是否调用 activate_skill
  -> 进入统一 skill executor
```

这意味着你写 skill 时，最重要的不是“我能不能把逻辑塞进去”，而是：

- 这个 skill 要解决什么稳定问题
- `SKILL.md` 要怎么指挥模型
- 需要哪些工具、脚本、fallback
- 是纯 Prompt，还是后面要长成 CLI / 本地脚本形态

如果你刚开始写 skill，**先从 PromptTool 开始**，不要一上来就写复杂脚本。

---

## Skill 的最小组成

当前比较推荐的起点是：

- `meta.yaml`
- `SKILL.md`

一个最小目录通常长这样：

```text
skills/my_skill/
├── meta.yaml
└── SKILL.md
```

它们的职责要分开：

| 文件 | 作用 | 读给谁 |
|------|------|--------|
| `meta.yaml` | 技能元数据、可用工具、依赖、fallback | 系统 |
| `SKILL.md` | 技能说明书、操作步骤、示例、约束 | LLM |

你可以把它理解成：

- `meta.yaml` 决定“这个 skill 是谁”
- `SKILL.md` 决定“这个 skill 怎么做事”

---

## 第一个 skill：先做 PromptTool

如果你的需求是“帮我整理、归类、总结、改写、查询，然后输出结果”，通常先写 PromptTool 就够了。

### 目录示例

```text
skills/notes_helper/
├── meta.yaml
└── SKILL.md
```

### `meta.yaml` 示例

```yaml
name: notes_helper
description: "帮助整理、摘要和重写文本"
tools:
  - read_file
  - write_file
fallback:
  strategy: degrade
  message: "当前无法完成整理任务，请稍后重试。"
```

### `SKILL.md` 示例

```markdown
# Notes Helper

## 目标
把用户给出的内容整理成更清晰的结构。

## 你要做的事
1. 先理解用户意图。
2. 如需查看上下文，使用允许的工具。
3. 输出简洁、结构化的结果。

## 约束
- 不要编造不存在的事实。
- 不要调用未声明的工具。
- 如果信息不足，先提问再继续。

## 输出格式
- 先给结论
- 再给分点说明
- 如有风险，单独提示
```

这个阶段最关键的目标只有一个：**让 skill 在稳定的约束下完成一件事**。

---

## `SKILL.md` 应该怎么写

初级阶段的 `SKILL.md` 建议包含四块内容：

- **目标**：这个 skill 用来做什么
- **输入**：用户会怎么提需求
- **步骤**：模型应该按什么顺序做事
- **输出**：最终结果要长什么样

如果你写的是中文技能，建议直接把规则写成“动作型句子”，例如：

- 先确认输入是否完整
- 再按顺序检查数据
- 最后给出结论和建议

不要在初级 skill 里堆太多概念。`SKILL.md` 的作用是帮助模型稳定执行，而不是写成一篇产品说明书。

---

## 现在的执行流程，你要记住这 4 步

这是当前 blockcell 里最重要的 skill 生命周期：

1. **加载**：启动时扫描技能目录，读取 `meta.yaml` 和 `SKILL.md`
2. **激活**：普通对话先进入 General，再由模型决定是否调用 `activate_skill`
3. **执行**：进入 skill 后，只暴露该 skill 声明的工具
4. **留痕**：执行结果和 trace 会写回会话，方便下一轮继续

另外还有一个很重要的旁路：

- 如果消息里已经带了 `forced_skill_name`，运行时会直接进入 skill

这个旁路常见于：

- CLI / 网关测试
- cron 调度
- 子智能体任务分发

你写教程时一定要避免一个旧误区：**现在不是“看到 trigger 就自动跑 skill”的老模型了**。

---

## 初级阶段先别碰什么

如果你刚开始写 skill，建议先不要碰这些东西：

- `SKILL.rhai`
- 复杂的本地脚本编排
- 多轮状态机
- 过多的工具白名单

原因很简单：

- 你还没有验证这个 skill 的“语言层”是否稳定
- 一旦逻辑过早下沉到脚本，调试成本会更高
- `SKILL.md` 还没写顺，后面加脚本也没意义

先把“模型能不能按你的规则干活”验证出来，再进入下一步。

---

## 如何测试一个初级 skill

最常用的命令是：

```bash
blockcell skills list
blockcell skills show notes_helper
blockcell skills test ./skills/notes_helper -i "帮我整理这段文字"
blockcell skills reload
```

当前 `blockcell skills test` 的重点是：

- 检查 `meta.yaml`
- 检查 `SKILL.md`
- 检查顶层脚本兼容性

它不是“把聊天里的整个 skill runtime 完整重放一遍”，所以不要把它误当成线上对话本身。

---

## 热更新是怎么来的

当你在对话里修改 skill 文件时，blockcell 会自动检测技能目录变化并热重载。

这带来的好处是：

- 改 `SKILL.md` 后不需要重启
- 调整提示词可以快速看到效果
- 非常适合做 skill 的迭代调优

所以初级阶段的推荐节奏是：

1. 先写最小 `meta.yaml` + `SKILL.md`
2. 用真实任务跑一次
3. 观察输出是否稳定
4. 再微调规则和示例

---

## 常见错误

- **以为 `SKILL.py` 会自动执行**

  不会。当前对话主路径仍然是 `SKILL.md` 驱动。

- **一开始就写太多工具**

  工具越多，模型越容易走偏。初学时只保留必要工具。

- **把 skill 写成普通文档**

  `SKILL.md` 不是介绍 skill 的文章，而是给模型执行的说明书。

- **忽略输出格式**

  你如果不规定输出格式，模型每次的结果都可能不一样。

---

## 初级阶段的目标

当你写完第一个 skill，应该达到这几个标准：

- 用户知道什么时候该用这个 skill
- 模型知道该调用哪些工具
- 输出结构稳定
- 改 `SKILL.md` 后可以快速迭代

如果你已经做到这一步，下一篇就该开始学：**如何把 Prompt Skill 变成更稳的 Hybrid Skill**。

