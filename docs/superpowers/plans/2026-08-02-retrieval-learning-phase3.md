# Retrieval Quality and Learning Consolidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete optimization-plan phase three by unifying prompt retrieval and budgets, enabling mode-specific recall, improving Chinese FTS, consolidating boundary learning calls, and exposing the three-subsystem configuration model.

**Architecture:** Add a single agent-side retrieval orchestrator that owns source aggregation, semantic deduplication, source labeling, conflict ordering, and token allocation. Keep canonical files authoritative while both knowledge and short-term SQLite indexes use trigram FTS. Extend the existing `LearningCoordinator` from trigger arbitration into one boundary job that performs one classification call, dispatches structured outputs, and records one audit event; Dream remains independent.

**Tech Stack:** Rust, Tokio, serde/json5, rusqlite bundled SQLite FTS5, existing BlockCell provider/tool/runtime abstractions.

---

### Task 1: Retrieval Orchestrator and global prompt budget

**Files:**
- Create: `crates/agent/src/retrieval.rs`
- Modify: `crates/agent/src/lib.rs`
- Modify: `crates/agent/src/context.rs`
- Modify: `crates/core/src/config/memory.rs`
- Test: `crates/agent/src/retrieval.rs`
- Test: `crates/agent/src/context.rs`

- [ ] **Step 1: Write failing budget and deduplication tests**

Add tests that construct duplicate canonical/short-term/session candidates and assert one source-tagged item survives, then build a prompt with `prompt_budget.total = 8_000` and oversized rules, retrieval, skill, and recovery content and assert `estimate_tokens(prompt) <= 8_000`.

```rust
#[test]
fn retrieval_deduplicates_semantic_content_across_sources() {
    let items = vec![
        RetrievedItem::new(RetrievalSource::UserProfile, "User prefers concise replies"),
        RetrievedItem::new(RetrievalSource::ShortTerm, " User   prefers concise replies "),
    ];
    let result = RetrievalOrchestrator::deduplicate(items);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].source, RetrievalSource::UserProfile);
}

#[test]
fn prompt_allocator_never_exceeds_total_budget() {
    let config = PromptBudgetConfig { total: 8_000, ..Default::default() };
    let prompt = PromptBudgetAllocator::new(config).assemble(oversized_sections());
    assert!(estimate_tokens(&prompt) <= 8_000);
}
```

- [ ] **Step 2: Run tests and verify RED**

Run: `cargo test -p blockcell-agent retrieval -- --nocapture`

Expected: FAIL because `retrieval` and the new configuration types do not exist.

- [ ] **Step 3: Add prompt budget configuration and retrieval types**

Add `PromptBudgetConfig` under `MemoryConfig` with `total`, `rules`, `user_profile`, `retrieved`, `active_skill`, and `session_recovery`. Implement `RetrievalSource`, `RetrievedItem`, `PromptSections`, `PromptBudgetAllocator`, and `RetrievalOrchestrator` in `retrieval.rs`. Normalize whitespace and metadata prefixes for deduplication; prefer user profile, verified canonical knowledge, session knowledge, short-term memory, skill index, then optional KG.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptBudgetConfig {
    pub total: usize,
    pub rules: usize,
    pub user_profile: usize,
    pub retrieved: usize,
    pub active_skill: usize,
    pub session_recovery: usize,
}
```

- [ ] **Step 4: Replace independent context injection paths**

Change `ContextBuilder` to retain the configuration and call the orchestrator once per prompt. Remove direct whole-file `USER.md`/`MEMORY.md` injection, the separate SQLite Memory Brief block, and Layer 5 prompt injection. Feed canonical `KnowledgeIndex`, session files, session-scoped SQLite short-term memory, and the skill-index summary into one `<retrieved-context>` block with `[user-profile]`, `[knowledge]`, `[session]`, `[short-term]`, and `[skill-index]` labels. Keep AGENTS/SOUL/tool rules, active skill, and recovery as separately budgeted sections.

- [ ] **Step 5: Run focused and context regression tests**

Run: `cargo test -p blockcell-agent retrieval -- --nocapture`

Run: `cargo test -p blockcell-agent context -- --nocapture`

Expected: PASS; duplicate facts appear once and the configured total budget is respected.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/config/memory.rs crates/agent/src/lib.rs crates/agent/src/retrieval.rs crates/agent/src/context.rs
git commit -m "重构：统一知识召回与提示词预算"
```

### Task 2: Recall policy independent of interaction mode

**Files:**
- Modify: `crates/core/src/config/memory.rs`
- Modify: `crates/agent/src/retrieval.rs`
- Modify: `crates/agent/src/context.rs`
- Modify: `crates/agent/src/runtime.rs`
- Test: `crates/core/src/config.rs`
- Test: `crates/agent/src/context.rs`

- [ ] **Step 1: Write failing mode-policy tests**

Deserialize the configuration below and assert Chat, General, and Skill prompts recall knowledge while internal channels do not.

```json5
{
  memory: {
    memoryRecall: { chat: true, general: true, skill: true, internal: false }
  }
}
```

Add a Chat-mode context test that writes a canonical preference, asks a matching question, and expects the source-tagged retrieval block.

- [ ] **Step 2: Run tests and verify RED**

Run: `cargo test -p blockcell-core memory_recall -- --nocapture`

Run: `cargo test -p blockcell-agent chat_mode_recall -- --nocapture`

Expected: FAIL because recall is still gated by the legacy Skill/General condition.

- [ ] **Step 3: Implement `MemoryRecallConfig`**

Add `chat`, `general`, `skill`, and `internal` booleans with safe defaults. Implement a single `allows(mode, channel)` decision and use it for retrieval and runtime Ghost recall. Internal channels are `ghost`, `cron`, `system`, and `subagent` unless `internal=true`.

- [ ] **Step 4: Run focused and runtime regression tests**

Run: `cargo test -p blockcell-core memory_recall -- --nocapture`

Run: `cargo test -p blockcell-agent recall -- --nocapture`

Expected: PASS; Chat recalls when enabled and internal traffic stays excluded by default.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/config/memory.rs crates/core/src/config.rs crates/agent/src/retrieval.rs crates/agent/src/context.rs crates/agent/src/runtime.rs
git commit -m "功能：按交互模式配置知识召回"
```

### Task 3: Chinese trigram retrieval

**Files:**
- Modify: `crates/storage/src/memory.rs`
- Modify: `crates/storage/src/memory/schema.rs`
- Modify: `crates/storage/src/knowledge_index.rs`
- Modify: `crates/agent/src/ghost_recall.rs`
- Test: `crates/storage/src/memory/tests.rs`
- Test: `crates/storage/tests/knowledge_index.rs`
- Test: `crates/agent/src/ghost_recall.rs`

- [ ] **Step 1: Write failing Chinese recall tests**

Store `发布前需要检查 changelog 和版本号` and query `发版检查什么` in both short-term memory and canonical knowledge tests. Assert the relevant entry is returned. Add a schema assertion that both FTS virtual tables contain `tokenize='trigram'`.

- [ ] **Step 2: Run tests and verify RED**

Run: `cargo test -p blockcell-storage chinese -- --nocapture`

Expected: FAIL because the existing unicode tokenizer and whole-phrase quoting do not match the query.

- [ ] **Step 3: Implement FTS migration and CJK query expansion**

Create a shared storage helper that sanitizes Latin tokens and expands contiguous CJK text into quoted bigrams joined with `OR`. During schema initialization, inspect `sqlite_master`; when an existing FTS table is not trigram-based, drop its sync triggers and virtual table, recreate it with `tokenize='trigram'`, recreate triggers, and issue an FTS `rebuild` command.

```rust
assert_eq!(
    sanitize_fts_query("发版检查什么"),
    "(\"发版\" OR \"版检\" OR \"检查\" OR \"查什\" OR \"什么\")"
);
```

- [ ] **Step 4: Remove contains-based ranking from file recall**

Make canonical file recall call `KnowledgeIndex::search(raw_query, limit)` once and preserve index ordering. Keep only session-file matching as a separate session-local path; reuse the CJK query tokenizer rather than `contains` scoring.

- [ ] **Step 5: Run storage and agent recall regressions**

Run: `cargo test -p blockcell-storage chinese -- --nocapture`

Run: `cargo test -p blockcell-agent ghost_recall -- --nocapture`

Expected: PASS, including `发版检查什么` → `发布前需要检查 changelog 和版本号`.

- [ ] **Step 6: Commit**

```bash
git add crates/storage/src/memory.rs crates/storage/src/memory/schema.rs crates/storage/src/knowledge_index.rs crates/storage/src/memory/tests.rs crates/storage/tests/knowledge_index.rs crates/agent/src/ghost_recall.rs
git commit -m "功能：增强中文知识全文召回"
```

### Task 4: One coordinated boundary-learning job

**Files:**
- Modify: `crates/agent/src/learning_coordinator.rs`
- Modify: `crates/agent/src/runtime/learning.rs`
- Modify: `crates/agent/src/runtime/compaction.rs`
- Modify: `crates/agent/src/ghost_background_review.rs`
- Modify: `crates/storage/src/ghost_ledger.rs`
- Test: `crates/agent/src/learning_coordinator.rs`
- Test: `crates/agent/src/runtime/tests.rs`
- Test: `crates/agent/src/ghost_background_review.rs`

- [ ] **Step 1: Write failing coalescing and call-count tests**

Add a coordinator test where session summary, durable memory, user preference, and skill candidate triggers arrive for the same boundary; assert one queued `LearningBoundaryJob` contains all requested outputs. Add a runtime test provider that counts calls and assert pre-compress/session-end learning uses at most two calls: one classifier and one optional skill deepening call.

- [ ] **Step 2: Run tests and verify RED**

Run: `cargo test -p blockcell-agent coordinated_boundary -- --nocapture`

Expected: FAIL because pre-compress flush, session extraction, and Ghost review still schedule independently.

- [ ] **Step 3: Add structured boundary output**

Define serializable `UnifiedLearningOutput` with `session_summary`, `durable_memory`, `user_preferences`, and `skill_candidate`. Add `LearningBoundaryJob` keyed by session and boundary, with merged reasons and one immutable history snapshot.

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UnifiedLearningOutput {
    pub session_summary: Option<String>,
    pub durable_memory: Vec<String>,
    pub user_preferences: Vec<String>,
    pub skill_candidate: Option<SkillCandidate>,
}
```

- [ ] **Step 4: Route one classification result**

At pre-compress, session rotate, session end, and delegation end, enqueue one job. Make one provider classification request over the captured history, write the summary through the session-summary store, route durable/user facts through `MemoryFileStoreRouter`, and record a skill candidate for the existing skill-review path. Remove the separate pre-compress `flush_memories` call and prevent the same boundary from also spawning a duplicate Ghost review. Keep Dream unchanged.

- [ ] **Step 5: Record one complete audit event**

Write one GhostLedger episode/result containing requested outputs, classifier call count, optional skill-deepening count, dispatched targets, failures, and stop reason. Partial dispatch failures must be present in the audit result without losing successful outputs.

- [ ] **Step 6: Run coordinated-learning and runtime regressions**

Run: `cargo test -p blockcell-agent coordinated_boundary -- --nocapture`

Run: `cargo test -p blockcell-agent pre_compress -- --nocapture`

Run: `cargo test -p blockcell-agent ghost_background_review -- --nocapture`

Expected: PASS; complex boundary call count is at most two and one audit record describes the whole job.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/learning_coordinator.rs crates/agent/src/runtime/learning.rs crates/agent/src/runtime/compaction.rs crates/agent/src/ghost_background_review.rs crates/storage/src/ghost_ledger.rs
git commit -m "重构：合并会话边界后台学习任务"
```

### Task 5: Three-subsystem configuration and documentation

**Files:**
- Modify: `crates/core/src/config/memory.rs`
- Modify: `crates/agent/src/runtime/wiring.rs`
- Modify: `docs/05_memory_system.md`
- Modify: `docs/en/05_memory_system.md`
- Modify: `docs/design/blockcell-session-memory-system-design.md`
- Modify: `docs/27_ghost_learning_design.md`
- Test: `crates/core/src/config.rs`

- [ ] **Step 1: Write failing compatibility tests**

Deserialize both the new `contextManagement`/`knowledgeSystem`/`learningSystem` shape and a legacy `memorySystem.layer7` shape. Assert legacy `layer7.enabled/maxTurns/timeoutSecs` map to `contextManagement.forkedAgent`, while explicitly configured new keys win.

- [ ] **Step 2: Run tests and verify RED**

Run: `cargo test -p blockcell-core three_subsystem -- --nocapture`

Expected: FAIL because only seven-layer configuration exists.

- [ ] **Step 3: Add the new configuration surface and compatibility mapping**

Expose:

```json5
memory: {
  contextManagement: { forkedAgent: { enabled: true, maxTurns: 10, timeoutSecs: 120 } },
  knowledgeSystem: { promptBudget: {}, memoryRecall: {} },
  learningSystem: { enabled: true }
}
```

Mark `memorySystem.layer7` deprecated in Rust docs and serialization guidance. Resolve effective forked-agent values from new keys first, then legacy Layer 7 for one compatibility version.

- [ ] **Step 4: Update architecture and user documentation**

Describe only three public subsystems:

- Context Management: tool cache, micro/full compact, recovery, forked-agent execution.
- Knowledge System: durable files, user profile, short-term memory, indexes, optional KG, retrieval budget.
- Learning System: capture, coordinated review, skill learning, and low-frequency Dream maintenance.

Document the new prompt budget, interaction-mode recall, trigram migration, boundary call-count semantics, and Layer 7 deprecation mapping.

- [ ] **Step 5: Run configuration, docs, and full regression checks**

Run: `cargo fmt --all -- --check`

Run: `cargo test -p blockcell-core`

Run: `cargo test -p blockcell-storage`

Run: `cargo test -p blockcell-tools`

Run: `cargo test -p blockcell-agent`

Run: `cargo test -p blockcell`

Run: `cargo check -p blockcell`

Run: `git diff --check`

Expected: all commands pass.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/config/memory.rs crates/agent/src/runtime/wiring.rs docs/05_memory_system.md docs/en/05_memory_system.md docs/design/blockcell-session-memory-system-design.md docs/27_ghost_learning_design.md
git commit -m "文档：统一上下文知识与学习子系统"
```

### Task 6: Final phase-three acceptance

**Files:**
- Modify only files required by failures found during final acceptance.

- [ ] **Step 1: Run the phase acceptance matrix**

Run all commands from Task 5 Step 5 and additionally:

```bash
cargo test -p blockcell-agent retrieval -- --nocapture
cargo test -p blockcell-agent coordinated_boundary -- --nocapture
cargo test -p blockcell-storage chinese -- --nocapture
```

- [ ] **Step 2: Verify plan requirements directly**

Confirm an 8k configured prompt stays within budget, Chat recall works when enabled, the Chinese acceptance phrase is recalled, one complex boundary uses at most two provider calls, and legacy Layer 7 configuration maps to the new subsystem key.

- [ ] **Step 3: Commit only if acceptance required a repair**

Use one Chinese commit scoped to the repaired requirement. Do not create an empty acceptance commit.

