//! LLM-driven skill description optimization.

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use skx_core::{SkillName, SkillRegistry};
use skx_select::{CascadeSelector, LlmClient, LlmError};

use crate::error::LearnError;
use crate::harness::{TestReport, TestSuite, TriggerTestHarness};

/// Configuration for the optimizer.
#[derive(Debug, Clone)]
pub struct OptimizerConfig {
    /// Maximum iterations.
    pub max_iterations: usize,

    /// Target accuracy to stop early.
    pub target_accuracy: f32,

    /// System prompt for the LLM.
    pub system_prompt: String,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            target_accuracy: 0.95,
            system_prompt: DEFAULT_OPTIMIZER_PROMPT.to_string(),
        }
    }
}

const DEFAULT_OPTIMIZER_PROMPT: &str = r#"You are a skill description optimizer. Given a skill's current description and test results, suggest an improved description that will help the skill be selected for appropriate queries.

Rules:
1. Keep the description concise (under 200 characters)
2. Include key trigger words that users might use
3. Be specific about what the skill does
4. Avoid overly generic terms

Respond with ONLY the new description, nothing else."#;

/// Result of a single optimization iteration.
#[derive(Debug, Clone)]
pub struct OptimizationIteration {
    /// Iteration number.
    pub iteration: usize,

    /// Description before optimization.
    pub old_description: String,

    /// Description after optimization.
    pub new_description: String,

    /// Accuracy before.
    pub old_accuracy: f32,

    /// Accuracy after.
    pub new_accuracy: f32,

    /// Whether we kept the new description.
    pub accepted: bool,
}

/// Result of the full optimization process.
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    /// Skill that was optimized.
    pub skill: SkillName,

    /// Final description.
    pub final_description: String,

    /// Iterations performed.
    pub iterations: Vec<OptimizationIteration>,

    /// Final accuracy.
    pub final_accuracy: f32,

    /// Initial accuracy.
    pub initial_accuracy: f32,
}

/// LLM-driven description optimizer.
pub struct DescriptionOptimizer {
    llm: Arc<dyn LlmClient>,
    config: OptimizerConfig,
}

impl DescriptionOptimizer {
    /// Create a new optimizer.
    pub fn new(llm: Arc<dyn LlmClient>, config: OptimizerConfig) -> Self {
        Self { llm, config }
    }

    /// Build a prompt for the LLM.
    fn build_prompt(
        &self,
        skill_name: &SkillName,
        current_description: &str,
        test_report: &TestReport,
    ) -> String {
        let skill_report = test_report.per_skill.get(skill_name);

        let mut prompt = self.config.system_prompt.clone();

        prompt.push_str(&format!("\n\nSkill: {}\n", skill_name));
        prompt.push_str(&format!("Current description: {}\n", current_description));

        if let Some(report) = skill_report {
            prompt.push_str(&format!("\nTest results:\n"));
            prompt.push_str(&format!("- Precision: {:.1}%\n", report.precision() * 100.0));
            prompt.push_str(&format!("- Recall: {:.1}%\n", report.recall() * 100.0));
            prompt.push_str(&format!("- False positives: {}\n", report.false_positives));
            prompt.push_str(&format!("- False negatives: {}\n", report.false_negatives));
        }

        // Add failed test cases for context
        prompt.push_str("\nFailed test cases:\n");
        for result in &test_report.results {
            if !result.passed {
                match &result.expected {
                    crate::harness::TestExpectation::Single(exp) if exp == skill_name => {
                        prompt.push_str(&format!(
                            "- Query: \"{}\" (expected {}, got {:?})\n",
                            result.name, skill_name, result.selected
                        ));
                    }
                    crate::harness::TestExpectation::AnyOf(exps) if exps.contains(skill_name) => {
                        prompt.push_str(&format!(
                            "- Query: \"{}\" (expected {:?}, got {:?})\n",
                            result.name, exps, result.selected
                        ));
                    }
                    _ => {}
                }
            }
        }

        prompt.push_str("\nSuggest an improved description:");

        prompt
    }

    /// Optimize a single skill's description.
    ///
    /// Note: This is a simplified implementation. In practice, you'd need
    /// to actually update the SKILL.md file and rebuild the index.
    pub async fn optimize(
        &self,
        skill: &SkillName,
        suite: &TestSuite,
        selector: &CascadeSelector,
        registry: &SkillRegistry,
    ) -> Result<OptimizationResult, LearnError> {
        let harness = TriggerTestHarness::new();

        // Get initial description
        let skill_meta = registry
            .get_metadata(skill)
            .await
            .ok_or_else(|| LearnError::Optimizer(format!("Skill not found: {}", skill)))?;

        let mut current_description = skill_meta.description.clone();
        let mut iterations = Vec::new();

        // Run initial test
        let initial_report = harness.run(suite, selector, registry).await?;
        let initial_accuracy = initial_report.accuracy();
        let mut best_accuracy = initial_accuracy;

        if initial_accuracy >= self.config.target_accuracy {
            return Ok(OptimizationResult {
                skill: skill.clone(),
                final_description: current_description,
                iterations,
                final_accuracy: initial_accuracy,
                initial_accuracy,
            });
        }

        for i in 0..self.config.max_iterations {
            // Generate new description
            let prompt = self.build_prompt(skill, &current_description, &initial_report);
            let new_description = self
                .llm
                .complete(&prompt, 200)
                .await
                .map_err(|e| LearnError::Optimizer(format!("LLM error: {}", e)))?
                .trim()
                .to_string();

            // In a real implementation, we would:
            // 1. Update the skill file
            // 2. Rebuild the index
            // 3. Re-run tests

            // For now, we just record the iteration
            let iteration = OptimizationIteration {
                iteration: i + 1,
                old_description: current_description.clone(),
                new_description: new_description.clone(),
                old_accuracy: best_accuracy,
                new_accuracy: best_accuracy, // Would be updated after re-testing
                accepted: true, // Would be based on improvement
            };

            iterations.push(iteration);
            current_description = new_description;

            // Check if we've reached target
            if best_accuracy >= self.config.target_accuracy {
                break;
            }
        }

        Ok(OptimizationResult {
            skill: skill.clone(),
            final_description: current_description,
            iterations,
            final_accuracy: best_accuracy,
            initial_accuracy,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockLlm;

    #[async_trait]
    impl LlmClient for MockLlm {
        async fn complete(&self, _prompt: &str, _max_tokens: usize) -> Result<String, LlmError> {
            Ok("Improved description for skill".to_string())
        }
    }

    #[test]
    fn test_optimizer_config_default() {
        let config = OptimizerConfig::default();
        assert_eq!(config.max_iterations, 5);
        assert_eq!(config.target_accuracy, 0.95);
    }

    #[test]
    fn test_build_prompt() {
        let llm = Arc::new(MockLlm);
        let optimizer = DescriptionOptimizer::new(llm, OptimizerConfig::default());

        let skill = SkillName::new("test-skill").unwrap();
        let report = TestReport {
            suite_name: "test".to_string(),
            results: Vec::new(),
            per_skill: std::collections::HashMap::new(),
            total: 0,
            passed: 0,
            avg_latency_ms: 0.0,
        };

        let prompt = optimizer.build_prompt(&skill, "Current description", &report);

        assert!(prompt.contains("test-skill"));
        assert!(prompt.contains("Current description"));
    }
}
