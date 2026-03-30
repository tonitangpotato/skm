//! # skm (Agent Skill Engine)
//!
//! The missing runtime layer for Agent Skills: selection, enforcement, and optimization
//! as a framework-agnostic Rust crate.
//!
//! ## Quick Start
//!
//! ```text
//! use skm::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 1. Build registry
//!     let registry = SkillRegistry::new(&["./skills"]).await?;
//!
//!     // 2. Build embedding provider (BGE-M3 by default)
//!     let embedder = BgeM3Provider::new()?;
//!
//!     // 3. Build selection engine
//!     let selector = CascadeSelector::builder()
//!         .with_triggers()
//!         .with_semantic(Arc::new(embedder), SemanticConfig::default())
//!         .build(&registry)?;
//!
//!     // 4. Select a skill
//!     let outcome = selector.select(
//!         "Extract tables from this PDF",
//!         &registry,
//!         &SelectionContext::default(),
//!     ).await?;
//!
//!     println!("Selected: {:?}", outcome.selected);
//!     Ok(())
//! }
//! ```
//!
//! ## Crate Structure
//!
//! - `skm-core` - Skill schema, parser, registry
//! - `skm-embed` - Embedding abstraction + providers
//! - `skm-select` - Multi-strategy selection engine
//! - `skm-disclose` - Progressive disclosure / context management
//! - `skm-enforce` - Execution guardrails & hooks
//! - `skm-learn` - Evaluation, metrics, optimization

pub use skm_core;
pub use skm_disclose;
pub use skm_embed;
pub use skm_enforce;
pub use skm_learn;
pub use skm_select;

/// Prelude module with commonly used types.
pub mod prelude {
    // Core types
    pub use skm_core::{
        CoreError, ParseError, Skill, SkillMetadata, SkillName, SkillParser, SkillRegistry,
        SkillStats, ValidationError,
    };

    // Embedding types
    pub use skm_embed::{
        ComponentWeights, EmbedError, Embedding, EmbeddingIndex, EmbeddingProvider,
        ScoredSkill,
    };

    #[cfg(feature = "embed-bge-m3")]
    pub use skm_embed::BgeM3Provider;

    #[cfg(feature = "embed-minilm")]
    pub use skm_embed::MiniLmProvider;

    // Selection types
    pub use skm_select::{
        CascadeConfig, CascadeSelector, CascadeSelectorBuilder, Confidence, LlmClient,
        MergeStrategy, SelectError, SelectionContext, SelectionOutcome, SelectionResult,
        SelectionStrategy, SemanticConfig, TriggerStrategy,
    };

    // Disclosure types
    pub use skm_disclose::{
        ActivationPayload, ContextManager, DisclosureLevel, DiscloseError, TokenBudget,
        TokenEstimator,
    };

    // Enforcement types
    pub use skm_enforce::{
        AfterSkillExecution, BeforeSkillActivation, EnforceError, EnforcementContext,
        EnforcementPipeline, HookDecision, Policy, PolicyEngine,
    };

    // Learning types
    pub use skm_learn::{
        DescriptionOptimizer, LearnError, SelectionMetrics, TestCase, TestSuite,
        TriggerTestHarness, UsageAnalytics,
    };

    pub use std::sync::Arc;
}
