//! SelectionStrategy trait and related types.

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use skx_core::{SkillMetadata, SkillName};

use crate::error::SelectError;

/// Confidence level for a selection result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Confidence {
    /// No match at all.
    None,

    /// Weak signal, definitely try next strategy.
    Low,

    /// Decent match, but slower strategy might do better.
    Medium,

    /// Strong match, no need for slower strategies.
    High,

    /// Single clear match, proceed without fallback.
    Definite,
}

impl Confidence {
    /// Check if this confidence level is high enough to stop the cascade.
    pub fn is_high_enough(&self, threshold: Confidence) -> bool {
        *self >= threshold
    }

    /// Convert to a numeric score (0.0 - 1.0).
    pub fn as_score(&self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Low => 0.25,
            Self::Medium => 0.5,
            Self::High => 0.75,
            Self::Definite => 1.0,
        }
    }

    /// Create from a numeric score.
    pub fn from_score(score: f32) -> Self {
        if score >= 0.9 {
            Self::Definite
        } else if score >= 0.7 {
            Self::High
        } else if score >= 0.5 {
            Self::Medium
        } else if score >= 0.25 {
            Self::Low
        } else {
            Self::None
        }
    }
}

impl Default for Confidence {
    fn default() -> Self {
        Self::None
    }
}

/// Latency class for a strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyClass {
    /// Trigger matching: µs latency.
    Microseconds,

    /// Semantic search: ms latency.
    Milliseconds,

    /// LLM classification: s latency.
    Seconds,
}

/// Result of a selection operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionResult {
    /// The selected skill.
    pub skill: SkillName,

    /// Normalized score (0.0 - 1.0).
    pub score: f32,

    /// Confidence level.
    pub confidence: Confidence,

    /// Which strategy produced this result.
    pub strategy: String,

    /// Optional reasoning (for LLM strategies).
    pub reasoning: Option<String>,
}

impl SelectionResult {
    /// Create a new selection result.
    pub fn new(
        skill: SkillName,
        score: f32,
        confidence: Confidence,
        strategy: impl Into<String>,
    ) -> Self {
        Self {
            skill,
            score,
            confidence,
            strategy: strategy.into(),
            reasoning: None,
        }
    }

    /// Add reasoning.
    pub fn with_reasoning(mut self, reasoning: impl Into<String>) -> Self {
        self.reasoning = Some(reasoning.into());
        self
    }
}

/// Context available during selection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SelectionContext {
    /// Recent conversation history.
    pub conversation_history: Vec<String>,

    /// Currently active/loaded skills.
    pub active_skills: Vec<SkillName>,

    /// User locale (e.g., "zh-CN", "en-US").
    pub user_locale: Option<String>,

    /// Custom context data.
    pub custom: HashMap<String, String>,
}

impl SelectionContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add conversation history.
    pub fn with_history(mut self, history: Vec<String>) -> Self {
        self.conversation_history = history;
        self
    }

    /// Add active skills.
    pub fn with_active_skills(mut self, skills: Vec<SkillName>) -> Self {
        self.active_skills = skills;
        self
    }

    /// Set user locale.
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.user_locale = Some(locale.into());
        self
    }

    /// Add custom context data.
    pub fn with_custom(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom.insert(key.into(), value.into());
        self
    }
}

/// A skill selection strategy.
///
/// Strategies are composable and async. The cascade selector runs strategies
/// in order until one returns with sufficient confidence.
#[async_trait]
pub trait SelectionStrategy: Send + Sync {
    /// Rank skills for a given query. Returns scored candidates.
    async fn select(
        &self,
        query: &str,
        candidates: &[&SkillMetadata],
        ctx: &SelectionContext,
    ) -> Result<Vec<SelectionResult>, SelectError>;

    /// Strategy name for logging/metrics.
    fn name(&self) -> &str;

    /// Expected latency class.
    fn latency_class(&self) -> LatencyClass;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_ordering() {
        assert!(Confidence::Definite > Confidence::High);
        assert!(Confidence::High > Confidence::Medium);
        assert!(Confidence::Medium > Confidence::Low);
        assert!(Confidence::Low > Confidence::None);
    }

    #[test]
    fn test_confidence_as_score() {
        assert_eq!(Confidence::None.as_score(), 0.0);
        assert_eq!(Confidence::Definite.as_score(), 1.0);
    }

    #[test]
    fn test_confidence_from_score() {
        assert_eq!(Confidence::from_score(0.95), Confidence::Definite);
        assert_eq!(Confidence::from_score(0.8), Confidence::High);
        assert_eq!(Confidence::from_score(0.6), Confidence::Medium);
        assert_eq!(Confidence::from_score(0.3), Confidence::Low);
        assert_eq!(Confidence::from_score(0.1), Confidence::None);
    }

    #[test]
    fn test_is_high_enough() {
        assert!(Confidence::Definite.is_high_enough(Confidence::High));
        assert!(Confidence::High.is_high_enough(Confidence::High));
        assert!(!Confidence::Medium.is_high_enough(Confidence::High));
    }

    #[test]
    fn test_selection_result() {
        let skill = SkillName::new("test").unwrap();
        let result = SelectionResult::new(skill.clone(), 0.9, Confidence::High, "trigger")
            .with_reasoning("Matched keyword");

        assert_eq!(result.skill, skill);
        assert_eq!(result.score, 0.9);
        assert_eq!(result.reasoning, Some("Matched keyword".to_string()));
    }

    #[test]
    fn test_selection_context() {
        let ctx = SelectionContext::new()
            .with_locale("zh-CN")
            .with_custom("key", "value");

        assert_eq!(ctx.user_locale, Some("zh-CN".to_string()));
        assert_eq!(ctx.custom.get("key"), Some(&"value".to_string()));
    }
}
