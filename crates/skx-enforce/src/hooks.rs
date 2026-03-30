//! Hook traits for skill activation and execution.

use std::collections::HashMap;
use std::time::Duration;

use skx_core::SkillName;

/// Context available to enforcement hooks.
#[derive(Debug, Clone, Default)]
pub struct EnforcementContext {
    /// User identifier (if available).
    pub user_id: Option<String>,

    /// Session identifier (if available).
    pub session_id: Option<String>,

    /// Recent conversation history.
    pub conversation_history: Vec<String>,

    /// Active policy names.
    pub active_policies: Vec<String>,

    /// Custom context data.
    pub custom: HashMap<String, serde_json::Value>,
}

impl EnforcementContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set user ID.
    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set session ID.
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Add conversation history.
    pub fn with_history(mut self, history: Vec<String>) -> Self {
        self.conversation_history = history;
        self
    }

    /// Add custom context data.
    pub fn with_custom(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.custom.insert(key.into(), value);
        self
    }
}

/// Decision returned by a hook.
#[derive(Debug, Clone)]
pub enum HookDecision {
    /// Allow the operation to proceed.
    Allow,

    /// Allow, but modify the input/output.
    Modify(String),

    /// Cancel the operation with a reason.
    /// The LLM receives this message instead of executing.
    Cancel {
        reason: String,
        suggest_alternative: Option<SkillName>,
    },

    /// Require human approval before proceeding.
    RequireApproval { reason: String, timeout: Duration },
}

impl HookDecision {
    /// Create an Allow decision.
    pub fn allow() -> Self {
        Self::Allow
    }

    /// Create a Modify decision.
    pub fn modify(content: impl Into<String>) -> Self {
        Self::Modify(content.into())
    }

    /// Create a Cancel decision.
    pub fn cancel(reason: impl Into<String>) -> Self {
        Self::Cancel {
            reason: reason.into(),
            suggest_alternative: None,
        }
    }

    /// Create a Cancel decision with an alternative suggestion.
    pub fn cancel_with_alternative(reason: impl Into<String>, alternative: SkillName) -> Self {
        Self::Cancel {
            reason: reason.into(),
            suggest_alternative: Some(alternative),
        }
    }

    /// Create a RequireApproval decision.
    pub fn require_approval(reason: impl Into<String>, timeout: Duration) -> Self {
        Self::RequireApproval {
            reason: reason.into(),
            timeout,
        }
    }

    /// Check if this is an allow decision.
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow | Self::Modify(_))
    }

    /// Check if this is a cancel decision.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancel { .. })
    }

    /// Check if this requires approval.
    pub fn requires_approval(&self) -> bool {
        matches!(self, Self::RequireApproval { .. })
    }
}

/// Hook that runs BEFORE a skill is activated.
/// Can inspect, modify, or cancel the activation.
pub trait BeforeSkillActivation: Send + Sync {
    /// Called before a skill is activated.
    fn before_activate(
        &self,
        skill: &SkillName,
        query: &str,
        ctx: &EnforcementContext,
    ) -> HookDecision;

    /// Hook name for logging.
    fn name(&self) -> &str {
        "unnamed-before-hook"
    }
}

/// Hook that runs AFTER a skill produces output.
/// Can inspect, modify, or reject the output.
pub trait AfterSkillExecution: Send + Sync {
    /// Called after a skill executes.
    fn after_execute(
        &self,
        skill: &SkillName,
        output: &str,
        ctx: &EnforcementContext,
    ) -> HookDecision;

    /// Hook name for logging.
    fn name(&self) -> &str {
        "unnamed-after-hook"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestBeforeHook;

    impl BeforeSkillActivation for TestBeforeHook {
        fn before_activate(
            &self,
            skill: &SkillName,
            _query: &str,
            _ctx: &EnforcementContext,
        ) -> HookDecision {
            if skill.as_str() == "dangerous-skill" {
                HookDecision::cancel("This skill is dangerous")
            } else {
                HookDecision::allow()
            }
        }

        fn name(&self) -> &str {
            "test-before-hook"
        }
    }

    struct TestAfterHook;

    impl AfterSkillExecution for TestAfterHook {
        fn after_execute(
            &self,
            _skill: &SkillName,
            output: &str,
            _ctx: &EnforcementContext,
        ) -> HookDecision {
            if output.contains("secret") {
                HookDecision::modify(output.replace("secret", "[REDACTED]"))
            } else {
                HookDecision::allow()
            }
        }

        fn name(&self) -> &str {
            "test-after-hook"
        }
    }

    #[test]
    fn test_hook_decision_allow() {
        let decision = HookDecision::allow();
        assert!(decision.is_allowed());
        assert!(!decision.is_cancelled());
    }

    #[test]
    fn test_hook_decision_cancel() {
        let decision = HookDecision::cancel("Not allowed");
        assert!(!decision.is_allowed());
        assert!(decision.is_cancelled());
    }

    #[test]
    fn test_hook_decision_require_approval() {
        let decision = HookDecision::require_approval("Needs review", Duration::from_secs(300));
        assert!(!decision.is_allowed());
        assert!(decision.requires_approval());
    }

    #[test]
    fn test_before_hook() {
        let hook = TestBeforeHook;
        let ctx = EnforcementContext::new();

        let safe = SkillName::new("safe-skill").unwrap();
        let dangerous = SkillName::new("dangerous-skill").unwrap();

        assert!(hook.before_activate(&safe, "query", &ctx).is_allowed());
        assert!(hook.before_activate(&dangerous, "query", &ctx).is_cancelled());
    }

    #[test]
    fn test_after_hook() {
        let hook = TestAfterHook;
        let ctx = EnforcementContext::new();
        let skill = SkillName::new("test").unwrap();

        let decision = hook.after_execute(&skill, "normal output", &ctx);
        assert!(decision.is_allowed());

        let decision = hook.after_execute(&skill, "contains secret data", &ctx);
        if let HookDecision::Modify(modified) = decision {
            assert!(modified.contains("[REDACTED]"));
            assert!(!modified.contains("secret"));
        } else {
            panic!("Expected Modify decision");
        }
    }

    #[test]
    fn test_enforcement_context() {
        let ctx = EnforcementContext::new()
            .with_user("user123")
            .with_session("session456")
            .with_custom("key", serde_json::json!("value"));

        assert_eq!(ctx.user_id, Some("user123".to_string()));
        assert_eq!(ctx.session_id, Some("session456".to_string()));
        assert!(ctx.custom.contains_key("key"));
    }
}
