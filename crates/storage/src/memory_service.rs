use std::str::FromStr;

use blockcell_core::{Error, Result};
use chrono::{Duration, Utc};

use crate::memory::{MemoryItem, MemoryStore, UpsertParams};
use crate::memory_contract::{MemoryType, MemoryUpsertRequest, DEFAULT_SHORT_TERM_TTL_DAYS};

/// Canonical storage-layer memory service that applies normalization and defaults.
#[derive(Clone)]
pub struct MemoryService {
    store: MemoryStore,
}

impl MemoryService {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    pub fn upsert(&self, request: MemoryUpsertRequest) -> Result<MemoryItem> {
        if request.content.trim().is_empty() {
            return Err(Error::Validation("content cannot be empty".to_string()));
        }

        if request.scope != "short_term" && request.scope != "long_term" {
            return Err(Error::Validation(format!(
                "Invalid memory scope: {}",
                request.scope
            )));
        }

        let item_type = MemoryType::from_str(&request.item_type).map_err(Error::Validation)?;
        let expires_at = if request.scope == "short_term" && request.expires_at.is_none() {
            Some((Utc::now() + Duration::days(DEFAULT_SHORT_TERM_TTL_DAYS)).to_rfc3339())
        } else {
            request.expires_at
        };

        let params = UpsertParams {
            scope: request.scope,
            item_type: item_type.as_str().to_string(),
            title: request.title,
            content: request.content,
            summary: request.summary,
            tags: request.tags,
            source: request.source,
            channel: request.channel,
            session_key: request.session_key,
            importance: request.importance,
            dedup_key: request.dedup_key,
            expires_at,
        };

        self.store.upsert(params)
    }
}
