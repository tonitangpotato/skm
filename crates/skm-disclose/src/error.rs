//! Error types for skm-disclose.

use skm_core::SkillName;
use thiserror::Error;

/// Errors from disclosure operations.
#[derive(Debug, Error)]
pub enum DiscloseError {
    /// Skill not found.
    #[error("Skill not found: {0}")]
    SkillNotFound(SkillName),

    /// Token budget exceeded.
    #[error("Token budget exceeded: need {needed}, have {available}")]
    BudgetExceeded { needed: usize, available: usize },

    /// Skill not activated.
    #[error("Skill not activated: {0}")]
    NotActivated(SkillName),

    /// Reference file not found.
    #[error("Reference file not found: {skill}/{file}")]
    ReferenceNotFound { skill: SkillName, file: String },

    /// Core error.
    #[error("Core error: {0}")]
    Core(#[from] skm_core::CoreError),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = DiscloseError::BudgetExceeded {
            needed: 5000,
            available: 3000,
        };
        assert!(err.to_string().contains("5000"));
        assert!(err.to_string().contains("3000"));
    }
}
