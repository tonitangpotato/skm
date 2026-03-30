//! # skx-learn
//!
//! Evaluation, metrics, and optimization for Agent Skills.
//!
//! This crate provides:
//! - `TriggerTestHarness` for testing skill selection accuracy
//! - `SelectionMetrics` for real-time performance tracking
//! - `DescriptionOptimizer` for LLM-driven description improvement
//! - `UsageAnalytics` for long-term usage pattern analysis

mod error;
mod harness;
mod metrics;
mod optimizer;
mod analytics;

pub use error::LearnError;
pub use harness::{
    SkillTestReport, TestCase, TestCaseResult, TestExpectation, TestReport, TestSuite,
    TriggerTestHarness,
};
pub use metrics::{MetricsSummary, SelectionMetrics};
pub use optimizer::{DescriptionOptimizer, OptimizerConfig, OptimizationIteration, OptimizationResult};
pub use analytics::{
    AnalyticsStore, Feedback, InMemoryAnalyticsStore, SelectionEvent, UsageAnalytics,
};

#[cfg(feature = "analytics-sqlite")]
pub use analytics::SqliteAnalyticsStore;
