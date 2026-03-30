//! SKILL.md parser for YAML frontmatter + markdown body.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::error::{ParseError, ValidationError};
use crate::schema::{Skill, SkillMetadata, SkillName};

/// Parser for SKILL.md files.
///
/// SKILL.md format:
/// ```text
/// ---
/// name: skill-name
/// description: A brief description of what this skill does
/// license: MIT
/// metadata:
///   triggers: "keyword1, keyword2"
///   tags: "category1, category2"
/// ---
///
/// # Skill Instructions
///
/// The markdown body with actual instructions...
/// ```
#[derive(Debug, Clone)]
pub struct SkillParser {
    /// If true, reject skills with invalid frontmatter.
    /// If false, attempt to recover and use defaults where possible.
    strict: bool,
}

/// Raw frontmatter structure for deserialization.
#[derive(Debug, Deserialize)]
struct RawFrontmatter {
    name: String,
    description: String,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    compatibility: Option<String>,
    #[serde(default)]
    metadata: HashMap<String, String>,
}

impl Default for SkillParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillParser {
    /// Create a new parser with default settings (strict mode off).
    pub fn new() -> Self {
        Self { strict: false }
    }

    /// Create a parser with strict mode enabled.
    pub fn strict() -> Self {
        Self { strict: true }
    }

    /// Set strict mode.
    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    /// Parse a SKILL.md file into a Skill struct.
    pub fn parse_file(&self, path: &Path) -> Result<Skill, ParseError> {
        // Check file exists
        if !path.exists() {
            return Err(ParseError::FileNotFound(path.to_path_buf()));
        }

        // Read file content
        let content = fs::read_to_string(path).map_err(|e| ParseError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        // Parse content
        let mut skill = self.parse_str(&content).map_err(|e| match e {
            ParseError::MissingFrontmatter { .. } => ParseError::MissingFrontmatter {
                path: path.to_path_buf(),
            },
            ParseError::InvalidFrontmatter { reason, .. } => ParseError::InvalidFrontmatter {
                path: path.to_path_buf(),
                reason,
            },
            ParseError::MissingRequiredField { field, .. } => ParseError::MissingRequiredField {
                path: path.to_path_buf(),
                field,
            },
            ParseError::Validation { source, .. } => ParseError::Validation {
                path: path.to_path_buf(),
                source,
            },
            _ => e,
        })?;

        // Set source path
        skill.source_path = path.to_path_buf();

        Ok(skill)
    }

    /// Parse SKILL.md content from a string.
    pub fn parse_str(&self, content: &str) -> Result<Skill, ParseError> {
        let (frontmatter_str, body) = self.split_frontmatter(content)?;
        let raw = self.parse_frontmatter(&frontmatter_str)?;

        // Validate and convert name
        let name = SkillName::new(&raw.name).map_err(|e| ParseError::Validation {
            path: Default::default(),
            source: e,
        })?;

        // Validate description
        if raw.description.is_empty() {
            if self.strict {
                return Err(ParseError::Validation {
                    path: Default::default(),
                    source: ValidationError::EmptyDescription,
                });
            }
        }

        if raw.description.len() > 2000 {
            return Err(ParseError::Validation {
                path: Default::default(),
                source: ValidationError::DescriptionTooLong {
                    len: raw.description.len(),
                },
            });
        }

        Ok(Skill {
            name,
            description: raw.description,
            license: raw.license,
            compatibility: raw.compatibility,
            metadata: raw.metadata,
            instructions: body.to_string(),
            source_path: Default::default(),
        })
    }

    /// Parse only frontmatter (name + description + metadata).
    /// Used for catalog-level loading without parsing the full body.
    pub fn parse_metadata(&self, path: &Path) -> Result<SkillMetadata, ParseError> {
        // Check file exists
        if !path.exists() {
            return Err(ParseError::FileNotFound(path.to_path_buf()));
        }

        // Read file content
        let content = fs::read_to_string(path).map_err(|e| ParseError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        // Compute content hash
        let content_hash = xxhash_rust::xxh64::xxh64(content.as_bytes(), 0);

        // Parse skill
        let skill = self.parse_str(&content).map_err(|e| match e {
            ParseError::MissingFrontmatter { .. } => ParseError::MissingFrontmatter {
                path: path.to_path_buf(),
            },
            ParseError::InvalidFrontmatter { reason, .. } => ParseError::InvalidFrontmatter {
                path: path.to_path_buf(),
                reason,
            },
            _ => e,
        })?;

        let tags = skill.tags();
        let triggers = skill.triggers();
        let estimated_tokens = skill.estimated_tokens();
        
        Ok(SkillMetadata {
            name: skill.name,
            description: skill.description,
            tags,
            triggers,
            source_path: path.to_path_buf(),
            content_hash,
            estimated_tokens,
        })
    }

    /// Split content into frontmatter and body.
    fn split_frontmatter<'a>(&self, content: &'a str) -> Result<(&'a str, &'a str), ParseError> {
        let content = content.trim_start();

        // Check for opening delimiter
        if !content.starts_with("---") {
            return Err(ParseError::MissingFrontmatter {
                path: Default::default(),
            });
        }

        // Find the closing delimiter
        let after_open = &content[3..];
        let close_pos = after_open.find("\n---").or_else(|| after_open.find("\r\n---"));

        match close_pos {
            Some(pos) => {
                let frontmatter = &after_open[..pos].trim();
                // Skip past the closing "---" and any trailing newline
                let after_close = &after_open[pos + 4..];
                let body = after_close.trim_start_matches(['\n', '\r']);
                Ok((frontmatter, body))
            }
            None => Err(ParseError::MissingFrontmatter {
                path: Default::default(),
            }),
        }
    }

    /// Parse the YAML frontmatter string.
    fn parse_frontmatter(&self, yaml: &str) -> Result<RawFrontmatter, ParseError> {
        serde_yaml::from_str(yaml).map_err(|e| ParseError::InvalidFrontmatter {
            path: Default::default(),
            reason: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    const VALID_SKILL: &str = r#"---
name: pdf-processing
description: Extract text and tables from PDF files
license: MIT
metadata:
  triggers: "pdf, .pdf, extract text from pdf"
  tags: "document, extraction"
---

# PDF Processing Skill

This skill helps you work with PDF files.

## Usage

1. Upload a PDF
2. Ask to extract text or tables
"#;

    const MINIMAL_SKILL: &str = r#"---
name: minimal
description: A minimal skill
---

Instructions here.
"#;

    #[test]
    fn test_parse_valid_skill() {
        let parser = SkillParser::new();
        let skill = parser.parse_str(VALID_SKILL).unwrap();

        assert_eq!(skill.name.as_str(), "pdf-processing");
        assert_eq!(skill.description, "Extract text and tables from PDF files");
        assert_eq!(skill.license, Some("MIT".to_string()));
        assert!(skill.instructions.contains("PDF Processing Skill"));
        assert!(skill.instructions.contains("Upload a PDF"));
    }

    #[test]
    fn test_parse_minimal_skill() {
        let parser = SkillParser::new();
        let skill = parser.parse_str(MINIMAL_SKILL).unwrap();

        assert_eq!(skill.name.as_str(), "minimal");
        assert_eq!(skill.description, "A minimal skill");
        assert!(skill.license.is_none());
        assert!(skill.instructions.contains("Instructions here"));
    }

    #[test]
    fn test_parse_triggers() {
        let parser = SkillParser::new();
        let skill = parser.parse_str(VALID_SKILL).unwrap();
        let triggers = skill.triggers();

        assert_eq!(triggers, vec!["pdf", ".pdf", "extract text from pdf"]);
    }

    #[test]
    fn test_parse_tags() {
        let parser = SkillParser::new();
        let skill = parser.parse_str(VALID_SKILL).unwrap();
        let tags = skill.tags();

        assert_eq!(tags, vec!["document", "extraction"]);
    }

    #[test]
    fn test_parse_file() {
        let parser = SkillParser::new();

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(VALID_SKILL.as_bytes()).unwrap();

        let skill = parser.parse_file(file.path()).unwrap();
        assert_eq!(skill.name.as_str(), "pdf-processing");
        assert_eq!(skill.source_path, file.path());
    }

    #[test]
    fn test_parse_metadata() {
        let parser = SkillParser::new();

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(VALID_SKILL.as_bytes()).unwrap();

        let metadata = parser.parse_metadata(file.path()).unwrap();
        assert_eq!(metadata.name.as_str(), "pdf-processing");
        assert!(metadata.content_hash != 0);
        assert!(metadata.estimated_tokens > 0);
        assert_eq!(metadata.tags, vec!["document", "extraction"]);
    }

    #[test]
    fn test_missing_frontmatter() {
        let parser = SkillParser::new();
        let content = "# Just markdown\n\nNo frontmatter here.";

        let err = parser.parse_str(content).unwrap_err();
        assert!(matches!(err, ParseError::MissingFrontmatter { .. }));
    }

    #[test]
    fn test_unclosed_frontmatter() {
        let parser = SkillParser::new();
        let content = "---\nname: test\n\nNo closing delimiter";

        let err = parser.parse_str(content).unwrap_err();
        assert!(matches!(err, ParseError::MissingFrontmatter { .. }));
    }

    #[test]
    fn test_invalid_yaml() {
        let parser = SkillParser::new();
        let content = r#"---
name: [invalid yaml
description: missing bracket
---

Body
"#;

        let err = parser.parse_str(content).unwrap_err();
        assert!(matches!(err, ParseError::InvalidFrontmatter { .. }));
    }

    #[test]
    fn test_invalid_skill_name() {
        let parser = SkillParser::new();
        let content = r#"---
name: "skill with spaces"
description: Test
---

Body
"#;

        let err = parser.parse_str(content).unwrap_err();
        assert!(matches!(err, ParseError::Validation { .. }));
    }

    #[test]
    fn test_file_not_found() {
        let parser = SkillParser::new();
        let err = parser.parse_file(Path::new("/nonexistent/skill.md")).unwrap_err();
        assert!(matches!(err, ParseError::FileNotFound(_)));
    }

    #[test]
    fn test_case_insensitive_name() {
        let parser = SkillParser::new();
        let content = r#"---
name: PDF-Processing
description: Test
---

Body
"#;

        let skill = parser.parse_str(content).unwrap();
        assert_eq!(skill.name.as_str(), "pdf-processing");
    }

    #[test]
    fn test_windows_line_endings() {
        let parser = SkillParser::new();
        let content = "---\r\nname: test\r\ndescription: Test skill\r\n---\r\n\r\nBody content\r\n";

        let skill = parser.parse_str(content).unwrap();
        assert_eq!(skill.name.as_str(), "test");
        assert!(skill.instructions.contains("Body content"));
    }

    #[test]
    fn test_estimated_tokens() {
        let parser = SkillParser::new();
        let skill = parser.parse_str(VALID_SKILL).unwrap();

        // Should estimate some reasonable number of tokens
        let tokens = skill.estimated_tokens();
        assert!(tokens > 10);
        assert!(tokens < 200);
    }
}
