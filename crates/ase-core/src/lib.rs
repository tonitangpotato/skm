//! # ase-core
//!
//! Core types, parser, and registry for Agent Skills (SKILL.md).
//!
//! This crate provides:
//! - `Skill` and `SkillMetadata` types compatible with agentskills.io
//! - `SkillName` validated identifier type
//! - `SkillParser` for parsing SKILL.md files (YAML frontmatter + markdown)
//! - `SkillRegistry` with lazy loading and filesystem watching

mod error;
mod schema;
mod parser;
mod registry;
mod watcher;

pub use error::{CoreError, ParseError, ValidationError};
pub use schema::{Skill, SkillMetadata, SkillName, SkillStats};
pub use parser::SkillParser;
pub use registry::{RefreshReport, SkillRegistry};
