//! Cascading skill selector.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ase_core::{SkillMetadata, SkillRegistry};
use ase_embed::EmbeddingProvider;

use crate::error::SelectError;
use crate::semantic::{SemanticConfig, SemanticStrategy};
use crate::strategy::{Confidence, SelectionContext, SelectionResult, SelectionStrategy};
use crate::trigger::TriggerStrategy;

/// How to merge results from multiple strategies.
#[derive(Debug, Clone, Copy)]
pub enum MergeStrategy {
    /// Take the highest-scoring result across all strategies.
    MaxScore,

    /// Weighted average of scores, with strategy-level weights.
    WeightedAverage,

    /// Reciprocal Rank Fusion across strategy rankings.
    RRF { k: f32 },
}

impl Default for MergeStrategy {
    fn default() -> Self {
        Self::MaxScore
    }
}

/// Configuration for the cascade selector.
#[derive(Debug, Clone)]
pub struct CascadeConfig {
    /// If true, always run all strategies and merge results.
    /// If false (default), stop at first confident result.
    pub exhaustive: bool,

    /// Maximum total latency budget. Skip slow strategies if exceeded.
    pub timeout: Duration,

    /// How to merge results from multiple strategies.
    pub merge_strategy: MergeStrategy,
}

impl Default for CascadeConfig {
    fn default() -> Self {
        Self {
            exhaustive: false,
            timeout: Duration::from_secs(5),
            merge_strategy: MergeStrategy::MaxScore,
        }
    }
}

impl CascadeConfig {
    /// Enable exhaustive mode.
    pub fn exhaustive(mut self) -> Self {
        self.exhaustive = true;
        self
    }

    /// Set timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set merge strategy.
    pub fn with_merge_strategy(mut self, strategy: MergeStrategy) -> Self {
        self.merge_strategy = strategy;
        self
    }
}

/// Complete outcome of a selection, with full audit trail.
#[derive(Debug)]
pub struct SelectionOutcome {
    /// Final ranked results.
    pub selected: Vec<SelectionResult>,

    /// Which strategies ran.
    pub strategies_used: Vec<String>,

    /// Total latency.
    pub total_latency: Duration,

    /// Per-strategy latency.
    pub per_strategy_latency: Vec<(String, Duration)>,

    /// Did we reach LLM fallback?
    pub fallback_used: bool,
}

/// Cascading skill selector.
///
/// Tries strategies in order, stopping when confidence is high enough.
pub struct CascadeSelector {
    strategies: Vec<(Box<dyn SelectionStrategy>, Confidence)>,
    config: CascadeConfig,
}

impl CascadeSelector {
    /// Create a new cascade selector builder.
    pub fn builder() -> CascadeSelectorBuilder {
        CascadeSelectorBuilder::new()
    }

    /// Select the best skill(s) for a query.
    pub async fn select(
        &self,
        query: &str,
        registry: &SkillRegistry,
        ctx: &SelectionContext,
    ) -> Result<SelectionOutcome, SelectError> {
        let start = Instant::now();
        let catalog = registry.catalog().await;
        let candidates: Vec<_> = catalog.iter().collect();

        let mut all_results: Vec<SelectionResult> = Vec::new();
        let mut strategies_used = Vec::new();
        let mut per_strategy_latency = Vec::new();
        let mut fallback_used = false;

        for (strategy, stop_confidence) in &self.strategies {
            // Check timeout
            if start.elapsed() >= self.config.timeout {
                tracing::warn!("Cascade timeout exceeded, stopping");
                break;
            }

            let strategy_start = Instant::now();
            strategies_used.push(strategy.name().to_string());

            // Track if this is LLM fallback
            if strategy.name() == "llm" {
                fallback_used = true;
            }

            // Run strategy
            match strategy.select(query, &candidates, ctx).await {
                Ok(results) => {
                    per_strategy_latency
                        .push((strategy.name().to_string(), strategy_start.elapsed()));

                    if !results.is_empty() {
                        // Check if we should stop
                        let best_confidence = results
                            .iter()
                            .map(|r| r.confidence)
                            .max()
                            .unwrap_or(Confidence::None);

                        all_results.extend(results);

                        if !self.config.exhaustive && best_confidence >= *stop_confidence {
                            tracing::debug!(
                                "Stopping cascade at {} with confidence {:?}",
                                strategy.name(),
                                best_confidence
                            );
                            break;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Strategy {} failed: {}", strategy.name(), e);
                    per_strategy_latency
                        .push((strategy.name().to_string(), strategy_start.elapsed()));
                }
            }
        }

        // Merge results
        let selected = self.merge_results(all_results);

        Ok(SelectionOutcome {
            selected,
            strategies_used,
            total_latency: start.elapsed(),
            per_strategy_latency,
            fallback_used,
        })
    }

    /// Merge results from multiple strategies.
    fn merge_results(&self, mut results: Vec<SelectionResult>) -> Vec<SelectionResult> {
        if results.is_empty() {
            return results;
        }

        match self.config.merge_strategy {
            MergeStrategy::MaxScore => {
                // Sort by score descending
                results
                    .sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

                // Deduplicate by skill name, keeping highest score
                let mut seen = std::collections::HashSet::new();
                results.retain(|r| seen.insert(r.skill.clone()));

                results
            }
            MergeStrategy::WeightedAverage => {
                // Group by skill
                let mut by_skill: std::collections::HashMap<_, Vec<_>> =
                    std::collections::HashMap::new();
                for r in results {
                    by_skill.entry(r.skill.clone()).or_default().push(r);
                }

                // Average scores for each skill
                let mut merged: Vec<_> = by_skill
                    .into_iter()
                    .map(|(skill, group)| {
                        let avg_score =
                            group.iter().map(|r| r.score).sum::<f32>() / group.len() as f32;
                        let best = group.into_iter().max_by(|a, b| {
                            a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal)
                        }).unwrap();
                        
                        SelectionResult {
                            skill,
                            score: avg_score,
                            confidence: Confidence::from_score(avg_score),
                            strategy: best.strategy,
                            reasoning: best.reasoning,
                        }
                    })
                    .collect();

                merged.sort_by(|a, b| {
                    b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
                });
                merged
            }
            MergeStrategy::RRF { k } => {
                // Reciprocal Rank Fusion
                let mut scores: std::collections::HashMap<_, f32> =
                    std::collections::HashMap::new();
                let mut best_results: std::collections::HashMap<_, SelectionResult> =
                    std::collections::HashMap::new();

                // Group results by strategy
                let mut by_strategy: std::collections::HashMap<_, Vec<_>> =
                    std::collections::HashMap::new();
                for r in results {
                    by_strategy
                        .entry(r.strategy.clone())
                        .or_default()
                        .push(r);
                }

                // Calculate RRF scores
                for (_, mut strategy_results) in by_strategy {
                    strategy_results.sort_by(|a, b| {
                        b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
                    });

                    for (rank, r) in strategy_results.into_iter().enumerate() {
                        let rrf_score = 1.0 / (k + rank as f32 + 1.0);
                        *scores.entry(r.skill.clone()).or_default() += rrf_score;

                        best_results
                            .entry(r.skill.clone())
                            .and_modify(|existing| {
                                if r.score > existing.score {
                                    *existing = r.clone();
                                }
                            })
                            .or_insert(r);
                    }
                }

                // Build final results
                let mut merged: Vec<_> = scores
                    .into_iter()
                    .map(|(skill, score)| {
                        let best = best_results.remove(&skill).unwrap();
                        SelectionResult {
                            skill,
                            score,
                            confidence: Confidence::from_score(score.min(1.0)),
                            strategy: best.strategy,
                            reasoning: best.reasoning,
                        }
                    })
                    .collect();

                merged.sort_by(|a, b| {
                    b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
                });
                merged
            }
        }
    }
}

/// Builder for ergonomic cascade construction.
pub struct CascadeSelectorBuilder {
    strategies: Vec<(Box<dyn SelectionStrategy>, Confidence)>,
    config: CascadeConfig,
}

impl CascadeSelectorBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            strategies: Vec::new(),
            config: CascadeConfig::default(),
        }
    }

    /// Add trigger strategy as first cascade level.
    /// Stops cascade if confidence >= High.
    pub fn with_triggers(mut self, strategy: TriggerStrategy) -> Self {
        self.strategies
            .push((Box::new(strategy), Confidence::High));
        self
    }

    /// Add semantic strategy as second cascade level.
    pub fn with_semantic(
        mut self,
        provider: Arc<dyn EmbeddingProvider>,
        index: ase_embed::EmbeddingIndex,
        config: SemanticConfig,
    ) -> Self {
        let strategy = SemanticStrategy::new(provider, index, config);
        self.strategies
            .push((Box::new(strategy), Confidence::Medium));
        self
    }

    /// Add a custom strategy at a specific cascade position.
    pub fn with_custom(
        mut self,
        strategy: Box<dyn SelectionStrategy>,
        stop_confidence: Confidence,
    ) -> Self {
        self.strategies.push((strategy, stop_confidence));
        self
    }

    /// Set the cascade config.
    pub fn config(mut self, config: CascadeConfig) -> Self {
        self.config = config;
        self
    }

    /// Build the CascadeSelector.
    pub fn build(self) -> CascadeSelector {
        CascadeSelector {
            strategies: self.strategies,
            config: self.config,
        }
    }
}

impl Default for CascadeSelectorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ase_core::SkillName;
    use std::fs;
    use tempfile::TempDir;

    async fn setup_registry() -> (TempDir, SkillRegistry) {
        let temp = TempDir::new().unwrap();

        let skill = r#"---
name: pdf-skill
description: Extract text from PDF files
metadata:
  triggers: "pdf, extract text"
---

Instructions here.
"#;

        let skill_dir = temp.path().join("pdf-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), skill).unwrap();

        let registry = SkillRegistry::new(&[temp.path()]).await.unwrap();
        (temp, registry)
    }

    #[tokio::test]
    async fn test_cascade_trigger_only() {
        let (_temp, registry) = setup_registry().await;

        let trigger = TriggerStrategy::from_registry(&registry).await.unwrap();
        let selector = CascadeSelector::builder().with_triggers(trigger).build();

        let ctx = SelectionContext::new();
        let outcome = selector.select("extract pdf text", &registry, &ctx).await.unwrap();

        assert!(!outcome.selected.is_empty());
        assert_eq!(outcome.selected[0].skill.as_str(), "pdf-skill");
        assert!(outcome.strategies_used.contains(&"trigger".to_string()));
    }

    #[tokio::test]
    async fn test_cascade_no_match() {
        let (_temp, registry) = setup_registry().await;

        let trigger = TriggerStrategy::from_registry(&registry).await.unwrap();
        let selector = CascadeSelector::builder().with_triggers(trigger).build();

        let ctx = SelectionContext::new();
        let outcome = selector.select("play music", &registry, &ctx).await.unwrap();

        assert!(outcome.selected.is_empty());
    }

    #[test]
    fn test_merge_max_score() {
        let selector = CascadeSelector::builder()
            .config(CascadeConfig::default())
            .build();

        let results = vec![
            SelectionResult::new(
                SkillName::new("skill-a").unwrap(),
                0.8,
                Confidence::High,
                "trigger",
            ),
            SelectionResult::new(
                SkillName::new("skill-a").unwrap(),
                0.6,
                Confidence::Medium,
                "semantic",
            ),
            SelectionResult::new(
                SkillName::new("skill-b").unwrap(),
                0.7,
                Confidence::High,
                "trigger",
            ),
        ];

        let merged = selector.merge_results(results);

        // skill-a should have highest score (0.8), then skill-b (0.7)
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].skill.as_str(), "skill-a");
        assert_eq!(merged[0].score, 0.8);
    }

    #[test]
    fn test_merge_rrf() {
        let selector = CascadeSelector::builder()
            .config(CascadeConfig::default().with_merge_strategy(MergeStrategy::RRF { k: 60.0 }))
            .build();

        let results = vec![
            SelectionResult::new(
                SkillName::new("skill-a").unwrap(),
                0.9,
                Confidence::High,
                "trigger",
            ),
            SelectionResult::new(
                SkillName::new("skill-b").unwrap(),
                0.7,
                Confidence::Medium,
                "trigger",
            ),
            SelectionResult::new(
                SkillName::new("skill-b").unwrap(),
                0.8,
                Confidence::High,
                "semantic",
            ),
            SelectionResult::new(
                SkillName::new("skill-a").unwrap(),
                0.6,
                Confidence::Medium,
                "semantic",
            ),
        ];

        let merged = selector.merge_results(results);

        // Both skills should have RRF scores
        assert_eq!(merged.len(), 2);
    }
}
