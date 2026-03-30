//! Declarative policy rules for skill access control.

use std::path::Path;

use glob::Pattern;
use serde::{Deserialize, Serialize};

use skm_core::SkillName;

use crate::error::EnforceError;
use crate::hooks::EnforcementContext;

/// A declarative policy with rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    /// Policy name.
    pub name: String,

    /// Policy rules.
    pub rules: Vec<PolicyRule>,
}

/// A single policy rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    /// Glob pattern matching skill names.
    pub skill_pattern: String,

    /// Action to take when matched.
    pub action: PolicyAction,

    /// Conditions that must be met.
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

/// Action to take when a policy rule matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAction {
    /// Allow the skill.
    Allow,

    /// Deny the skill with a reason.
    Deny { reason: String },

    /// Require human approval.
    RequireApproval,

    /// Rate limit the skill.
    RateLimit { max_per_minute: u32 },
}

/// Condition for a policy rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    /// User must be in the list.
    UserIn(Vec<String>),

    /// User must not be in the list.
    UserNotIn(Vec<String>),

    /// Time must be within window (HH:MM format).
    TimeWindow { start: String, end: String },

    /// Skill must have this tag.
    SkillTagIs(String),

    /// Skill must not have this tag.
    SkillTagNot(String),

    /// Custom condition with key-value check.
    Custom { key: String, value: String },
}

/// Result of policy evaluation.
#[derive(Debug)]
pub struct PolicyDecision {
    /// Whether the skill is allowed.
    pub allowed: bool,

    /// Rules that matched.
    pub matched_rules: Vec<String>,

    /// Reason if denied.
    pub reason: Option<String>,

    /// Requires approval.
    pub requires_approval: bool,

    /// Rate limit if applicable.
    pub rate_limit: Option<u32>,
}

impl PolicyDecision {
    fn allow() -> Self {
        Self {
            allowed: true,
            matched_rules: Vec::new(),
            reason: None,
            requires_approval: false,
            rate_limit: None,
        }
    }

    fn deny(reason: String) -> Self {
        Self {
            allowed: false,
            matched_rules: Vec::new(),
            reason: Some(reason),
            requires_approval: false,
            rate_limit: None,
        }
    }
}

/// Policy engine evaluates rules against context.
pub struct PolicyEngine {
    policies: Vec<Policy>,
}

impl PolicyEngine {
    /// Create a new policy engine.
    pub fn new(policies: Vec<Policy>) -> Self {
        Self { policies }
    }

    /// Create an empty policy engine (allows all).
    pub fn allow_all() -> Self {
        Self {
            policies: Vec::new(),
        }
    }

    /// Load policies from a YAML file.
    pub fn from_file(path: &Path) -> Result<Self, EnforceError> {
        let content = std::fs::read_to_string(path)?;
        let policies: Vec<Policy> = serde_yaml::from_str(&content)?;
        Ok(Self::new(policies))
    }

    /// Add a policy.
    pub fn add_policy(&mut self, policy: Policy) {
        self.policies.push(policy);
    }

    /// Evaluate whether a skill activation is allowed.
    pub fn evaluate(&self, skill: &SkillName, ctx: &EnforcementContext) -> PolicyDecision {
        let mut decision = PolicyDecision::allow();

        for policy in &self.policies {
            for rule in &policy.rules {
                // Check if skill pattern matches
                if !matches_skill_pattern(&rule.skill_pattern, skill) {
                    continue;
                }

                // Check conditions
                if !evaluate_conditions(&rule.conditions, ctx) {
                    continue;
                }

                // Rule matches - apply action
                decision
                    .matched_rules
                    .push(format!("{}:{}", policy.name, rule.skill_pattern));

                match &rule.action {
                    PolicyAction::Allow => {
                        // Explicit allow
                    }
                    PolicyAction::Deny { reason } => {
                        decision.allowed = false;
                        decision.reason = Some(reason.clone());
                        return decision; // Deny is final
                    }
                    PolicyAction::RequireApproval => {
                        decision.requires_approval = true;
                    }
                    PolicyAction::RateLimit { max_per_minute } => {
                        decision.rate_limit = Some(*max_per_minute);
                    }
                }
            }
        }

        decision
    }

    /// Get all loaded policies.
    pub fn policies(&self) -> &[Policy] {
        &self.policies
    }
}

/// Check if a skill name matches a glob pattern.
fn matches_skill_pattern(pattern: &str, skill: &SkillName) -> bool {
    if pattern == "*" {
        return true;
    }

    // Try as glob pattern
    if let Ok(glob) = Pattern::new(pattern) {
        return glob.matches(skill.as_str());
    }

    // Fall back to exact match
    pattern == skill.as_str()
}

/// Evaluate conditions against context.
fn evaluate_conditions(conditions: &[Condition], ctx: &EnforcementContext) -> bool {
    for condition in conditions {
        match condition {
            Condition::UserIn(users) => {
                if let Some(ref user_id) = ctx.user_id {
                    if !users.contains(user_id) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            Condition::UserNotIn(users) => {
                if let Some(ref user_id) = ctx.user_id {
                    if users.contains(user_id) {
                        return false;
                    }
                }
            }
            Condition::TimeWindow { start, end } => {
                // Simplified time check - in production would parse HH:MM
                let _ = (start, end);
                // For now, always pass
            }
            Condition::SkillTagIs(_tag) => {
                // Would need skill metadata to check
                // For now, always pass
            }
            Condition::SkillTagNot(_tag) => {
                // Would need skill metadata to check
                // For now, always pass
            }
            Condition::Custom { key, value } => {
                if let Some(ctx_value) = ctx.custom.get(key) {
                    if ctx_value.as_str() != Some(value.as_str()) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_policy() -> Policy {
        Policy {
            name: "test-policy".to_string(),
            rules: vec![
                PolicyRule {
                    skill_pattern: "admin-*".to_string(),
                    action: PolicyAction::Deny {
                        reason: "Admin skills are restricted".to_string(),
                    },
                    conditions: vec![],
                },
                PolicyRule {
                    skill_pattern: "dangerous-*".to_string(),
                    action: PolicyAction::RequireApproval,
                    conditions: vec![],
                },
                PolicyRule {
                    skill_pattern: "api-*".to_string(),
                    action: PolicyAction::RateLimit { max_per_minute: 10 },
                    conditions: vec![],
                },
            ],
        }
    }

    #[test]
    fn test_policy_allow() {
        let engine = PolicyEngine::new(vec![test_policy()]);
        let ctx = EnforcementContext::new();

        let skill = SkillName::new("safe-skill").unwrap();
        let decision = engine.evaluate(&skill, &ctx);

        assert!(decision.allowed);
        assert!(decision.reason.is_none());
    }

    #[test]
    fn test_policy_deny() {
        let engine = PolicyEngine::new(vec![test_policy()]);
        let ctx = EnforcementContext::new();

        let skill = SkillName::new("admin-delete").unwrap();
        let decision = engine.evaluate(&skill, &ctx);

        assert!(!decision.allowed);
        assert!(decision.reason.is_some());
    }

    #[test]
    fn test_policy_require_approval() {
        let engine = PolicyEngine::new(vec![test_policy()]);
        let ctx = EnforcementContext::new();

        let skill = SkillName::new("dangerous-action").unwrap();
        let decision = engine.evaluate(&skill, &ctx);

        assert!(decision.allowed);
        assert!(decision.requires_approval);
    }

    #[test]
    fn test_policy_rate_limit() {
        let engine = PolicyEngine::new(vec![test_policy()]);
        let ctx = EnforcementContext::new();

        let skill = SkillName::new("api-call").unwrap();
        let decision = engine.evaluate(&skill, &ctx);

        assert!(decision.allowed);
        assert_eq!(decision.rate_limit, Some(10));
    }

    #[test]
    fn test_condition_user_in() {
        let policy = Policy {
            name: "user-policy".to_string(),
            rules: vec![PolicyRule {
                skill_pattern: "*".to_string(),
                action: PolicyAction::Allow,
                conditions: vec![Condition::UserIn(vec!["admin".to_string()])],
            }],
        };

        let engine = PolicyEngine::new(vec![policy]);

        // Without user - condition fails
        let ctx = EnforcementContext::new();
        let skill = SkillName::new("test").unwrap();
        let decision = engine.evaluate(&skill, &ctx);
        assert!(decision.matched_rules.is_empty()); // Rule didn't match

        // With correct user
        let ctx = EnforcementContext::new().with_user("admin");
        let decision = engine.evaluate(&skill, &ctx);
        assert!(!decision.matched_rules.is_empty());
    }

    #[test]
    fn test_allow_all() {
        let engine = PolicyEngine::allow_all();
        let ctx = EnforcementContext::new();
        let skill = SkillName::new("anything").unwrap();

        let decision = engine.evaluate(&skill, &ctx);
        assert!(decision.allowed);
    }

    #[test]
    fn test_glob_pattern() {
        assert!(matches_skill_pattern("*", &SkillName::new("anything").unwrap()));
        assert!(matches_skill_pattern("test-*", &SkillName::new("test-skill").unwrap()));
        assert!(!matches_skill_pattern("test-*", &SkillName::new("other-skill").unwrap()));
        assert!(matches_skill_pattern("*-skill", &SkillName::new("test-skill").unwrap()));
    }
}
