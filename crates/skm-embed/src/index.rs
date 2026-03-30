//! Persistent embedding index for fast skill lookup.

use std::path::Path;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use skm_core::{SkillName, SkillRegistry};

use crate::embedding::Embedding;
use crate::error::EmbedError;
use crate::multicomp::{ComponentScores, ComponentWeights, SkillEmbeddings};
use crate::provider::EmbeddingProvider;

/// A skill with its similarity score.
#[derive(Debug, Clone)]
pub struct ScoredSkill {
    /// The skill name.
    pub name: SkillName,

    /// Overall weighted similarity score.
    pub score: f32,

    /// Per-component score breakdown.
    pub component_scores: ComponentScores,
}

/// Persistent embedding index.
///
/// Serialized to disk via bincode for fast startup.
/// Rebuilds only when skill content changes (tracked via content_hash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingIndex {
    /// Embeddings for all indexed skills.
    entries: Vec<SkillEmbeddings>,

    /// Model identifier (invalidate if model changes).
    model_id: String,

    /// When this index was built.
    built_at: SystemTime,
}

impl EmbeddingIndex {
    /// Build index from registry using the given embedding provider.
    ///
    /// Embeds all skills in parallel where possible.
    pub async fn build(
        registry: &SkillRegistry,
        provider: &dyn EmbeddingProvider,
        weights: ComponentWeights,
    ) -> Result<Self, EmbedError> {
        let catalog = registry.catalog().await;
        let mut entries = Vec::with_capacity(catalog.len());

        // Collect all texts to embed
        let mut texts: Vec<String> = Vec::new();
        let mut skill_indices: Vec<(usize, SkillName)> = Vec::new();

        for meta in &catalog {
            let base_idx = texts.len();
            skill_indices.push((base_idx, meta.name.clone()));

            // Description
            texts.push(meta.description.clone());

            // Triggers (concatenated)
            let triggers_text = if meta.triggers.is_empty() {
                meta.description.clone() // Fallback to description
            } else {
                meta.triggers.join(", ")
            };
            texts.push(triggers_text);

            // Tags (concatenated)
            let tags_text = if meta.tags.is_empty() {
                meta.description.clone() // Fallback to description
            } else {
                meta.tags.join(", ")
            };
            texts.push(tags_text);

            // Examples (we don't have them in metadata, use description as placeholder)
            texts.push(meta.description.clone());
        }

        // Batch embed all texts
        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();

        // Split into batches if needed
        let max_batch = provider.max_batch_size();
        let mut all_embeddings = Vec::with_capacity(texts.len());

        for chunk in text_refs.chunks(max_batch) {
            let batch_embeddings = provider.embed(chunk).await?;
            all_embeddings.extend(batch_embeddings);
        }

        // Reconstruct skill embeddings from flat list
        for (base_idx, skill_name) in skill_indices {
            let skill_embedding = SkillEmbeddings::new(
                skill_name,
                all_embeddings[base_idx].clone(),     // description
                all_embeddings[base_idx + 1].clone(), // triggers
                all_embeddings[base_idx + 2].clone(), // tags
                all_embeddings[base_idx + 3].clone(), // examples
                weights.clone(),
            );
            entries.push(skill_embedding);
        }

        Ok(Self {
            entries,
            model_id: provider.model_id().to_string(),
            built_at: SystemTime::now(),
        })
    }

    /// Load from disk cache. Returns None if cache is stale or doesn't exist.
    pub fn load_cached(path: &Path, registry: &SkillRegistry) -> Result<Option<Self>, EmbedError> {
        if !path.exists() {
            return Ok(None);
        }

        let data = std::fs::read(path).map_err(EmbedError::Io)?;
        let index: Self = bincode::deserialize(&data)
            .map_err(|e| EmbedError::Serialization(e.to_string()))?;

        // TODO: Validate cache against registry content hashes
        // For now, just return the loaded index
        let _ = registry; // Unused for now

        Ok(Some(index))
    }

    /// Save to disk.
    pub fn save(&self, path: &Path) -> Result<(), EmbedError> {
        let data = bincode::serialize(self)
            .map_err(|e| EmbedError::Serialization(e.to_string()))?;

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, data)?;
        Ok(())
    }

    /// Query: return top-k skills by similarity.
    pub fn query(&self, query_embedding: &Embedding, top_k: usize) -> Vec<ScoredSkill> {
        let mut scored: Vec<ScoredSkill> = self
            .entries
            .iter()
            .map(|e| ScoredSkill {
                name: e.skill_name.clone(),
                score: e.score(query_embedding),
                component_scores: e.component_scores(query_embedding),
            })
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Take top-k
        scored.truncate(top_k);
        scored
    }

    /// Query with adaptive k: return skills above threshold,
    /// with automatic gap detection.
    pub fn query_adaptive(
        &self,
        query_embedding: &Embedding,
        min_score: f32,
        max_k: usize,
        gap_threshold: f32,
    ) -> Vec<ScoredSkill> {
        let mut scored: Vec<ScoredSkill> = self
            .entries
            .iter()
            .map(|e| ScoredSkill {
                name: e.skill_name.clone(),
                score: e.score(query_embedding),
                component_scores: e.component_scores(query_embedding),
            })
            .filter(|s| s.score >= min_score)
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Find cutoff based on gap detection
        let mut cutoff = scored.len().min(max_k);

        for i in 1..cutoff {
            let gap = scored[i - 1].score - scored[i].score;
            if gap >= gap_threshold {
                cutoff = i;
                break;
            }
        }

        scored.truncate(cutoff);
        scored
    }

    /// Get the model ID this index was built with.
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Get when this index was built.
    pub fn built_at(&self) -> SystemTime {
        self.built_at
    }

    /// Number of skills in the index.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get all skill names in the index.
    pub fn skill_names(&self) -> Vec<&SkillName> {
        self.entries.iter().map(|e| &e.skill_name).collect()
    }

    /// Get embeddings for a specific skill.
    pub fn get(&self, name: &SkillName) -> Option<&SkillEmbeddings> {
        self.entries.iter().find(|e| &e.skill_name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::tests::MockProvider;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;

    async fn setup_test_registry() -> (TempDir, SkillRegistry) {
        let temp = TempDir::new().unwrap();

        // Create test skills
        let skill1 = r#"---
name: pdf-processing
description: Extract text and tables from PDF files
metadata:
  triggers: "pdf, extract text"
  tags: "document, extraction"
---

Instructions for PDF processing.
"#;

        let skill2 = r#"---
name: weather-lookup
description: Get current weather and forecasts
metadata:
  triggers: "weather, forecast, temperature"
  tags: "weather, api"
---

Instructions for weather lookup.
"#;

        // Write skills
        let skill1_dir = temp.path().join("pdf-processing");
        std::fs::create_dir_all(&skill1_dir).unwrap();
        std::fs::write(skill1_dir.join("SKILL.md"), skill1).unwrap();

        let skill2_dir = temp.path().join("weather-lookup");
        std::fs::create_dir_all(&skill2_dir).unwrap();
        std::fs::write(skill2_dir.join("SKILL.md"), skill2).unwrap();

        let registry = SkillRegistry::new(&[temp.path()]).await.unwrap();
        (temp, registry)
    }

    #[tokio::test]
    async fn test_index_build() {
        let (_temp, registry) = setup_test_registry().await;
        let provider = MockProvider::new(384);

        let index = EmbeddingIndex::build(&registry, &provider, ComponentWeights::default())
            .await
            .unwrap();

        assert_eq!(index.len(), 2);
        assert_eq!(index.model_id(), "mock-embedding-model");
    }

    #[tokio::test]
    async fn test_index_query() {
        let (_temp, registry) = setup_test_registry().await;
        let provider = MockProvider::new(384);

        let index = EmbeddingIndex::build(&registry, &provider, ComponentWeights::default())
            .await
            .unwrap();

        let query = provider.embed_one("extract text from pdf").await.unwrap();
        let results = index.query(&query, 5);

        assert!(!results.is_empty());
        assert!(results[0].score >= -1.0 && results[0].score <= 1.0);
    }

    #[tokio::test]
    async fn test_index_query_adaptive() {
        let (_temp, registry) = setup_test_registry().await;
        let provider = MockProvider::new(384);

        let index = EmbeddingIndex::build(&registry, &provider, ComponentWeights::default())
            .await
            .unwrap();

        let query = provider.embed_one("pdf").await.unwrap();
        let results = index.query_adaptive(&query, 0.0, 10, 0.1);

        // Should return some results
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_index_save_load() {
        let (_temp, registry) = setup_test_registry().await;
        let provider = MockProvider::new(384);

        let index = EmbeddingIndex::build(&registry, &provider, ComponentWeights::default())
            .await
            .unwrap();

        let cache_dir = TempDir::new().unwrap();
        let cache_path = cache_dir.path().join("index.bin");

        // Save
        index.save(&cache_path).unwrap();
        assert!(cache_path.exists());

        // Load
        let loaded = EmbeddingIndex::load_cached(&cache_path, &registry)
            .unwrap()
            .unwrap();

        assert_eq!(loaded.len(), index.len());
        assert_eq!(loaded.model_id(), index.model_id());
    }

    #[tokio::test]
    async fn test_index_get() {
        let (_temp, registry) = setup_test_registry().await;
        let provider = MockProvider::new(384);

        let index = EmbeddingIndex::build(&registry, &provider, ComponentWeights::default())
            .await
            .unwrap();

        let name = SkillName::new("pdf-processing").unwrap();
        let embeddings = index.get(&name).unwrap();

        assert_eq!(embeddings.skill_name, name);
    }
}
