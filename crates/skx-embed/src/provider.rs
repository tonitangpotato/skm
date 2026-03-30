//! EmbeddingProvider trait definition.

use async_trait::async_trait;

use crate::embedding::Embedding;
use crate::error::EmbedError;

/// Core embedding provider trait.
///
/// All implementations must be Send + Sync for use across async boundaries.
/// `embed` is async to support both local inference (via spawn_blocking)
/// and API-based providers (network IO).
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embeddings for a batch of texts.
    ///
    /// Returns one embedding per input text, in the same order.
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>, EmbedError>;

    /// Vector dimensionality of this provider.
    fn dimensions(&self) -> usize;

    /// Human-readable model identifier.
    fn model_id(&self) -> &str;

    /// Maximum batch size supported.
    fn max_batch_size(&self) -> usize {
        128 // Default, can be overridden
    }

    /// Embed a single text (convenience method).
    async fn embed_one(&self, text: &str) -> Result<Embedding, EmbedError> {
        if text.is_empty() {
            return Err(EmbedError::EmptyInput);
        }
        let mut embeddings = self.embed(&[text]).await?;
        embeddings.pop().ok_or_else(|| EmbedError::Embedding("No embedding returned".into()))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Mock embedding provider for testing.
    pub struct MockProvider {
        pub dimensions: usize,
        pub call_count: AtomicUsize,
    }

    impl MockProvider {
        pub fn new(dimensions: usize) -> Self {
            Self {
                dimensions,
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl EmbeddingProvider for MockProvider {
        async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>, EmbedError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);

            let mut results = Vec::with_capacity(texts.len());
            for text in texts {
                if text.is_empty() {
                    return Err(EmbedError::EmptyInput);
                }

                // Create a deterministic "embedding" based on text hash
                let hash = xxhash_rust::xxh64::xxh64(text.as_bytes(), 0);
                let mut vector = vec![0.0f32; self.dimensions];

                // Fill with deterministic values
                for (i, v) in vector.iter_mut().enumerate() {
                    *v = ((hash.wrapping_add(i as u64)) % 1000) as f32 / 1000.0;
                }

                results.push(Embedding::new(vector, hash));
            }

            Ok(results)
        }

        fn dimensions(&self) -> usize {
            self.dimensions
        }

        fn model_id(&self) -> &str {
            "mock-embedding-model"
        }
    }

    #[tokio::test]
    async fn test_mock_provider_embed() {
        let provider = MockProvider::new(384);
        let texts = vec!["hello", "world"];
        let embeddings = provider.embed(&texts).await.unwrap();

        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].dimensions(), 384);
        assert!(embeddings[0].is_normalized());
    }

    #[tokio::test]
    async fn test_mock_provider_embed_one() {
        let provider = MockProvider::new(384);
        let embedding = provider.embed_one("test").await.unwrap();

        assert_eq!(embedding.dimensions(), 384);
    }

    #[tokio::test]
    async fn test_mock_provider_empty_input() {
        let provider = MockProvider::new(384);
        let result = provider.embed_one("").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EmbedError::EmptyInput));
    }

    #[tokio::test]
    async fn test_mock_provider_deterministic() {
        let provider = MockProvider::new(384);

        let e1 = provider.embed_one("test").await.unwrap();
        let e2 = provider.embed_one("test").await.unwrap();

        // Same input should produce same embedding
        assert_eq!(e1.text_hash, e2.text_hash);
        assert_eq!(e1.vector, e2.vector);
    }
}
