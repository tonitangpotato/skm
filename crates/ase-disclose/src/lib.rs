//! # ase-disclose
//!
//! Progressive disclosure and context management for Agent Skills.
//!
//! This crate provides:
//! - `DisclosureLevel` for tracking skill loading state (Catalog/Activated/Referenced)
//! - `TokenEstimator` for fast token count estimation (CJK-aware)
//! - `ContextManager` for token budget management and skill activation

mod error;
mod levels;
mod tokens;
mod context;

pub use error::DiscloseError;
pub use levels::{DisclosureLevel, LoadedSkill};
pub use tokens::TokenEstimator;
pub use context::{ActivationPayload, ContextManager, TokenBudget};
