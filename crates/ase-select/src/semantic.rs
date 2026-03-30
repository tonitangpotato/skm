//! Semantic embedding-based selection strategy (ms latency).

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use ase_core::SkillMetadata;
use ase_embed::{ComponentWeights, EmbeddingIndex, EmbeddingProvider};

use crate::error::SelectError;
use crate::strategy::{
    Confidence, LatencyClass, SelectionContext, SelectionResult, SelectionStrategy,
};

/// Configuration for semantic selection.
#[derive(Debug, Clone)]
pub struct SemanticConfig {
    /// Maximum candidates to return.
    pub top_k: usize,

    /// Minimum similarity threshold.
    pub min_score: f32,

    /// Score gap for adaptive cutoff.
    pub gap_threshold: f32,

    /// Component weights for multi-component embeddings.
    pub component_weights: ComponentWeights,

    /// Enable gap-based adaptive selection.
    pub use_adaptive_k: bool,
}

impl Default for SemanticConfig {
    fn default() -> Self {
        Self {
            top_k: 5,
            min_score: 0.3,
            gap_threshold: 0.15,
            component_weights: ComponentWeights::default(),
            use_adaptive_k: true,
        }
    }
}

impl SemanticConfig {
    /// Create with custom top_k.
    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k = top_k;
        self
    }

    /// Create with custom minimum score.
    pub fn with_min_score(mut self, min_score: f32) -> Self {
        self.min_score = min_score;
        self
    }

    /// Create with custom gap threshold.
    pub fn with_gap_threshold(mut self, gap: f32) -> Self {
        self.gap_threshold = gap;
        self
    }

    /// Disable adaptive k (always return top_k).
    pub fn without_adaptive_k(mut self) -> Self {
        self.use_adaptive_k = false;
        self
    }
}

/// Embedding-based semantic similarity search.
pub struct SemanticStrategy {
    provider: Arc<dyn EmbeddingProvider>,
    index: Arc<RwLock<EmbeddingIndex>>,
    config: SemanticConfig,
}

impl SemanticStrategy {
    /// Create a new semantic strategy.
    pub fn new(
        provider: Arc<dyn EmbeddingProvider>,
        index: EmbeddingIndex,
        config: SemanticConfig,
    ) -> Self {
        Self {
            provider,
            index: Arc::new(RwLock::new(index)),
            config,
        }
    }

    /// Update the index.
    pub async fn update_index(&self, index: EmbeddingIndex) {
        let mut guard = self.index.write().await;
        *guard = index;
    }

    /// Get the current index.
    pub async fn index(&self) -> EmbeddingIndex {
        self.index.read().await.clone()
    }
}

#[async_trait]
impl SelectionStrategy for SemanticStrategy {
    async fn select(
        &self,
        query: &str,
        candidates: &[&SkillMetadata],
        _ctx: &SelectionContext,
    ) -> Result<Vec<SelectionResult>, SelectError> {
        // Embed the query
        let query_embedding = self.provider.embed_one(query).await?;

        // Get index
        let index = self.index.read().await;

        // Query the index
        let scored = if self.config.use_adaptive_k {
            index.query_adaptive(
                &query_embedding,
                self.config.min_score,
                self.config.top_k,
                self.config.gap_threshold,
            )
        } else {
            index.query(&query_embedding, self.config.top_k)
        };

        // Filter to only candidates
        let candidate_names: std::collections::HashSet<_> =
            candidates.iter().map(|c| &c.name).collect();

        let results: Vec<SelectionResult> = scored
            .into_iter()
            .filter(|s| candidate_names.contains(&s.name))
            .filter(|s| s.score >= self.config.min_score)
            .map(|s| {
                let confidence = Confidence::from_score(s.score);
                SelectionResult::new(s.name, s.score, confidence, "semantic").with_reasoning(
                    format!(
                        "desc={:.2}, trig={:.2}, tags={:.2}, ex={:.2}",
                        s.component_scores.description,
                        s.component_scores.triggers,
                        s.component_scores.tags,
                        s.component_scores.examples
                    ),
                )
            })
            .collect();

        Ok(results)
    }

    fn name(&self) -> &str {
        "semantic"
    }

    fn latency_class(&self) -> LatencyClass {
        LatencyClass::Milliseconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_config() {
        let config = SemanticConfig::default()
            .with_top_k(3)
            .with_min_score(0.5)
            .with_gap_threshold(0.2)
            .without_adaptive_k();

        assert_eq!(config.top_k, 3);
        assert_eq!(config.min_score, 0.5);
        assert_eq!(config.gap_threshold, 0.2);
        assert!(!config.use_adaptive_k);
    }

    #[test]
    fn test_semantic_config_defaults() {
        let config = SemanticConfig::default();
        assert_eq!(config.top_k, 5);
        assert_eq!(config.min_score, 0.3);
        assert!(config.use_adaptive_k);
    }
}
