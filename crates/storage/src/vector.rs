use blockcell_core::Result;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct VectorMeta {
    pub scope: String,
    pub item_type: String,
    pub tags: Vec<String>,
    pub session_key: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VectorFilter {
    pub session_key: Option<String>,
    pub scope: Option<String>,
    pub item_type: Option<String>,
    pub tags: Option<Vec<String>>,
}

impl VectorFilter {
    pub fn matches(&self, meta: &VectorMeta) -> bool {
        if let Some(session_key) = &self.session_key {
            if meta
                .session_key
                .as_ref()
                .is_some_and(|owner| owner != session_key)
            {
                return false;
            }
        }
        if self
            .scope
            .as_ref()
            .is_some_and(|scope| meta.scope != *scope)
        {
            return false;
        }
        if self
            .item_type
            .as_ref()
            .is_some_and(|item_type| meta.item_type != *item_type)
        {
            return false;
        }
        if let Some(tags) = &self.tags {
            if !tags.is_empty()
                && !meta
                    .tags
                    .iter()
                    .any(|tag| tags.iter().any(|item| item == tag))
            {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorHit {
    pub id: String,
    pub score: f64,
}

pub trait Embedder: Send + Sync {
    fn model_id(&self) -> &str;
    fn dimensions(&self) -> usize;
    fn embed_query(&self, text: &str) -> Result<Vec<f32>>;
    fn embed_document(&self, text: &str) -> Result<Vec<f32>>;
}

pub trait VectorIndex: Send + Sync {
    fn upsert(&self, id: &str, vector: &[f32], meta: &VectorMeta) -> Result<()>;
    fn delete_ids(&self, ids: &[String]) -> Result<()>;
    fn search(
        &self,
        vector: &[f32],
        top_k: usize,
        filter: Option<&VectorFilter>,
    ) -> Result<Vec<VectorHit>>;
    fn health(&self) -> Result<()>;
    fn stats(&self) -> Result<Value>;
    fn reset(&self) -> Result<()>;
}

#[derive(Clone)]
pub struct VectorRuntime {
    pub embedder: Arc<dyn Embedder>,
    pub index: Arc<dyn VectorIndex>,
}
