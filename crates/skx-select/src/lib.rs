//! # skx-select
//!
//! Multi-strategy cascading skill selection engine.
//!
//! This crate provides:
//! - `SelectionStrategy` trait for pluggable selection algorithms
//! - `TriggerStrategy` for fast regex/keyword matching (µs)
//! - `SemanticStrategy` for embedding-based similarity (ms)
//! - `LlmStrategy` for LLM classification fallback (s)
//! - `FewShotEnhanced` wrapper for dynamic few-shot injection
//! - `CascadeSelector` for composing strategies with early-exit

mod error;
mod strategy;
mod trigger;
mod semantic;
mod llm;
mod fewshot;
mod cascade;

pub use error::{LlmError, SelectError};
pub use strategy::{
    Confidence, LatencyClass, SelectionContext, SelectionResult, SelectionStrategy,
};
pub use trigger::TriggerStrategy;
pub use semantic::{SemanticConfig, SemanticStrategy};
pub use llm::{LlmClient, LlmStrategy, LlmStrategyConfig};
pub use fewshot::{FewShotEnhanced, FewShotExample};
pub use cascade::{
    CascadeConfig, CascadeSelector, CascadeSelectorBuilder, MergeStrategy, SelectionOutcome,
};
