//! Error types for ase-core.

use std::path::PathBuf;
use thiserror::Error;

use crate::SkillName;

/// Core errors for skill operations.
#[derive(Debug, Error)]
pub enum CoreError {
    /// Invalid skill name.
    #[error("Invalid skill name: {0}")]
    InvalidName(String),

    /// Parse error in a SKILL.md file.
    #[error("Parse error in {path}: {reason}")]
    Parse { path: PathBuf, reason: String },

    /// Validation error.
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    /// Skill not found.
    #[error("Skill not found: {0}")]
    NotFound(SkillName),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Duplicate skill name.
    #[error("Duplicate skill name: {0}")]
    Duplicate(SkillName),

    /// Watcher error.
    #[error("Filesystem watcher error: {0}")]
    Watcher(#[from] notify::Error),
}

/// Validation errors for skill content.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// Skill name is empty.
    #[error("Skill name cannot be empty")]
    EmptyName,

    /// Skill name is too long.
    #[error("Skill name too long: {len} chars (max 64)")]
    NameTooLong { len: usize },

    /// Skill name contains invalid characters.
    #[error("Skill name contains invalid character '{ch}' at position {pos} (allowed: a-zA-Z0-9._-)")]
    InvalidNameChar { ch: char, pos: usize },

    /// Description is empty.
    #[error("Skill description cannot be empty")]
    EmptyDescription,

    /// Description is too long.
    #[error("Description too long: {len} chars (max 2000)")]
    DescriptionTooLong { len: usize },

    /// Required field missing.
    #[error("Required field missing: {field}")]
    MissingField { field: String },

    /// Invalid field value.
    #[error("Invalid value for field '{field}': {reason}")]
    InvalidFieldValue { field: String, reason: String },
}

/// Parse errors for SKILL.md files.
#[derive(Debug, Error)]
pub enum ParseError {
    /// File not found.
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    /// IO error while reading file.
    #[error("IO error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Missing frontmatter.
    #[error("Missing YAML frontmatter in {path}")]
    MissingFrontmatter { path: PathBuf },

    /// Invalid frontmatter.
    #[error("Invalid YAML frontmatter in {path}: {reason}")]
    InvalidFrontmatter { path: PathBuf, reason: String },

    /// Missing required field in frontmatter.
    #[error("Missing required field '{field}' in {path}")]
    MissingRequiredField { path: PathBuf, field: String },

    /// Validation error.
    #[error("Validation error in {path}: {source}")]
    Validation {
        path: PathBuf,
        #[source]
        source: ValidationError,
    },
}

impl ParseError {
    /// Get the path associated with this error, if any.
    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            Self::FileNotFound(p) => Some(p),
            Self::Io { path, .. } => Some(path),
            Self::MissingFrontmatter { path } => Some(path),
            Self::InvalidFrontmatter { path, .. } => Some(path),
            Self::MissingRequiredField { path, .. } => Some(path),
            Self::Validation { path, .. } => Some(path),
        }
    }
}

impl From<ParseError> for CoreError {
    fn from(err: ParseError) -> Self {
        let path = err.path().cloned().unwrap_or_default();
        CoreError::Parse {
            path,
            reason: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_error_display() {
        let err = ValidationError::EmptyName;
        assert_eq!(err.to_string(), "Skill name cannot be empty");

        let err = ValidationError::NameTooLong { len: 100 };
        assert_eq!(err.to_string(), "Skill name too long: 100 chars (max 64)");

        let err = ValidationError::InvalidNameChar { ch: '@', pos: 5 };
        assert!(err.to_string().contains("@"));
    }

    #[test]
    fn test_parse_error_path() {
        let path = PathBuf::from("/test/skill.md");
        let err = ParseError::MissingFrontmatter { path: path.clone() };
        assert_eq!(err.path(), Some(&path));
    }

    #[test]
    fn test_core_error_from_parse_error() {
        let err = ParseError::MissingFrontmatter {
            path: PathBuf::from("/test/skill.md"),
        };
        let core_err: CoreError = err.into();
        assert!(matches!(core_err, CoreError::Parse { .. }));
    }
}
