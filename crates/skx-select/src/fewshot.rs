//! Few-shot enhanced selection strategy.

use std::marker::PhantomData;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use skx_core::{SkillMetadata, SkillName};

use crate::error::SelectError;
use crate::strategy::{LatencyClass, SelectionContext, SelectionResult, SelectionStrategy};

/// A few-shot example for training.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FewShotExample {
    /// Example user query.
    pub input: String,

    /// Correct skill for this query.
    pub expected_skill: SkillName,

    /// Why this skill is correct.
    pub reasoning: Option<String>,
}

impl FewShotExample {
    /// Create a new few-shot example.
    pub fn new(input: impl Into<String>, expected_skill: SkillName) -> Self {
        Self {
            input: input.into(),
            expected_skill,
            reasoning: None,
        }
    }

    /// Add reasoning.
    pub fn with_reasoning(mut self, reasoning: impl Into<String>) -> Self {
        self.reasoning = Some(reasoning.into());
        self
    }
}

/// Wraps any strategy with dynamic few-shot example injection.
///
/// Selects examples semantically similar to the current query
/// and provides them as context to the inner strategy.
pub struct FewShotEnhanced<S: SelectionStrategy> {
    inner: S,
    examples: Vec<FewShotExample>,
    top_k_examples: usize,
    _marker: PhantomData<S>,
}

impl<S: SelectionStrategy> FewShotEnhanced<S> {
    /// Create a new few-shot enhanced strategy.
    pub fn new(inner: S, examples: Vec<FewShotExample>) -> Self {
        Self {
            inner,
            examples,
            top_k_examples: 3,
            _marker: PhantomData,
        }
    }

    /// Set the number of examples to include.
    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k_examples = top_k;
        self
    }

    /// Add an example.
    pub fn add_example(&mut self, example: FewShotExample) {
        self.examples.push(example);
    }

    /// Get examples relevant to a query.
    ///
    /// Currently uses simple substring matching.
    /// Could be enhanced with embedding similarity.
    fn get_relevant_examples(&self, query: &str) -> Vec<&FewShotExample> {
        let query_lower = query.to_lowercase();

        // Score examples by word overlap
        let mut scored: Vec<_> = self
            .examples
            .iter()
            .map(|ex| {
                let ex_lower = ex.input.to_lowercase();
                let score = query_lower
                    .split_whitespace()
                    .filter(|word| ex_lower.contains(word))
                    .count();
                (score, ex)
            })
            .filter(|(score, _)| *score > 0)
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.0.cmp(&a.0));

        // Take top k
        scored
            .into_iter()
            .take(self.top_k_examples)
            .map(|(_, ex)| ex)
            .collect()
    }
}

#[async_trait]
impl<S: SelectionStrategy> SelectionStrategy for FewShotEnhanced<S> {
    async fn select(
        &self,
        query: &str,
        candidates: &[&SkillMetadata],
        ctx: &SelectionContext,
    ) -> Result<Vec<SelectionResult>, SelectError> {
        // Get relevant examples
        let examples = self.get_relevant_examples(query);

        // Augment context with examples
        let mut augmented_ctx = ctx.clone();
        for ex in &examples {
            let example_str = format!(
                "Example: \"{}\" -> {}{}",
                ex.input,
                ex.expected_skill,
                ex.reasoning
                    .as_ref()
                    .map(|r| format!(" ({})", r))
                    .unwrap_or_default()
            );
            augmented_ctx.conversation_history.push(example_str);
        }

        // Call inner strategy with augmented context
        self.inner.select(query, candidates, &augmented_ctx).await
    }

    fn name(&self) -> &str {
        "few-shot"
    }

    fn latency_class(&self) -> LatencyClass {
        self.inner.latency_class()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trigger::TriggerStrategy;
    use std::path::PathBuf;

    fn make_metadata(name: &str, triggers: Vec<&str>) -> SkillMetadata {
        SkillMetadata {
            name: SkillName::new(name).unwrap(),
            description: "Test skill".to_string(),
            tags: Vec::new(),
            triggers: triggers.into_iter().map(String::from).collect(),
            source_path: PathBuf::new(),
            content_hash: 0,
            estimated_tokens: 100,
        }
    }

    #[tokio::test]
    async fn test_fewshot_enhanced() {
        let skills = vec![
            make_metadata("pdf-skill", vec!["pdf"]),
            make_metadata("weather-skill", vec!["weather"]),
        ];

        let inner = TriggerStrategy::from_metadata(&skills).unwrap();

        let examples = vec![
            FewShotExample::new("extract text from PDF", SkillName::new("pdf-skill").unwrap())
                .with_reasoning("PDF extraction task"),
            FewShotExample::new(
                "what's the weather",
                SkillName::new("weather-skill").unwrap(),
            ),
        ];

        let strategy = FewShotEnhanced::new(inner, examples);

        let refs: Vec<_> = skills.iter().collect();
        let ctx = SelectionContext::new();

        let results = strategy.select("get pdf text", &refs, &ctx).await.unwrap();

        assert!(!results.is_empty());
    }

    #[test]
    fn test_get_relevant_examples() {
        let inner = TriggerStrategy::new();
        let examples = vec![
            FewShotExample::new("extract text from PDF", SkillName::new("pdf-skill").unwrap()),
            FewShotExample::new(
                "what's the weather today",
                SkillName::new("weather-skill").unwrap(),
            ),
            FewShotExample::new("merge two PDFs", SkillName::new("pdf-skill").unwrap()),
        ];

        let strategy = FewShotEnhanced::new(inner, examples);

        let relevant = strategy.get_relevant_examples("extract from PDF file");

        // Should find "extract text from PDF" and "merge two PDFs" (contain "PDF")
        assert!(!relevant.is_empty());
        assert!(relevant
            .iter()
            .any(|ex| ex.expected_skill.as_str() == "pdf-skill"));
    }
}
