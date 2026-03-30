//! Disclosure level types.

use std::path::PathBuf;

use ase_core::SkillName;
use serde::{Deserialize, Serialize};

/// Disclosure level for a skill.
///
/// - Level 0 (Catalog): Name + description only (~30-50 tokens/skill)
/// - Level 1 (Activated): Full SKILL.md body loaded (~2000-5000 tokens)
/// - Level 2 (Referenced): Additional files from scripts/, references/ loaded on demand
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisclosureLevel {
    /// Level 0: Only name and description in context.
    Catalog,

    /// Level 1: Full SKILL.md instructions loaded.
    Activated,

    /// Level 2: Additional reference files loaded.
    Referenced,
}

impl DisclosureLevel {
    /// Get the numeric level (0, 1, or 2).
    pub fn level(&self) -> u8 {
        match self {
            Self::Catalog => 0,
            Self::Activated => 1,
            Self::Referenced => 2,
        }
    }

    /// Check if this level includes full instructions.
    pub fn has_instructions(&self) -> bool {
        matches!(self, Self::Activated | Self::Referenced)
    }
}

impl Default for DisclosureLevel {
    fn default() -> Self {
        Self::Catalog
    }
}

/// A skill loaded at some disclosure level.
#[derive(Debug, Clone)]
pub struct LoadedSkill {
    /// The skill name.
    pub name: SkillName,

    /// Current disclosure level.
    pub level: DisclosureLevel,

    /// Tokens used by this skill in context.
    pub tokens_used: usize,

    /// Reference files loaded (for Level 2).
    pub loaded_files: Vec<PathBuf>,
}

impl LoadedSkill {
    /// Create a new loaded skill entry.
    pub fn new(name: SkillName, level: DisclosureLevel, tokens_used: usize) -> Self {
        Self {
            name,
            level,
            tokens_used,
            loaded_files: Vec::new(),
        }
    }

    /// Add a loaded reference file.
    pub fn add_file(&mut self, path: PathBuf, additional_tokens: usize) {
        self.loaded_files.push(path);
        self.tokens_used += additional_tokens;
        self.level = DisclosureLevel::Referenced;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disclosure_level_ordering() {
        assert_eq!(DisclosureLevel::Catalog.level(), 0);
        assert_eq!(DisclosureLevel::Activated.level(), 1);
        assert_eq!(DisclosureLevel::Referenced.level(), 2);
    }

    #[test]
    fn test_has_instructions() {
        assert!(!DisclosureLevel::Catalog.has_instructions());
        assert!(DisclosureLevel::Activated.has_instructions());
        assert!(DisclosureLevel::Referenced.has_instructions());
    }

    #[test]
    fn test_loaded_skill() {
        let name = SkillName::new("test").unwrap();
        let mut skill = LoadedSkill::new(name, DisclosureLevel::Activated, 1000);

        assert_eq!(skill.tokens_used, 1000);
        assert!(skill.loaded_files.is_empty());

        skill.add_file(PathBuf::from("references/data.md"), 500);

        assert_eq!(skill.tokens_used, 1500);
        assert_eq!(skill.level, DisclosureLevel::Referenced);
        assert_eq!(skill.loaded_files.len(), 1);
    }
}
