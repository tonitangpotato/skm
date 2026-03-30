//! Error types for skx-select.

use thiserror::Error;

/// Errors from selection operations.
#[derive(Debug, Error)]
pub enum SelectError {
    /// Strategy initialization failed.
    #[error("Strategy initialization failed: {0}")]
    StrategyInit(String),

    /// Selection failed.
    #[error("Selection failed: {0}")]
    Selection(String),

    /// Timeout exceeded.
    #[error("Selection timeout exceeded: {0}")]
    Timeout(String),

    /// Embedding error.
    #[error("Embedding error: {0}")]
    Embedding(#[from] skx_embed::EmbedError),

    /// Core error.
    #[error("Core error: {0}")]
    Core(#[from] skx_core::CoreError),

    /// LLM error.
    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),

    /// Regex error.
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
}

/// LLM-specific errors.
#[derive(Debug, Error)]
pub enum LlmError {
    /// API request failed.
    #[error("LLM API request failed: {0}")]
    Api(String),

    /// Parse error from LLM response.
    #[error("Failed to parse LLM response: {0}")]
    ParseError(String),

    /// Timeout.
    #[error("LLM request timed out")]
    Timeout,

    /// Rate limit.
    #[error("LLM rate limit exceeded")]
    RateLimit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = SelectError::Timeout("5s exceeded".into());
        assert!(err.to_string().contains("5s"));
    }

    #[test]
    fn test_llm_error() {
        let err = LlmError::ParseError("invalid JSON".into());
        assert!(err.to_string().contains("invalid JSON"));
    }
}
