//! Multi-component embeddings for skills.
//!
//! Skills are decomposed into semantic components (description, triggers, tags, examples)
//! and embedded separately. Final scores use weighted combination.

use serde::{Deserialize, Serialize};

use skm_core::SkillName;

use crate::embedding::Embedding;

/// Component weights for multi-component scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentWeights {
    /// Weight for description component.
    pub description: f32,

    /// Weight for triggers component.
    pub triggers: f32,

    /// Weight for tags component.
    pub tags: f32,

    /// Weight for examples component.
    pub examples: f32,
}

impl Default for ComponentWeights {
    fn default() -> Self {
        Self {
            description: 0.45,
            triggers: 0.25,
            tags: 0.15,
            examples: 0.15,
        }
    }
}

impl ComponentWeights {
    /// Create uniform weights (0.25 each).
    pub fn uniform() -> Self {
        Self {
            description: 0.25,
            triggers: 0.25,
            tags: 0.25,
            examples: 0.25,
        }
    }

    /// Create description-only weights.
    pub fn description_only() -> Self {
        Self {
            description: 1.0,
            triggers: 0.0,
            tags: 0.0,
            examples: 0.0,
        }
    }

    /// Normalize weights to sum to 1.0.
    pub fn normalize(&mut self) {
        let sum = self.description + self.triggers + self.tags + self.examples;
        if sum > 0.0 {
            self.description /= sum;
            self.triggers /= sum;
            self.tags /= sum;
            self.examples /= sum;
        }
    }

    /// Check if weights sum to approximately 1.0.
    pub fn is_normalized(&self) -> bool {
        let sum = self.description + self.triggers + self.tags + self.examples;
        (sum - 1.0).abs() < 1e-5
    }
}

/// Multi-component embedding for a single skill.
/// Each component is embedded separately and scored with configurable weights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEmbeddings {
    /// The skill name this embedding belongs to.
    pub skill_name: SkillName,

    /// Embedding of the main description text.
    pub description: Embedding,

    /// Embedding of trigger keywords concatenated.
    pub triggers: Embedding,

    /// Embedding of tags concatenated.
    pub tags: Embedding,

    /// Embedding of example inputs concatenated.
    pub examples: Embedding,

    /// Weights for combining component scores.
    pub weights: ComponentWeights,
}

impl SkillEmbeddings {
    /// Create new skill embeddings.
    pub fn new(
        skill_name: SkillName,
        description: Embedding,
        triggers: Embedding,
        tags: Embedding,
        examples: Embedding,
        weights: ComponentWeights,
    ) -> Self {
        Self {
            skill_name,
            description,
            triggers,
            tags,
            examples,
            weights,
        }
    }

    /// Compute weighted similarity score against a query embedding.
    pub fn score(&self, query: &Embedding) -> f32 {
        self.weights.description * self.description.cosine_similarity(query)
            + self.weights.triggers * self.triggers.cosine_similarity(query)
            + self.weights.tags * self.tags.cosine_similarity(query)
            + self.weights.examples * self.examples.cosine_similarity(query)
    }

    /// Compute per-component scores against a query.
    pub fn component_scores(&self, query: &Embedding) -> ComponentScores {
        ComponentScores {
            description: self.description.cosine_similarity(query),
            triggers: self.triggers.cosine_similarity(query),
            tags: self.tags.cosine_similarity(query),
            examples: self.examples.cosine_similarity(query),
        }
    }

    /// Update weights.
    pub fn with_weights(mut self, weights: ComponentWeights) -> Self {
        self.weights = weights;
        self
    }
}

/// Per-component similarity scores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentScores {
    /// Score from description component.
    pub description: f32,

    /// Score from triggers component.
    pub triggers: f32,

    /// Score from tags component.
    pub tags: f32,

    /// Score from examples component.
    pub examples: f32,
}

impl ComponentScores {
    /// Compute weighted sum using given weights.
    pub fn weighted_sum(&self, weights: &ComponentWeights) -> f32 {
        weights.description * self.description
            + weights.triggers * self.triggers
            + weights.tags * self.tags
            + weights.examples * self.examples
    }

    /// Get the maximum component score.
    pub fn max(&self) -> f32 {
        self.description
            .max(self.triggers)
            .max(self.tags)
            .max(self.examples)
    }

    /// Get the component with highest score.
    pub fn best_component(&self) -> &'static str {
        let max = self.max();
        if (self.description - max).abs() < 1e-6 {
            "description"
        } else if (self.triggers - max).abs() < 1e-6 {
            "triggers"
        } else if (self.tags - max).abs() < 1e-6 {
            "tags"
        } else {
            "examples"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_embedding(seed: u64) -> Embedding {
        // Create a deterministic embedding based on seed
        let mut vector = vec![0.0f32; 4];
        for (i, v) in vector.iter_mut().enumerate() {
            *v = ((seed + i as u64) % 100) as f32 / 100.0;
        }
        Embedding::new(vector, seed)
    }

    #[test]
    fn test_component_weights_default() {
        let weights = ComponentWeights::default();
        assert!(weights.is_normalized());
        assert!((weights.description - 0.45).abs() < 1e-5);
    }

    #[test]
    fn test_component_weights_uniform() {
        let weights = ComponentWeights::uniform();
        assert!(weights.is_normalized());
        assert!((weights.description - 0.25).abs() < 1e-5);
    }

    #[test]
    fn test_component_weights_normalize() {
        let mut weights = ComponentWeights {
            description: 2.0,
            triggers: 1.0,
            tags: 1.0,
            examples: 0.0,
        };
        weights.normalize();
        assert!(weights.is_normalized());
        assert!((weights.description - 0.5).abs() < 1e-5);
        assert!((weights.triggers - 0.25).abs() < 1e-5);
    }

    #[test]
    fn test_skill_embeddings_score() {
        let skill_name = SkillName::new("test").unwrap();
        let embeddings = SkillEmbeddings::new(
            skill_name,
            make_embedding(1),
            make_embedding(2),
            make_embedding(3),
            make_embedding(4),
            ComponentWeights::uniform(),
        );

        let query = make_embedding(1);
        let score = embeddings.score(&query);

        // Score should be between -1 and 1
        assert!(score >= -1.0 && score <= 1.0);
    }

    #[test]
    fn test_component_scores() {
        let skill_name = SkillName::new("test").unwrap();
        let embeddings = SkillEmbeddings::new(
            skill_name,
            make_embedding(1),
            make_embedding(2),
            make_embedding(3),
            make_embedding(4),
            ComponentWeights::uniform(),
        );

        let query = make_embedding(1);
        let scores = embeddings.component_scores(&query);

        // Description should have highest score (same seed)
        assert!(scores.description > scores.triggers);
    }

    #[test]
    fn test_component_scores_weighted_sum() {
        let scores = ComponentScores {
            description: 0.8,
            triggers: 0.6,
            tags: 0.4,
            examples: 0.2,
        };

        let weights = ComponentWeights::uniform();
        let sum = scores.weighted_sum(&weights);

        // (0.8 + 0.6 + 0.4 + 0.2) * 0.25 = 0.5
        assert!((sum - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_component_scores_best() {
        let scores = ComponentScores {
            description: 0.3,
            triggers: 0.9,
            tags: 0.4,
            examples: 0.2,
        };

        assert_eq!(scores.best_component(), "triggers");
        assert!((scores.max() - 0.9).abs() < 1e-5);
    }
}
