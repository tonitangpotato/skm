//! Output validators for skill execution results.

use ase_core::SkillName;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of validation.
#[derive(Debug, Clone)]
pub enum ValidationResult {
    /// Output is valid.
    Valid,

    /// Output is invalid.
    Invalid { errors: Vec<String> },

    /// Output was repaired (auto-fixed).
    Repaired {
        output: String,
        warnings: Vec<String>,
    },
}

impl ValidationResult {
    /// Check if the result is valid or repaired.
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Valid | Self::Repaired { .. })
    }

    /// Check if the result is invalid.
    pub fn is_invalid(&self) -> bool {
        matches!(self, Self::Invalid { .. })
    }

    /// Get errors if invalid.
    pub fn errors(&self) -> Option<&[String]> {
        match self {
            Self::Invalid { errors } => Some(errors),
            _ => None,
        }
    }
}

/// Validator trait for output validation.
pub trait Validator: Send + Sync {
    /// Validate output.
    fn validate(&self, output: &str) -> ValidationResult;

    /// Validator name.
    fn name(&self) -> &str {
        "unnamed-validator"
    }
}

/// JSON Schema validator.
#[derive(Debug, Clone)]
pub struct JsonSchemaValidator {
    schema: serde_json::Value,
    name: String,
}

impl JsonSchemaValidator {
    /// Create a new JSON schema validator.
    pub fn new(schema: serde_json::Value) -> Self {
        Self {
            schema,
            name: "json-schema".to_string(),
        }
    }

    /// Create with a name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl Validator for JsonSchemaValidator {
    fn validate(&self, output: &str) -> ValidationResult {
        // Parse output as JSON
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(output);

        match parsed {
            Ok(value) => {
                // Basic type checking against schema
                // In production, use a full JSON Schema validator like jsonschema-rs
                if let Some(expected_type) = self.schema.get("type").and_then(|t| t.as_str()) {
                    let actual_type = match &value {
                        serde_json::Value::Null => "null",
                        serde_json::Value::Bool(_) => "boolean",
                        serde_json::Value::Number(_) => "number",
                        serde_json::Value::String(_) => "string",
                        serde_json::Value::Array(_) => "array",
                        serde_json::Value::Object(_) => "object",
                    };

                    if expected_type != actual_type {
                        return ValidationResult::Invalid {
                            errors: vec![format!(
                                "Expected type '{}', got '{}'",
                                expected_type, actual_type
                            )],
                        };
                    }
                }

                ValidationResult::Valid
            }
            Err(e) => ValidationResult::Invalid {
                errors: vec![format!("Invalid JSON: {}", e)],
            },
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Regex validator.
#[derive(Debug, Clone)]
pub struct RegexValidator {
    pattern: Regex,
    name: String,
    must_match: bool,
}

impl RegexValidator {
    /// Create a validator that requires the pattern to match.
    pub fn must_match(pattern: &str) -> Result<Self, regex::Error> {
        Ok(Self {
            pattern: Regex::new(pattern)?,
            name: "regex-must-match".to_string(),
            must_match: true,
        })
    }

    /// Create a validator that requires the pattern to NOT match.
    pub fn must_not_match(pattern: &str) -> Result<Self, regex::Error> {
        Ok(Self {
            pattern: Regex::new(pattern)?,
            name: "regex-must-not-match".to_string(),
            must_match: false,
        })
    }

    /// Set the validator name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl Validator for RegexValidator {
    fn validate(&self, output: &str) -> ValidationResult {
        let matches = self.pattern.is_match(output);

        if self.must_match && !matches {
            ValidationResult::Invalid {
                errors: vec![format!("Output must match pattern: {}", self.pattern)],
            }
        } else if !self.must_match && matches {
            ValidationResult::Invalid {
                errors: vec![format!("Output must not match pattern: {}", self.pattern)],
            }
        } else {
            ValidationResult::Valid
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Length validator.
#[derive(Debug, Clone)]
pub struct LengthValidator {
    min: Option<usize>,
    max: Option<usize>,
    name: String,
}

impl LengthValidator {
    /// Create a new length validator.
    pub fn new(min: Option<usize>, max: Option<usize>) -> Self {
        Self {
            min,
            max,
            name: "length".to_string(),
        }
    }

    /// Create a validator with minimum length.
    pub fn min(min: usize) -> Self {
        Self::new(Some(min), None)
    }

    /// Create a validator with maximum length.
    pub fn max(max: usize) -> Self {
        Self::new(None, Some(max))
    }

    /// Create a validator with both min and max.
    pub fn range(min: usize, max: usize) -> Self {
        Self::new(Some(min), Some(max))
    }

    /// Set the validator name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl Validator for LengthValidator {
    fn validate(&self, output: &str) -> ValidationResult {
        let len = output.len();
        let mut errors = Vec::new();

        if let Some(min) = self.min {
            if len < min {
                errors.push(format!("Output too short: {} < {} chars", len, min));
            }
        }

        if let Some(max) = self.max {
            if len > max {
                errors.push(format!("Output too long: {} > {} chars", len, max));
            }
        }

        if errors.is_empty() {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid { errors }
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Collection of validators for different skills.
pub struct OutputValidator {
    validators: HashMap<SkillName, Vec<Box<dyn Validator>>>,
    default_validators: Vec<Box<dyn Validator>>,
}

impl OutputValidator {
    /// Create a new output validator collection.
    pub fn new() -> Self {
        Self {
            validators: HashMap::new(),
            default_validators: Vec::new(),
        }
    }

    /// Add a validator for a specific skill.
    pub fn add_for_skill(&mut self, skill: SkillName, validator: Box<dyn Validator>) {
        self.validators
            .entry(skill)
            .or_insert_with(Vec::new)
            .push(validator);
    }

    /// Add a default validator (applies to all skills).
    pub fn add_default(&mut self, validator: Box<dyn Validator>) {
        self.default_validators.push(validator);
    }

    /// Validate output for a skill.
    pub fn validate(&self, skill: &SkillName, output: &str) -> ValidationResult {
        let mut all_errors = Vec::new();

        // Run default validators
        for validator in &self.default_validators {
            if let ValidationResult::Invalid { errors } = validator.validate(output) {
                all_errors.extend(errors);
            }
        }

        // Run skill-specific validators
        if let Some(skill_validators) = self.validators.get(skill) {
            for validator in skill_validators {
                if let ValidationResult::Invalid { errors } = validator.validate(output) {
                    all_errors.extend(errors);
                }
            }
        }

        if all_errors.is_empty() {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid { errors: all_errors }
        }
    }
}

impl Default for OutputValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_schema_valid() {
        let schema = serde_json::json!({ "type": "object" });
        let validator = JsonSchemaValidator::new(schema);

        let result = validator.validate(r#"{"key": "value"}"#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_json_schema_invalid_json() {
        let schema = serde_json::json!({ "type": "object" });
        let validator = JsonSchemaValidator::new(schema);

        let result = validator.validate("not json");
        assert!(result.is_invalid());
    }

    #[test]
    fn test_json_schema_wrong_type() {
        let schema = serde_json::json!({ "type": "object" });
        let validator = JsonSchemaValidator::new(schema);

        let result = validator.validate(r#"["array", "not", "object"]"#);
        assert!(result.is_invalid());
    }

    #[test]
    fn test_regex_must_match() {
        let validator = RegexValidator::must_match(r"^\d+$").unwrap();

        assert!(validator.validate("12345").is_ok());
        assert!(validator.validate("abc").is_invalid());
    }

    #[test]
    fn test_regex_must_not_match() {
        let validator = RegexValidator::must_not_match(r"secret").unwrap();

        assert!(validator.validate("normal text").is_ok());
        assert!(validator.validate("contains secret").is_invalid());
    }

    #[test]
    fn test_length_min() {
        let validator = LengthValidator::min(5);

        assert!(validator.validate("hello").is_ok());
        assert!(validator.validate("hi").is_invalid());
    }

    #[test]
    fn test_length_max() {
        let validator = LengthValidator::max(10);

        assert!(validator.validate("short").is_ok());
        assert!(validator.validate("this is too long").is_invalid());
    }

    #[test]
    fn test_length_range() {
        let validator = LengthValidator::range(3, 10);

        assert!(validator.validate("hello").is_ok());
        assert!(validator.validate("hi").is_invalid());
        assert!(validator.validate("this is too long").is_invalid());
    }

    #[test]
    fn test_output_validator() {
        let mut validators = OutputValidator::new();

        let skill = SkillName::new("api-skill").unwrap();
        validators.add_for_skill(
            skill.clone(),
            Box::new(JsonSchemaValidator::new(serde_json::json!({ "type": "object" }))),
        );

        // Valid JSON object
        let result = validators.validate(&skill, r#"{"ok": true}"#);
        assert!(result.is_ok());

        // Invalid (not JSON)
        let result = validators.validate(&skill, "not json");
        assert!(result.is_invalid());

        // Other skill without validator
        let other = SkillName::new("other-skill").unwrap();
        let result = validators.validate(&other, "anything");
        assert!(result.is_ok());
    }

    #[test]
    fn test_default_validator() {
        let mut validators = OutputValidator::new();
        validators.add_default(Box::new(LengthValidator::max(100)));

        let skill = SkillName::new("any-skill").unwrap();

        assert!(validators.validate(&skill, "short").is_ok());
        assert!(validators.validate(&skill, &"x".repeat(200)).is_invalid());
    }
}
