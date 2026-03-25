use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Canonical memory item type definitions for storage-layer normalization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    Fact,
    Preference,
    Project,
    Task,
    Glossary,
    Contact,
    Snippet,
    Policy,
    Note,
    SessionSummary,
}

impl MemoryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryType::Fact => "fact",
            MemoryType::Preference => "preference",
            MemoryType::Project => "project",
            MemoryType::Task => "task",
            MemoryType::Glossary => "glossary",
            MemoryType::Contact => "contact",
            MemoryType::Snippet => "snippet",
            MemoryType::Policy => "policy",
            MemoryType::Note => "note",
            MemoryType::SessionSummary => "session_summary",
        }
    }
}

impl FromStr for MemoryType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "fact" => Ok(MemoryType::Fact),
            "preference" => Ok(MemoryType::Preference),
            "project" => Ok(MemoryType::Project),
            "task" => Ok(MemoryType::Task),
            "glossary" => Ok(MemoryType::Glossary),
            "contact" => Ok(MemoryType::Contact),
            "snippet" => Ok(MemoryType::Snippet),
            "policy" => Ok(MemoryType::Policy),
            "note" => Ok(MemoryType::Note),
            "session_summary" => Ok(MemoryType::SessionSummary),
            _ => Err(format!("Invalid memory type: {}", s)),
        }
    }
}

pub const DEFAULT_SHORT_TERM_TTL_DAYS: i64 = 3;

/// Canonical upsert request for storage-layer memory normalization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUpsertRequest {
    pub scope: String,
    pub item_type: String,
    pub title: Option<String>,
    pub content: String,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub source: String,
    pub channel: Option<String>,
    pub session_key: Option<String>,
    pub importance: f64,
    pub dedup_key: Option<String>,
    pub expires_at: Option<String>,
}
