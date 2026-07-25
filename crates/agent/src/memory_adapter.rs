use blockcell_core::Result;
use blockcell_storage::memory::{MemoryStore, QueryParams};
use blockcell_storage::memory_contract::MemoryUpsertRequest;
use blockcell_storage::memory_service::MemoryService;
use blockcell_tools::MemoryStoreOps;
use serde_json::Value;

/// Adapter that implements the tools crate's `MemoryStoreOps` trait
/// by delegating to the storage crate's `MemoryStore`.
pub struct MemoryStoreAdapter {
    store: MemoryStore,
}

impl MemoryStoreAdapter {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    /// Parse comma-separated tags from JSON value
    fn parse_tags(value: &Value, key: &str) -> Vec<String> {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
            .unwrap_or_default()
    }

    /// Get string value from JSON, converting to owned String
    fn get_string(value: &Value, key: &str) -> Option<String> {
        value.get(key).and_then(|v| v.as_str()).map(String::from)
    }

    /// Get string value with default
    fn get_string_or(value: &Value, key: &str, default: &str) -> String {
        Self::get_string(value, key).unwrap_or_else(|| default.to_string())
    }
}

impl MemoryStoreOps for MemoryStoreAdapter {
    fn upsert_json(&self, params_json: Value) -> Result<Value> {
        let request = MemoryUpsertRequest {
            scope: Self::get_string_or(&params_json, "scope", "short_term"),
            item_type: Self::get_string_or(&params_json, "type", "note"),
            title: Self::get_string(&params_json, "title"),
            content: Self::get_string_or(&params_json, "content", ""),
            summary: Self::get_string(&params_json, "summary"),
            tags: Self::parse_tags(&params_json, "tags"),
            source: Self::get_string_or(&params_json, "source", "user"),
            channel: Self::get_string(&params_json, "channel"),
            session_key: Self::get_string(&params_json, "session_key"),
            importance: params_json
                .get("importance")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5),
            dedup_key: Self::get_string(&params_json, "dedup_key"),
            expires_at: Self::get_string(&params_json, "expires_at"),
        };

        let item = MemoryService::new(self.store.clone()).upsert(request)?;
        serde_json::to_value(item).map_err(|e| {
            blockcell_core::Error::Storage(format!("Failed to serialize memory item: {}", e))
        })
    }

    fn query_json(&self, params_json: Value) -> Result<Value> {
        let tags = Self::parse_tags(&params_json, "tags");
        let tags = if tags.is_empty() { None } else { Some(tags) };

        let params = QueryParams {
            query: Self::get_string(&params_json, "query"),
            session_key: Self::get_string(&params_json, "session_key"),
            scope: Self::get_string(&params_json, "scope"),
            item_type: Self::get_string(&params_json, "type"),
            tags,
            time_range_days: params_json.get("time_range_days").and_then(|v| v.as_i64()),
            top_k: params_json
                .get("top_k")
                .and_then(|v| v.as_i64())
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value > 0)
                .unwrap_or(20)
                .min(blockcell_storage::memory::MAX_MEMORY_QUERY_RESULTS),
            include_deleted: params_json
                .get("include_deleted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        };

        let results = self.store.query(&params)?;
        serde_json::to_value(results).map_err(|e| {
            blockcell_core::Error::Storage(format!("Failed to serialize query results: {}", e))
        })
    }

    fn soft_delete(&self, id: &str) -> Result<bool> {
        self.store.soft_delete(id)
    }

    fn soft_delete_in_session(&self, id: &str, session_key: &str) -> Result<bool> {
        self.store.soft_delete_in_session(id, session_key)
    }

    fn batch_soft_delete_json(&self, params_json: Value) -> Result<usize> {
        let scope = params_json.get("scope").and_then(|v| v.as_str());
        let item_type = params_json.get("type").and_then(|v| v.as_str());

        let tags = Self::parse_tags(&params_json, "tags");
        let tags_ref = if tags.is_empty() {
            None
        } else {
            Some(tags.as_slice())
        };

        let time_before = params_json
            .get("before_days")
            .and_then(|v| v.as_i64())
            .map(|days| (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339());

        self.store
            .batch_soft_delete(scope, item_type, tags_ref, time_before.as_deref())
    }

    fn batch_soft_delete_in_session_json(&self, params_json: Value) -> Result<usize> {
        let session_key = Self::get_string(&params_json, "session_key").ok_or_else(|| {
            blockcell_core::Error::Validation(
                "'session_key' is required for scoped batch delete".to_string(),
            )
        })?;
        let scope = params_json.get("scope").and_then(|v| v.as_str());
        let item_type = params_json.get("type").and_then(|v| v.as_str());
        let tags = Self::parse_tags(&params_json, "tags");
        let tags_ref = (!tags.is_empty()).then_some(tags.as_slice());
        let time_before = params_json
            .get("before_days")
            .and_then(|v| v.as_i64())
            .map(|days| (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339());

        self.store.batch_soft_delete_in_session(
            &session_key,
            scope,
            item_type,
            tags_ref,
            time_before.as_deref(),
        )
    }

    fn restore(&self, id: &str) -> Result<bool> {
        self.store.restore(id)
    }

    fn restore_in_session(&self, id: &str, session_key: &str) -> Result<bool> {
        self.store.restore_in_session(id, session_key)
    }

    fn stats_json(&self) -> Result<Value> {
        self.store.stats()
    }

    fn stats_in_session_json(&self, session_key: &str) -> Result<Value> {
        self.store.stats_in_session(session_key)
    }

    fn generate_brief(&self, long_term_max: usize, short_term_max: usize) -> Result<String> {
        self.store.generate_brief(long_term_max, short_term_max)
    }

    fn generate_brief_in_session(
        &self,
        session_key: &str,
        long_term_max: usize,
        short_term_max: usize,
    ) -> Result<String> {
        self.store
            .generate_brief_in_session(session_key, long_term_max, short_term_max)
    }

    fn generate_brief_for_query(&self, query: &str, max_items: usize) -> Result<String> {
        self.store.generate_brief_for_query(query, max_items)
    }

    fn generate_brief_for_query_in_session(
        &self,
        session_key: &str,
        query: &str,
        max_items: usize,
    ) -> Result<String> {
        self.store
            .generate_brief_for_query_in_session(session_key, query, max_items)
    }

    fn upsert_session_summary(&self, session_key: &str, summary: &str) -> Result<()> {
        self.store.upsert_session_summary(session_key, summary)
    }

    fn get_session_summary(&self, session_key: &str) -> Result<Option<String>> {
        self.store.get_session_summary(session_key)
    }

    fn maintenance(&self, recycle_days: i64) -> Result<(usize, usize)> {
        self.store.maintenance(recycle_days)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockcell_storage::memory::MemoryStore;

    fn test_store() -> MemoryStore {
        let db_path = std::env::temp_dir().join(format!(
            "blockcell-memory-adapter-test-{}.db",
            uuid::Uuid::new_v4()
        ));
        MemoryStore::open(&db_path).expect("open memory store")
    }

    #[test]
    fn test_upsert_json_applies_default_short_term_ttl_via_service() {
        let store = test_store();
        let adapter = MemoryStoreAdapter::new(store);

        let item = adapter
            .upsert_json(serde_json::json!({
                "scope": "short_term",
                "type": "note",
                "content": "remember this"
            }))
            .expect("upsert_json should succeed");

        assert!(item["expires_at"].as_str().is_some());
    }

    #[test]
    fn scoped_mutations_reject_other_sessions_and_global_rows() {
        let adapter = MemoryStoreAdapter::new(test_store());
        let private_a = adapter
            .upsert_json(serde_json::json!({
                "scope": "long_term",
                "type": "fact",
                "content": "private a",
                "session_key": "ws:a"
            }))
            .unwrap();
        let private_b = adapter
            .upsert_json(serde_json::json!({
                "scope": "long_term",
                "type": "fact",
                "content": "private b",
                "session_key": "ws:b"
            }))
            .unwrap();
        let global = adapter
            .upsert_json(serde_json::json!({
                "scope": "long_term",
                "type": "fact",
                "content": "global"
            }))
            .unwrap();

        assert!(adapter
            .soft_delete_in_session(private_a["id"].as_str().unwrap(), "ws:a")
            .unwrap());
        assert!(!adapter
            .soft_delete_in_session(private_b["id"].as_str().unwrap(), "ws:a")
            .unwrap());
        assert!(!adapter
            .soft_delete_in_session(global["id"].as_str().unwrap(), "ws:a")
            .unwrap());
        assert!(!adapter
            .restore_in_session(private_a["id"].as_str().unwrap(), "ws:b")
            .unwrap());
        assert!(adapter
            .restore_in_session(private_a["id"].as_str().unwrap(), "ws:a")
            .unwrap());
    }

    #[test]
    fn batch_delete_is_scoped_to_caller_session() {
        let adapter = MemoryStoreAdapter::new(test_store());
        for session_key in ["ws:a", "ws:b"] {
            adapter
                .upsert_json(serde_json::json!({
                    "scope": "long_term",
                    "type": "fact",
                    "content": format!("private {session_key}"),
                    "session_key": session_key
                }))
                .unwrap();
        }

        let deleted = adapter
            .batch_soft_delete_in_session_json(serde_json::json!({
                "scope": "long_term",
                "session_key": "ws:a"
            }))
            .unwrap();
        assert_eq!(deleted, 1);

        let remaining = adapter
            .query_json(serde_json::json!({"session_key": "ws:b"}))
            .unwrap();
        assert_eq!(remaining.as_array().unwrap().len(), 1);
        assert_eq!(remaining[0]["item"]["content"], "private ws:b");
    }

    #[test]
    fn negative_top_k_never_becomes_unbounded() {
        let adapter = MemoryStoreAdapter::new(test_store());
        for index in 0..60 {
            adapter
                .upsert_json(serde_json::json!({
                    "scope": "long_term",
                    "type": "fact",
                    "content": format!("bounded item {index}")
                }))
                .unwrap();
        }

        let results = adapter
            .query_json(serde_json::json!({"top_k": -1}))
            .unwrap();
        assert_eq!(results.as_array().unwrap().len(), 20);
    }
}
