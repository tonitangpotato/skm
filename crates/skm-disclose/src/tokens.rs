//! Fast token count estimation.

use serde::{Deserialize, Serialize};

/// Fast token count estimator.
///
/// Uses a chars-per-token heuristic. Not exact, but good enough for budgeting.
/// Default: ~3.5 chars per token for English, ~2 chars for CJK.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenEstimator {
    /// Characters per token for non-CJK text.
    chars_per_token: f32,

    /// Characters per token for CJK text.
    cjk_chars_per_token: f32,
}

impl Default for TokenEstimator {
    fn default() -> Self {
        Self {
            chars_per_token: 3.5,
            cjk_chars_per_token: 2.0,
        }
    }
}

impl TokenEstimator {
    /// Create a new estimator with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an estimator with custom chars-per-token ratio.
    pub fn with_ratio(chars_per_token: f32) -> Self {
        Self {
            chars_per_token,
            cjk_chars_per_token: chars_per_token * 0.57, // Approximate ratio
        }
    }

    /// Create an estimator with separate ratios for ASCII and CJK.
    pub fn with_cjk_ratio(chars_per_token: f32, cjk_chars_per_token: f32) -> Self {
        Self {
            chars_per_token,
            cjk_chars_per_token,
        }
    }

    /// Estimate token count for text (simple heuristic).
    pub fn estimate(&self, text: &str) -> usize {
        let chars = text.chars().count();
        (chars as f32 / self.chars_per_token).ceil() as usize
    }

    /// Estimate token count with CJK awareness.
    /// Better for mixed Chinese/English text.
    pub fn estimate_cjk_aware(&self, text: &str) -> usize {
        let mut cjk_chars = 0;
        let mut other_chars = 0;

        for ch in text.chars() {
            if is_cjk(ch) {
                cjk_chars += 1;
            } else {
                other_chars += 1;
            }
        }

        let cjk_tokens = cjk_chars as f32 / self.cjk_chars_per_token;
        let other_tokens = other_chars as f32 / self.chars_per_token;

        (cjk_tokens + other_tokens).ceil() as usize
    }

    /// Estimate tokens for multiple texts.
    pub fn estimate_batch(&self, texts: &[&str]) -> usize {
        texts.iter().map(|t| self.estimate_cjk_aware(t)).sum()
    }
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
    fn test_estimate_english() {
        let estimator = TokenEstimator::new();
        let text = "Hello, world! This is a test.";
        let tokens = estimator.estimate(text);

        // ~30 chars / 3.5 ≈ 9 tokens
        assert!(tokens >= 5 && tokens <= 15);
    }

    #[test]
    fn test_estimate_cjk() {
        let estimator = TokenEstimator::new();
        let text = "你好世界";
        let tokens = estimator.estimate_cjk_aware(text);

        // 4 CJK chars / 2.0 = 2 tokens
        assert_eq!(tokens, 2);
    }

    #[test]
    fn test_estimate_mixed() {
        let estimator = TokenEstimator::new();
        let text = "Hello 世界 World";
        let tokens = estimator.estimate_cjk_aware(text);

        // Mixed: should be reasonable
        assert!(tokens >= 3 && tokens <= 10);
    }

    #[test]
    fn test_estimate_empty() {
        let estimator = TokenEstimator::new();
        assert_eq!(estimator.estimate(""), 0);
        assert_eq!(estimator.estimate_cjk_aware(""), 0);
    }

    #[test]
    fn test_custom_ratio() {
        let estimator = TokenEstimator::with_ratio(4.0);
        let text = "Hello world"; // 11 chars
        let tokens = estimator.estimate(text);

        // 11 / 4.0 = 2.75 → 3
        assert_eq!(tokens, 3);
    }

    #[test]
    fn test_batch_estimate() {
        let estimator = TokenEstimator::new();
        let texts = vec!["Hello", "World", "Test"];
        let total = estimator.estimate_batch(&texts);

        assert!(total > 0);
    }

    #[test]
    fn test_is_cjk() {
        assert!(is_cjk('中'));
        assert!(is_cjk('日'));
        assert!(is_cjk('한'));
        assert!(!is_cjk('A'));
        assert!(!is_cjk('1'));
        assert!(!is_cjk(' '));
    }
}
