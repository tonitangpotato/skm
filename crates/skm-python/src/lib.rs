//! Python bindings for SKM Agent Skill Engine.
//!
//! This module provides PyO3 bindings for:
//! - Core types: SkillMetadata, SkillRegistry
//! - Selection: CascadeSelector, TriggerStrategy, SelectionResult
//! - Embedding: BgeM3Provider, MiniLmProvider
//! - Enforcement: EnforcementPipeline

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use tokio::runtime::Runtime;

// Import SelectionStrategy trait for .select() method
use skm_select::SelectionStrategy;

// ============================================================================
// Core Types
// ============================================================================

/// Metadata for a skill (lightweight view).
#[pyclass(name = "SkillMetadata")]
#[derive(Clone)]
pub struct PySkillMetadata {
    name: String,
    description: String,
    triggers: Vec<String>,
    tags: Vec<String>,
    source_path: String,
    content_hash: u64,
    estimated_tokens: usize,
}

#[pymethods]
impl PySkillMetadata {
    /// Skill name.
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }

    /// Skill description.
    #[getter]
    fn description(&self) -> &str {
        &self.description
    }

    /// Trigger patterns for fast matching.
    #[getter]
    fn triggers(&self) -> Vec<String> {
        self.triggers.clone()
    }

    /// Tags/categories.
    #[getter]
    fn tags(&self) -> Vec<String> {
        self.tags.clone()
    }

    /// Path to SKILL.md file.
    #[getter]
    fn source_path(&self) -> &str {
        &self.source_path
    }

    /// Content hash for cache invalidation.
    #[getter]
    fn content_hash(&self) -> u64 {
        self.content_hash
    }

    /// Estimated token count.
    #[getter]
    fn estimated_tokens(&self) -> usize {
        self.estimated_tokens
    }

    fn __repr__(&self) -> String {
        format!(
            "SkillMetadata(name={:?}, triggers={:?})",
            self.name, self.triggers
        )
    }
}

impl From<skm_core::SkillMetadata> for PySkillMetadata {
    fn from(meta: skm_core::SkillMetadata) -> Self {
        Self {
            name: meta.name.to_string(),
            description: meta.description,
            triggers: meta.triggers,
            tags: meta.tags,
            source_path: meta.source_path.to_string_lossy().to_string(),
            content_hash: meta.content_hash,
            estimated_tokens: meta.estimated_tokens,
        }
    }
}

/// In-memory skill registry with lazy loading.
#[pyclass(name = "SkillRegistry")]
pub struct PySkillRegistry {
    inner: Arc<tokio::sync::RwLock<skm_core::SkillRegistry>>,
    runtime: Arc<Runtime>,
}

#[pymethods]
impl PySkillRegistry {
    /// Create a new registry scanning the given directories.
    ///
    /// Args:
    ///     paths: List of directory paths to scan for SKILL.md files.
    #[new]
    fn new(paths: Vec<String>) -> PyResult<Self> {
        let runtime = Runtime::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        let path_bufs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();

        let registry = runtime
            .block_on(async { skm_core::SkillRegistry::new(&path_bufs).await })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(Self {
            inner: Arc::new(tokio::sync::RwLock::new(registry)),
            runtime: Arc::new(runtime),
        })
    }

    /// Get metadata for a skill by name.
    fn get(&self, name: &str) -> PyResult<Option<PySkillMetadata>> {
        let skill_name = skm_core::SkillName::new(name)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        let result = self.runtime.block_on(async {
            let registry = self.inner.read().await;
            registry.get_metadata(&skill_name).await
        });

        Ok(result.map(PySkillMetadata::from))
    }

    /// List all skill names.
    fn list(&self) -> Vec<String> {
        self.runtime.block_on(async {
            let registry = self.inner.read().await;
            registry.names().await.into_iter().map(|n| n.to_string()).collect()
        })
    }

    /// Get the full catalog of skill metadata.
    fn catalog(&self) -> Vec<PySkillMetadata> {
        self.runtime.block_on(async {
            let registry = self.inner.read().await;
            registry
                .catalog()
                .await
                .into_iter()
                .map(PySkillMetadata::from)
                .collect()
        })
    }

    /// Number of registered skills.
    fn __len__(&self) -> usize {
        self.runtime.block_on(async {
            let registry = self.inner.read().await;
            registry.len().await
        })
    }

    /// Refresh skills from disk.
    fn refresh(&self) -> PyResult<HashMap<String, Vec<String>>> {
        let report = self.runtime.block_on(async {
            let mut registry = self.inner.write().await;
            registry.refresh().await
        }).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        let mut result = HashMap::new();
        result.insert("added".to_string(), report.added.iter().map(|n| n.to_string()).collect());
        result.insert("updated".to_string(), report.updated.iter().map(|n| n.to_string()).collect());
        result.insert("removed".to_string(), report.removed.iter().map(|n| n.to_string()).collect());
        Ok(result)
    }

    fn __repr__(&self) -> String {
        let len = self.__len__();
        format!("SkillRegistry(skills={})", len)
    }
}

// ============================================================================
// Selection Types
// ============================================================================

/// Result of a skill selection operation.
#[pyclass(name = "SelectionResult")]
#[derive(Clone)]
pub struct PySelectionResult {
    /// Name of the selected skill.
    #[pyo3(get)]
    skill: String,

    /// Normalized score (0.0 - 1.0).
    #[pyo3(get)]
    score: f32,

    /// Confidence level as string (None, Low, Medium, High, Definite).
    #[pyo3(get)]
    confidence: String,

    /// Which strategy produced this result.
    #[pyo3(get)]
    strategy: String,

    /// Optional reasoning.
    #[pyo3(get)]
    reasoning: Option<String>,
}

#[pymethods]
impl PySelectionResult {
    fn __repr__(&self) -> String {
        format!(
            "SelectionResult(skill={:?}, score={:.3}, confidence={:?}, strategy={:?})",
            self.skill, self.score, self.confidence, self.strategy
        )
    }
}

impl From<skm_select::SelectionResult> for PySelectionResult {
    fn from(result: skm_select::SelectionResult) -> Self {
        let confidence = match result.confidence {
            skm_select::Confidence::None => "None",
            skm_select::Confidence::Low => "Low",
            skm_select::Confidence::Medium => "Medium",
            skm_select::Confidence::High => "High",
            skm_select::Confidence::Definite => "Definite",
        };

        Self {
            skill: result.skill.to_string(),
            score: result.score,
            confidence: confidence.to_string(),
            strategy: result.strategy,
            reasoning: result.reasoning,
        }
    }
}

/// Fast trigger-based skill selection (µs latency).
#[pyclass(name = "TriggerStrategy")]
pub struct PyTriggerStrategy {
    inner: skm_select::TriggerStrategy,
}

#[pymethods]
impl PyTriggerStrategy {
    /// Create a TriggerStrategy from a SkillRegistry.
    #[staticmethod]
    fn from_registry(registry: &PySkillRegistry) -> PyResult<Self> {
        let strategy = registry.runtime.block_on(async {
            let reg = registry.inner.read().await;
            skm_select::TriggerStrategy::from_registry(&reg).await
        }).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(Self { inner: strategy })
    }

    /// Select skills matching the query using triggers.
    fn select(&self, query: &str, registry: &PySkillRegistry) -> PyResult<Vec<PySelectionResult>> {
        let results: Result<Vec<skm_select::SelectionResult>, skm_select::SelectError> = 
            registry.runtime.block_on(async {
                let reg = registry.inner.read().await;
                let catalog = reg.catalog().await;
                let candidates: Vec<_> = catalog.iter().collect();
                let ctx = skm_select::SelectionContext::new();
                self.inner.select(query, &candidates, &ctx).await
            });
        
        let results = results.map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(results.into_iter().map(PySelectionResult::from).collect())
    }

    fn __repr__(&self) -> String {
        "TriggerStrategy()".to_string()
    }
}

/// Cascading skill selector with multiple strategies.
#[pyclass(name = "CascadeSelector")]
pub struct PyCascadeSelector {
    inner: skm_select::CascadeSelector,
    runtime: Arc<Runtime>,
}

#[pymethods]
impl PyCascadeSelector {
    /// Create a CascadeSelector with trigger strategy.
    ///
    /// Args:
    ///     registry: SkillRegistry to build triggers from.
    #[new]
    fn new(registry: &PySkillRegistry) -> PyResult<Self> {
        let runtime = registry.runtime.clone();

        let selector = runtime.block_on(async {
            let reg = registry.inner.read().await;
            let trigger = skm_select::TriggerStrategy::from_registry(&reg)
                .await
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

            let selector = skm_select::CascadeSelector::builder()
                .with_triggers(trigger)
                .build();

            Ok::<_, PyErr>(selector)
        })?;

        Ok(Self {
            inner: selector,
            runtime,
        })
    }

    /// Select the best skills for a query.
    ///
    /// Returns a list of SelectionResult ordered by score.
    fn select(&self, query: &str, registry: &PySkillRegistry) -> PyResult<Vec<PySelectionResult>> {
        let results = self.runtime.block_on(async {
            let reg = registry.inner.read().await;
            let ctx = skm_select::SelectionContext::new();
            self.inner.select(query, &reg, &ctx).await
        }).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(results.selected.into_iter().map(PySelectionResult::from).collect())
    }

    /// Get selection outcome with full audit trail.
    fn select_with_stats(
        &self,
        query: &str,
        registry: &PySkillRegistry,
    ) -> PyResult<HashMap<String, PyObject>> {
        Python::with_gil(|py| {
            let outcome = self.runtime.block_on(async {
                let reg = registry.inner.read().await;
                let ctx = skm_select::SelectionContext::new();
                self.inner.select(query, &reg, &ctx).await
            }).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

            let mut result = HashMap::new();

            // Selected results
            let selected: Vec<PySelectionResult> = outcome
                .selected
                .into_iter()
                .map(PySelectionResult::from)
                .collect();
            result.insert("selected".to_string(), selected.into_pyobject(py)?.into_any().unbind());

            // Strategies used
            result.insert(
                "strategies_used".to_string(),
                outcome.strategies_used.into_pyobject(py)?.into_any().unbind(),
            );

            // Latency (milliseconds)
            result.insert(
                "total_latency_ms".to_string(),
                (outcome.total_latency.as_millis() as u64).into_pyobject(py)?.into_any().unbind(),
            );

            // Fallback used
            result.insert(
                "fallback_used".to_string(),
                outcome.fallback_used.into_pyobject(py)?.to_owned().into_any().unbind(),
            );

            Ok(result)
        })
    }

    fn __repr__(&self) -> String {
        "CascadeSelector()".to_string()
    }
}

// ============================================================================
// Embedding Types
// ============================================================================

/// BGE-M3 embedding provider (1024-dim, multilingual).
#[cfg(feature = "embed-bge-m3")]
#[pyclass(name = "BgeM3Provider")]
pub struct PyBgeM3Provider {
    inner: Arc<skm_embed::BgeM3Provider>,
    runtime: Arc<Runtime>,
}

#[cfg(feature = "embed-bge-m3")]
#[pymethods]
impl PyBgeM3Provider {
    /// Create a new BGE-M3 provider.
    ///
    /// Args:
    ///     cache_size: Number of embeddings to cache (default: 1000).
    #[new]
    #[pyo3(signature = (cache_size=1000))]
    fn new(cache_size: usize) -> PyResult<Self> {
        let runtime = Runtime::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        let provider = skm_embed::BgeM3Provider::with_cache_size(cache_size)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(Self {
            inner: Arc::new(provider),
            runtime: Arc::new(runtime),
        })
    }

    /// Embed a single text.
    ///
    /// Returns a list of floats (embedding vector).
    fn embed(&self, text: &str) -> PyResult<Vec<f32>> {
        use skm_embed::EmbeddingProvider;

        let embedding = self
            .runtime
            .block_on(async { self.inner.embed_one(text).await })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(embedding.vector.clone())
    }

    /// Embed multiple texts.
    ///
    /// Returns a list of embedding vectors.
    fn embed_batch(&self, texts: Vec<String>) -> PyResult<Vec<Vec<f32>>> {
        use skm_embed::EmbeddingProvider;

        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let embeddings = self
            .runtime
            .block_on(async { self.inner.embed(&text_refs).await })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(embeddings.into_iter().map(|e| e.vector).collect())
    }

    /// Vector dimensions (1024 for BGE-M3).
    #[getter]
    fn dimensions(&self) -> usize {
        use skm_embed::EmbeddingProvider;
        self.inner.dimensions()
    }

    /// Model identifier.
    #[getter]
    fn model_id(&self) -> &str {
        use skm_embed::EmbeddingProvider;
        self.inner.model_id()
    }

    fn __repr__(&self) -> String {
        format!("BgeM3Provider(dimensions={})", self.dimensions())
    }
}

/// MiniLM embedding provider (384-dim, English only, fast).
#[cfg(feature = "embed-minilm")]
#[pyclass(name = "MiniLmProvider")]
pub struct PyMiniLmProvider {
    inner: Arc<skm_embed::MiniLmProvider>,
    runtime: Arc<Runtime>,
}

#[cfg(feature = "embed-minilm")]
#[pymethods]
impl PyMiniLmProvider {
    /// Create a new MiniLM provider.
    ///
    /// Args:
    ///     cache_size: Number of embeddings to cache (default: 1000).
    #[new]
    #[pyo3(signature = (cache_size=1000))]
    fn new(cache_size: usize) -> PyResult<Self> {
        let runtime = Runtime::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        let provider = skm_embed::MiniLmProvider::with_cache_size(cache_size)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(Self {
            inner: Arc::new(provider),
            runtime: Arc::new(runtime),
        })
    }

    /// Embed a single text.
    fn embed(&self, text: &str) -> PyResult<Vec<f32>> {
        use skm_embed::EmbeddingProvider;

        let embedding = self
            .runtime
            .block_on(async { self.inner.embed_one(text).await })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(embedding.vector.clone())
    }

    /// Embed multiple texts.
    fn embed_batch(&self, texts: Vec<String>) -> PyResult<Vec<Vec<f32>>> {
        use skm_embed::EmbeddingProvider;

        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let embeddings = self
            .runtime
            .block_on(async { self.inner.embed(&text_refs).await })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(embeddings.into_iter().map(|e| e.vector).collect())
    }

    /// Vector dimensions (384 for MiniLM).
    #[getter]
    fn dimensions(&self) -> usize {
        use skm_embed::EmbeddingProvider;
        self.inner.dimensions()
    }

    /// Model identifier.
    #[getter]
    fn model_id(&self) -> &str {
        use skm_embed::EmbeddingProvider;
        self.inner.model_id()
    }

    fn __repr__(&self) -> String {
        format!("MiniLmProvider(dimensions={})", self.dimensions())
    }
}

// ============================================================================
// Enforcement Types
// ============================================================================

/// Hook decision for enforcement pipeline.
#[pyclass(name = "HookDecision")]
#[derive(Clone)]
pub struct PyHookDecision {
    #[pyo3(get)]
    decision_type: String,
    #[pyo3(get)]
    reason: Option<String>,
    #[pyo3(get)]
    modified_output: Option<String>,
}

#[pymethods]
impl PyHookDecision {
    /// Check if the decision allows the action.
    fn is_allowed(&self) -> bool {
        self.decision_type == "allow"
    }

    /// Check if the decision cancels the action.
    fn is_cancelled(&self) -> bool {
        self.decision_type == "cancel"
    }

    fn __repr__(&self) -> String {
        format!("HookDecision(type={:?})", self.decision_type)
    }
}

impl From<skm_enforce::HookDecision> for PyHookDecision {
    fn from(decision: skm_enforce::HookDecision) -> Self {
        match decision {
            skm_enforce::HookDecision::Allow => Self {
                decision_type: "allow".to_string(),
                reason: None,
                modified_output: None,
            },
            skm_enforce::HookDecision::Cancel { reason, .. } => Self {
                decision_type: "cancel".to_string(),
                reason: Some(reason),
                modified_output: None,
            },
            skm_enforce::HookDecision::Modify(output) => Self {
                decision_type: "modify".to_string(),
                reason: None,
                modified_output: Some(output),
            },
            skm_enforce::HookDecision::RequireApproval { reason, .. } => Self {
                decision_type: "require_approval".to_string(),
                reason: Some(reason),
                modified_output: None,
            },
        }
    }
}

/// Enforcement pipeline for pre/post skill execution checks.
#[pyclass(name = "EnforcementPipeline")]
pub struct PyEnforcementPipeline {
    inner: skm_enforce::EnforcementPipeline,
}

#[pymethods]
impl PyEnforcementPipeline {
    /// Create a new enforcement pipeline (allow-all by default).
    #[new]
    fn new() -> Self {
        let pipeline = skm_enforce::EnforcementPipeline::builder().build();
        Self { inner: pipeline }
    }

    /// Run pre-activation checks.
    ///
    /// Args:
    ///     skill_name: Name of the skill to check.
    ///     query: The user query.
    ///     user_id: Optional user identifier.
    ///     session_id: Optional session identifier.
    #[pyo3(signature = (skill_name, query, user_id=None, session_id=None))]
    fn check_before(
        &self,
        skill_name: &str,
        query: &str,
        user_id: Option<String>,
        session_id: Option<String>,
    ) -> PyResult<PyHookDecision> {
        let skill = skm_core::SkillName::new(skill_name)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        let mut ctx = skm_enforce::EnforcementContext::new();
        if let Some(uid) = user_id {
            ctx = ctx.with_user(uid);
        }
        if let Some(sid) = session_id {
            ctx = ctx.with_session(sid);
        }

        let decision = self
            .inner
            .check_before(&skill, query, &ctx)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(PyHookDecision::from(decision))
    }

    /// Run post-execution checks.
    ///
    /// Args:
    ///     skill_name: Name of the skill that was executed.
    ///     output: The skill's output to check.
    ///     user_id: Optional user identifier.
    ///     session_id: Optional session identifier.
    #[pyo3(signature = (skill_name, output, user_id=None, session_id=None))]
    fn check_after(
        &self,
        skill_name: &str,
        output: &str,
        user_id: Option<String>,
        session_id: Option<String>,
    ) -> PyResult<PyHookDecision> {
        let skill = skm_core::SkillName::new(skill_name)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        let mut ctx = skm_enforce::EnforcementContext::new();
        if let Some(uid) = user_id {
            ctx = ctx.with_user(uid);
        }
        if let Some(sid) = session_id {
            ctx = ctx.with_session(sid);
        }

        let decision = self
            .inner
            .check_after(&skill, output, &ctx)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(PyHookDecision::from(decision))
    }

    fn __repr__(&self) -> String {
        "EnforcementPipeline()".to_string()
    }
}

// ============================================================================
// Module Definition
// ============================================================================

/// SKM - Agent Skill Engine for Python
///
/// Provides fast skill selection, embedding, and enforcement.
#[pymodule]
fn _skm_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Core types
    m.add_class::<PySkillMetadata>()?;
    m.add_class::<PySkillRegistry>()?;

    // Selection
    m.add_class::<PySelectionResult>()?;
    m.add_class::<PyTriggerStrategy>()?;
    m.add_class::<PyCascadeSelector>()?;

    // Embedding
    #[cfg(feature = "embed-bge-m3")]
    m.add_class::<PyBgeM3Provider>()?;

    #[cfg(feature = "embed-minilm")]
    m.add_class::<PyMiniLmProvider>()?;

    // Enforcement
    m.add_class::<PyHookDecision>()?;
    m.add_class::<PyEnforcementPipeline>()?;

    Ok(())
}
