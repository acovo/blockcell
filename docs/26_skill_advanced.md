# 第26篇：Skill 开发高级教程 —— 把 Skill 设计成可测试、可发布的 CLI 工作流

> 系列文章：《blockcell 开源项目深度解析》—— 技能开发高级篇

---

## 这一篇解决什么问题

到了高级阶段，Skill 不能只追求“能完成任务”。

你需要追求的是：

- 能稳定复用
- 能独立测试
- 能快速迭代
- 能被 CLI、Cron、Gateway、子智能体等入口一致调用
- 能兼容当前的 `SKILL.md` 控制面模型

换句话说，高级 Skill 的目标不是“更会聊天”，而是：

> 把 Skill 做成一个有控制面、有执行面、有验证面的微型产品。

这也是 blockcell 当前最明显的演进方向之一：**Skill 正在朝 CLI 化、脚本化、可编排化发展**。

---

## 先把当前事实说清楚

在当前实现里，Skill 的主路径并不是直接执行 `SKILL.py` 或 `SKILL.rhai`，而是：

- 通过 `SKILL.md` 组织行为
- 由模型决定是否调用技能
- 进入统一 skill executor
- 再根据技能能力开放有限工具

同时，代码里也保留了两个很重要的能力：

- `exec_local`：在 active skill 目录内执行受控本地脚本
- `exec_skill_script`：统一执行 skill 本地脚本资产，`.rhai` 走 in-process，其它路径走 process

所以高级 Skill 的设计重点是：

1. **不要假设某个单一脚本后缀就是主入口**
2. **不要把控制逻辑和执行逻辑混在一起**
3. **把 CLI 能力当成一等公民设计**

---

## 高级 Skill 的三层架构

我建议你把一个成熟 Skill 拆成三层：

### 1. 控制面：`SKILL.md`

负责回答这些问题：

- 什么时候直接回答
- 什么时候调用脚本
- 什么时候调用工具
- 失败时怎么降级
- 输出给用户什么摘要

### 2. 执行面：脚本资产

负责做确定性事情，例如：

- 读取文件
- 解析参数
- 生成产物
- 格式转换
- 批量处理
- 校验输入输出

### 3. 交付面：用户看到的结果

负责把脚本结果包装成可读的最终答复，例如：

- 生成了什么文件
- 哪些步骤成功了
- 哪些步骤失败了
- 下一步怎么处理

高级 Skill 的核心，不是让某一层承担全部责任，而是让每层各司其职。

---

## 为什么 Skill 会越来越像 CLI

因为真实任务里，很多“技能”最终都不是为了聊天，而是为了执行。

比如：

- 生成报告
- 批量整理文档
- 提取结构化数据
- 跑语法检查
- 生成补丁
- 调用本地工具链
- 把结果交给下一步流水线

这些事情天然是 CLI 思维：

- 输入明确
- 输出明确
- 错误码明确
- 可重复执行
- 容易集成自动化

所以高级 Skill 的设计方法，和写一个好 CLI 很像：

- 入口清晰
- 参数明确
- 输出结构化
- 失败可诊断
- 默认行为稳定

---

## 真实案例：把 `ppt-generator` 升级成 CLI 型 Skill

`ppt-generator` 现在已经很接近一个产品级 Skill，但它仍然是以 `SKILL.md` 为控制面。

我们先看它当前的事实：

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

它的 `SKILL.md` 已经非常像一个“命令协议”了：

```markdown
# PPT Generator

## Shared {#shared}

- 这个 skill 用于将用户讲稿转换为极简科技风的竖屏 HTML 演示稿。
- 最终产物是单个可直接运行的 HTML 文件。
- 默认推断规则：
  - 未指定页数时，自动生成 8 到 20 页
  - 未指定风格时，默认采用乔布斯风极简科技风
  - 未指定输出路径但用户要求直接生成文件时，默认写入工作区相对路径 `generated/ppt-generator-output.html`

## Prompt {#prompt}

- 先判断用户是否已经提供可用的原始内容。
- 只有在没有正文、提纲或可读取文件时才先澄清。
- 生成流程必须严格遵循：
  1. 保留原始内容的事实含义
  2. 生成提炼版讲稿，输出 Markdown
  3. 设计幻灯片结构
  4. 基于模板生成最终 HTML
```

这类 skill 的高级化方向，不是继续加很多形容词，而是把它拆成**可测试的执行链路**。

### 可以怎么升级

```text
skills/ppt-generator/
├── meta.yaml
├── SKILL.md
├── scripts/
│   ├── outline.py
│   ├── render_html.py
│   └── write_preview.sh
├── bin/
│   └── ppt-generator
└── tests/
    ├── test_outline.py
    └── test_render_smoke.sh
```

这个结构表达的核心思想是：

- `SKILL.md` 负责决策：这次是总结、排版、还是直接出文件
- `scripts/` 负责确定性实现：提纲、渲染、落盘
- `bin/` 负责像 CLI 一样被调用
- `tests/` 负责验证产物是否符合合同

### 对应的 runtime 约束

现在 blockcell 的实际运行约束是这样的：

```rust
let mut declared_tools = active_skill.tools.clone();
if self
    .context_builder
    .skill_manager()
    .and_then(|manager| manager.get(&active_skill.name))
    .map(blockcell_skills::SkillManager::build_skill_card)
    .is_some_and(|card| card.supports_local_exec)
{
    declared_tools.push("exec_skill_script".to_string());
    declared_tools.push("exec_local".to_string());
}
```

这段代码说明了一件事：

- 不是所有 skill 都天然拥有本地脚本能力
- 一旦 skill 支持本地执行，runtime 才会补上 `exec_local` / `exec_skill_script`
- 所以高级 Skill 的“CLI 化”必须先在 skill 目录结构和 `SKILL.md` 里表达清楚

### 让 `ppt-generator` 拆出真正的脚本层

假设你要把它拆成脚本资产，最自然的做法是：

```python
# scripts/outline.py
import json
import sys

text = sys.stdin.read().strip()
slides = [
    {"title": "开场", "type": "hook", "body": text[:80]},
    {"title": "核心观点", "type": "argument", "body": "提炼后的观点"},
]

print(json.dumps({"slides": slides}, ensure_ascii=False, indent=2))
```

```bash
# scripts/render_html.py
python3 scripts/outline.py < "$1" > /tmp/ppt-outline.json
node bin/ppt-generator --outline /tmp/ppt-outline.json --output "$2"
```

```bash
# scripts/write_preview.sh
#!/bin/sh
set -e
python3 scripts/render_html.py "$1" "$2"
echo "generated: $2"
```

这个拆法的意义在于：

- 提纲生成可单测
- HTML 渲染可单测
- 写文件可单测
- `SKILL.md` 只保留调度规则，不堆实现细节

### 对应的测试也应该像 CLI 测试

当前仓库里的 `exec_skill_script` 测试已经给出了一个很好的模板：

```rust
#[tokio::test]
async fn test_exec_skill_script_runs_process_script() {
    let skill_dir = temp_skill_dir("blockcell-exec-skill-script-process");
    let scripts_dir = skill_dir.join("scripts");
    fs::create_dir_all(&scripts_dir).expect("create scripts dir");
    let script_path = scripts_dir.join("hello.sh");
    fs::write(&script_path, "#!/bin/sh\necho \"process $1\"\n").expect("write shell script");

    let result = run_exec_skill_script(
        skill_dir,
        json!({"path": "scripts/hello.sh", "args": ["ok"]}),
    )
    .await;

    assert_eq!(result["runtime"], "process");
    assert_eq!(result["success"], true);
    assert_eq!(result["exit_code"], 0);
}
```

这就是高级 Skill 该有的测试形态：

- 不是只看“有没有输出”
- 而是看运行时、退出码、stdout、stderr、路径是否都符合合同

### 高级阶段的 `SKILL.md` 应该写成什么样

对于 `ppt-generator` 这类 Skill，`SKILL.md` 最好继续保留这几个信息：

- 输入源优先级
- 默认页数和默认风格
- 产物文件名规则
- 何时调用脚本
- 失败时如何回退

更像一个命令式协议，而不是一篇散文。

### 如果你要把它进一步产品化

可以按下面的方式收敛：

1. 让 `SKILL.md` 只负责“什么时候生成、什么时候保存、什么时候追问”
2. 让 `scripts/outline.py` 只负责“把讲稿变成页面结构”
3. 让 `bin/ppt-generator` 只负责“把结构变成 HTML 文件”
4. 让测试固定检查输入/输出合同

这样之后，这个 skill 就已经非常接近一个真正的 CLI 工具了。

---

## 设计一个“像 CLI 一样”的 Skill

如果你希望一个 Skill 真的像 CLI，那就按 CLI 的思路设计它。

### 入口参数

先定义 skill 需要什么参数：

- `target`
- `mode`
- `format`
- `output`
- `dry_run`
- `timeout`

### 子动作

再把它拆成命令风格的子动作：

- `inspect`
- `build`
- `validate`
- `export`
- `rollback`

### 输出格式

建议固定成三段：

1. 结论
2. 结果摘要
3. 后续动作

### 错误处理

建议固定成四类：

- 参数错误
- 数据错误
- 执行错误
- 权限错误

这样一来，Skill 就会像一个真正的 CLI：

- 先理解命令
- 再执行动作
- 最后返回结构化结果

---

## 高级 Skill 的推荐实现策略

### 策略一：先写控制协议，再写脚本

不要先埋头写脚本。

先把 `SKILL.md` 写清楚：

- 这个 skill 的任务边界是什么
- 它应该怎么被调用
- 产物是什么
- 失败怎么办

### 策略二：把脚本做成可单独运行

如果脚本只能在模型环境里跑，那它就不够成熟。

高级 Skill 的脚本应该满足：

- 单独执行也能工作
- 输入输出尽量明确
- 依赖越少越好
- 可以在本地快速重现问题

### 策略三：让脚本返回结构化结果

推荐 JSON、明确退出码、固定目录产物。

不要只靠一大段 stdout。

### 策略四：把“判断”留在 `SKILL.md`

脚本负责执行，`SKILL.md` 负责决策。

如果所有判断都下沉到脚本里，后面维护会非常痛苦。

---

## `SKILL.rhai`、`SKILL.py`、`scripts/` 怎么看待

高级阶段最容易犯的错误，是把它们当成三套互相竞争的主入口。

更合理的理解是：

- `SKILL.md` 是控制面
- `SKILL.rhai` 是保留的确定性编排资产
- `SKILL.py` 和 `scripts/` 是本地脚本资产
- `exec_local` / `exec_skill_script` 是运行这些资产的统一手段

如果你的 skill 是“控制多、执行少”，`SKILL.md` 优先。

如果你的 skill 是“执行多、协议明确”，CLI / 脚本优先。

如果你的 skill 需要强约束流程，Rhai 仍然有价值，但它不应该掩盖控制面。

---

## 如何测试高级 Skill

高级 Skill 不能只靠手工点几次。

你至少要有三类测试：

### 1. 结构测试

检查：

- `meta.yaml` 是否完整
- `SKILL.md` 是否存在
- 脚本入口是否存在
- 目录结构是否符合预期

### 2. 语法测试

检查：

- `SKILL.py` 是否可解析
- Shell / Node 脚本是否能跑基本语法校验
- `.rhai` 是否可编译

### 3. 场景测试

检查：

- 正常输入能否完成
- 缺参时是否会追问
- 失败时是否会降级
- 输出格式是否稳定

当前 `blockcell skills test` 已经在做基础校验，但高级 Skill 不能只满足它。

你最好再补一层自己的场景测试：

- 最小输入
- 边界输入
- 异常输入
- 重复执行

---

## 适合高级 Skill 的几个设计模式

### 模式一：控制面 + 产物生成器

`SKILL.md` 决策，脚本生成文件。

适合：

- 日报
- 周报
- 文档生成
- 代码补丁生成

### 模式二：控制面 + 校验器

`SKILL.md` 决策，脚本做严格校验。

适合：

- 配置检查
- 目录扫描
- 合规审计
- 规则验证

### 模式三：控制面 + 批处理器

`SKILL.md` 决策，脚本对一批数据执行相同动作。

适合：

- 批量重命名
- 批量导出
- 批量转换
- 批量整理

### 模式四：控制面 + 细粒度子命令

把 skill 设计成多个命令风格动作。

适合：

- 复杂维护工具
- 发布助手
- 运维助手
- 研发辅助工具

---

## 高级 Skill 的验收标准

一个成熟 Skill 至少要满足这些要求：

- 结构清晰
- 入口明确
- 输出稳定
- 测试可跑
- 失败可解释
- 脚本可单独重现
- `SKILL.md` 仍然是唯一控制面
- 能适应 CLI / Cron / Gateway 等入口变化

如果这些都满足，这个 skill 才算真正接近“产品级”。

---

## 高级阶段最重要的原则

最后再强调一次：

> 不要让 Skill 变成一团 prompt、脚本和临时逻辑的混合物。

高级 Skill 的目标，是把复杂任务收束成一个可靠的工作流。

如果你写得好，它会同时具备：

- Prompt 的灵活性
- CLI 的稳定性
- 脚本的确定性
- runtime 的可控性

这就是 blockcell 里 Skill 的真正上限。
