//! Error types for skm-learn.

use thiserror::Error;

/// Errors from learning operations.
#[derive(Debug, Error)]
pub enum LearnError {
    /// Test harness error.
    #[error("Test harness error: {0}")]
    Harness(String),

    /// Metrics error.
    #[error("Metrics error: {0}")]
    Metrics(String),

    /// Optimizer error.
    #[error("Optimizer error: {0}")]
    Optimizer(String),

    /// Analytics error.
    #[error("Analytics error: {0}")]
    Analytics(String),

    /// Core error.
    #[error("Core error: {0}")]
    Core(#[from] skm_core::CoreError),

    /// Select error.
    #[error("Selection error: {0}")]
    Select(#[from] skm_select::SelectError),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML error.
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// JSON error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// SQLite error.
    #[cfg(feature = "analytics-sqlite")]
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = LearnError::Harness("test failed".into());
        assert!(err.to_string().contains("test failed"));
    }
}
