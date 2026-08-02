# Article 05: The Memory System — Letting AI Remember What You Said

> Series: *In-Depth Analysis of the Open Source Project “blockcell”* — Article 5
---

## Why AI needs memory

Anyone who has used ChatGPT knows a pain point: **every time you start a new chat, the AI forgets who you are.**

Last time you told it “I do quantitative trading and focus on China A-share tech,” and this time you have to explain it all over again.

Worse, even within the same conversation, if it gets too long, early messages get “forgotten” once they exceed the context window.

blockcell’s memory system is designed to solve this.

---

## Memory System Architecture

blockcell’s memory system is built on **SQLite canonical storage + FTS5 full-text search + optional RabitQ vector indexing**.

```
~/.blockcell/workspace/memory/memory.db
```

It’s a local SQLite database that lives entirely on your machine — nothing is uploaded to any server. When vector recall is enabled, blockcell also maintains a local sync queue and stores embeddings in a RabitQ index.

### Database schema

```sql
-- Memory items table
CREATE TABLE memory_items (
    id          TEXT PRIMARY KEY,
    scope       TEXT,    -- 'short_term' | 'long_term'
    type        TEXT,    -- 'fact' | 'preference' | 'project' | 'task' | 'note'
    title       TEXT,
    content     TEXT,
    summary     TEXT,
    tags        TEXT,    -- JSON array
    importance  INTEGER, -- 1-10
    created_at  INTEGER,
    updated_at  INTEGER,
    expires_at  INTEGER, -- optional expiration
    deleted_at  INTEGER, -- soft delete
    dedup_key   TEXT     -- deduplication key
);

-- FTS5 full-text search virtual table
CREATE VIRTUAL TABLE memory_fts USING fts5(
    title, summary, content, tags,
    content=memory_items
);

-- Optional vector-index sync queue, used when memory.vector is enabled
CREATE TABLE memory_vector_queue (
    id          TEXT PRIMARY KEY,
    operation   TEXT NOT NULL,
    attempts    INTEGER NOT NULL DEFAULT 0,
    last_error  TEXT,
    updated_at  TEXT NOT NULL
);
```

---

## Memory categories

blockcell categorizes memory along two dimensions:

### By retention (scope)

| Type | Description | Typical usage |
|------|-------------|---------------|
| `long_term` | Long-term memory, kept permanently | user preferences, important facts, project info |
| `short_term` | Short-term memory, can expire | current task state, temporary data |

### By content (type)

| Type | Description |
|------|-------------|
| `fact` | objective facts (“Moutai’s symbol is 600519”) |
| `preference` | user preferences (“prefers Python over JS”) |
| `project` | project information (“building a quant trading system”) |
| `task` | task status (“analyzing Q3 earnings”) |
| `note` | general notes |

---

## Three memory tools

### `memory_upsert` — save memory

```json
{
  "tool": "memory_upsert",
  "params": {
    "title": "User preference: programming language",
    "content": "User prefers Python and dislikes JavaScript",
    "type": "preference",
    "scope": "long_term",
    "importance": 8,
    "tags": ["programming", "preference"],
    "dedup_key": "user_lang_preference"
  }
}
```

`dedup_key` is used for deduplication: if a memory item with the same key already exists, it will be updated instead of creating a new entry.

### `memory_query` — search memory

```json
{
  "tool": "memory_query",
  "params": {
    "query": "stocks preference",
    "scope": "long_term",
    "top_k": 5
  }
}
```

This uses hybrid retrieval: FTS5 recalls text-relevant candidates, the optional RabitQ vector index adds semantic candidates when enabled, and blockcell returns the highest fused scores.

### `memory_forget` — delete memory

```json
{
  "tool": "memory_forget",
  "params": {
    "action": "delete",
    "id": "MEMORY_ID"
  }
}
```

Supports soft delete (recoverable) and batch deletes (filtered by scope/type/tags).

---

## Enabling Vector Recall

Vector recall is optional and disabled by default. Enable it in `config.json5` under `memory.vector`:

```json
{
  "memory": {
    "vector": {
      "enabled": true,
      "provider": "openai",
      "model": "text-embedding-3-small",
      "uri": "./memory/vectors.rabitq",
      "table": "memory_vectors"
    }
  }
}
```

Field meanings:

- `enabled`: whether the vector runtime is enabled; default is `false`
- `provider` / `model`: embedding provider and model used for vector generation
- `uri`: RabitQ index location, preferably relative to the workspace
- `table`: vector table name; defaults to `memory_vectors`

When enabled, blockcell first writes memory items into SQLite, then syncs embedding operations into RabitQ. If the provider/model is missing or does not support OpenAI-compatible embeddings, vector runtime initialization fails fast. Without vector recall, SQLite + FTS5 remains fully usable.

---

## How memory is injected into conversations

When building context for **Skill / General mode**, blockcell generates a **memory brief** and injects it into the system prompt. **Chat mode currently does not inject the Memory Brief**:

```
[Memory Brief]
Long-term (top 20):
- User prefers Python and focuses on China A-share tech [preference, importance:8]
- Building a quant trading system with backtrader [project, importance:9]
- Moutai (600519) is a key watchlist stock [fact, importance:7]

Short-term (top 10):
- Today: analyzing Q3 financial statement data [task, expires: 2h]
```

This lets Skill / General tasks reuse relevant background without repetition; Chat mode currently keeps the lighter context without a Memory Brief.

---

## Real-world scenarios

### Scenario 1: remember user preferences

```
You: I do quant trading and focus on China A-share tech.
    I prefer Python and dislike Excel.

AI: Got it — I’ll remember your preferences.
    [calls memory_upsert]
```

Next time:

```
You: Analyze the recent trend of the tech sector

AI: [knows you care about China A-share tech]
    Sure — let’s analyze China A-share tech...
```

### Scenario 2: remember project info

```
You: I’m building a project called “Smart Stock Picker”,
    using Python + backtrader, aiming to beat CSI 300.

AI: Project info recorded.
    [memory_upsert type=project]
```

### Scenario 3: track task state

```
You: Analyze Moutai’s financial data over the past three months

AI: Sure — starting the analysis...
    [memory_upsert type=task scope=short_term expires_in_days=1]
    ...
    Done — the result has been saved.
```

---

## Memory query scoring

Memory search results are ranked by a fused score:

```
Final score = FTS/vector fused rank + importance bonus + recency bonus
```

- **FTS/vector fusion**: FTS5 candidates and optional RabitQ vector candidates are merged with rank fusion
- **Importance bonus**: `importance` (1–10), higher ranks earlier
- **Recency bonus**: more recently updated items get extra weight

This ensures that the most relevant, most important, and newest memories appear first in briefs. If vector runtime is disabled, the search automatically falls back to SQLite + FTS5 only.

---

## Memory maintenance

blockcell runs an automatic maintenance task every 60 seconds:

```rust
// tick logic in runtime.rs
store.maintenance(30)  // purge soft-deleted items older than 30 days
```

Maintenance includes:
1. Purging expired short-term memories
2. Purging soft-deleted memories older than 30 days (trash)
3. Updating FTS5 indexes
4. Retrying or writing pending vector-sync operations

---

## Managing memory from the CLI

```bash
# List all memories
blockcell memory list

# Search memories
blockcell memory search "stocks"

# Show one memory item
blockcell memory show <ID>

# Delete one memory item
blockcell memory delete <ID>

# Clear memories by scope/type
blockcell memory clear --scope short_term

# Memory statistics
blockcell memory stats

# Clean expired memories
blockcell memory maintenance --recycle-days 30

# Retry vector sync queue
blockcell memory retry-vector-sync --limit 100

# Rebuild vector index
blockcell memory reindex
```

---

## Migrating from the old version

If you previously used the Markdown-file-based memory system (`MEMORY.md`), blockcell will automatically migrate on first start:

```rust
// agent.rs
store.migrate_from_files(&paths)
// read sections from MEMORY.md + historical daily notes
// import into SQLite
```

---

## Why SQLite + FTS5?

A common question: why not put memory entirely in a vector database?

Reasons blockcell chose SQLite + FTS5:

1. **Zero extra services**: no additional server required
2. **Local-first**: all data stays on-device for privacy
3. **Good enough baseline**: structured memories work well with SQLite + FTS5
4. **Fast**: excellent read/write performance
5. **Reliable**: SQLite is one of the most widely used databases in the world

The vector index is now an optional enhancement layer. When enabled, blockcell syncs memory embeddings to RabitQ and uses hybrid recall; when disabled, the system still works entirely through SQLite + FTS5.

---

## Summary

blockcell’s memory system provides:

- **Persistence**: local SQLite storage; no loss after restart
- **Full-text search**: FTS5 supports Chinese keyword search
- **Hybrid retrieval**: optional RabitQ vectors add semantic recall
- **Smart injection**: injects the most relevant memory brief in Skill / General mode; Chat mode currently skips it
- **Automatic maintenance**: expiration cleanup and soft-delete trash
- **Privacy**: fully local; nothing is uploaded

With memory, blockcell becomes a personal AI assistant that understands you — not just a stateless chat tool.

---

*Previous: [The Skill system — extending AI capabilities with Rhai scripts](./04_skill_system.md)*
*Next: [Multi-channel access — Telegram/Slack/Discord/Feishu all supported](./06_channels.md)*

*Repo: https://github.com/blockcell-labs/blockcell*
*Website: https://blockcell.dev*
