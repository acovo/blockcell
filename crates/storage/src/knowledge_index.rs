use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use blockcell_core::{Error, Paths, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnowledgeIndexEntry {
    pub id: String,
    pub file: String,
    pub anchor: String,
    pub content: String,
    pub content_hash: String,
    pub scope: String,
    pub source: String,
    pub updated_at: String,
    pub supersedes: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnowledgeIndexRebuildResult {
    pub indexed: usize,
    pub removed: usize,
    pub unchanged_files: usize,
}

#[derive(Clone)]
pub struct KnowledgeIndex {
    inner: Arc<Mutex<Connection>>,
}

impl std::fmt::Debug for KnowledgeIndex {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KnowledgeIndex")
            .finish_non_exhaustive()
    }
}

impl KnowledgeIndex {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)
            .map_err(|error| Error::Storage(format!("Failed to open knowledge index: {error}")))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
            .map_err(map_sqlite_error)?;
        let index = Self {
            inner: Arc::new(Mutex::new(conn)),
        };
        index.init_schema()?;
        Ok(index)
    }

    pub fn rebuild_from_files(&self, paths: &Paths) -> Result<KnowledgeIndexRebuildResult> {
        let mut result = KnowledgeIndexRebuildResult::default();
        for (label, path, scope) in [
            ("USER.md", paths.user_md(), "user"),
            ("memory/MEMORY.md", paths.memory_md(), "workspace"),
        ] {
            let file_result = self.rebuild_file(label, &path, scope)?;
            result.indexed += file_result.indexed;
            result.removed += file_result.removed;
            result.unchanged_files += file_result.unchanged_files;
        }
        Ok(result)
    }

    pub fn rebuild_file(
        &self,
        file: &str,
        path: &Path,
        default_scope: &str,
    ) -> Result<KnowledgeIndexRebuildResult> {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        let file_hash = sha256_hex(content.as_bytes());
        let entries = parse_entries(file, &content, default_scope);
        let mut conn = self
            .inner
            .lock()
            .map_err(|_| Error::Storage("Knowledge index lock poisoned".to_string()))?;
        let previous_hash = conn
            .query_row(
                "SELECT content_hash FROM knowledge_files WHERE file = ?1",
                params![file],
                |row| row.get::<_, String>(0),
            )
            .ok();
        if previous_hash.as_deref() == Some(file_hash.as_str()) {
            return Ok(KnowledgeIndexRebuildResult {
                unchanged_files: 1,
                ..KnowledgeIndexRebuildResult::default()
            });
        }

        let tx = conn.transaction().map_err(map_sqlite_error)?;
        let previous_count: usize = tx
            .query_row(
                "SELECT COUNT(*) FROM knowledge_entries WHERE file = ?1",
                params![file],
                |row| row.get(0),
            )
            .map_err(map_sqlite_error)?;
        tx.execute(
            "DELETE FROM knowledge_entries WHERE file = ?1",
            params![file],
        )
        .map_err(map_sqlite_error)?;
        for entry in &entries {
            tx.execute(
                "INSERT INTO knowledge_entries (
                    id, file, anchor, content, content_hash, scope, source, updated_at, supersedes
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    entry.id,
                    entry.file,
                    entry.anchor,
                    entry.content,
                    entry.content_hash,
                    entry.scope,
                    entry.source,
                    entry.updated_at,
                    entry.supersedes,
                ],
            )
            .map_err(map_sqlite_error)?;
        }
        tx.execute(
            "INSERT INTO knowledge_files (file, content_hash, indexed_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(file) DO UPDATE SET content_hash=excluded.content_hash, indexed_at=excluded.indexed_at",
            params![file, file_hash, Utc::now().to_rfc3339()],
        )
        .map_err(map_sqlite_error)?;
        tx.commit().map_err(map_sqlite_error)?;

        Ok(KnowledgeIndexRebuildResult {
            indexed: entries.len(),
            removed: previous_count.saturating_sub(entries.len()),
            unchanged_files: 0,
        })
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeIndexEntry>> {
        if query.trim().is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let fts_query = query
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .map(|token| format!("\"{}\"", token.replace('"', " ")))
            .collect::<Vec<_>>()
            .join(" ");
        let conn = self
            .inner
            .lock()
            .map_err(|_| Error::Storage("Knowledge index lock poisoned".to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT e.id, e.file, e.anchor, e.content, e.content_hash,
                        e.scope, e.source, e.updated_at, e.supersedes
                 FROM knowledge_entries e
                 JOIN knowledge_fts f ON f.rowid = e.rowid
                 WHERE knowledge_fts MATCH ?1
                 ORDER BY bm25(knowledge_fts) ASC
                 LIMIT ?2",
            )
            .map_err(map_sqlite_error)?;
        let rows = stmt
            .query_map(params![fts_query, limit.min(100) as i64], |row| {
                Ok(KnowledgeIndexEntry {
                    id: row.get(0)?,
                    file: row.get(1)?,
                    anchor: row.get(2)?,
                    content: row.get(3)?,
                    content_hash: row.get(4)?,
                    scope: row.get(5)?,
                    source: row.get(6)?,
                    updated_at: row.get(7)?,
                    supersedes: row.get(8)?,
                })
            })
            .map_err(map_sqlite_error)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_sqlite_error)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self
            .inner
            .lock()
            .map_err(|_| Error::Storage("Knowledge index lock poisoned".to_string()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS knowledge_entries (
                id TEXT PRIMARY KEY,
                file TEXT NOT NULL,
                anchor TEXT NOT NULL,
                content TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                scope TEXT NOT NULL,
                source TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                supersedes TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_knowledge_file ON knowledge_entries(file);
            CREATE INDEX IF NOT EXISTS idx_knowledge_hash ON knowledge_entries(content_hash);
            CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts USING fts5(
                content,
                file,
                anchor,
                content='knowledge_entries',
                content_rowid='rowid'
            );
            CREATE TRIGGER IF NOT EXISTS knowledge_ai AFTER INSERT ON knowledge_entries BEGIN
                INSERT INTO knowledge_fts(rowid, content, file, anchor)
                VALUES (new.rowid, new.content, new.file, new.anchor);
            END;
            CREATE TRIGGER IF NOT EXISTS knowledge_ad AFTER DELETE ON knowledge_entries BEGIN
                INSERT INTO knowledge_fts(knowledge_fts, rowid, content, file, anchor)
                VALUES ('delete', old.rowid, old.content, old.file, old.anchor);
            END;
            CREATE TRIGGER IF NOT EXISTS knowledge_au AFTER UPDATE ON knowledge_entries BEGIN
                INSERT INTO knowledge_fts(knowledge_fts, rowid, content, file, anchor)
                VALUES ('delete', old.rowid, old.content, old.file, old.anchor);
                INSERT INTO knowledge_fts(rowid, content, file, anchor)
                VALUES (new.rowid, new.content, new.file, new.anchor);
            END;
            CREATE TABLE IF NOT EXISTS knowledge_files (
                file TEXT PRIMARY KEY,
                content_hash TEXT NOT NULL,
                indexed_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS knowledge_vectors (
                entry_id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                dimensions INTEGER NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(entry_id) REFERENCES knowledge_entries(id) ON DELETE CASCADE
            );
            CREATE TRIGGER IF NOT EXISTS knowledge_vector_ad AFTER DELETE ON knowledge_entries BEGIN
                DELETE FROM knowledge_vectors WHERE entry_id = old.id;
            END;",
        )
        .map_err(map_sqlite_error)?;
        Ok(())
    }
}

fn parse_entries(file: &str, content: &str, default_scope: &str) -> Vec<KnowledgeIndexEntry> {
    let mut entries = Vec::new();
    let mut anchor = "root".to_string();
    let mut paragraph = Vec::new();

    let flush_paragraph =
        |entries: &mut Vec<KnowledgeIndexEntry>, paragraph: &mut Vec<String>, anchor: &str| {
            let content = paragraph.join("\n").trim().to_string();
            paragraph.clear();
            if !content.is_empty() && content != "---" {
                entries.push(build_entry(file, anchor, &content, default_scope));
            }
        };

    for line in content.lines() {
        if line.starts_with('#') {
            flush_paragraph(&mut entries, &mut paragraph, &anchor);
            anchor = slugify(line.trim_start_matches('#').trim());
        } else if line.trim().is_empty() {
            flush_paragraph(&mut entries, &mut paragraph, &anchor);
        } else if line.trim_start().starts_with("- [") {
            flush_paragraph(&mut entries, &mut paragraph, &anchor);
            entries.push(build_entry(file, &anchor, line.trim(), default_scope));
        } else {
            paragraph.push(line.to_string());
        }
    }
    flush_paragraph(&mut entries, &mut paragraph, &anchor);
    entries
}

fn build_entry(file: &str, anchor: &str, raw: &str, default_scope: &str) -> KnowledgeIndexEntry {
    let mut metadata = HashMap::new();
    let mut content = raw.trim().trim_start_matches('-').trim().to_string();
    while content.starts_with('[') {
        let Some(end) = content.find(']') else {
            break;
        };
        let token = &content[1..end];
        if let Some((key, value)) = token.split_once(':') {
            metadata.insert(key.trim().to_string(), value.trim().to_string());
        }
        content = content[end + 1..].trim_start().to_string();
    }
    let content_hash = sha256_hex(content.as_bytes());
    let id = metadata.remove("id").unwrap_or_else(|| {
        format!(
            "legacy-{}",
            &sha256_hex(format!("{file}:{anchor}:{content}").as_bytes())[..16]
        )
    });
    KnowledgeIndexEntry {
        id,
        file: file.to_string(),
        anchor: if anchor.is_empty() {
            "root".to_string()
        } else {
            anchor.to_string()
        },
        content,
        content_hash,
        scope: metadata
            .remove("scope")
            .unwrap_or_else(|| default_scope.to_string()),
        source: metadata
            .remove("source")
            .unwrap_or_else(|| "verified".to_string()),
        updated_at: metadata
            .remove("updated")
            .unwrap_or_else(|| Utc::now().date_naive().to_string()),
        supersedes: metadata.remove("supersedes"),
    }
}

fn slugify(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "root".to_string()
    } else {
        slug
    }
}

fn sha256_hex(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn map_sqlite_error(error: rusqlite::Error) -> Error {
    Error::Storage(format!("Knowledge index SQLite error: {error}"))
}
