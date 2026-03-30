//! NAPI-RS Node.js bindings for SKM (Agent Skill Engine).
//!
//! This crate provides JavaScript/TypeScript bindings for:
//! - `JsSkillMetadata` / `JsSkillRegistry` — skill registration and lookup
//! - `JsSelectionResult` / `JsCascadeSelector` — skill selection
//! - `JsBgeM3Provider` / `JsMiniLmProvider` — local embedding providers

#![deny(clippy::all)]

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use napi::bindgen_prelude::*;
use napi_derive::napi;

use skm_core::{SkillMetadata, SkillName, SkillRegistry};
use skm_embed::EmbeddingProvider;
use skm_select::{
    CascadeConfig, CascadeSelectorBuilder, Confidence, SelectionContext,
    SelectionResult, TriggerStrategy,
};

// ============================================================================
// Global Tokio Runtime
// ============================================================================

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn get_runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime")
    })
}

// ============================================================================
// Error Handling
// ============================================================================

/// Convert any error to napi::Error.
fn to_napi_error<E: std::fmt::Display>(e: E) -> napi::Error {
    napi::Error::from_reason(e.to_string())
}

// ============================================================================
// Core Types: JsSkillMetadata
// ============================================================================

/// Skill metadata (lightweight view without full instructions).
#[napi]
pub struct JsSkillMetadata {
    inner: SkillMetadata,
}

#[napi]
impl JsSkillMetadata {
    /// Skill name (1-64 chars, lowercase).
    #[napi(getter)]
    pub fn name(&self) -> String {
        self.inner.name.to_string()
    }

    /// Skill description.
    #[napi(getter)]
    pub fn description(&self) -> String {
        self.inner.description.clone()
    }

    /// Trigger patterns for fast matching.
    #[napi(getter)]
    pub fn triggers(&self) -> Vec<String> {
        self.inner.triggers.clone()
    }

    /// Tags/categories.
    #[napi(getter)]
    pub fn tags(&self) -> Vec<String> {
        self.inner.tags.clone()
    }

    /// Filesystem path to SKILL.md.
    #[napi(getter)]
    pub fn source_path(&self) -> String {
        self.inner.source_path.to_string_lossy().to_string()
    }

    /// Content hash for cache invalidation.
    #[napi(getter)]
    pub fn content_hash(&self) -> String {
        format!("{:016x}", self.inner.content_hash)
    }

    /// Estimated token count.
    #[napi(getter)]
    pub fn estimated_tokens(&self) -> u32 {
        self.inner.estimated_tokens as u32
    }
}

impl From<SkillMetadata> for JsSkillMetadata {
    fn from(inner: SkillMetadata) -> Self {
        Self { inner }
    }
}

// ============================================================================
// Core Types: JsSkillRegistry
// ============================================================================

/// In-memory skill registry with lazy loading.
#[napi]
pub struct JsSkillRegistry {
    inner: Arc<tokio::sync::RwLock<SkillRegistry>>,
}

#[napi]
impl JsSkillRegistry {
    /// Create a new registry scanning the given directories.
    ///
    /// @param paths - Array of directory paths to scan for SKILL.md files
    #[napi(factory)]
    pub async fn new(paths: Vec<String>) -> Result<Self> {
        let path_bufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        let registry = get_runtime()
            .spawn(async move {
                SkillRegistry::new(&path_bufs).await
            })
            .await
            .map_err(to_napi_error)?
            .map_err(to_napi_error)?;

        Ok(Self {
            inner: Arc::new(tokio::sync::RwLock::new(registry)),
        })
    }

    /// Get metadata for a skill by name.
    ///
    /// @param name - Skill name to look up
    /// @returns Skill metadata or null if not found
    #[napi]
    pub async fn get(&self, name: String) -> Result<Option<JsSkillMetadata>> {
        let inner = self.inner.clone();
        get_runtime()
            .spawn(async move {
                let skill_name = SkillName::new(&name).map_err(to_napi_error)?;
                let registry = inner.read().await;
                Ok(registry.get_metadata(&skill_name).await.map(JsSkillMetadata::from))
            })
            .await
            .map_err(to_napi_error)?
    }

    /// List all registered skills.
    ///
    /// @returns Array of skill metadata
    #[napi]
    pub async fn list(&self) -> Result<Vec<JsSkillMetadata>> {
        let inner = self.inner.clone();
        get_runtime()
            .spawn(async move {
                let registry = inner.read().await;
                Ok(registry
                    .catalog()
                    .await
                    .into_iter()
                    .map(JsSkillMetadata::from)
                    .collect())
            })
            .await
            .map_err(to_napi_error)?
    }

    /// Number of registered skills.
    #[napi]
    pub async fn len(&self) -> Result<u32> {
        let inner = self.inner.clone();
        get_runtime()
            .spawn(async move {
                let registry = inner.read().await;
                Ok(registry.len().await as u32)
            })
            .await
            .map_err(to_napi_error)?
    }

    /// Check if registry is empty.
    #[napi(js_name = "isEmpty")]
    pub async fn is_empty(&self) -> Result<bool> {
        let inner = self.inner.clone();
        get_runtime()
            .spawn(async move {
                let registry = inner.read().await;
                Ok(registry.is_empty().await)
            })
            .await
            .map_err(to_napi_error)?
    }

    /// Refresh skills from disk.
    #[napi]
    pub async fn refresh(&self) -> Result<JsRefreshReport> {
        let inner = self.inner.clone();
        get_runtime()
            .spawn(async move {
                let mut registry = inner.write().await;
                let report = registry.refresh().await.map_err(to_napi_error)?;
                Ok(JsRefreshReport::from(report))
            })
            .await
            .map_err(to_napi_error)?
    }
}

/// Report from a registry refresh operation.
#[napi(object)]
pub struct JsRefreshReport {
    /// Skills that were added.
    pub added: Vec<String>,
    /// Skills that were updated.
    pub updated: Vec<String>,
    /// Skills that were removed.
    pub removed: Vec<String>,
    /// Errors encountered (path, message).
    pub errors: Vec<JsRefreshError>,
}

#[napi(object)]
pub struct JsRefreshError {
    pub path: String,
    pub message: String,
}

impl From<skm_core::RefreshReport> for JsRefreshReport {
    fn from(report: skm_core::RefreshReport) -> Self {
        Self {
            added: report.added.into_iter().map(|n| n.to_string()).collect(),
            updated: report.updated.into_iter().map(|n| n.to_string()).collect(),
            removed: report.removed.into_iter().map(|n| n.to_string()).collect(),
            errors: report
                .errors
                .into_iter()
                .map(|(p, m)| JsRefreshError {
                    path: p.to_string_lossy().to_string(),
                    message: m,
                })
                .collect(),
        }
    }
}

// ============================================================================
// Selection: JsSelectionResult
// ============================================================================

/// Result of a skill selection.
#[napi(object)]
pub struct JsSelectionResult {
    /// Selected skill name.
    pub skill_name: String,
    /// Normalized score (0.0 - 1.0).
    pub score: f64,
    /// Confidence level: "none" | "low" | "medium" | "high" | "definite".
    pub confidence: String,
    /// Which strategy produced this result.
    pub strategy: String,
    /// Optional reasoning (for LLM strategies).
    pub reasoning: Option<String>,
}

impl From<SelectionResult> for JsSelectionResult {
    fn from(r: SelectionResult) -> Self {
        Self {
            skill_name: r.skill.to_string(),
            score: r.score as f64,
            confidence: match r.confidence {
                Confidence::None => "none",
                Confidence::Low => "low",
                Confidence::Medium => "medium",
                Confidence::High => "high",
                Confidence::Definite => "definite",
            }
            .to_string(),
            strategy: r.strategy,
            reasoning: r.reasoning,
        }
    }
}

// ============================================================================
// Selection: JsCascadeSelector
// ============================================================================

/// Configuration for the cascade selector.
#[napi(object)]
pub struct JsCascadeConfig {
    /// If true, always run all strategies and merge results.
    pub exhaustive: Option<bool>,
    /// Maximum timeout in milliseconds.
    pub timeout_ms: Option<u32>,
    /// Use trigger-only mode (no semantic/LLM fallback).
    pub trigger_only: Option<bool>,
}

/// Cascading skill selector with trigger-based fast path.
#[napi]
pub struct JsCascadeSelector {
    registry: Arc<tokio::sync::RwLock<SkillRegistry>>,
    trigger_only: bool,
}

#[napi]
impl JsCascadeSelector {
    /// Create a new cascade selector.
    ///
    /// @param skillsDir - Directory containing SKILL.md files
    /// @param config - Optional configuration
    #[napi(factory)]
    pub async fn new(skills_dir: String, config: Option<JsCascadeConfig>) -> Result<Self> {
        let path = PathBuf::from(skills_dir);
        let trigger_only = config.as_ref().and_then(|c| c.trigger_only).unwrap_or(false);

        let registry = get_runtime()
            .spawn(async move {
                SkillRegistry::new(&[path]).await
            })
            .await
            .map_err(to_napi_error)?
            .map_err(to_napi_error)?;

        Ok(Self {
            registry: Arc::new(tokio::sync::RwLock::new(registry)),
            trigger_only,
        })
    }

    /// Select the best skill(s) for a query.
    ///
    /// @param query - User query to match against skills
    /// @returns Array of selection results, sorted by score descending
    #[napi]
    pub async fn select(&self, query: String) -> Result<Vec<JsSelectionResult>> {
        let registry = self.registry.clone();
        let _trigger_only = self.trigger_only; // Reserved for future semantic/LLM modes

        get_runtime()
            .spawn(async move {
                let reg = registry.read().await;

                // Build trigger strategy from registry
                let trigger = TriggerStrategy::from_registry(&*reg)
                    .await
                    .map_err(to_napi_error)?;

                // Build cascade selector
                let selector = CascadeSelectorBuilder::new()
                    .with_triggers(trigger)
                    .config(CascadeConfig::default())
                    .build();

                let ctx = SelectionContext::new();
                let outcome = selector
                    .select(&query, &*reg, &ctx)
                    .await
                    .map_err(to_napi_error)?;

                Ok(outcome.selected.into_iter().map(JsSelectionResult::from).collect())
            })
            .await
            .map_err(to_napi_error)?
    }

    /// Select with custom context (conversation history, locale, etc.).
    ///
    /// @param query - User query
    /// @param context - Selection context
    #[napi]
    pub async fn select_with_context(
        &self,
        query: String,
        context: JsSelectionContext,
    ) -> Result<Vec<JsSelectionResult>> {
        let registry = self.registry.clone();

        get_runtime()
            .spawn(async move {
                let reg = registry.read().await;

                let trigger = TriggerStrategy::from_registry(&*reg)
                    .await
                    .map_err(to_napi_error)?;

                let selector = CascadeSelectorBuilder::new()
                    .with_triggers(trigger)
                    .config(CascadeConfig::default())
                    .build();

                let mut ctx = SelectionContext::new();
                if let Some(history) = context.conversation_history {
                    ctx = ctx.with_history(history);
                }
                if let Some(locale) = context.user_locale {
                    ctx = ctx.with_locale(locale);
                }

                let outcome = selector
                    .select(&query, &*reg, &ctx)
                    .await
                    .map_err(to_napi_error)?;

                Ok(outcome.selected.into_iter().map(JsSelectionResult::from).collect())
            })
            .await
            .map_err(to_napi_error)?
    }
}

/// Context for skill selection.
#[napi(object)]
pub struct JsSelectionContext {
    /// Recent conversation history.
    pub conversation_history: Option<Vec<String>>,
    /// User locale (e.g., "zh-CN", "en-US").
    pub user_locale: Option<String>,
}

// ============================================================================
// Embedding Providers
// ============================================================================

/// BGE-M3 embedding provider (1024 dimensions, multilingual).
#[napi]
#[cfg(feature = "embed-bge-m3")]
pub struct JsBgeM3Provider {
    inner: Arc<skm_embed::BgeM3Provider>,
}

#[napi]
#[cfg(feature = "embed-bge-m3")]
impl JsBgeM3Provider {
    /// Create a new BGE-M3 provider.
    /// Downloads the model on first use (~500MB).
    #[napi(factory)]
    pub fn new() -> Result<Self> {
        let provider = skm_embed::BgeM3Provider::new().map_err(to_napi_error)?;

        Ok(Self {
            inner: Arc::new(provider),
        })
    }

    /// Generate embedding for a single text.
    ///
    /// @param text - Text to embed
    /// @returns Float32 array of embeddings
    #[napi]
    pub async fn embed(&self, text: String) -> Result<Vec<f64>> {
        let provider = self.inner.clone();
        get_runtime()
            .spawn(async move {
                let embedding = provider.embed_one(&text).await.map_err(to_napi_error)?;
                Ok(embedding.vector.into_iter().map(|v| v as f64).collect())
            })
            .await
            .map_err(to_napi_error)?
    }

    /// Generate embeddings for multiple texts.
    ///
    /// @param texts - Array of texts to embed
    /// @returns Array of Float32 arrays
    #[napi(js_name = "embedBatch")]
    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f64>>> {
        let provider = self.inner.clone();
        get_runtime()
            .spawn(async move {
                let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
                let embeddings = provider.embed(&text_refs).await.map_err(to_napi_error)?;
                Ok(embeddings
                    .into_iter()
                    .map(|e| e.vector.into_iter().map(|v| v as f64).collect())
                    .collect())
            })
            .await
            .map_err(to_napi_error)?
    }

    /// Get the embedding dimensions.
    #[napi(getter)]
    pub fn dimensions(&self) -> u32 {
        self.inner.dimensions() as u32
    }

    /// Get the model identifier.
    #[napi(getter, js_name = "modelId")]
    pub fn model_id(&self) -> String {
        self.inner.model_id().to_string()
    }
}

/// MiniLM embedding provider (384 dimensions, English-optimized).
#[napi]
#[cfg(feature = "embed-minilm")]
pub struct JsMiniLmProvider {
    inner: Arc<skm_embed::MiniLmProvider>,
}

#[napi]
#[cfg(feature = "embed-minilm")]
impl JsMiniLmProvider {
    /// Create a new MiniLM provider.
    /// Downloads the model on first use (~80MB).
    #[napi(factory)]
    pub fn new() -> Result<Self> {
        let provider = skm_embed::MiniLmProvider::new().map_err(to_napi_error)?;

        Ok(Self {
            inner: Arc::new(provider),
        })
    }

    /// Generate embedding for a single text.
    ///
    /// @param text - Text to embed
    /// @returns Float32 array of embeddings
    #[napi]
    pub async fn embed(&self, text: String) -> Result<Vec<f64>> {
        let provider = self.inner.clone();
        get_runtime()
            .spawn(async move {
                let embedding = provider.embed_one(&text).await.map_err(to_napi_error)?;
                Ok(embedding.vector.into_iter().map(|v| v as f64).collect())
            })
            .await
            .map_err(to_napi_error)?
    }

    /// Generate embeddings for multiple texts.
    ///
    /// @param texts - Array of texts to embed
    /// @returns Array of Float32 arrays
    #[napi(js_name = "embedBatch")]
    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f64>>> {
        let provider = self.inner.clone();
        get_runtime()
            .spawn(async move {
                let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
                let embeddings = provider.embed(&text_refs).await.map_err(to_napi_error)?;
                Ok(embeddings
                    .into_iter()
                    .map(|e| e.vector.into_iter().map(|v| v as f64).collect())
                    .collect())
            })
            .await
            .map_err(to_napi_error)?
    }

    /// Get the embedding dimensions.
    #[napi(getter)]
    pub fn dimensions(&self) -> u32 {
        self.inner.dimensions() as u32
    }

    /// Get the model identifier.
    #[napi(getter, js_name = "modelId")]
    pub fn model_id(&self) -> String {
        self.inner.model_id().to_string()
    }
}
