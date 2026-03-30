//! # ase-embed
//!
//! Embedding abstraction layer for Agent Skills.
//!
//! This crate provides:
//! - `EmbeddingProvider` trait for pluggable backends
//! - `Embedding` struct with similarity operations
//! - `SkillEmbeddings` for multi-component weighted embeddings
//! - `EmbeddingIndex` for persistent, queryable skill embeddings
//! - Providers: BGE-M3, MiniLM (local), OpenAI/Cohere/Compatible (API)

mod error;
mod embedding;
mod provider;
mod multicomp;
mod index;
mod simd;

#[cfg(feature = "embed-bge-m3")]
mod bge;

#[cfg(feature = "embed-minilm")]
mod minilm;

#[cfg(any(feature = "embed-openai", feature = "embed-cohere", feature = "embed-compat"))]
mod api;

pub use error::EmbedError;
pub use embedding::Embedding;
pub use provider::EmbeddingProvider;
pub use multicomp::{ComponentWeights, SkillEmbeddings};
pub use index::{EmbeddingIndex, ScoredSkill};
pub use multicomp::ComponentScores;

#[cfg(feature = "embed-bge-m3")]
pub use bge::BgeM3Provider;

#[cfg(feature = "embed-minilm")]
pub use minilm::MiniLmProvider;

#[cfg(feature = "embed-openai")]
pub use api::OpenAiEmbedProvider;

#[cfg(feature = "embed-cohere")]
pub use api::CohereEmbedProvider;

#[cfg(feature = "embed-compat")]
pub use api::OpenAiCompatibleProvider;
