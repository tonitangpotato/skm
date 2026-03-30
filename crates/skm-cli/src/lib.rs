//! # skm-cli
//!
//! Developer CLI for Agent Skills.
//!
//! Commands:
//! - `init` - Create a new skill from template
//! - `validate` - Validate SKILL.md files
//! - `list` - List skills in a directory
//! - `select` - Interactive skill selection (debugging)
//! - `test` - Run trigger test suites
//! - `bench` - Benchmark selection performance
//! - `optimize` - Optimize skill descriptions
//! - `stats` - Show usage analytics
//! - `serve` - Start HTTP API server

pub mod commands;

#[cfg(feature = "http-server")]
pub mod server;
