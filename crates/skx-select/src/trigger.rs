//! Trigger-based selection strategy (µs latency).

use async_trait::async_trait;
use regex::Regex;

use skx_core::{SkillMetadata, SkillName, SkillRegistry};

use crate::error::SelectError;
use crate::strategy::{
    Confidence, LatencyClass, SelectionContext, SelectionResult, SelectionStrategy,
};

/// Matcher for a single skill.
struct SkillMatcher {
    skill_name: SkillName,
    keywords: Vec<String>,
    patterns: Vec<Regex>,
    negative_patterns: Vec<Regex>,
}

impl SkillMatcher {
    /// Check if the query matches this skill.
    fn matches(&self, query: &str) -> Option<(f32, &'static str)> {
        let query_lower = query.to_lowercase();

        // Check negative patterns first
        for pattern in &self.negative_patterns {
            if pattern.is_match(&query_lower) {
                return None;
            }
        }

        // Check exact keywords
        for keyword in &self.keywords {
            if query_lower.contains(keyword) {
                // Longer keywords get higher scores
                let score = (keyword.len() as f32 / query_lower.len().max(1) as f32).min(1.0);
                return Some((score.max(0.7), "keyword"));
            }
        }

        // Check regex patterns
        for pattern in &self.patterns {
            if pattern.is_match(&query_lower) {
                return Some((0.8, "pattern"));
            }
        }

        None
    }
}

/// Fast-path matching using patterns defined in skill metadata.
///
/// Supports: exact keywords, regex patterns, glob patterns.
/// Latency: µs (microseconds).
pub struct TriggerStrategy {
    matchers: Vec<SkillMatcher>,
}

impl TriggerStrategy {
    /// Build from registry. Extracts `metadata.triggers` from each skill.
    pub async fn from_registry(registry: &SkillRegistry) -> Result<Self, SelectError> {
        let catalog = registry.catalog().await;
        Self::from_metadata(&catalog)
    }

    /// Build from skill metadata.
    pub fn from_metadata(skills: &[SkillMetadata]) -> Result<Self, SelectError> {
        let mut matchers = Vec::new();

        for meta in skills {
            let mut keywords = Vec::new();
            let mut patterns = Vec::new();
            let mut negative_patterns = Vec::new();

            // Parse triggers
            for trigger in &meta.triggers {
                let trigger = trigger.trim();
                if trigger.is_empty() {
                    continue;
                }

                // Check if it's a regex pattern (starts with ^ or contains special chars)
                if trigger.starts_with('^') || trigger.contains('$') || trigger.contains('|') {
                    match Regex::new(&trigger.to_lowercase()) {
                        Ok(re) => patterns.push(re),
                        Err(e) => {
                            tracing::warn!(
                                "Invalid regex trigger '{}' for skill {}: {}",
                                trigger,
                                meta.name,
                                e
                            );
                        }
                    }
                } else {
                    // Treat as keyword
                    keywords.push(trigger.to_lowercase());
                }
            }

            // Add skill name as implicit keyword
            keywords.push(meta.name.as_str().to_string());

            // Parse negative triggers if present
            // (Would need to extend SkillMetadata to include negative_triggers)

            if !keywords.is_empty() || !patterns.is_empty() {
                matchers.push(SkillMatcher {
                    skill_name: meta.name.clone(),
                    keywords,
                    patterns,
                    negative_patterns,
                });
            }
        }

        Ok(Self { matchers })
    }

    /// Create an empty strategy.
    pub fn new() -> Self {
        Self {
            matchers: Vec::new(),
        }
    }

    /// Add a matcher manually.
    pub fn add_matcher(
        &mut self,
        skill_name: SkillName,
        keywords: Vec<String>,
        patterns: Vec<Regex>,
    ) {
        self.matchers.push(SkillMatcher {
            skill_name,
            keywords,
            patterns,
            negative_patterns: Vec::new(),
        });
    }
}

impl Default for TriggerStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SelectionStrategy for TriggerStrategy {
    async fn select(
        &self,
        query: &str,
        candidates: &[&SkillMetadata],
        _ctx: &SelectionContext,
    ) -> Result<Vec<SelectionResult>, SelectError> {
        let candidate_names: std::collections::HashSet<_> =
            candidates.iter().map(|c| &c.name).collect();

        let mut results = Vec::new();

        for matcher in &self.matchers {
            // Skip if not in candidates
            if !candidate_names.contains(&matcher.skill_name) {
                continue;
            }

            if let Some((score, match_type)) = matcher.matches(query) {
                let confidence = if score >= 0.9 {
                    Confidence::Definite
                } else if score >= 0.7 {
                    Confidence::High
                } else {
                    Confidence::Medium
                };

                results.push(
                    SelectionResult::new(matcher.skill_name.clone(), score, confidence, "trigger")
                        .with_reasoning(format!("Matched {}", match_type)),
                );
            }
        }

        // Sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        Ok(results)
    }

    fn name(&self) -> &str {
        "trigger"
    }

    fn latency_class(&self) -> LatencyClass {
        LatencyClass::Microseconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    async fn test_trigger_keyword_match() {
        let skills = vec![make_metadata("pdf-skill", vec!["pdf", ".pdf", "extract text"])];

        let strategy = TriggerStrategy::from_metadata(&skills).unwrap();
        let ctx = SelectionContext::new();
        let refs: Vec<_> = skills.iter().collect();

        let results = strategy.select("extract text from pdf", &refs, &ctx).await.unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].skill.as_str(), "pdf-skill");
    }

    #[tokio::test]
    async fn test_trigger_no_match() {
        let skills = vec![make_metadata("pdf-skill", vec!["pdf"])];

        let strategy = TriggerStrategy::from_metadata(&skills).unwrap();
        let ctx = SelectionContext::new();
        let refs: Vec<_> = skills.iter().collect();

        let results = strategy.select("create a spreadsheet", &refs, &ctx).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_trigger_regex_match() {
        let skills = vec![make_metadata("email-skill", vec!["^send.*email"])];

        let strategy = TriggerStrategy::from_metadata(&skills).unwrap();
        let ctx = SelectionContext::new();
        let refs: Vec<_> = skills.iter().collect();

        let results = strategy.select("send an email to john", &refs, &ctx).await.unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].skill.as_str(), "email-skill");
    }

    #[tokio::test]
    async fn test_trigger_case_insensitive() {
        let skills = vec![make_metadata("test-skill", vec!["PDF", "Document"])];

        let strategy = TriggerStrategy::from_metadata(&skills).unwrap();
        let ctx = SelectionContext::new();
        let refs: Vec<_> = skills.iter().collect();

        let results = strategy.select("open the PDF document", &refs, &ctx).await.unwrap();

        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_trigger_implicit_name_match() {
        let skills = vec![make_metadata("weather-lookup", vec![])];

        let strategy = TriggerStrategy::from_metadata(&skills).unwrap();
        let ctx = SelectionContext::new();
        let refs: Vec<_> = skills.iter().collect();

        let results = strategy.select("use weather-lookup", &refs, &ctx).await.unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].skill.as_str(), "weather-lookup");
    }

    #[tokio::test]
    async fn test_trigger_multiple_matches() {
        let skills = vec![
            make_metadata("pdf-skill", vec!["pdf", "document"]),
            make_metadata("doc-skill", vec!["document", "word"]),
        ];

        let strategy = TriggerStrategy::from_metadata(&skills).unwrap();
        let ctx = SelectionContext::new();
        let refs: Vec<_> = skills.iter().collect();

        let results = strategy.select("open the document", &refs, &ctx).await.unwrap();

        // Both should match "document"
        assert_eq!(results.len(), 2);
    }
}
