//! # ase (Agent Skill Engine)
//!
//! The missing runtime layer for Agent Skills: selection, enforcement, and optimization
//! as a framework-agnostic Rust crate.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ase::prelude::*;
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
//! - `ase-core` - Skill schema, parser, registry
//! - `ase-embed` - Embedding abstraction + providers
//! - `ase-select` - Multi-strategy selection engine
//! - `ase-disclose` - Progressive disclosure / context management
//! - `ase-enforce` - Execution guardrails & hooks
//! - `ase-learn` - Evaluation, metrics, optimization

pub use ase_core;
pub use ase_disclose;
pub use ase_embed;
pub use ase_enforce;
pub use ase_learn;
pub use ase_select;

/// Prelude module with commonly used types.
pub mod prelude {
    // Core types
    pub use ase_core::{
        CoreError, ParseError, Skill, SkillMetadata, SkillName, SkillParser, SkillRegistry,
        SkillStats, ValidationError,
    };

    // Embedding types
    pub use ase_embed::{
        ComponentWeights, EmbedError, Embedding, EmbeddingIndex, EmbeddingProvider,
        ScoredSkill,
    };

    #[cfg(feature = "embed-bge-m3")]
    pub use ase_embed::BgeM3Provider;

    #[cfg(feature = "embed-minilm")]
    pub use ase_embed::MiniLmProvider;

    // Selection types
    pub use ase_select::{
        CascadeConfig, CascadeSelector, CascadeSelectorBuilder, Confidence, LlmClient,
        MergeStrategy, SelectError, SelectionContext, SelectionOutcome, SelectionResult,
        SelectionStrategy, SemanticConfig, TriggerStrategy,
    };

    // Disclosure types
    pub use ase_disclose::{
        ActivationPayload, ContextManager, DisclosureLevel, DiscloseError, TokenBudget,
        TokenEstimator,
    };

    // Enforcement types
    pub use ase_enforce::{
        AfterSkillExecution, BeforeSkillActivation, EnforceError, EnforcementContext,
        EnforcementPipeline, HookDecision, Policy, PolicyEngine,
    };

    // Learning types
    pub use ase_learn::{
        DescriptionOptimizer, LearnError, SelectionMetrics, TestCase, TestSuite,
        TriggerTestHarness, UsageAnalytics,
    };

    pub use std::sync::Arc;
}
