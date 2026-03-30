//! Skill schema types compatible with agentskills.io specification.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::ValidationError;

/// Validated skill name: 1-64 chars, allowed chars: [a-zA-Z0-9._-]
/// Stored as lowercase internally for case-insensitive matching.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SkillName(String);

impl SkillName {
    /// Maximum length for skill names.
    pub const MAX_LEN: usize = 64;

    /// Create a new validated skill name.
    ///
    /// Validates charset [a-zA-Z0-9._-], 1-64 chars.
    /// Stores as lowercase internally.
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        // Check empty
        if s.is_empty() {
            return Err(ValidationError::EmptyName);
        }

        // Check length
        if s.len() > Self::MAX_LEN {
            return Err(ValidationError::NameTooLong { len: s.len() });
        }

        // Check characters
        for (pos, ch) in s.chars().enumerate() {
            if !ch.is_ascii_alphanumeric() && ch != '.' && ch != '_' && ch != '-' {
                return Err(ValidationError::InvalidNameChar { ch, pos });
            }
        }

        Ok(Self(s.to_lowercase()))
    }

    /// Raw string value (always lowercase).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SkillName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SkillName({:?})", self.0)
    }
}

impl fmt::Display for SkillName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for SkillName {
    type Error = ValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(&s)
    }
}

impl TryFrom<&str> for SkillName {
    type Error = ValidationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<SkillName> for String {
    fn from(name: SkillName) -> String {
        name.0
    }
}

impl AsRef<str> for SkillName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A parsed Agent Skill, compatible with agentskills.io spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    // === Required fields (spec) ===
    /// Skill name: 1-64 chars, lowercase + allowed special chars.
    pub name: SkillName,

    /// Free text description, used for selection.
    pub description: String,

    // === Optional fields (spec) ===
    /// License identifier (e.g., "MIT", "Apache-2.0").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    /// Compatibility information (1-500 chars).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<String>,

    /// Extensible key-value pairs for custom metadata.
    /// Extended metadata uses reserved keys:
    /// - `triggers`: comma-separated trigger patterns
    /// - `examples`: JSON array of {input, expected_skill} pairs
    /// - `allowed_tools`: comma-separated tool names
    /// - `tags`: comma-separated category tags
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,

    // === Body ===
    /// Markdown body after frontmatter (the actual instructions).
    #[serde(skip)]
    pub instructions: String,

    /// Filesystem path to the SKILL.md file.
    #[serde(skip)]
    pub source_path: PathBuf,
}

impl Skill {
    /// Get triggers from metadata if present.
    pub fn triggers(&self) -> Vec<String> {
        self.metadata
            .get("triggers")
            .map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
            .unwrap_or_default()
    }

    /// Get tags from metadata if present.
    pub fn tags(&self) -> Vec<String> {
        self.metadata
            .get("tags")
            .map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
            .unwrap_or_default()
    }

    /// Get allowed tools from metadata if present.
    pub fn allowed_tools(&self) -> Vec<String> {
        self.metadata
            .get("allowed_tools")
            .or_else(|| self.metadata.get("allowed-tools"))
            .map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
            .unwrap_or_default()
    }

    /// Get negative triggers from metadata if present.
    pub fn negative_triggers(&self) -> Vec<String> {
        self.metadata
            .get("negative_triggers")
            .or_else(|| self.metadata.get("negative-triggers"))
            .map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
            .unwrap_or_default()
    }

    /// Estimate token count for this skill's instructions.
    /// Uses a simple heuristic: ~3.5 chars per token on average.
    pub fn estimated_tokens(&self) -> usize {
        estimate_tokens(&self.instructions)
    }
}

/// Lightweight metadata-only view (for progressive disclosure Level 0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// Skill name.
    pub name: SkillName,

    /// Skill description.
    pub description: String,

    /// Tags extracted from metadata.
    pub tags: Vec<String>,

    /// Triggers extracted from metadata.
    pub triggers: Vec<String>,

    /// Filesystem path to the SKILL.md file.
    pub source_path: PathBuf,

    /// Content hash for cache invalidation (xxhash64).
    pub content_hash: u64,

    /// Estimated token count of the full body.
    pub estimated_tokens: usize,
}

impl SkillMetadata {
    /// Create metadata from a full skill.
    pub fn from_skill(skill: &Skill, content_hash: u64) -> Self {
        Self {
            name: skill.name.clone(),
            description: skill.description.clone(),
            tags: skill.tags(),
            triggers: skill.triggers(),
            source_path: skill.source_path.clone(),
            content_hash,
            estimated_tokens: skill.estimated_tokens(),
        }
    }
}

/// Usage and selection statistics for a skill.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillStats {
    /// Number of times this skill was selected.
    pub selection_count: u64,

    /// Number of times this skill was activated (full load).
    pub activation_count: u64,

    /// Number of times this skill was rejected by enforcement.
    pub rejection_count: u64,

    /// Average selection score when selected.
    pub avg_selection_score: f32,

    /// Last selection timestamp (Unix millis).
    pub last_selected_at: Option<u64>,
}

/// Estimate token count for text.
/// Uses a simple heuristic: ~3.5 chars per token on average,
/// adjusted for CJK characters (~2 chars per token).
fn estimate_tokens(text: &str) -> usize {
    let mut cjk_chars = 0;
    let mut other_chars = 0;

    for ch in text.chars() {
        if is_cjk(ch) {
            cjk_chars += 1;
        } else {
            other_chars += 1;
        }
    }

    // CJK: ~2 chars per token, other: ~3.5 chars per token
    let cjk_tokens = cjk_chars as f32 / 2.0;
    let other_tokens = other_chars as f32 / 3.5;

    (cjk_tokens + other_tokens).ceil() as usize
}

/// Check if a character is CJK (Chinese, Japanese, Korean).
fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{4E00}'..='\u{9FFF}' |   // CJK Unified Ideographs
        '\u{3400}'..='\u{4DBF}' |   // CJK Unified Ideographs Extension A
        '\u{20000}'..='\u{2A6DF}' | // CJK Unified Ideographs Extension B
        '\u{2A700}'..='\u{2B73F}' | // CJK Unified Ideographs Extension C
        '\u{2B740}'..='\u{2B81F}' | // CJK Unified Ideographs Extension D
        '\u{2B820}'..='\u{2CEAF}' | // CJK Unified Ideographs Extension E
        '\u{2CEB0}'..='\u{2EBEF}' | // CJK Unified Ideographs Extension F
        '\u{30000}'..='\u{3134F}' | // CJK Unified Ideographs Extension G
        '\u{3040}'..='\u{309F}' |   // Hiragana
        '\u{30A0}'..='\u{30FF}' |   // Katakana
        '\u{AC00}'..='\u{D7AF}'     // Hangul Syllables
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_name_valid() {
        assert!(SkillName::new("pdf-processing").is_ok());
        assert!(SkillName::new("my_skill").is_ok());
        assert!(SkillName::new("skill.v2").is_ok());
        assert!(SkillName::new("Skill123").is_ok());
        assert!(SkillName::new("a").is_ok());
    }

    #[test]
    fn test_skill_name_lowercase() {
        let name = SkillName::new("PDF-Processing").unwrap();
        assert_eq!(name.as_str(), "pdf-processing");
    }

    #[test]
    fn test_skill_name_empty() {
        let err = SkillName::new("").unwrap_err();
        assert!(matches!(err, ValidationError::EmptyName));
    }

    #[test]
    fn test_skill_name_too_long() {
        let long_name = "a".repeat(65);
        let err = SkillName::new(&long_name).unwrap_err();
        assert!(matches!(err, ValidationError::NameTooLong { len: 65 }));
    }

    #[test]
    fn test_skill_name_invalid_char() {
        let err = SkillName::new("skill@name").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidNameChar { ch: '@', pos: 5 }));

        let err = SkillName::new("skill name").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidNameChar { ch: ' ', pos: 5 }));
    }

    #[test]
    fn test_skill_triggers() {
        let mut skill = Skill {
            name: SkillName::new("test").unwrap(),
            description: "Test skill".to_string(),
            license: None,
            compatibility: None,
            metadata: HashMap::new(),
            instructions: String::new(),
            source_path: PathBuf::new(),
        };

        // No triggers
        assert!(skill.triggers().is_empty());

        // With triggers
        skill.metadata.insert("triggers".to_string(), "pdf, .pdf, extract text".to_string());
        let triggers = skill.triggers();
        assert_eq!(triggers, vec!["pdf", ".pdf", "extract text"]);
    }

    #[test]
    fn test_estimate_tokens_english() {
        let text = "This is a simple English text for testing token estimation.";
        let tokens = estimate_tokens(text);
        // ~60 chars / 3.5 ≈ 17 tokens
        assert!(tokens > 10 && tokens < 25);
    }

    #[test]
    fn test_estimate_tokens_cjk() {
        let text = "这是一段中文测试文本";
        let tokens = estimate_tokens(text);
        // 10 CJK chars / 2 = 5 tokens
        assert_eq!(tokens, 5);
    }

    #[test]
    fn test_estimate_tokens_mixed() {
        let text = "Hello 世界 World 你好";
        let tokens = estimate_tokens(text);
        // Mixed: should be reasonable
        assert!(tokens > 3 && tokens < 10);
    }

    #[test]
    fn test_skill_name_serde() {
        let name = SkillName::new("test-skill").unwrap();
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, r#""test-skill""#);

        let deserialized: SkillName = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, name);
    }

    #[test]
    fn test_skill_name_display() {
        let name = SkillName::new("my-skill").unwrap();
        assert_eq!(format!("{}", name), "my-skill");
    }
}
