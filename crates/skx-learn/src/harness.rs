//! Trigger test harness for evaluating skill selection accuracy.

use std::path::Path;

use serde::{Deserialize, Serialize};

use skx_core::{SkillName, SkillRegistry};
use skx_select::{CascadeSelector, SelectionContext, SelectionStrategy};

use crate::error::LearnError;

/// What skill(s) are expected for a test case.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TestExpectation {
    /// Expect a single skill.
    Single(SkillName),

    /// Expect any of these skills.
    AnyOf(Vec<SkillName>),

    /// Expect no skill.
    None,
}

impl TestExpectation {
    /// Check if a result matches the expectation.
    pub fn matches(&self, result: Option<&SkillName>) -> bool {
        match (self, result) {
            (Self::None, None) => true,
            (Self::None, Some(_)) => false,
            (Self::Single(expected), Some(actual)) => expected == actual,
            (Self::Single(_), None) => false,
            (Self::AnyOf(expected), Some(actual)) => expected.contains(actual),
            (Self::AnyOf(_), None) => false,
        }
    }
}

/// A single test case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    /// Test case name/ID.
    pub name: String,

    /// User query to test.
    pub input: String,

    /// Expected skill(s).
    pub expected: TestExpectation,

    /// Optional context to provide.
    #[serde(default)]
    pub context: SelectionContext,
}

/// Result of running a single test case.
#[derive(Debug, Clone)]
pub struct TestCaseResult {
    /// Test case name.
    pub name: String,

    /// Whether the test passed.
    pub passed: bool,

    /// The selected skill (if any).
    pub selected: Option<SkillName>,

    /// The expected skill(s).
    pub expected: TestExpectation,

    /// Selection score (if any).
    pub score: Option<f32>,

    /// Latency in milliseconds.
    pub latency_ms: u64,
}

/// A collection of test cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuite {
    /// Suite name.
    pub name: String,

    /// Test cases.
    pub cases: Vec<TestCase>,
}

impl TestSuite {
    /// Load from YAML file.
    pub fn from_file(path: &Path) -> Result<Self, LearnError> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&content)?)
    }

    /// Create a new empty suite.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cases: Vec::new(),
        }
    }

    /// Add a test case.
    pub fn add_case(&mut self, case: TestCase) {
        self.cases.push(case);
    }
}

/// Report for a single skill across test runs.
#[derive(Debug, Clone, Default)]
pub struct SkillTestReport {
    /// Total tests targeting this skill.
    pub total: usize,

    /// Tests that correctly selected this skill.
    pub correct: usize,

    /// Tests where this skill was selected incorrectly.
    pub false_positives: usize,

    /// Tests where this skill should have been selected but wasn't.
    pub false_negatives: usize,
}

impl SkillTestReport {
    /// Precision: correct / (correct + false_positives)
    pub fn precision(&self) -> f32 {
        let denom = self.correct + self.false_positives;
        if denom == 0 {
            0.0
        } else {
            self.correct as f32 / denom as f32
        }
    }

    /// Recall: correct / (correct + false_negatives)
    pub fn recall(&self) -> f32 {
        let denom = self.correct + self.false_negatives;
        if denom == 0 {
            0.0
        } else {
            self.correct as f32 / denom as f32
        }
    }

    /// F1 score.
    pub fn f1(&self) -> f32 {
        let p = self.precision();
        let r = self.recall();
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }
}

/// Full test report.
#[derive(Debug, Clone)]
pub struct TestReport {
    /// Suite name.
    pub suite_name: String,

    /// Individual test results.
    pub results: Vec<TestCaseResult>,

    /// Per-skill breakdown.
    pub per_skill: std::collections::HashMap<SkillName, SkillTestReport>,

    /// Total tests run.
    pub total: usize,

    /// Tests passed.
    pub passed: usize,

    /// Average latency in ms.
    pub avg_latency_ms: f32,
}

impl TestReport {
    /// Overall accuracy.
    pub fn accuracy(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.passed as f32 / self.total as f32
        }
    }
}

/// Test harness for running trigger test suites.
pub struct TriggerTestHarness {
    /// Number of runs per test case (for stability).
    pub runs_per_case: usize,
}

impl Default for TriggerTestHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl TriggerTestHarness {
    /// Create a new harness with default settings.
    pub fn new() -> Self {
        Self { runs_per_case: 1 }
    }

    /// Set runs per test case.
    pub fn with_runs(mut self, runs: usize) -> Self {
        self.runs_per_case = runs;
        self
    }

    /// Run a test suite against a selector.
    pub async fn run(
        &self,
        suite: &TestSuite,
        selector: &CascadeSelector,
        registry: &SkillRegistry,
    ) -> Result<TestReport, LearnError> {
        let mut results = Vec::new();
        let mut per_skill: std::collections::HashMap<SkillName, SkillTestReport> =
            std::collections::HashMap::new();

        for case in &suite.cases {
            let start = std::time::Instant::now();

            // Run selection
            let outcome = selector.select(&case.input, registry, &case.context).await?;

            let latency = start.elapsed();
            let selected = outcome.selected.first().map(|r| r.skill.clone());
            let score = outcome.selected.first().map(|r| r.score);

            let passed = case.expected.matches(selected.as_ref());

            // Update per-skill stats
            let expected_skills = match &case.expected {
                TestExpectation::Single(s) => vec![s.clone()],
                TestExpectation::AnyOf(list) => list.clone(),
                TestExpectation::None => vec![],
            };

            for skill in &expected_skills {
                let report = per_skill.entry(skill.clone()).or_default();
                report.total += 1;

                if selected.as_ref() == Some(skill) {
                    report.correct += 1;
                } else {
                    report.false_negatives += 1;
                }
            }

            if let Some(ref sel) = selected {
                if !expected_skills.contains(sel) {
                    let report = per_skill.entry(sel.clone()).or_default();
                    report.false_positives += 1;
                }
            }

            results.push(TestCaseResult {
                name: case.name.clone(),
                passed,
                selected,
                expected: case.expected.clone(),
                score,
                latency_ms: latency.as_millis() as u64,
            });
        }

        let passed = results.iter().filter(|r| r.passed).count();
        let total = results.len();
        let avg_latency = if total > 0 {
            results.iter().map(|r| r.latency_ms as f32).sum::<f32>() / total as f32
        } else {
            0.0
        };

        Ok(TestReport {
            suite_name: suite.name.clone(),
            results,
            per_skill,
            total,
            passed,
            avg_latency_ms: avg_latency,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expectation_single_match() {
        let exp = TestExpectation::Single(SkillName::new("test").unwrap());
        assert!(exp.matches(Some(&SkillName::new("test").unwrap())));
        assert!(!exp.matches(Some(&SkillName::new("other").unwrap())));
        assert!(!exp.matches(None));
    }

    #[test]
    fn test_expectation_any_of() {
        let exp = TestExpectation::AnyOf(vec![
            SkillName::new("a").unwrap(),
            SkillName::new("b").unwrap(),
        ]);
        assert!(exp.matches(Some(&SkillName::new("a").unwrap())));
        assert!(exp.matches(Some(&SkillName::new("b").unwrap())));
        assert!(!exp.matches(Some(&SkillName::new("c").unwrap())));
    }

    #[test]
    fn test_expectation_none() {
        let exp = TestExpectation::None;
        assert!(exp.matches(None));
        assert!(!exp.matches(Some(&SkillName::new("test").unwrap())));
    }

    #[test]
    fn test_skill_report_metrics() {
        let report = SkillTestReport {
            total: 10,
            correct: 8,
            false_positives: 2,
            false_negatives: 2,
        };

        // Precision: 8 / (8 + 2) = 0.8
        assert!((report.precision() - 0.8).abs() < 1e-5);

        // Recall: 8 / (8 + 2) = 0.8
        assert!((report.recall() - 0.8).abs() < 1e-5);

        // F1: 2 * 0.8 * 0.8 / 1.6 = 0.8
        assert!((report.f1() - 0.8).abs() < 1e-5);
    }

    #[test]
    fn test_test_suite_new() {
        let suite = TestSuite::new("my-suite");
        assert_eq!(suite.name, "my-suite");
        assert!(suite.cases.is_empty());
    }
}
