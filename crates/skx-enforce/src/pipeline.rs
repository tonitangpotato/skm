//! Complete enforcement pipeline.

use std::sync::Arc;
use std::time::SystemTime;

use skx_core::SkillName;

use crate::error::EnforceError;
use crate::hooks::{AfterSkillExecution, BeforeSkillActivation, EnforcementContext, HookDecision};
use crate::policy::{PolicyDecision, PolicyEngine};
use crate::validator::{OutputValidator, ValidationResult};

/// Audit entry for skill activation.
#[derive(Debug, Clone)]
pub struct ActivationAuditEntry {
    pub timestamp: SystemTime,
    pub skill: SkillName,
    pub query: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub decision: String,
    pub reason: Option<String>,
}

/// Audit entry for enforcement decisions.
#[derive(Debug, Clone)]
pub struct EnforcementAuditEntry {
    pub timestamp: SystemTime,
    pub skill: SkillName,
    pub action: String,
    pub decision: String,
    pub hooks_run: Vec<String>,
    pub policies_matched: Vec<String>,
}

/// Audit log trait for compliance tracking.
pub trait AuditLog: Send + Sync {
    /// Log an activation event.
    fn log_activation(&self, entry: ActivationAuditEntry);

    /// Log an enforcement event.
    fn log_enforcement(&self, entry: EnforcementAuditEntry);
}

/// In-memory audit log for testing.
pub struct InMemoryAuditLog {
    activations: std::sync::Mutex<Vec<ActivationAuditEntry>>,
    enforcements: std::sync::Mutex<Vec<EnforcementAuditEntry>>,
}

impl InMemoryAuditLog {
    pub fn new() -> Self {
        Self {
            activations: std::sync::Mutex::new(Vec::new()),
            enforcements: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn activations(&self) -> Vec<ActivationAuditEntry> {
        self.activations.lock().unwrap().clone()
    }

    pub fn enforcements(&self) -> Vec<EnforcementAuditEntry> {
        self.enforcements.lock().unwrap().clone()
    }
}

impl Default for InMemoryAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditLog for InMemoryAuditLog {
    fn log_activation(&self, entry: ActivationAuditEntry) {
        self.activations.lock().unwrap().push(entry);
    }

    fn log_enforcement(&self, entry: EnforcementAuditEntry) {
        self.enforcements.lock().unwrap().push(entry);
    }
}

/// Complete enforcement pipeline combining hooks, policies, and validators.
pub struct EnforcementPipeline {
    before_hooks: Vec<Box<dyn BeforeSkillActivation>>,
    after_hooks: Vec<Box<dyn AfterSkillExecution>>,
    policy_engine: PolicyEngine,
    output_validators: OutputValidator,
    audit_log: Option<Arc<dyn AuditLog>>,
}

impl EnforcementPipeline {
    /// Create a new pipeline builder.
    pub fn builder() -> EnforcementPipelineBuilder {
        EnforcementPipelineBuilder::new()
    }

    /// Run pre-activation checks.
    pub fn check_before(
        &self,
        skill: &SkillName,
        query: &str,
        ctx: &EnforcementContext,
    ) -> Result<HookDecision, EnforceError> {
        let mut hooks_run = Vec::new();
        let mut final_decision = HookDecision::Allow;

        // Run policy check first
        let policy_decision = self.policy_engine.evaluate(skill, ctx);

        if !policy_decision.allowed {
            self.log_activation(skill, query, ctx, "denied", policy_decision.reason.clone());
            return Ok(HookDecision::Cancel {
                reason: policy_decision
                    .reason
                    .unwrap_or_else(|| "Denied by policy".to_string()),
                suggest_alternative: None,
            });
        }

        if policy_decision.requires_approval {
            self.log_activation(skill, query, ctx, "requires_approval", None);
            return Ok(HookDecision::RequireApproval {
                reason: "Policy requires approval".to_string(),
                timeout: std::time::Duration::from_secs(300),
            });
        }

        // Run before hooks
        for hook in &self.before_hooks {
            hooks_run.push(hook.name().to_string());
            let decision = hook.before_activate(skill, query, ctx);

            match &decision {
                HookDecision::Cancel { .. } => {
                    self.log_enforcement(skill, "before_activate", "cancelled", &hooks_run, &policy_decision);
                    return Ok(decision);
                }
                HookDecision::RequireApproval { .. } => {
                    self.log_enforcement(skill, "before_activate", "requires_approval", &hooks_run, &policy_decision);
                    return Ok(decision);
                }
                HookDecision::Modify(modified) => {
                    final_decision = HookDecision::Modify(modified.clone());
                }
                HookDecision::Allow => {}
            }
        }

        self.log_activation(skill, query, ctx, "allowed", None);
        self.log_enforcement(skill, "before_activate", "allowed", &hooks_run, &policy_decision);

        Ok(final_decision)
    }

    /// Run post-execution checks.
    pub fn check_after(
        &self,
        skill: &SkillName,
        output: &str,
        ctx: &EnforcementContext,
    ) -> Result<HookDecision, EnforceError> {
        let mut hooks_run = Vec::new();
        let mut current_output = output.to_string();

        // Run after hooks
        for hook in &self.after_hooks {
            hooks_run.push(hook.name().to_string());
            let decision = hook.after_execute(skill, &current_output, ctx);

            match decision {
                HookDecision::Cancel { reason, suggest_alternative } => {
                    return Ok(HookDecision::Cancel { reason, suggest_alternative });
                }
                HookDecision::Modify(modified) => {
                    current_output = modified;
                }
                HookDecision::Allow => {}
                HookDecision::RequireApproval { reason, timeout } => {
                    return Ok(HookDecision::RequireApproval { reason, timeout });
                }
            }
        }

        // Run validators
        let validation = self.output_validators.validate(skill, &current_output);
        match validation {
            ValidationResult::Valid => {}
            ValidationResult::Invalid { errors } => {
                return Err(EnforceError::ValidationFailed {
                    skill: skill.clone(),
                    reason: errors.join("; "),
                });
            }
            ValidationResult::Repaired { output: repaired, warnings } => {
                tracing::warn!("Output repaired for {}: {:?}", skill, warnings);
                current_output = repaired;
            }
        }

        // Return final output
        if current_output != output {
            Ok(HookDecision::Modify(current_output))
        } else {
            Ok(HookDecision::Allow)
        }
    }

    fn log_activation(
        &self,
        skill: &SkillName,
        query: &str,
        ctx: &EnforcementContext,
        decision: &str,
        reason: Option<String>,
    ) {
        if let Some(ref log) = self.audit_log {
            log.log_activation(ActivationAuditEntry {
                timestamp: SystemTime::now(),
                skill: skill.clone(),
                query: query.to_string(),
                user_id: ctx.user_id.clone(),
                session_id: ctx.session_id.clone(),
                decision: decision.to_string(),
                reason,
            });
        }
    }

    fn log_enforcement(
        &self,
        skill: &SkillName,
        action: &str,
        decision: &str,
        hooks_run: &[String],
        policy_decision: &PolicyDecision,
    ) {
        if let Some(ref log) = self.audit_log {
            log.log_enforcement(EnforcementAuditEntry {
                timestamp: SystemTime::now(),
                skill: skill.clone(),
                action: action.to_string(),
                decision: decision.to_string(),
                hooks_run: hooks_run.to_vec(),
                policies_matched: policy_decision.matched_rules.clone(),
            });
        }
    }
}

/// Builder for EnforcementPipeline.
pub struct EnforcementPipelineBuilder {
    before_hooks: Vec<Box<dyn BeforeSkillActivation>>,
    after_hooks: Vec<Box<dyn AfterSkillExecution>>,
    policy_engine: Option<PolicyEngine>,
    output_validators: OutputValidator,
    audit_log: Option<Arc<dyn AuditLog>>,
}

impl EnforcementPipelineBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            before_hooks: Vec::new(),
            after_hooks: Vec::new(),
            policy_engine: None,
            output_validators: OutputValidator::new(),
            audit_log: None,
        }
    }

    /// Add a before-activation hook.
    pub fn with_before_hook(mut self, hook: Box<dyn BeforeSkillActivation>) -> Self {
        self.before_hooks.push(hook);
        self
    }

    /// Add an after-execution hook.
    pub fn with_after_hook(mut self, hook: Box<dyn AfterSkillExecution>) -> Self {
        self.after_hooks.push(hook);
        self
    }

    /// Set the policy engine.
    pub fn with_policy_engine(mut self, engine: PolicyEngine) -> Self {
        self.policy_engine = Some(engine);
        self
    }

    /// Set the output validators.
    pub fn with_validators(mut self, validators: OutputValidator) -> Self {
        self.output_validators = validators;
        self
    }

    /// Set the audit log.
    pub fn with_audit_log(mut self, log: Arc<dyn AuditLog>) -> Self {
        self.audit_log = Some(log);
        self
    }

    /// Build the pipeline.
    pub fn build(self) -> EnforcementPipeline {
        EnforcementPipeline {
            before_hooks: self.before_hooks,
            after_hooks: self.after_hooks,
            policy_engine: self.policy_engine.unwrap_or_else(PolicyEngine::allow_all),
            output_validators: self.output_validators,
            audit_log: self.audit_log,
        }
    }
}

impl Default for EnforcementPipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{Policy, PolicyAction, PolicyRule};

    struct AllowAllHook;
    impl BeforeSkillActivation for AllowAllHook {
        fn before_activate(&self, _: &SkillName, _: &str, _: &EnforcementContext) -> HookDecision {
            HookDecision::Allow
        }
        fn name(&self) -> &str {
            "allow-all"
        }
    }

    struct DenyHook;
    impl BeforeSkillActivation for DenyHook {
        fn before_activate(&self, _: &SkillName, _: &str, _: &EnforcementContext) -> HookDecision {
            HookDecision::cancel("Denied by hook")
        }
        fn name(&self) -> &str {
            "deny-hook"
        }
    }

    struct RedactHook;
    impl AfterSkillExecution for RedactHook {
        fn after_execute(&self, _: &SkillName, output: &str, _: &EnforcementContext) -> HookDecision {
            if output.contains("secret") {
                HookDecision::modify(output.replace("secret", "[REDACTED]"))
            } else {
                HookDecision::allow()
            }
        }
        fn name(&self) -> &str {
            "redact-hook"
        }
    }

    #[test]
    fn test_pipeline_allow() {
        let pipeline = EnforcementPipeline::builder()
            .with_before_hook(Box::new(AllowAllHook))
            .build();

        let skill = SkillName::new("test").unwrap();
        let ctx = EnforcementContext::new();

        let decision = pipeline.check_before(&skill, "query", &ctx).unwrap();
        assert!(decision.is_allowed());
    }

    #[test]
    fn test_pipeline_deny_by_hook() {
        let pipeline = EnforcementPipeline::builder()
            .with_before_hook(Box::new(DenyHook))
            .build();

        let skill = SkillName::new("test").unwrap();
        let ctx = EnforcementContext::new();

        let decision = pipeline.check_before(&skill, "query", &ctx).unwrap();
        assert!(decision.is_cancelled());
    }

    #[test]
    fn test_pipeline_deny_by_policy() {
        let policy = Policy {
            name: "deny-all".to_string(),
            rules: vec![PolicyRule {
                skill_pattern: "*".to_string(),
                action: PolicyAction::Deny {
                    reason: "All denied".to_string(),
                },
                conditions: vec![],
            }],
        };

        let pipeline = EnforcementPipeline::builder()
            .with_policy_engine(PolicyEngine::new(vec![policy]))
            .build();

        let skill = SkillName::new("test").unwrap();
        let ctx = EnforcementContext::new();

        let decision = pipeline.check_before(&skill, "query", &ctx).unwrap();
        assert!(decision.is_cancelled());
    }

    #[test]
    fn test_pipeline_after_modify() {
        let pipeline = EnforcementPipeline::builder()
            .with_after_hook(Box::new(RedactHook))
            .build();

        let skill = SkillName::new("test").unwrap();
        let ctx = EnforcementContext::new();

        let decision = pipeline.check_after(&skill, "contains secret data", &ctx).unwrap();
        if let HookDecision::Modify(modified) = decision {
            assert!(modified.contains("[REDACTED]"));
            assert!(!modified.contains("secret"));
        } else {
            panic!("Expected Modify decision");
        }
    }

    #[test]
    fn test_pipeline_audit_log() {
        let audit_log = Arc::new(InMemoryAuditLog::new());

        let pipeline = EnforcementPipeline::builder()
            .with_audit_log(audit_log.clone())
            .build();

        let skill = SkillName::new("test").unwrap();
        let ctx = EnforcementContext::new().with_user("user1");

        pipeline.check_before(&skill, "test query", &ctx).unwrap();

        let activations = audit_log.activations();
        assert_eq!(activations.len(), 1);
        assert_eq!(activations[0].skill, skill);
        assert_eq!(activations[0].user_id, Some("user1".to_string()));
    }
}
