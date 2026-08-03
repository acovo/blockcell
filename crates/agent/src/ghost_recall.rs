use blockcell_core::config::MemoryRecallMode;
use blockcell_core::{Config, InboundMessage, Paths, Result};
use std::collections::HashSet;
use tracing::warn;

use crate::token::estimate_tokens;

pub(crate) fn should_inject_ghost_recall(
    config: &Config,
    msg: &InboundMessage,
    mode: MemoryRecallMode,
) -> bool {
    config.agents.ghost.learning.recall_enabled()
        && config.agents.ghost.learning.recall_max_items > 0
        && config
            .memory
            .effective_memory_recall()
            .allows(mode, &msg.channel)
}

pub fn build_ghost_recall_context_block(
    paths: &Paths,
    config: &Config,
    msg: &InboundMessage,
) -> Result<Option<String>> {
    if !should_inject_ghost_recall(config, msg, MemoryRecallMode::General) {
        return Ok(None);
    }

    let items = query_file_memory_recall_items(
        paths,
        Some(&msg.session_key()),
        &msg.content,
        config.agents.ghost.learning.recall_max_items as usize,
    )?;
    let Some(block) = build_memory_context_block(
        &items,
        config.agents.ghost.learning.recall_token_budget as usize,
    ) else {
        return Ok(None);
    };

    Ok(Some(block))
}

pub(crate) fn build_memory_context_block(
    items: &[FileMemoryRecallItem],
    token_budget: usize,
) -> Option<String> {
    if items.is_empty() || token_budget == 0 {
        return None;
    }

    let header = concat!(
        "<memory-context>\n",
        "Relevant durable file memory from USER.md and MEMORY.md.\n",
        "Use only when directly relevant. Current user instructions override this context.\n",
    );
    let footer = "</memory-context>";
    let mut body = String::new();
    let base_tokens = estimate_tokens(header) + estimate_tokens(footer);
    let mut used_tokens = base_tokens;
    let mut included = 0usize;

    for item in items {
        let entry = format!(
            "- [{}] {}\n",
            item.source,
            truncate_chars(item.content.trim(), 260)
        );
        let entry_tokens = estimate_tokens(&entry);
        if included > 0 && used_tokens + entry_tokens > token_budget {
            break;
        }
        if included == 0 && used_tokens + entry_tokens > token_budget {
            return None;
        }
        body.push_str(&entry);
        used_tokens += entry_tokens;
        included += 1;
    }

    if included == 0 {
        return None;
    }

    Some(format!("{header}{body}{footer}"))
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    match text.char_indices().nth(max_chars) {
        Some((idx, _)) => format!("{}...", &text[..idx]),
        None => text.to_string(),
    }
}

pub(crate) fn query_file_memory_recall_items(
    paths: &Paths,
    session_key: Option<&str>,
    raw_query: &str,
    limit: usize,
) -> Result<Vec<FileMemoryRecallItem>> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let query_tokens = normalize_recall_tokens(raw_query);
    if query_tokens.is_empty() {
        return Ok(Vec::new());
    }

    let mut collected = collect_file_memory_items(paths, session_key, raw_query, &query_tokens)?;
    collected.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.source.cmp(right.source))
            .then_with(|| left.content.cmp(&right.content))
    });
    collected.truncate(limit);
    Ok(collected)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileMemoryRecallItem {
    pub(crate) source: &'static str,
    pub(crate) content: String,
    pub(crate) score: usize,
}

fn collect_file_memory_items(
    paths: &Paths,
    session_key: Option<&str>,
    raw_query: &str,
    query_tokens: &[String],
) -> Result<Vec<FileMemoryRecallItem>> {
    let mut seen = HashSet::new();
    let mut items = Vec::new();

    let index = blockcell_storage::KnowledgeIndex::open(&paths.knowledge_index_db())?;
    index.rebuild_from_files(paths)?;
    for (rank, entry) in index.search(raw_query, 100)?.into_iter().enumerate() {
        if !seen.insert(format!("index:{}", entry.id)) {
            continue;
        }
        let source = if entry.file == "USER.md" {
            "USER.md"
        } else {
            "MEMORY.md"
        };
        items.push(FileMemoryRecallItem {
            source,
            content: entry.content,
            score: 10_000usize.saturating_sub(rank),
        });
    }

    let mut sources = Vec::new();
    if let Some(session_key) = session_key {
        let session_root = paths
            .memory_dir()
            .join("sessions")
            .join(blockcell_core::stable_hash_session_key(session_key));
        sources.push(("session/USER.md", session_root.join("USER.md")));
        sources.push(("session/MEMORY.md", session_root.join("MEMORY.md")));
    }
    for (source, path) in sources {
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                warn!(
                    path = %path.display(),
                    error = %err,
                    "Failed to read memory file (permission error or other I/O error)"
                );
                // Continue with other files — permission error on one file
                // should not block the other.
                continue;
            }
        };
        for chunk in memory_chunks(&content) {
            let score = recall_score(&chunk, query_tokens);
            if score == 0 {
                continue;
            }
            let key = format!("{source}:{chunk}");
            if seen.insert(key) {
                items.push(FileMemoryRecallItem {
                    source,
                    content: chunk,
                    score,
                });
            }
        }
    }
    Ok(items)
}

fn memory_chunks(content: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut paragraph = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !paragraph.trim().is_empty() {
                chunks.push(paragraph.trim().to_string());
                paragraph.clear();
            }
            continue;
        }
        if trimmed.starts_with('-') || trimmed.starts_with('*') || trimmed.starts_with('#') {
            if !paragraph.trim().is_empty() {
                chunks.push(paragraph.trim().to_string());
                paragraph.clear();
            }
            chunks.push(trimmed.to_string());
            continue;
        }
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(trimmed);
    }
    if !paragraph.trim().is_empty() {
        chunks.push(paragraph.trim().to_string());
    }
    chunks
}

fn recall_score(chunk: &str, query_tokens: &[String]) -> usize {
    let lower = chunk.to_lowercase();
    query_tokens
        .iter()
        .map(|token| {
            if lower.contains(token) {
                token.len().max(1)
            } else {
                0
            }
        })
        .sum()
}

fn normalize_recall_tokens(raw_query: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "a", "an", "and", "are", "do", "does", "how", "i", "if", "in", "is", "it", "like", "my",
        "of", "on", "or", "the", "to", "usually", "we", "what", "written", "would", "you",
    ];

    let mut tokens = Vec::new();
    let mut cjk_run = Vec::new();
    let mut latin_run = String::new();
    let flush_cjk = |run: &mut Vec<char>, output: &mut Vec<String>| {
        if run.len() == 1 {
            output.push(run[0].to_string());
        } else {
            output.extend(run.windows(2).map(|pair| pair.iter().collect::<String>()));
        }
        run.clear();
    };
    let flush_latin = |run: &mut String, output: &mut Vec<String>| {
        if run.is_empty() {
            return;
        }
        let token = std::mem::take(run).to_lowercase();
        if !STOP_WORDS.contains(&token.as_str()) {
            output.push(token);
        }
    };

    for character in raw_query.chars() {
        if is_cjk(character) {
            flush_latin(&mut latin_run, &mut tokens);
            cjk_run.push(character);
        } else {
            flush_cjk(&mut cjk_run, &mut tokens);
            if character.is_alphanumeric() {
                latin_run.push(character);
            } else {
                flush_latin(&mut latin_run, &mut tokens);
            }
        }
    }
    flush_cjk(&mut cjk_run, &mut tokens);
    flush_latin(&mut latin_run, &mut tokens);
    tokens.sort();
    tokens.dedup();
    tokens
}

fn is_cjk(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockcell_core::{Config, InboundMessage, Paths};

    fn test_msg(content: &str) -> InboundMessage {
        InboundMessage {
            channel: "cli".to_string(),
            account_id: None,
            sender_id: "user".to_string(),
            chat_id: "chat-1".to_string(),
            content: content.to_string(),
            media: vec![],
            metadata: serde_json::Value::Null,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        }
    }

    #[test]
    fn runtime_recall_respects_mode_and_internal_policy() {
        let mut config = Config::default();
        config.agents.ghost.learning.recall_enabled = Some(true);
        config.memory.memory_recall.chat = false;
        let cli = test_msg("release");

        assert!(!should_inject_ghost_recall(
            &config,
            &cli,
            MemoryRecallMode::Chat
        ));
        assert!(should_inject_ghost_recall(
            &config,
            &cli,
            MemoryRecallMode::General
        ));

        let mut system = test_msg("release");
        system.channel = "system".to_string();
        assert!(!should_inject_ghost_recall(
            &config,
            &system,
            MemoryRecallMode::General
        ));
        config.memory.memory_recall.internal = true;
        assert!(should_inject_ghost_recall(
            &config,
            &system,
            MemoryRecallMode::General
        ));
    }

    #[test]
    fn file_memory_recall_builds_memory_context_fence() {
        let base = std::env::temp_dir().join(format!(
            "blockcell-ghost-recall-test-{}",
            uuid::Uuid::new_v4()
        ));
        let paths = Paths::with_base(base);
        paths.ensure_dirs().expect("ensure dirs");
        std::fs::write(
            paths.memory_md(),
            "Deploy docs should include a rollback checklist.\n\nUnrelated note.",
        )
        .expect("write memory md");
        let mut config = Config::default();
        config.agents.ghost.learning.enabled = true;
        config.agents.ghost.learning.capture_enabled = Some(true);
        config.agents.ghost.learning.write_enabled = Some(true);
        config.agents.ghost.learning.recall_enabled = Some(true);
        config.agents.ghost.learning.recall_max_items = 2;
        config.agents.ghost.learning.recall_token_budget = 160;

        let text = build_ghost_recall_context_block(&paths, &config, &test_msg("deploy docs"))
            .expect("recall")
            .expect("context block");
        assert!(text.contains("<memory-context>"));
        assert!(text.contains("rollback checklist"));
        assert!(!text.contains("<ghost-recall>"));
    }

    #[test]
    fn file_memory_recall_skips_irrelevant_memory() {
        let base = std::env::temp_dir().join(format!(
            "blockcell-ghost-recall-test-{}",
            uuid::Uuid::new_v4()
        ));
        let paths = Paths::with_base(base);
        paths.ensure_dirs().expect("ensure dirs");
        std::fs::write(paths.memory_md(), "Only remember invoice formatting.")
            .expect("write memory md");
        let mut config = Config::default();
        config.agents.ghost.learning.enabled = true;
        config.agents.ghost.learning.capture_enabled = Some(true);
        config.agents.ghost.learning.write_enabled = Some(true);
        config.agents.ghost.learning.recall_enabled = Some(true);
        config.agents.ghost.learning.recall_max_items = 2;
        config.agents.ghost.learning.recall_token_budget = 160;

        let message = build_ghost_recall_context_block(&paths, &config, &test_msg("deploy docs"))
            .expect("recall");
        assert!(message.is_none());
    }

    #[test]
    fn chinese_ghost_recall_matches_related_canonical_memory() {
        let base = std::env::temp_dir().join(format!(
            "blockcell-ghost-recall-chinese-test-{}",
            uuid::Uuid::new_v4()
        ));
        let paths = Paths::with_base(base);
        paths.ensure_dirs().expect("ensure dirs");
        std::fs::write(paths.memory_md(), "发布前需要检查 changelog 和版本号")
            .expect("write memory md");
        let mut config = Config::default();
        config.agents.ghost.learning.recall_enabled = Some(true);

        let block = build_ghost_recall_context_block(&paths, &config, &test_msg("发版检查什么"))
            .expect("recall Chinese memory")
            .expect("Chinese context block");

        assert!(block.contains("changelog"));
    }

    #[test]
    fn ghost_session_recall_merges_global_but_not_other_sessions() {
        let base = std::env::temp_dir().join(format!(
            "blockcell-ghost-recall-session-test-{}",
            uuid::Uuid::new_v4()
        ));
        let paths = Paths::with_base(base);
        paths.ensure_dirs().expect("ensure dirs");
        crate::memory_file_store::MemoryFileStore::open(&paths)
            .unwrap()
            .add(
                crate::memory_file_store::MemoryFileTarget::Memory,
                "Global release checklist uses signed artifacts.",
            )
            .unwrap();
        crate::memory_file_store::MemoryFileStore::open_for_session(&paths, "cli:chat-1")
            .unwrap()
            .add(
                crate::memory_file_store::MemoryFileTarget::Memory,
                "Private narwhal deployment codename.",
            )
            .unwrap();

        let own = query_file_memory_recall_items(
            &paths,
            Some("cli:chat-1"),
            "narwhal signed artifacts",
            10,
        )
        .unwrap();
        let other = query_file_memory_recall_items(
            &paths,
            Some("cli:chat-2"),
            "narwhal signed artifacts",
            10,
        )
        .unwrap();

        assert!(own.iter().any(|item| item.content.contains("narwhal")));
        assert!(own
            .iter()
            .any(|item| item.content.contains("signed artifacts")));
        assert!(!other.iter().any(|item| item.content.contains("narwhal")));
        assert!(other
            .iter()
            .any(|item| item.content.contains("signed artifacts")));
    }

    #[test]
    fn ghost_recall_uses_knowledge_conflict_resolution() {
        let base = std::env::temp_dir().join(format!(
            "blockcell-ghost-recall-conflict-test-{}",
            uuid::Uuid::new_v4()
        ));
        let paths = Paths::with_base(base);
        paths.ensure_dirs().expect("ensure dirs");
        std::fs::write(
            paths.user_md(),
            concat!(
                "- [id:pref-old] [scope:user] [source:inferred] [updated:2026-07-01] User prefers detailed answers.\n",
                "- [id:pref-new] [scope:user] [source:user_statement] [updated:2026-08-01] [supersedes:pref-old] User prefers concise answers.\n",
                "- [id:pref-copy] [scope:user] [source:inferred] [updated:2026-07-15] User prefers concise answers.\n",
            ),
        )
        .expect("write conflicts");

        let concise = query_file_memory_recall_items(&paths, None, "concise answers", 10)
            .expect("recall concise preference");
        assert_eq!(concise.len(), 1);
        assert!(concise[0].content.contains("concise answers"));
        let detailed = query_file_memory_recall_items(&paths, None, "detailed answers", 10)
            .expect("recall superseded preference");
        assert!(detailed
            .iter()
            .all(|item| !item.content.contains("detailed answers")));
    }
}
