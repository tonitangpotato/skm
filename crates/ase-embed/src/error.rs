//! Error types for ase-embed.

use thiserror::Error;

/// Errors from embedding operations.
#[derive(Debug, Error)]
pub enum EmbedError {
    /// Model initialization failed.
    #[error("Failed to initialize embedding model: {0}")]
    ModelInit(String),

    /// Model not found or could not be loaded.
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Embedding computation failed.
    #[error("Embedding failed: {0}")]
    Embedding(String),

    /// Dimension mismatch between embeddings.
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Index serialization/deserialization failed.
    #[error("Index serialization error: {0}")]
    Serialization(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// API error (for remote providers).
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    /// Rate limit exceeded.
    #[error("Rate limit exceeded, retry after {retry_after_secs} seconds")]
    RateLimit { retry_after_secs: u64 },

    /// Invalid API key.
    #[error("Invalid API key")]
    InvalidApiKey,

    /// Empty input.
    #[error("Cannot embed empty text")]
    EmptyInput,

    /// Batch too large.
    #[error("Batch size {size} exceeds maximum {max}")]
    BatchTooLarge { size: usize, max: usize },
}

impl EmbedError {
    /// Check if this is a retryable error.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimit { .. } => true,
            Self::Api { status, .. } if *status >= 500 => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = EmbedError::DimensionMismatch {
            expected: 1024,
            actual: 384,
        };
        assert!(err.to_string().contains("1024"));
        assert!(err.to_string().contains("384"));
    }

    #[test]
    fn test_retryable() {
        assert!(EmbedError::RateLimit { retry_after_secs: 60 }.is_retryable());
        assert!(EmbedError::Api {
            status: 500,
            message: "Server error".into()
        }
        .is_retryable());
        assert!(!EmbedError::InvalidApiKey.is_retryable());
        assert!(!EmbedError::Api {
            status: 400,
            message: "Bad request".into()
        }
        .is_retryable());
    }
}
