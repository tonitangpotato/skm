//! API-based embedding providers (OpenAI, Cohere, compatible endpoints).

use async_trait::async_trait;
use lru::LruCache;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;
use std::sync::Mutex;

use crate::embedding::Embedding;
use crate::error::EmbedError;
use crate::provider::EmbeddingProvider;

/// OpenAI embedding provider.
#[cfg(feature = "embed-openai")]
pub struct OpenAiEmbedProvider {
    client: Client,
    api_key: String,
    model: String,
    dimensions: usize,
    cache: Mutex<LruCache<u64, Vec<f32>>>,
}

#[cfg(feature = "embed-openai")]
impl OpenAiEmbedProvider {
    /// Create a new OpenAI embedding provider.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let model = model.into();
        let dimensions = match model.as_str() {
            "text-embedding-3-small" => 1536,
            "text-embedding-3-large" => 3072,
            "text-embedding-ada-002" => 1536,
            _ => 1536, // Default
        };

        Self {
            client: Client::new(),
            api_key: api_key.into(),
            model,
            dimensions,
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())),
        }
    }

    /// Create with custom dimensions (for text-embedding-3-* models).
    pub fn with_dimensions(mut self, dimensions: usize) -> Self {
        self.dimensions = dimensions;
        self
    }
}

#[cfg(feature = "embed-openai")]
#[async_trait]
impl EmbeddingProvider for OpenAiEmbedProvider {
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>, EmbedError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        for text in texts {
            if text.is_empty() {
                return Err(EmbedError::EmptyInput);
            }
        }

        let hashes: Vec<u64> = texts
            .iter()
            .map(|t| xxhash_rust::xxh64::xxh64(t.as_bytes(), 0))
            .collect();

        // Check cache
        let mut results = vec![None; texts.len()];
        let mut to_embed: Vec<(usize, &str)> = Vec::new();

        for (i, (text, hash)) in texts.iter().zip(hashes.iter()).enumerate() {
            if let Some(cached) = self.cache.lock().ok().and_then(|mut c| c.get(hash).cloned()) {
                results[i] = Some(Embedding::from_normalized(cached, *hash));
            } else {
                to_embed.push((i, *text));
            }
        }

        if to_embed.is_empty() {
            return Ok(results.into_iter().map(|r| r.unwrap()).collect());
        }

        // Make API request
        #[derive(Serialize)]
        struct Request<'a> {
            model: &'a str,
            input: Vec<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            dimensions: Option<usize>,
        }

        #[derive(Deserialize)]
        struct Response {
            data: Vec<EmbeddingData>,
        }

        #[derive(Deserialize)]
        struct EmbeddingData {
            embedding: Vec<f32>,
        }

        let request = Request {
            model: &self.model,
            input: to_embed.iter().map(|(_, t)| *t).collect(),
            dimensions: if self.model.starts_with("text-embedding-3") {
                Some(self.dimensions)
            } else {
                None
            },
        };

        let response = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| EmbedError::Api {
                status: 0,
                message: e.to_string(),
            })?;

        let status = response.status().as_u16();
        if status == 401 {
            return Err(EmbedError::InvalidApiKey);
        }
        if status == 429 {
            return Err(EmbedError::RateLimit { retry_after_secs: 60 });
        }
        if !response.status().is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(EmbedError::Api { status, message });
        }

        let resp: Response = response.json().await.map_err(|e| EmbedError::Api {
            status: 0,
            message: e.to_string(),
        })?;

        for ((idx, _), data) in to_embed.iter().zip(resp.data.into_iter()) {
            let hash = hashes[*idx];
            if let Ok(mut cache) = self.cache.lock() {
                cache.put(hash, data.embedding.clone());
            }
            results[*idx] = Some(Embedding::new(data.embedding, hash));
        }

        Ok(results.into_iter().map(|r| r.unwrap()).collect())
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_id(&self) -> &str {
        &self.model
    }

    fn max_batch_size(&self) -> usize {
        2048 // OpenAI supports large batches
    }
}

/// Cohere embedding provider.
#[cfg(feature = "embed-cohere")]
pub struct CohereEmbedProvider {
    client: Client,
    api_key: String,
    model: String,
    dimensions: usize,
    cache: Mutex<LruCache<u64, Vec<f32>>>,
}

#[cfg(feature = "embed-cohere")]
impl CohereEmbedProvider {
    /// Create a new Cohere embedding provider.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let model = model.into();
        let dimensions = match model.as_str() {
            "embed-english-v3.0" => 1024,
            "embed-multilingual-v3.0" => 1024,
            "embed-english-light-v3.0" => 384,
            "embed-multilingual-light-v3.0" => 384,
            _ => 1024,
        };

        Self {
            client: Client::new(),
            api_key: api_key.into(),
            model,
            dimensions,
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())),
        }
    }
}

#[cfg(feature = "embed-cohere")]
#[async_trait]
impl EmbeddingProvider for CohereEmbedProvider {
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>, EmbedError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        for text in texts {
            if text.is_empty() {
                return Err(EmbedError::EmptyInput);
            }
        }

        let hashes: Vec<u64> = texts
            .iter()
            .map(|t| xxhash_rust::xxh64::xxh64(t.as_bytes(), 0))
            .collect();

        #[derive(Serialize)]
        struct Request<'a> {
            model: &'a str,
            texts: Vec<&'a str>,
            input_type: &'a str,
        }

        #[derive(Deserialize)]
        struct Response {
            embeddings: Vec<Vec<f32>>,
        }

        let request = Request {
            model: &self.model,
            texts: texts.to_vec(),
            input_type: "search_query",
        };

        let response = self
            .client
            .post("https://api.cohere.ai/v1/embed")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| EmbedError::Api {
                status: 0,
                message: e.to_string(),
            })?;

        let status = response.status().as_u16();
        if status == 401 {
            return Err(EmbedError::InvalidApiKey);
        }
        if status == 429 {
            return Err(EmbedError::RateLimit { retry_after_secs: 60 });
        }
        if !response.status().is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(EmbedError::Api { status, message });
        }

        let resp: Response = response.json().await.map_err(|e| EmbedError::Api {
            status: 0,
            message: e.to_string(),
        })?;

        let results: Vec<Embedding> = resp
            .embeddings
            .into_iter()
            .zip(hashes.into_iter())
            .map(|(vec, hash)| Embedding::new(vec, hash))
            .collect();

        Ok(results)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_id(&self) -> &str {
        &self.model
    }

    fn max_batch_size(&self) -> usize {
        96 // Cohere limit
    }
}

/// OpenAI-compatible API provider (Ollama, vLLM, LiteLLM, etc.)
#[cfg(feature = "embed-compat")]
pub struct OpenAiCompatibleProvider {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
    dimensions: usize,
    cache: Mutex<LruCache<u64, Vec<f32>>>,
}

#[cfg(feature = "embed-compat")]
impl OpenAiCompatibleProvider {
    /// Create a new OpenAI-compatible provider.
    pub fn new(
        base_url: impl Into<String>,
        api_key: Option<String>,
        model: impl Into<String>,
        dimensions: usize,
    ) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            api_key,
            model: model.into(),
            dimensions,
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())),
        }
    }
}

#[cfg(feature = "embed-compat")]
#[async_trait]
impl EmbeddingProvider for OpenAiCompatibleProvider {
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>, EmbedError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        for text in texts {
            if text.is_empty() {
                return Err(EmbedError::EmptyInput);
            }
        }

        let hashes: Vec<u64> = texts
            .iter()
            .map(|t| xxhash_rust::xxh64::xxh64(t.as_bytes(), 0))
            .collect();

        #[derive(Serialize)]
        struct Request<'a> {
            model: &'a str,
            input: Vec<&'a str>,
        }

        #[derive(Deserialize)]
        struct Response {
            data: Vec<EmbeddingData>,
        }

        #[derive(Deserialize)]
        struct EmbeddingData {
            embedding: Vec<f32>,
        }

        let request = Request {
            model: &self.model,
            input: texts.to_vec(),
        };

        let url = format!("{}/v1/embeddings", self.base_url.trim_end_matches('/'));

        let mut req = self.client.post(&url).json(&request);

        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.send().await.map_err(|e| EmbedError::Api {
            status: 0,
            message: e.to_string(),
        })?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(EmbedError::Api { status, message });
        }

        let resp: Response = response.json().await.map_err(|e| EmbedError::Api {
            status: 0,
            message: e.to_string(),
        })?;

        let results: Vec<Embedding> = resp
            .data
            .into_iter()
            .zip(hashes.into_iter())
            .map(|(data, hash)| Embedding::new(data.embedding, hash))
            .collect();

        Ok(results)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_id(&self) -> &str {
        &self.model
    }

    fn max_batch_size(&self) -> usize {
        128
    }
}

#[cfg(test)]
mod tests {
    // API tests require credentials, skipped by default

    #[test]
    fn test_api_module_compiles() {
        // Just check that the module compiles
        assert!(true);
    }
}
