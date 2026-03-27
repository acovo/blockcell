use blockcell_core::Result;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct VectorMeta {
    pub scope: String,
    pub item_type: String,
    pub tags: Vec<String>,
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
    fn search(&self, vector: &[f32], top_k: usize) -> Result<Vec<VectorHit>>;
    fn health(&self) -> Result<()>;
    fn stats(&self) -> Result<Value>;
    fn reset(&self) -> Result<()>;
}

#[derive(Clone)]
pub struct VectorRuntime {
    pub embedder: Arc<dyn Embedder>,
    pub index: Arc<dyn VectorIndex>,
}
