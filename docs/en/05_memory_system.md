# Article 05: The Memory System — Letting AI Remember What You Said

> Series: *In-Depth Analysis of the Open Source Project “blockcell”* — Article 5

---

## Why AI needs memory

A new model conversation does not inherently remember user preferences, project facts, or previously verified lessons. blockcell separates that information into durable knowledge and short-term memory, then recalls it when relevant.

---

## Memory system architecture

BlockCell exposes three operational subsystems:

| Subsystem | Responsibility |
|---|---|
| Context Management | tool-result cache, micro/full compact, recovery budgets, and forked-agent execution |
| Knowledge System | durable files, short-term memory, indexes, unified retrieval, and prompt budgets |
| Learning System | boundary capture, coordinated review, skill learning, and low-frequency Dream maintenance |

Recommended configuration:

```json5
memory: {
  contextManagement: {
    forkedAgent: { enabled: true, maxTurns: 10, timeoutSecs: 120 }
  },
  knowledgeSystem: {
    promptBudget: {
      total: 12000,
      rules: 2000,
      userProfile: 800,
      retrieved: 3000,
      activeSkill: 4000,
      sessionRecovery: 2000
    },
    memoryRecall: { chat: true, general: true, skill: true, internal: false }
  },
  learningSystem: { enabled: true }
}
```

The legacy `memory.memorySystem.layer7` key is supported for one compatibility
version and maps to `memory.contextManagement.forkedAgent`; explicit new keys win.

blockcell uses **files as the durable source of truth, with SQLite for rebuildable indexes, short-term storage, and audit**:

```text
~/.blockcell/workspace/
├── USER.md                         # user preferences and durable constraints
└── memory/
    ├── MEMORY.md                   # project, feedback, and reference knowledge
    ├── knowledge_index.db          # disposable/rebuildable FTS and vector index
    └── memory.db                   # TTL-based short-term memory and migration source

~/.blockcell/workspace/ghost/
└── ghost_ledger.db                 # Ghost learning and forget audit
```

| Data | Role | Source of truth? |
|------|------|------------------|
| `USER.md` | stable user preferences, constraints, and explicit statements | Yes |
| `memory/MEMORY.md` | project, environment, feedback, and reference knowledge | Yes |
| `knowledge_index.db` | FTS5, metadata, and optional vector index over the files | No; rebuildable |
| `memory.db` | short-term, expiring memory for the current work | No |
| `GhostLedger` | audit trail for automatic learning and forgetting | No |

Only `USER.md` and `memory/MEMORY.md` are writable durable knowledge sources. A missing or damaged index is rebuilt from those files; index contents must never overwrite them as authoritative data.

---

## Memory categories

### Durable knowledge

- `USER.md`: explicit user preferences, communication style, and long-lived constraints.
- `memory/MEMORY.md`: project facts, verified lessons, feedback, and reference material.

`MEMORY.md` uses stable category headings:

```markdown
## Project

## Feedback

## Reference
```

Durable entries may include machine-readable metadata:

```markdown
- [id:pref-new] [scope:user] [source:user_statement] [updated:2026-08-01] [supersedes:pref-old] User prefers concise replies
```

- `id`: stable entry identifier.
- `scope`: `user` or `workspace`.
- `source`: `user_statement`, `verified`, or `inferred`.
- `updated`: most recent confirmation date.
- `supersedes`: ID of an older entry replaced by this one.

### Short-term memory

Short-term memory lives in `memory.db`, always uses `scope=short_term`, and is appropriate for task state, temporary data, and expiring information. It is not a durable knowledge source.

---

## Memory tools

### `memory_manage` — manage durable knowledge

Add a user preference:

```json
{
  "action": "add",
  "target": "user",
  "scope": "user",
  "content": "The user prefers concise summaries after code changes."
}
```

Add workspace knowledge:

```json
{
  "action": "add",
  "target": "memory",
  "scope": "workspace",
  "content": "Run targeted tests and git diff --check before release."
}
```

The tool also supports `replace`, `remove`, and `undo_latest`. Writes are normalized, safety-scanned, snapshotted, atomically persisted, and synchronized to the knowledge index.

### `memory_upsert` — save short-term memory

```json
{
  "title": "Current task",
  "content": "Analyzing Q3 financial statements",
  "type": "task",
  "scope": "short_term",
  "expires_in_days": 1
}
```

`memory_upsert` accepts only `short_term`. A `long_term` write is rejected with guidance to use `memory_manage`.

### `memory_query` — query SQLite short-term memory

```json
{
  "query": "Q3 statements",
  "scope": "short_term",
  "top_k": 5
}
```

This tool queries structured short-term entries in `memory.db`. Durable file knowledge is recalled by `KnowledgeIndex` at runtime.

### `memory_forget` — soft-delete legacy SQLite memory

```json
{
  "action": "delete",
  "id": "MEMORY_ID"
}
```

This retains the existing SQLite soft-delete and restore semantics, mainly for short-term and compatibility data.

### `knowledge_forget` — forget across all knowledge sources

Unified forgetting uses a two-phase confirmation flow across canonical files, current-session files, and SQLite short-term memory.

Preview the affected entries:

```json
{
  "action": "preview",
  "query": "concise replies"
}
```

Confirm with the exact token returned by preview:

```json
{
  "action": "confirm",
  "query": "concise replies",
  "reason": "user request",
  "preview_token": "..."
}
```

Confirmation removes the matches, rebuilds the index, records forget tombstones, and writes a GhostLedger audit event. Tombstones prevent Ghost background learning, normal writes, or snapshot restoration from resurrecting the same content.

---

## How durable knowledge is recalled

The system prompt contains one source-tagged `<retrieved-context>` block. Candidates
are deduplicated and ordered as `user-profile > knowledge > session > short-term >
skill-index`, then constrained by the global prompt budget. Recall is configurable for
Chat, General, and Skill modes; internal `ghost`, `cron`, `system`, and `subagent`
traffic is excluded by default.

Both `knowledge_fts` and `memory_fts` use the trigram tokenizer. Legacy indexes are
rebuilt on startup, while contiguous CJK queries are expanded into overlapping bigrams.

`KnowledgeIndex` incrementally indexes `USER.md` and `memory/MEMORY.md`. Candidates converge through this order:

```text
deduplicate content
→ discard entries targeted by supersedes
→ user_statement > verified > inferred
→ newer updated date first
→ FTS relevance
```

Ghost recall searches this index and separately merges relevant current-session file memory. The runtime injector reads only the two canonical durable files, not the legacy Layer 5 files.

Recall is ephemeral context rather than a system instruction, and current user instructions always take precedence.

---

## Real-world scenarios

### Remember a user preference

```text
You: I prefer Python, and give me a concise summary after code changes.

AI: Got it — I will remember that.
    [memory_manage target=user scope=user]
```

### Remember project information

```text
You: This project requires targeted tests and a rollback check before release.

AI: Recorded as durable workspace knowledge.
    [memory_manage target=memory scope=workspace]
```

### Track temporary task state

```text
You: Analyze the last three months of financial data.

AI: [memory_upsert scope=short_term expires_in_days=1]
```

### Fully forget incorrect knowledge

Call `knowledge_forget(action="preview")` first, inspect the impact, then call `confirm` with the returned `preview_token` to avoid accidental fuzzy deletion.

---

## Maintenance and safety

blockcell maintains expired short-term entries, the soft-delete recycle bin, and rebuildable indexes. Durable file writes include:

- normalization and deduplication;
- prompt-injection, credential, and hidden-control-character scanning;
- snapshots before writes;
- in-process mutexes, cross-process lockdirs, and atomic writes;
- index synchronization after writes;
- tombstones that prevent forgotten knowledge from returning.

---

## Managing memory from the CLI

```bash
# List, search, and inspect SQLite memory
blockcell memory list
blockcell memory search "statements"
blockcell memory show <ID>

# Delete, clean, and inspect SQLite memory
blockcell memory delete <ID>
blockcell memory clear --scope short_term
blockcell memory stats
blockcell memory maintenance --recycle-days 30

# Move legacy SQLite long_term rows into canonical files
blockcell memory migrate-canonical
```

---

## Migrating from older versions

### SQLite long-term rows

Run:

```bash
blockcell memory migrate-canonical
```

The command normalizes and deduplicates legacy `long_term` rows, writes them to `USER.md` or `memory/MEMORY.md`, soft-deletes each source row after a successful file write, and synchronizes the index. Re-running it is idempotent.

### Legacy Layer 5 files

At runtime, blockcell automatically consolidates these files into the two canonical files:

```text
base/memory/user.md       → workspace/USER.md
base/memory/project.md    → workspace/memory/MEMORY.md / Project
base/memory/feedback.md   → workspace/memory/MEMORY.md / Feedback
base/memory/reference.md  → workspace/memory/MEMORY.md / Reference
```

Migration deduplicates normalized content. After success, legacy files are reset to compatibility templates and are no longer injection sources.

---

## Why files are canonical while SQLite still exists

File-based durable knowledge is easy to inspect, edit, diff in Git, back up, and migrate. It also gives one unambiguous answer to “which data is true?” SQLite remains useful for the jobs it handles best:

- `knowledge_index.db` provides fast full-text retrieval, metadata ranking, and optional vector caching.
- `memory.db` manages structured, expiring short-term memory.
- `GhostLedger` stores learning and forgetting audit records.

Deleting the index therefore does not delete durable knowledge; changing canonical files allows the index to be regenerated.

---

## Summary

The knowledge and memory system follows four rules:

- `USER.md` and `memory/MEMORY.md` are the only durable sources of truth.
- `memory_upsert` writes only short-term memory; `memory_manage` owns durable knowledge.
- `KnowledgeIndex` provides incremental retrieval and conflict resolution but is always rebuildable.
- `knowledge_forget` provides real cross-source forgetting through preview, confirmation, tombstones, and audit.

---

*Previous: [The Skill system — extending AI capabilities with Rhai scripts](./04_skill_system.md)*

*Next: [Multi-channel access — Telegram/Slack/Discord/Feishu all supported](./06_channels.md)*

*Repo: https://github.com/blockcell-labs/blockcell*

*Website: https://blockcell.dev*
