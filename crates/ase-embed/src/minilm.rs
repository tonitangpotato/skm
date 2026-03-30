//! MiniLM embedding provider via fastembed.
//!
//! all-MiniLM-L6-v2: 22M params, English only, 384-dim.
//! ~22MB, <5ms/query. For English-only lightweight deployments.

use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use lru::LruCache;
use tokio::task::spawn_blocking;

use crate::embedding::Embedding;
use crate::error::EmbedError;
use crate::provider::EmbeddingProvider;

/// MiniLM embedding provider.
///
/// Uses fastembed for ONNX inference.
/// 384-dimensional embeddings, English only.
/// Very fast and lightweight.
pub struct MiniLmProvider {
    /// The fastembed model (Arc for thread safety).
    model: Arc<Mutex<TextEmbedding>>,

    /// LRU cache for embeddings.
    cache: Mutex<LruCache<u64, Vec<f32>>>,
}

impl MiniLmProvider {
    /// Create a new MiniLM provider with default settings.
    pub fn new() -> Result<Self, EmbedError> {
        Self::with_cache_size(1000)
    }

    /// Create a new MiniLM provider with custom cache size.
    pub fn with_cache_size(cache_size: usize) -> Result<Self, EmbedError> {
        let model = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::AllMiniLML6V2))
            .map_err(|e| EmbedError::ModelInit(e.to_string()))?;

        let cache = Mutex::new(LruCache::new(
            NonZeroUsize::new(cache_size).unwrap_or(NonZeroUsize::new(1).unwrap()),
        ));

        Ok(Self { 
            model: Arc::new(Mutex::new(model)), 
            cache,
        })
    }

    /// Check cache for an embedding.
    fn get_cached(&self, text_hash: u64) -> Option<Vec<f32>> {
        self.cache.lock().ok()?.get(&text_hash).cloned()
    }

    /// Store embedding in cache.
    fn set_cached(&self, text_hash: u64, vector: Vec<f32>) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.put(text_hash, vector);
        }
    }
}

#[async_trait]
impl EmbeddingProvider for MiniLmProvider {
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>, EmbedError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Check for empty texts
        for text in texts {
            if text.is_empty() {
                return Err(EmbedError::EmptyInput);
            }
        }

        // Compute hashes and check cache
        let hashes: Vec<u64> = texts
            .iter()
            .map(|t| xxhash_rust::xxh64::xxh64(t.as_bytes(), 0))
            .collect();

        let mut results = vec![None; texts.len()];
        let mut to_embed: Vec<(usize, String)> = Vec::new();

        for (i, (text, hash)) in texts.iter().zip(hashes.iter()).enumerate() {
            if let Some(cached) = self.get_cached(*hash) {
                results[i] = Some(Embedding::from_normalized(cached, *hash));
            } else {
                to_embed.push((i, text.to_string()));
            }
        }

        // Embed uncached texts
        if !to_embed.is_empty() {
            let texts_to_embed: Vec<String> = to_embed.iter().map(|(_, t)| t.clone()).collect();

            // Run embedding in blocking thread
            let model = Arc::clone(&self.model);
            let embeddings = spawn_blocking(move || {
                let guard = model.lock().map_err(|e| EmbedError::Embedding(e.to_string()))?;
                guard.embed(texts_to_embed, None)
                    .map_err(|e| EmbedError::Embedding(e.to_string()))
            })
            .await
            .map_err(|e| EmbedError::Embedding(e.to_string()))??;

            for ((idx, _), vector) in to_embed.iter().zip(embeddings.into_iter()) {
                let hash = hashes[*idx];
                self.set_cached(hash, vector.clone());
                results[*idx] = Some(Embedding::new(vector, hash));
            }
        }

        Ok(results.into_iter().map(|r| r.unwrap()).collect())
    }

    fn dimensions(&self) -> usize {
        384
    }

    fn model_id(&self) -> &str {
        "all-MiniLM-L6-v2"
    }

    fn max_batch_size(&self) -> usize {
        64 // MiniLM is smaller, can handle larger batches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires model download"]
    async fn test_minilm_embed() {
        let provider = MiniLmProvider::new().unwrap();
        let embedding = provider.embed_one("Hello, world!").await.unwrap();

        assert_eq!(embedding.dimensions(), 384);
        assert!(embedding.is_normalized());
    }

    #[tokio::test]
    #[ignore = "requires model download"]
    async fn test_minilm_batch() {
        let provider = MiniLmProvider::new().unwrap();
        let texts = vec!["Hello", "World", "Test"];
        let embeddings = provider.embed(&texts).await.unwrap();

        assert_eq!(embeddings.len(), 3);
    }

    #[tokio::test]
    #[ignore = "requires model download"]
    async fn test_minilm_cache() {
        let provider = MiniLmProvider::new().unwrap();

        let e1 = provider.embed_one("test text").await.unwrap();
        let e2 = provider.embed_one("test text").await.unwrap();

        assert_eq!(e1.text_hash, e2.text_hash);
    }
}
