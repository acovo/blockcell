# Blockcell Skills Development Guide

This guide describes the current target model for skills under `skills/`.

The runtime now treats a skill as a three-stage flow:

1. decision
2. execute
3. summary

The core rule is simple:

- `meta.yaml` is the runtime contract
- `SKILL.md` is the human/LLM guide

`SKILL.md` should explain intent and output style. It should not carry machine-only execution contracts that the runtime must parse.

---

## 1. Standard Directory Layout

Each skill lives in its own directory:

```text
skills/
  your_skill/
    meta.yaml
    SKILL.md
    SKILL.rhai      # optional
    SKILL.py        # optional
    tests/          # optional
```

Constraints:

- `meta.yaml` is required for all new skills
- `SKILL.md` is required for all new skills
- `SKILL.rhai` and `SKILL.py` must not coexist unless you are doing temporary migration work
- `tests/` is optional but strongly recommended

---

## 2. Runtime Responsibilities

### 2.1 Decision Stage

The decision stage uses:

- current user question
- limited recent history
- `meta.yaml`

The decision stage does not use:

- `SKILL.md`
- global filesystem exploration
- arbitrary tool access
- subagent spawning

For structured skills, the decision output is a JSON invocation:

```json
{
  "method": "search",
  "arguments": {
    "query": "blockcell"
  }
}
```

### 2.2 Execute Stage

Execution depends on `execution.kind`:

- `markdown`: prompt-only execution
- `python`: script execution
- `rhai`: script execution

### 2.3 Summary Stage

Summary uses:

- original user question
- skill name
- method name when applicable
- `SKILL.md` brief
- execution result

Summary does not do routing or call tools again.

---

## 3. `meta.yaml` Contract

Minimal recommended template:

```yaml
name: your_skill
description: "One-line functional description"
requires:
  bins: []
  env: []
permissions: []
always: false
triggers:
  - "trigger phrase"
tools:
  - "web_search"
fallback:
  strategy: "degrade"
  message: "Fallback message shown to the user"
execution:
  kind: markdown
  dispatch_kind: prompt
  summary_mode: direct
```

### 3.1 Required Semantics

`name`

- must match the directory name

`description`

- should be functional, not promotional

`triggers`

- should be specific enough to avoid accidental activation

`tools`

- should list the real scoped tools used by the skill
- prefer `tools` over legacy `capabilities`

`fallback`

- should give the runtime a user-safe failure message

### 3.2 `execution`

`execution` is the runtime contract for modern skills.

Supported fields:

```yaml
execution:
  kind: markdown | python | rhai
  entry: SKILL.md | SKILL.py | SKILL.rhai
  dispatch_kind: prompt | argv | context
  summary_mode: direct | llm | none
  actions: []
```

Defaults:

- `markdown` => `dispatch_kind: prompt`, `summary_mode: direct`
- `python` => `dispatch_kind: argv`, `summary_mode: llm`
- `rhai` => `dispatch_kind: context`, `summary_mode: llm`

### 3.3 Structured Actions

For Python and Rhai skills, new implementations should use `execution.actions`.

Example:

```yaml
execution:
  kind: python
  entry: SKILL.py
  dispatch_kind: argv
  summary_mode: llm
  actions:
    - name: search
      description: Search AI news
      triggers:
        - "search"
      arguments:
        query:
          type: string
          required: true
          description: Search term
      argv:
        - "--query"
        - "{query}"
```

Rules:

- `actions` defines the machine-readable method schema
- `arguments` must describe required fields accurately
- `argv` is only for `python + argv`
- Rhai skills should still define `actions`, but runtime passes them via `ctx.invocation`

---

## 4. `SKILL.md` Contract

`SKILL.md` is for understanding and summarization.

It should explain:

- what the skill is for
- how the skill interprets user requests
- which result shape is preferred
- what to emphasize in the final answer
- fallback phrasing and output style

It should not be the only source of:

- command-line parameters
- method names
- required argument schema
- runtime branching rules

Recommended structure:

```markdown
# Skill Name

## Purpose

## Typical User Requests

## Execution Intent

## Output Style

## Fallback
```

For prompt-only skills, `SKILL.md` is the main execution manual.

For structured script skills, `SKILL.md` is mainly used for:

- meaning and scope explanation
- summary guidance
- user-facing output organization

---

## 5. Prompt-Only Skills

Target contract:

```yaml
execution:
  kind: markdown
  dispatch_kind: prompt
  summary_mode: direct
```

Behavior:

- routed into the prompt skill executor
- uses only skill-scoped tools
- does not re-run skill matching
- does not expand to global tool scope
- does not spawn subagents

Use prompt-only skills when:

- the workflow is model-led
- deterministic scripting is unnecessary
- the main value is reasoning, analysis, or flexible tool choice

---

## 6. Python Skills

Target contract:

```yaml
execution:
  kind: python
  entry: SKILL.py
  dispatch_kind: argv
  summary_mode: llm
  actions:
    - name: search
      arguments: ...
      argv: ...
```

Behavior:

- decision stage picks `method + arguments`
- runtime validates required arguments and enums
- runtime expands `argv`
- runtime executes `SKILL.py`
- summary stage uses `SKILL.md` brief plus script result

Recommendations:

- support structured arguments explicitly
- keep stdout compact and user-safe
- prefer JSON or concise readable text

---

## 7. Rhai Skills

Target contract:

```yaml
execution:
  kind: rhai
  entry: SKILL.rhai
  dispatch_kind: context
  summary_mode: llm
  actions:
    - name: search
      arguments: ...
```

Behavior:

- decision stage picks `method + arguments`
- runtime injects:

```json
{
  "invocation": {
    "method": "search",
    "arguments": {
      "query": "blockcell"
    }
  }
}
```

- Rhai reads it from `ctx.invocation`
- summary stage uses `SKILL.md` brief plus script result

Recommendations:

- use `ctx.invocation.method`
- use `ctx.invocation.arguments`
- keep `set_output(...)` concise
- avoid dumping raw full-page data into the final output

---

## 8. Legacy Compatibility

Current compatibility tiers:

- Tier 1: `SKILL.md` only
  - treated as prompt-only
- Tier 2: script exists but `execution.actions` is missing
  - treated as legacy script skill
  - still runs through compatibility paths
- Tier 3: script exists and `execution.actions` is complete
  - uses the new structured path

New skills should target Tier 1 or Tier 3.

Tier 2 should be treated as a temporary migration state.

---

## 9. Migration Checklist

### 9.1 From Legacy Python to Structured Python

1. Add `execution.kind: python`
2. Add `execution.entry: SKILL.py`
3. Add `dispatch_kind: argv`
4. Add `summary_mode: llm`
5. Move runtime method/argument contract into `execution.actions`
6. Keep `stdin` compatibility only as fallback, not as the primary contract

### 9.2 From Legacy Rhai to Structured Rhai

1. Add `execution.kind: rhai`
2. Add `execution.entry: SKILL.rhai`
3. Add `dispatch_kind: context`
4. Add `summary_mode: llm`
5. Move method/argument contract into `execution.actions`
6. Update script logic to read `ctx.invocation`

### 9.3 From Overloaded `SKILL.md` to Clean Separation

1. Move machine-readable action schema into `meta.yaml`
2. Leave only human/LLM guidance in `SKILL.md`
3. Remove hidden runtime-only parameter rules from prose
4. Keep `SKILL.md` focused on intent, output style, and fallback wording

---

## 10. Authoring Rules

- Do not rely on `SKILL.md` as the runtime schema source
- Do not make triggers too broad
- Do not expose tools that the skill does not actually need
- Do not depend on re-entering the general agent loop for prompt-only execution
- Do not use Tier 2 compatibility mode for new skills

For all new skills, the preferred target is:

- Prompt-only: clean `markdown` contract
- Scripted: fully structured `execution.actions`
