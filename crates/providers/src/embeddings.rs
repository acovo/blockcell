use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use blockcell_core::{Config, Error, Result};
use blockcell_storage::vector::Embedder;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;

use crate::client::build_blocking_http_client;
use crate::factory::default_api_base;

pub struct OpenAICompatibleEmbedder {
    client: Client,
    api_key: String,
    api_base: String,
    model: String,
    dimensions: AtomicUsize,
}

impl OpenAICompatibleEmbedder {
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_proxy(
        api_key: &str,
        api_base: Option<&str>,
        model: &str,
        provider_proxy: Option<&str>,
        global_proxy: Option<&str>,
        no_proxy: &[String],
    ) -> Self {
        let resolved_base = api_base
            .unwrap_or("https://api.openai.com/v1")
            .trim_end_matches('/')
            .to_string();
        let client = build_blocking_http_client(
            provider_proxy,
            global_proxy,
            no_proxy,
            &resolved_base,
            Duration::from_secs(120),
        );

        Self {
            client,
            api_key: api_key.to_string(),
            api_base: resolved_base,
            model: model.to_string(),
            dimensions: AtomicUsize::new(0),
        }
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let response = self
            .client
            .post(format!("{}/embeddings", self.api_base))
            .bearer_auth(&self.api_key)
            .json(&json!({
                "model": self.model,
                "input": text,
            }))
            .send()
            .map_err(|error| Error::Provider(format!("Embedding request failed: {}", error)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(Error::Provider(format!(
                "Embedding request failed with status {}: {}",
                status, body
            )));
        }

        let payload: EmbeddingResponse = response
            .json()
            .map_err(|error| Error::Provider(format!("Embedding decode failed: {}", error)))?;
        let vector = payload
            .data
            .into_iter()
            .next()
            .map(|item| item.embedding)
            .ok_or_else(|| Error::Provider("Embedding response returned no vectors".to_string()))?;
        self.dimensions.store(vector.len(), Ordering::SeqCst);
        Ok(vector)
    }
}

impl Embedder for OpenAICompatibleEmbedder {
    fn model_id(&self) -> &str {
        &self.model
    }

    fn dimensions(&self) -> usize {
        self.dimensions.load(Ordering::SeqCst)
    }

    fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        self.embed(text)
    }

    fn embed_document(&self, text: &str) -> Result<Vec<f32>> {
        self.embed(text)
    }
}

pub fn create_embedder(config: &Config) -> anyhow::Result<Option<Arc<dyn Embedder>>> {
    let vector_cfg = &config.memory.vector;
    if !vector_cfg.enabled {
        return Ok(None);
    }

    let provider_name = vector_cfg.provider.trim();
    let model = vector_cfg.model.trim();
    if provider_name.is_empty() || model.is_empty() {
        return Err(anyhow!(
            "memory.vector is enabled but provider/model are not fully configured"
        ));
    }

    let provider_cfg = config
        .providers
        .get(provider_name)
        .with_context(|| format!("Provider '{}' not found for memory.vector", provider_name))?;

    if matches!(
        provider_cfg.api_type.as_str(),
        "anthropic" | "gemini" | "ollama"
    ) {
        return Err(anyhow!(
            "memory.vector provider '{}' is not OpenAI-compatible",
            provider_name
        ));
    }

    if provider_cfg.api_key.trim().is_empty() || provider_cfg.api_key == "dummy" {
        return Err(anyhow!(
            "Provider '{}' has no API key for memory.vector embeddings",
            provider_name
        ));
    }

    let api_base = provider_cfg
        .api_base
        .as_deref()
        .unwrap_or_else(|| default_api_base(provider_name));
    let embedder = OpenAICompatibleEmbedder::new_with_proxy(
        &provider_cfg.api_key,
        Some(api_base),
        model,
        provider_cfg.proxy.as_deref(),
        config.network.proxy.as_deref(),
        &config.network.no_proxy,
    );

    Ok(Some(Arc::new(embedder)))
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_embedder_returns_none_when_disabled() {
        let config = Config::default();
        assert!(create_embedder(&config).unwrap().is_none());
    }

    #[test]
    fn test_create_embedder_validates_provider_configuration() {
        let mut config = Config::default();
        config.memory.vector.enabled = true;
        config.memory.vector.provider = "openai".to_string();
        config.memory.vector.model = "text-embedding-3-small".to_string();
        config.providers.get_mut("openai").unwrap().api_key = "sk-test".to_string();

        let embedder = create_embedder(&config).unwrap();
        assert!(embedder.is_some());
        assert_eq!(embedder.unwrap().model_id(), "text-embedding-3-small");
    }
}
