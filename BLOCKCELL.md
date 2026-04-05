# BLOCKCELL.md — Global Agent Rules

> 此文件由自进化系统自动加载为全局上下文。
> 编辑此文件可记录适用于所有 skill 的稳定规则、约定和经验。

---

## Skill 开发约定

- 所有用户 skill 位于 `~/.blockcell/workspace/skills/<skill_name>/`
- 每个 skill 必须有 `meta.yaml` + `SKILL.md`（运行时契约）
- `meta.yaml` 必填字段：`name`、`description`
- `meta.yaml` 可选字段：`tools`、`requires`、`permissions`、`fallback`
- **禁止** 使用遗留路由字段：`triggers`、`capabilities`、`always`、`output_format`
- `SKILL.md` 必须包含 `## Shared {#shared}` 和 `## Prompt {#prompt}` 两个 section

---

## Skill 类型对照

| 类型 | 主文件 | 适用场景 |
|------|--------|---------|
| Prompt Tool Skill | 仅 `SKILL.md` | 纯 LLM + blockcell 内置工具 |
| Local Script Skill | `SKILL.py` 或 `scripts/` | 需要 Python/Shell/Node 本地执行 |
| Hybrid Skill | `SKILL.md` + `SKILL.py` | 工具调用 + 本地脚本混合 |

---

## 自进化原则

1. **最小改动** — 只修改修复问题所必需的内容
2. **保持 SKILL.md 完整** — 不要删除 `## Shared` 或 `## Prompt` section
3. **禁止危险操作** — 避免 `os.remove`、`shutil.rmtree`、未经验证输入的 `subprocess`
4. **优雅降级** — 每次外部调用都要有 fallback 处理
5. **优先 `display_text`** — skill 产出用户可见内容时，使用 `display_text` 字段返回

---

## 工具使用规则

- `meta.yaml` 的 `tools` 字段只列真实用到的工具
- `exec_local` 由内核自动提供，**不要**写入 `tools`
- 使用 `web_fetch` 或 `http_request` 时，必须处理非 200 响应

---

## Python Skill 错误处理模板

```python
import sys
import json

try:
    result = do_work()
    print(json.dumps({"display_text": result}))
except Exception as e:
    print(json.dumps({"error": str(e)}), file=sys.stderr)
    sys.exit(1)
```

---

## 常见修复模式

| 问题 | 修复方式 |
|------|---------|
| API 返回空响应 | 加重试逻辑或优雅降级 |
| Import 报错 | 写入 `requires.bins` 或只用标准库 |
| JSON 解析失败 | 先校验 `response.status == 200` 再解析 |
| 输出格式异常 | 确保 `display_text` 或 `summary_data` 字段存在 |
| follow-up 场景断连 | SKILL.md Prompt section 中加"先复用历史，再决定是否重取"规则 |

---

## 自进化固定流程

自进化流程固定为完整检查链路，不再支持自定义 workflow 配置：

1. 生成补丁
2. 静态审计
3. LLM 审计
4. 编译检查
5. 契约检查
6. 部署并进入观察窗口

---

## 目录结构速查

```text
~/.blockcell/
├── config.json5          # 主配置
├── BLOCKCELL.md          # 本文件：全局规则（自进化上下文）
└── workspace/
    └── skills/
        └── <skill_name>/
            ├── meta.yaml
            ├── SKILL.md
            ├── SKILL.py  (或 SKILL.rhai / scripts/)
            └── manual/
                └── evolution.md  # 自动记录的修复经验
```
