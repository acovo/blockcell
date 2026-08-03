use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use blockcell_core::Paths;
use serde::Serialize;

#[derive(Debug, Default)]
pub struct GhostMetrics {
    episodes_captured: AtomicU64,
    reviews_started: AtomicU64,
    reviews_failed: AtomicU64,
    dead_letters: AtomicU64,
    prompt_input_tokens: AtomicU64,
    cache_read_input_tokens: AtomicU64,
    cache_creation_input_tokens: AtomicU64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhostMetricsSnapshot {
    pub episodes_captured: u64,
    pub reviews_started: u64,
    pub reviews_failed: u64,
    pub dead_letters: u64,
    pub prompt_input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
}

impl GhostMetrics {
    pub fn record_episode_captured(&self) {
        self.episodes_captured.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_review_started(&self) {
        self.reviews_started.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_review_failed(&self) {
        self.reviews_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_dead_letter(&self) {
        self.dead_letters.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_prompt_cache_usage(
        &self,
        prompt_input_tokens: u64,
        cache_read_input_tokens: u64,
        cache_creation_input_tokens: u64,
    ) {
        self.prompt_input_tokens
            .fetch_add(prompt_input_tokens, Ordering::Relaxed);
        self.cache_read_input_tokens
            .fetch_add(cache_read_input_tokens, Ordering::Relaxed);
        self.cache_creation_input_tokens
            .fetch_add(cache_creation_input_tokens, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> GhostMetricsSnapshot {
        GhostMetricsSnapshot {
            episodes_captured: self.episodes_captured.load(Ordering::Relaxed),
            reviews_started: self.reviews_started.load(Ordering::Relaxed),
            reviews_failed: self.reviews_failed.load(Ordering::Relaxed),
            dead_letters: self.dead_letters.load(Ordering::Relaxed),
            prompt_input_tokens: self.prompt_input_tokens.load(Ordering::Relaxed),
            cache_read_input_tokens: self.cache_read_input_tokens.load(Ordering::Relaxed),
            cache_creation_input_tokens: self.cache_creation_input_tokens.load(Ordering::Relaxed),
        }
    }

    /// Reset all counters. Uses SeqCst ordering to establish a synchronization
    /// point, ensuring concurrent snapshot() calls see either fully-reset or
    /// fully-not-reset state (not partial reset).
    pub fn reset(&self) {
        self.episodes_captured.store(0, Ordering::SeqCst);
        self.reviews_started.store(0, Ordering::SeqCst);
        self.reviews_failed.store(0, Ordering::SeqCst);
        self.dead_letters.store(0, Ordering::SeqCst);
        self.prompt_input_tokens.store(0, Ordering::SeqCst);
        self.cache_read_input_tokens.store(0, Ordering::SeqCst);
        self.cache_creation_input_tokens.store(0, Ordering::SeqCst);
    }
}

fn metrics_registry() -> &'static Mutex<HashMap<String, Arc<GhostMetrics>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, Arc<GhostMetrics>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn registry_key(paths: &Paths) -> String {
    paths.base.display().to_string()
}

pub fn get_ghost_metrics(paths: &Paths) -> Arc<GhostMetrics> {
    let key = registry_key(paths);
    let mut registry = metrics_registry()
        .lock()
        .expect("ghost metrics registry lock poisoned");
    registry
        .entry(key)
        .or_insert_with(|| Arc::new(GhostMetrics::default()))
        .clone()
}

pub fn ghost_metrics_summary(paths: &Paths) -> GhostMetricsSnapshot {
    get_ghost_metrics(paths).snapshot()
}

pub fn reset_ghost_metrics_for_paths(paths: &Paths) {
    get_ghost_metrics(paths).reset();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_cache_usage_is_exposed_in_snapshot() {
        let metrics = GhostMetrics::default();
        metrics.record_prompt_cache_usage(200, 140, 20);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.prompt_input_tokens, 200);
        assert_eq!(snapshot.cache_read_input_tokens, 140);
        assert_eq!(snapshot.cache_creation_input_tokens, 20);
    }
}
