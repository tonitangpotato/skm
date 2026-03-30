//! Error types for ase-enforce.

use ase_core::SkillName;
use thiserror::Error;

/// Errors from enforcement operations.
#[derive(Debug, Error)]
pub enum EnforceError {
    /// Policy file not found.
    #[error("Policy file not found: {0}")]
    PolicyNotFound(String),

    /// Invalid policy format.
    #[error("Invalid policy format: {0}")]
    InvalidPolicy(String),

    /// Hook execution failed.
    #[error("Hook execution failed: {0}")]
    HookFailed(String),

    /// Validation failed.
    #[error("Validation failed for {skill}: {reason}")]
    ValidationFailed { skill: SkillName, reason: String },

    /// Core error.
    #[error("Core error: {0}")]
    Core(#[from] ase_core::CoreError),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML parse error.
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// JSON parse error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = EnforceError::ValidationFailed {
            skill: SkillName::new("test").unwrap(),
            reason: "Output too long".into(),
        };
        assert!(err.to_string().contains("test"));
        assert!(err.to_string().contains("Output too long"));
    }
}
