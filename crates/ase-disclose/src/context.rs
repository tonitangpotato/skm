//! Context manager for progressive disclosure.

use std::collections::HashMap;

use ase_core::{SkillName, SkillRegistry};

use crate::error::DiscloseError;
use crate::levels::{DisclosureLevel, LoadedSkill};
use crate::tokens::TokenEstimator;

/// Token budget configuration.
#[derive(Debug, Clone)]
pub struct TokenBudget {
    /// Total budget for skills in context.
    pub max_tokens: usize,

    /// Reserved for Level 0 catalog.
    pub catalog_reserve: usize,

    /// Max tokens per activated skill.
    pub per_skill_max: usize,
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self {
            max_tokens: 50000,
            catalog_reserve: 5000,
            per_skill_max: 10000,
        }
    }
}

impl TokenBudget {
    /// Create a budget with the specified total.
    pub fn with_max(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            catalog_reserve: max_tokens / 10,
            per_skill_max: max_tokens / 5,
        }
    }
}

/// Payload returned when activating a skill.
#[derive(Debug, Clone)]
pub struct ActivationPayload {
    /// The skill name.
    pub skill_name: SkillName,

    /// Full SKILL.md instructions.
    pub instructions: String,

    /// Files that can be loaded on demand (references/, scripts/).
    pub available_references: Vec<String>,

    /// Token count of the instructions.
    pub tokens: usize,
}

/// Manages what skill content is loaded into the LLM's context window.
pub struct ContextManager {
    /// Token budget.
    budget: TokenBudget,

    /// Currently loaded skills.
    loaded: HashMap<SkillName, LoadedSkill>,

    /// Token estimator.
    estimator: TokenEstimator,

    /// Tokens used by catalog.
    catalog_tokens: usize,
}

impl ContextManager {
    /// Create a new context manager with the given budget.
    pub fn new(budget: TokenBudget) -> Self {
        Self {
            budget,
            loaded: HashMap::new(),
            estimator: TokenEstimator::new(),
            catalog_tokens: 0,
        }
    }

    /// Generate the Level 0 catalog string for the system prompt.
    /// Only name + description for each skill.
    pub async fn catalog_prompt(&mut self, registry: &SkillRegistry) -> String {
        let catalog = registry.catalog().await;

        let mut lines = Vec::with_capacity(catalog.len() + 2);
        lines.push("Available skills:".to_string());

        for meta in &catalog {
            lines.push(format!("- {}: {}", meta.name, meta.description));
        }

        let prompt = lines.join("\n");
        self.catalog_tokens = self.estimator.estimate_cjk_aware(&prompt);

        prompt
    }

    /// Activate a skill: load full SKILL.md into context.
    /// Returns the content to inject into the LLM prompt.
    pub async fn activate(
        &mut self,
        name: &SkillName,
        registry: &SkillRegistry,
    ) -> Result<ActivationPayload, DiscloseError> {
        // Check if already activated
        if let Some(loaded) = self.loaded.get(name) {
            if loaded.level.has_instructions() {
                // Already activated, get the skill again for the payload
                let skill = registry.get(name).await?;
                return Ok(ActivationPayload {
                    skill_name: name.clone(),
                    instructions: skill.instructions.clone(),
                    available_references: find_references(&skill.source_path),
                    tokens: loaded.tokens_used,
                });
            }
        }

        // Load the full skill
        let skill = registry.get(name).await?;
        let tokens = self.estimator.estimate_cjk_aware(&skill.instructions);

        // Check budget
        let available = self.tokens_remaining();
        if tokens > available {
            return Err(DiscloseError::BudgetExceeded {
                needed: tokens,
                available,
            });
        }

        // Check per-skill max
        if tokens > self.budget.per_skill_max {
            tracing::warn!(
                "Skill {} exceeds per-skill max ({} > {})",
                name,
                tokens,
                self.budget.per_skill_max
            );
        }

        // Record activation
        self.loaded
            .insert(name.clone(), LoadedSkill::new(name.clone(), DisclosureLevel::Activated, tokens));

        Ok(ActivationPayload {
            skill_name: name.clone(),
            instructions: skill.instructions.clone(),
            available_references: find_references(&skill.source_path),
            tokens,
        })
    }

    /// Load a specific reference file from an activated skill.
    pub async fn load_reference(
        &mut self,
        skill: &SkillName,
        file: &str,
        registry: &SkillRegistry,
    ) -> Result<String, DiscloseError> {
        // Check skill is activated
        {
            let loaded = self
                .loaded
                .get(skill)
                .ok_or_else(|| DiscloseError::NotActivated(skill.clone()))?;

            if !loaded.level.has_instructions() {
                return Err(DiscloseError::NotActivated(skill.clone()));
            }
        }

        // Get skill for source path
        let skill_data = registry.get(skill).await?;
        let skill_dir = skill_data.source_path.parent().unwrap_or(&skill_data.source_path);
        let ref_path = skill_dir.join(file);

        if !ref_path.exists() {
            return Err(DiscloseError::ReferenceNotFound {
                skill: skill.clone(),
                file: file.to_string(),
            });
        }

        // Read the file
        let content = std::fs::read_to_string(&ref_path)?;
        let tokens = self.estimator.estimate_cjk_aware(&content);

        // Check budget
        let available = self.tokens_remaining();
        if tokens > available {
            return Err(DiscloseError::BudgetExceeded {
                needed: tokens,
                available,
            });
        }

        // Update loaded skill
        if let Some(loaded) = self.loaded.get_mut(skill) {
            loaded.add_file(ref_path, tokens);
        }

        Ok(content)
    }

    /// Deactivate a skill, freeing context budget.
    pub fn deactivate(&mut self, name: &SkillName) {
        self.loaded.remove(name);
    }

    /// Deactivate all skills.
    pub fn deactivate_all(&mut self) {
        self.loaded.clear();
    }

    /// Current total token usage.
    pub fn tokens_used(&self) -> usize {
        self.catalog_tokens + self.loaded.values().map(|s| s.tokens_used).sum::<usize>()
    }

    /// Remaining budget.
    pub fn tokens_remaining(&self) -> usize {
        self.budget.max_tokens.saturating_sub(self.tokens_used())
    }

    /// Number of activated skills.
    pub fn activated_count(&self) -> usize {
        self.loaded
            .values()
            .filter(|s| s.level.has_instructions())
            .count()
    }

    /// Get loaded skills.
    pub fn loaded_skills(&self) -> Vec<&LoadedSkill> {
        self.loaded.values().collect()
    }

    /// Check if a skill is activated.
    pub fn is_activated(&self, name: &SkillName) -> bool {
        self.loaded
            .get(name)
            .map(|s| s.level.has_instructions())
            .unwrap_or(false)
    }
}

/// Find reference files in a skill directory.
fn find_references(skill_path: &std::path::Path) -> Vec<String> {
    let skill_dir = skill_path.parent().unwrap_or(skill_path);
    let mut refs = Vec::new();

    // Check references/ directory
    let refs_dir = skill_dir.join("references");
    if refs_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&refs_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    refs.push(format!("references/{}", name));
                }
            }
        }
    }

    // Check scripts/ directory
    let scripts_dir = skill_dir.join("scripts");
    if scripts_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    refs.push(format!("scripts/{}", name));
                }
            }
        }
    }

    refs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const TEST_SKILL: &str = r#"---
name: test-skill
description: A test skill
---

# Test Instructions

These are the instructions for the test skill.
They contain some content to test token counting.
"#;

    async fn setup_registry() -> (TempDir, SkillRegistry) {
        let temp = TempDir::new().unwrap();

        let skill_dir = temp.path().join("test-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), TEST_SKILL).unwrap();

        // Create references directory
        let refs_dir = skill_dir.join("references");
        fs::create_dir_all(&refs_dir).unwrap();
        fs::write(refs_dir.join("data.md"), "# Reference Data\n\nSome data here.").unwrap();

        let registry = SkillRegistry::new(&[temp.path()]).await.unwrap();
        (temp, registry)
    }

    #[tokio::test]
    async fn test_catalog_prompt() {
        let (_temp, registry) = setup_registry().await;
        let mut ctx = ContextManager::new(TokenBudget::default());

        let prompt = ctx.catalog_prompt(&registry).await;

        assert!(prompt.contains("Available skills:"));
        assert!(prompt.contains("test-skill"));
        assert!(ctx.catalog_tokens > 0);
    }

    #[tokio::test]
    async fn test_activate() {
        let (_temp, registry) = setup_registry().await;
        let mut ctx = ContextManager::new(TokenBudget::default());

        let name = SkillName::new("test-skill").unwrap();
        let payload = ctx.activate(&name, &registry).await.unwrap();

        assert_eq!(payload.skill_name, name);
        assert!(payload.instructions.contains("Test Instructions"));
        assert!(payload.tokens > 0);
        assert!(ctx.is_activated(&name));
    }

    #[tokio::test]
    async fn test_deactivate() {
        let (_temp, registry) = setup_registry().await;
        let mut ctx = ContextManager::new(TokenBudget::default());

        let name = SkillName::new("test-skill").unwrap();
        ctx.activate(&name, &registry).await.unwrap();
        assert!(ctx.is_activated(&name));

        ctx.deactivate(&name);
        assert!(!ctx.is_activated(&name));
    }

    #[tokio::test]
    async fn test_token_budget() {
        let (_temp, registry) = setup_registry().await;
        let budget = TokenBudget {
            max_tokens: 100,
            catalog_reserve: 10,
            per_skill_max: 50,
        };
        let mut ctx = ContextManager::new(budget);

        // Catalog uses some tokens
        ctx.catalog_prompt(&registry).await;
        assert!(ctx.tokens_used() > 0);
        assert!(ctx.tokens_remaining() < 100);
    }

    #[tokio::test]
    async fn test_load_reference() {
        let (_temp, registry) = setup_registry().await;
        let mut ctx = ContextManager::new(TokenBudget::default());

        let name = SkillName::new("test-skill").unwrap();
        ctx.activate(&name, &registry).await.unwrap();

        let content = ctx.load_reference(&name, "references/data.md", &registry).await.unwrap();

        assert!(content.contains("Reference Data"));
    }

    #[tokio::test]
    async fn test_load_reference_not_activated() {
        let (_temp, registry) = setup_registry().await;
        let mut ctx = ContextManager::new(TokenBudget::default());

        let name = SkillName::new("test-skill").unwrap();
        let result = ctx.load_reference(&name, "references/data.md", &registry).await;

        assert!(matches!(result, Err(DiscloseError::NotActivated(_))));
    }

    #[tokio::test]
    async fn test_budget_exceeded() {
        let (_temp, registry) = setup_registry().await;
        let budget = TokenBudget {
            max_tokens: 10, // Very small budget
            catalog_reserve: 5,
            per_skill_max: 5,
        };
        let mut ctx = ContextManager::new(budget);

        let name = SkillName::new("test-skill").unwrap();
        let result = ctx.activate(&name, &registry).await;

        assert!(matches!(result, Err(DiscloseError::BudgetExceeded { .. })));
    }
}
