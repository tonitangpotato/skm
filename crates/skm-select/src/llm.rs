//! LLM-based selection strategy (s latency).

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use skm_core::SkillMetadata;

use crate::error::{LlmError, SelectError};
use crate::strategy::{
    Confidence, LatencyClass, SelectionContext, SelectionResult, SelectionStrategy,
};

/// LLM client trait — users implement for their LLM provider.
///
/// Both methods are async (all LLM calls are network IO).
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Send a prompt, receive a text response.
    async fn complete(&self, prompt: &str, max_tokens: usize) -> Result<String, LlmError>;

    /// Send a prompt with structured output (JSON mode).
    /// Default implementation calls complete() and parses JSON.
    async fn complete_structured(
        &self,
        prompt: &str,
        _schema: &serde_json::Value,
        max_tokens: usize,
    ) -> Result<serde_json::Value, LlmError> {
        let text = self.complete(prompt, max_tokens).await?;
        serde_json::from_str(&text).map_err(|e| LlmError::ParseError(e.to_string()))
    }
}

/// Configuration for LLM strategy.
#[derive(Debug, Clone)]
pub struct LlmStrategyConfig {
    /// System prompt for the LLM.
    pub system_prompt: String,

    /// Include few-shot examples from skill metadata.
    pub include_few_shot: bool,

    /// Max skills to include in prompt.
    pub max_candidates: usize,

    /// Temperature for generation.
    pub temperature: f32,
}

impl Default for LlmStrategyConfig {
    fn default() -> Self {
        Self {
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            include_few_shot: true,
            max_candidates: 20,
            temperature: 0.0,
        }
    }
}

const DEFAULT_SYSTEM_PROMPT: &str = r#"You are a skill selector. Given a user query and a list of available skills, determine which skill(s) should handle the query.

Respond ONLY with valid JSON in this format:
{"skills": [{"name": "skill-name", "confidence": 0.0-1.0, "reasoning": "brief explanation"}]}

If no skill is appropriate, return: {"skills": []}
"#;

/// LLM response structure.
#[derive(Debug, Deserialize)]
struct LlmResponse {
    skills: Vec<SkillSelection>,
}

#[derive(Debug, Deserialize)]
struct SkillSelection {
    name: String,
    confidence: f32,
    #[serde(default)]
    reasoning: Option<String>,
}

/// LLM-based intent classification for ambiguous queries.
pub struct LlmStrategy {
    client: Arc<dyn LlmClient>,
    config: LlmStrategyConfig,
}

impl LlmStrategy {
    /// Create a new LLM strategy.
    pub fn new(client: Arc<dyn LlmClient>, config: LlmStrategyConfig) -> Self {
        Self { client, config }
    }

    /// Build the prompt for the LLM.
    fn build_prompt(&self, query: &str, candidates: &[&SkillMetadata]) -> String {
        let mut prompt = self.config.system_prompt.clone();
        prompt.push_str("\n\nAvailable skills:\n");

        for (i, skill) in candidates.iter().take(self.config.max_candidates).enumerate() {
            prompt.push_str(&format!("{}. {}: {}\n", i + 1, skill.name, skill.description));
        }

        prompt.push_str(&format!("\nUser query: \"{}\"\n", query));
        prompt.push_str("\nWhich skill(s) should handle this query? Respond with JSON:");

        prompt
    }

    /// Parse the LLM response.
    fn parse_response(
        &self,
        response: &str,
        candidates: &[&SkillMetadata],
    ) -> Result<Vec<SelectionResult>, SelectError> {
        // Try to find JSON in the response
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        let parsed: LlmResponse = serde_json::from_str(json_str)
            .map_err(|e| SelectError::Llm(LlmError::ParseError(e.to_string())))?;

        // Validate and convert
        let candidate_names: std::collections::HashSet<_> =
            candidates.iter().map(|c| c.name.as_str()).collect();

        let results: Vec<SelectionResult> = parsed
            .skills
            .into_iter()
            .filter(|s| candidate_names.contains(s.name.as_str()))
            .map(|s| {
                let confidence = Confidence::from_score(s.confidence);
                let skill_name = skm_core::SkillName::new(&s.name).unwrap_or_else(|_| {
                    skm_core::SkillName::new("unknown").unwrap()
                });
                
                let mut result = SelectionResult::new(skill_name, s.confidence, confidence, "llm");
                if let Some(reasoning) = s.reasoning {
                    result = result.with_reasoning(reasoning);
                }
                result
            })
            .collect();

        Ok(results)
    }
}

#[async_trait]
impl SelectionStrategy for LlmStrategy {
    async fn select(
        &self,
        query: &str,
        candidates: &[&SkillMetadata],
        _ctx: &SelectionContext,
    ) -> Result<Vec<SelectionResult>, SelectError> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        let prompt = self.build_prompt(query, candidates);
        let response = self.client.complete(&prompt, 500).await?;
        self.parse_response(&response, candidates)
    }

    fn name(&self) -> &str {
        "llm"
    }

    fn latency_class(&self) -> LatencyClass {
        LatencyClass::Seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct MockLlmClient {
        response: String,
    }

    impl MockLlmClient {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
            }
        }
    }

    #[async_trait]
    impl LlmClient for MockLlmClient {
        async fn complete(&self, _prompt: &str, _max_tokens: usize) -> Result<String, LlmError> {
            Ok(self.response.clone())
        }
    }

    fn make_metadata(name: &str, description: &str) -> SkillMetadata {
        SkillMetadata {
            name: skm_core::SkillName::new(name).unwrap(),
            description: description.to_string(),
            tags: Vec::new(),
            triggers: Vec::new(),
            source_path: PathBuf::new(),
            content_hash: 0,
            estimated_tokens: 100,
        }
    }

    #[tokio::test]
    async fn test_llm_strategy() {
        let response = r#"{"skills": [{"name": "pdf-skill", "confidence": 0.9, "reasoning": "Query mentions PDF"}]}"#;
        let client = Arc::new(MockLlmClient::new(response));

        let strategy = LlmStrategy::new(client, LlmStrategyConfig::default());

        let skills = vec![
            make_metadata("pdf-skill", "Extract text from PDFs"),
            make_metadata("weather-skill", "Get weather info"),
        ];
        let refs: Vec<_> = skills.iter().collect();
        let ctx = SelectionContext::new();

        let results = strategy.select("extract pdf text", &refs, &ctx).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].skill.as_str(), "pdf-skill");
        assert_eq!(results[0].score, 0.9);
        assert!(results[0].reasoning.is_some());
    }

    #[tokio::test]
    async fn test_llm_strategy_no_match() {
        let response = r#"{"skills": []}"#;
        let client = Arc::new(MockLlmClient::new(response));

        let strategy = LlmStrategy::new(client, LlmStrategyConfig::default());

        let skills = vec![make_metadata("pdf-skill", "Extract text from PDFs")];
        let refs: Vec<_> = skills.iter().collect();
        let ctx = SelectionContext::new();

        let results = strategy.select("play music", &refs, &ctx).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_llm_strategy_invalid_skill() {
        let response = r#"{"skills": [{"name": "nonexistent-skill", "confidence": 0.9}]}"#;
        let client = Arc::new(MockLlmClient::new(response));

        let strategy = LlmStrategy::new(client, LlmStrategyConfig::default());

        let skills = vec![make_metadata("pdf-skill", "Extract text from PDFs")];
        let refs: Vec<_> = skills.iter().collect();
        let ctx = SelectionContext::new();

        let results = strategy.select("query", &refs, &ctx).await.unwrap();

        // Should filter out the nonexistent skill
        assert!(results.is_empty());
    }

    #[test]
    fn test_build_prompt() {
        let client = Arc::new(MockLlmClient::new(""));
        let strategy = LlmStrategy::new(client, LlmStrategyConfig::default());

        let skills = vec![
            make_metadata("pdf-skill", "Extract text from PDFs"),
            make_metadata("weather-skill", "Get weather info"),
        ];
        let refs: Vec<_> = skills.iter().collect();

        let prompt = strategy.build_prompt("test query", &refs);

        assert!(prompt.contains("pdf-skill"));
        assert!(prompt.contains("weather-skill"));
        assert!(prompt.contains("test query"));
    }
}
