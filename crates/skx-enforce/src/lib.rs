//! # skx-enforce
//!
//! Execution guardrails and policy engine for Agent Skills.
//!
//! This crate provides:
//! - `BeforeSkillActivation` and `AfterSkillExecution` hook traits
//! - `HookDecision` for allow/modify/cancel/require-approval
//! - `Policy` and `PolicyEngine` for declarative access control
//! - `Validator` trait with JSON schema, regex, length validators
//! - `EnforcementPipeline` combining all enforcement mechanisms

mod error;
mod hooks;
mod policy;
mod validator;
mod pipeline;

pub use error::EnforceError;
pub use hooks::{
    AfterSkillExecution, BeforeSkillActivation, EnforcementContext, HookDecision,
};
pub use policy::{Condition, Policy, PolicyAction, PolicyDecision, PolicyEngine, PolicyRule};
pub use validator::{
    JsonSchemaValidator, LengthValidator, OutputValidator, RegexValidator, ValidationResult,
    Validator,
};
pub use pipeline::{
    ActivationAuditEntry, AuditLog, EnforcementAuditEntry, EnforcementPipeline,
    EnforcementPipelineBuilder,
};
