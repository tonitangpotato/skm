//! Real-time selection metrics.
//!
//! Provides comprehensive metrics collection for skill selection performance,
//! including latency histograms, hit rates, and per-skill/strategy breakdowns.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use hdrhistogram::Histogram;

use skm_core::SkillName;
use skm_select::Confidence;

/// Summary of metrics.
#[derive(Debug, Clone)]
pub struct MetricsSummary {
    /// Total selections.
    pub total_selections: u64,

    /// Total timeouts.
    pub total_timeouts: u64,

    /// Total queries with no match.
    pub total_no_match: u64,

    /// Selections by confidence level.
    pub by_confidence: HashMap<String, u64>,

    /// Latency percentiles (in milliseconds).
    pub latency_p50: f64,
    pub latency_p95: f64,
    pub latency_p99: f64,

    /// Mean latency (in milliseconds).
    pub latency_mean: f64,

    /// Min/Max latency (in milliseconds).
    pub latency_min: f64,
    pub latency_max: f64,

    /// Per-strategy selection counts.
    pub by_strategy: HashMap<String, u64>,

    /// Per-skill selection counts.
    pub by_skill: HashMap<SkillName, u64>,

    /// Trigger strategy hit rate (resolved by triggers without needing slower strategies).
    pub trigger_hit_rate: f64,

    /// Semantic strategy hit rate.
    pub semantic_hit_rate: f64,

    /// LLM fallback rate.
    pub llm_fallback_rate: f64,
}

/// Real-time metrics collector.
///
/// Thread-safe metrics collection for production use.
/// Supports Prometheus export format.
pub struct SelectionMetrics {
    /// Total selection count.
    total_selections: AtomicU64,

    /// Timeout count.
    total_timeouts: AtomicU64,

    /// No match count.
    total_no_match: AtomicU64,

    /// Selections by confidence level.
    by_confidence: Mutex<HashMap<String, u64>>,

    /// Selections by strategy.
    by_strategy: Mutex<HashMap<String, u64>>,

    /// Selections by skill.
    by_skill: Mutex<HashMap<SkillName, u64>>,

    /// Latency histogram (in microseconds).
    latency_histogram: Mutex<Histogram<u64>>,

    /// Cache stats
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
}

impl Default for SelectionMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionMetrics {
    /// Create a new metrics collector.
    pub fn new() -> Self {
        Self {
            total_selections: AtomicU64::new(0),
            total_timeouts: AtomicU64::new(0),
            total_no_match: AtomicU64::new(0),
            by_confidence: Mutex::new(HashMap::new()),
            by_strategy: Mutex::new(HashMap::new()),
            by_skill: Mutex::new(HashMap::new()),
            latency_histogram: Mutex::new(Histogram::new(3).unwrap()),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
        }
    }

    /// Record a successful selection.
    pub fn record_selection(
        &self,
        skill: &SkillName,
        strategy: &str,
        confidence: Confidence,
        latency: Duration,
    ) {
        self.total_selections.fetch_add(1, Ordering::Relaxed);

        // Record confidence
        {
            let mut by_conf = self.by_confidence.lock().unwrap();
            *by_conf.entry(format!("{:?}", confidence)).or_default() += 1;
        }

        // Record strategy
        {
            let mut by_strat = self.by_strategy.lock().unwrap();
            *by_strat.entry(strategy.to_string()).or_default() += 1;
        }

        // Record skill
        {
            let mut by_skill = self.by_skill.lock().unwrap();
            *by_skill.entry(skill.clone()).or_default() += 1;
        }

        // Record latency
        {
            let mut hist = self.latency_histogram.lock().unwrap();
            let micros = latency.as_micros() as u64;
            let high = hist.high();
            let _ = hist.record(micros.min(high));
        }
    }

    /// Record a timeout.
    pub fn record_timeout(&self, latency: Duration) {
        self.total_timeouts.fetch_add(1, Ordering::Relaxed);

        // Still record latency
        let mut hist = self.latency_histogram.lock().unwrap();
        let micros = latency.as_micros() as u64;
        let high = hist.high();
        let _ = hist.record(micros.min(high));
    }

    /// Record when no skill was selected.
    pub fn record_no_match(&self, latency: Duration) {
        self.total_selections.fetch_add(1, Ordering::Relaxed);
        self.total_no_match.fetch_add(1, Ordering::Relaxed);

        let mut by_conf = self.by_confidence.lock().unwrap();
        *by_conf.entry("None".to_string()).or_default() += 1;

        let mut hist = self.latency_histogram.lock().unwrap();
        let micros = latency.as_micros() as u64;
        let high = hist.high();
        let _ = hist.record(micros.min(high));
    }

    /// Record a cache hit.
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache miss.
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Get a summary of metrics.
    pub fn summary(&self) -> MetricsSummary {
        let hist = self.latency_histogram.lock().unwrap();
        let by_strategy = self.by_strategy.lock().unwrap().clone();
        let total = self.total_selections.load(Ordering::Relaxed);

        // Calculate hit rates
        let trigger_count = by_strategy.get("trigger").copied().unwrap_or(0);
        let semantic_count = by_strategy.get("semantic").copied().unwrap_or(0);
        let llm_count = by_strategy.get("llm").copied().unwrap_or(0);

        let total_f64 = total.max(1) as f64;

        MetricsSummary {
            total_selections: total,
            total_timeouts: self.total_timeouts.load(Ordering::Relaxed),
            total_no_match: self.total_no_match.load(Ordering::Relaxed),
            by_confidence: self.by_confidence.lock().unwrap().clone(),
            latency_p50: hist.value_at_quantile(0.5) as f64 / 1000.0,
            latency_p95: hist.value_at_quantile(0.95) as f64 / 1000.0,
            latency_p99: hist.value_at_quantile(0.99) as f64 / 1000.0,
            latency_mean: hist.mean() / 1000.0,
            latency_min: hist.min() as f64 / 1000.0,
            latency_max: hist.max() as f64 / 1000.0,
            by_strategy: by_strategy.clone(),
            by_skill: self.by_skill.lock().unwrap().clone(),
            trigger_hit_rate: trigger_count as f64 / total_f64,
            semantic_hit_rate: semantic_count as f64 / total_f64,
            llm_fallback_rate: llm_count as f64 / total_f64,
        }
    }

    /// Reset all metrics.
    pub fn reset(&self) {
        self.total_selections.store(0, Ordering::Relaxed);
        self.total_timeouts.store(0, Ordering::Relaxed);
        self.total_no_match.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.by_confidence.lock().unwrap().clear();
        self.by_strategy.lock().unwrap().clear();
        self.by_skill.lock().unwrap().clear();
        *self.latency_histogram.lock().unwrap() = Histogram::new(3).unwrap();
    }

    /// Export metrics in Prometheus text format.
    ///
    /// This format is compatible with Prometheus scraping and can be
    /// exposed via the `/metrics` HTTP endpoint.
    pub fn to_prometheus(&self) -> String {
        let summary = self.summary();
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);
        let cache_misses = self.cache_misses.load(Ordering::Relaxed);

        let mut lines = Vec::new();

        // Help and type annotations for better Prometheus integration
        lines.push("# HELP skm_selections_total Total number of skill selections".to_string());
        lines.push("# TYPE skm_selections_total counter".to_string());
        lines.push(format!("skm_selections_total {}", summary.total_selections));

        lines.push("# HELP skm_timeouts_total Total number of selection timeouts".to_string());
        lines.push("# TYPE skm_timeouts_total counter".to_string());
        lines.push(format!("skm_timeouts_total {}", summary.total_timeouts));

        lines.push("# HELP skm_no_match_total Total selections with no matching skill".to_string());
        lines.push("# TYPE skm_no_match_total counter".to_string());
        lines.push(format!("skm_no_match_total {}", summary.total_no_match));

        // Confidence breakdown
        lines.push("# HELP skm_selections_by_confidence Selections by confidence level".to_string());
        lines.push("# TYPE skm_selections_by_confidence counter".to_string());
        for (conf, count) in &summary.by_confidence {
            lines.push(format!(
                "skm_selections_by_confidence{{confidence=\"{}\"}} {}",
                conf, count
            ));
        }

        // Strategy breakdown
        lines.push("# HELP skm_selections_by_strategy Selections by strategy".to_string());
        lines.push("# TYPE skm_selections_by_strategy counter".to_string());
        for (strategy, count) in &summary.by_strategy {
            lines.push(format!(
                "skm_selections_by_strategy{{strategy=\"{}\"}} {}",
                strategy, count
            ));
        }

        // Per-skill breakdown (top 20 to avoid cardinality explosion)
        lines.push("# HELP skm_skill_selections Selections per skill".to_string());
        lines.push("# TYPE skm_skill_selections counter".to_string());
        let mut skill_vec: Vec<_> = summary.by_skill.iter().collect();
        skill_vec.sort_by(|a, b| b.1.cmp(a.1));
        for (skill, count) in skill_vec.into_iter().take(20) {
            lines.push(format!(
                "skm_skill_selections{{skill=\"{}\"}} {}",
                skill.as_str(),
                count
            ));
        }

        // Latency metrics
        lines.push("# HELP skm_latency_milliseconds Selection latency in milliseconds".to_string());
        lines.push("# TYPE skm_latency_milliseconds summary".to_string());
        lines.push(format!(
            "skm_latency_milliseconds{{quantile=\"0.5\"}} {:.3}",
            summary.latency_p50
        ));
        lines.push(format!(
            "skm_latency_milliseconds{{quantile=\"0.95\"}} {:.3}",
            summary.latency_p95
        ));
        lines.push(format!(
            "skm_latency_milliseconds{{quantile=\"0.99\"}} {:.3}",
            summary.latency_p99
        ));
        lines.push(format!(
            "skm_latency_milliseconds_sum {:.3}",
            summary.latency_mean * summary.total_selections as f64
        ));
        lines.push(format!(
            "skm_latency_milliseconds_count {}",
            summary.total_selections
        ));

        // Hit rates
        lines.push("# HELP skm_trigger_hit_rate Fraction of queries resolved by trigger matching".to_string());
        lines.push("# TYPE skm_trigger_hit_rate gauge".to_string());
        lines.push(format!("skm_trigger_hit_rate {:.4}", summary.trigger_hit_rate));

        lines.push("# HELP skm_semantic_hit_rate Fraction of queries resolved by semantic search".to_string());
        lines.push("# TYPE skm_semantic_hit_rate gauge".to_string());
        lines.push(format!("skm_semantic_hit_rate {:.4}", summary.semantic_hit_rate));

        lines.push("# HELP skm_llm_fallback_rate Fraction of queries requiring LLM fallback".to_string());
        lines.push("# TYPE skm_llm_fallback_rate gauge".to_string());
        lines.push(format!("skm_llm_fallback_rate {:.4}", summary.llm_fallback_rate));

        // Cache metrics
        lines.push("# HELP skm_cache_hits_total Total cache hits".to_string());
        lines.push("# TYPE skm_cache_hits_total counter".to_string());
        lines.push(format!("skm_cache_hits_total {}", cache_hits));

        lines.push("# HELP skm_cache_misses_total Total cache misses".to_string());
        lines.push("# TYPE skm_cache_misses_total counter".to_string());
        lines.push(format!("skm_cache_misses_total {}", cache_misses));

        let total_cache = cache_hits + cache_misses;
        if total_cache > 0 {
            let cache_hit_rate = cache_hits as f64 / total_cache as f64;
            lines.push("# HELP skm_cache_hit_rate Cache hit rate".to_string());
            lines.push("# TYPE skm_cache_hit_rate gauge".to_string());
            lines.push(format!("skm_cache_hit_rate {:.4}", cache_hit_rate));
        }

        lines.join("\n")
    }

    /// Export metrics as JSON for dashboarding.
    pub fn to_json(&self) -> serde_json::Value {
        let summary = self.summary();
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);
        let cache_misses = self.cache_misses.load(Ordering::Relaxed);

        serde_json::json!({
            "total_selections": summary.total_selections,
            "total_timeouts": summary.total_timeouts,
            "total_no_match": summary.total_no_match,
            "latency": {
                "p50_ms": summary.latency_p50,
                "p95_ms": summary.latency_p95,
                "p99_ms": summary.latency_p99,
                "mean_ms": summary.latency_mean,
                "min_ms": summary.latency_min,
                "max_ms": summary.latency_max
            },
            "by_confidence": summary.by_confidence,
            "by_strategy": summary.by_strategy,
            "by_skill": summary.by_skill.iter()
                .map(|(k, v)| (k.as_str().to_string(), *v))
                .collect::<HashMap<String, u64>>(),
            "hit_rates": {
                "trigger": summary.trigger_hit_rate,
                "semantic": summary.semantic_hit_rate,
                "llm_fallback": summary.llm_fallback_rate
            },
            "cache": {
                "hits": cache_hits,
                "misses": cache_misses,
                "hit_rate": if cache_hits + cache_misses > 0 {
                    cache_hits as f64 / (cache_hits + cache_misses) as f64
                } else {
                    0.0
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_record() {
        let metrics = SelectionMetrics::new();

        let skill = SkillName::new("test-skill").unwrap();
        metrics.record_selection(&skill, "trigger", Confidence::High, Duration::from_millis(10));
        metrics.record_selection(&skill, "trigger", Confidence::High, Duration::from_millis(20));
        metrics.record_selection(&skill, "semantic", Confidence::Medium, Duration::from_millis(100));

        let summary = metrics.summary();

        assert_eq!(summary.total_selections, 3);
        assert_eq!(*summary.by_strategy.get("trigger").unwrap_or(&0), 2);
        assert_eq!(*summary.by_strategy.get("semantic").unwrap_or(&0), 1);
        assert_eq!(*summary.by_skill.get(&skill).unwrap_or(&0), 3);
    }

    #[test]
    fn test_metrics_timeout() {
        let metrics = SelectionMetrics::new();

        metrics.record_timeout(Duration::from_secs(5));

        let summary = metrics.summary();
        assert_eq!(summary.total_timeouts, 1);
    }

    #[test]
    fn test_metrics_no_match() {
        let metrics = SelectionMetrics::new();

        metrics.record_no_match(Duration::from_millis(50));

        let summary = metrics.summary();
        assert_eq!(summary.total_selections, 1);
        assert_eq!(summary.total_no_match, 1);
        assert!(summary.by_confidence.contains_key("None"));
    }

    #[test]
    fn test_metrics_reset() {
        let metrics = SelectionMetrics::new();

        let skill = SkillName::new("test").unwrap();
        metrics.record_selection(&skill, "trigger", Confidence::High, Duration::from_millis(10));

        assert_eq!(metrics.summary().total_selections, 1);

        metrics.reset();

        assert_eq!(metrics.summary().total_selections, 0);
    }

    #[test]
    fn test_prometheus_export() {
        let metrics = SelectionMetrics::new();

        let skill = SkillName::new("test").unwrap();
        metrics.record_selection(&skill, "trigger", Confidence::High, Duration::from_millis(10));

        let prometheus = metrics.to_prometheus();

        assert!(prometheus.contains("skm_selections_total 1"));
        assert!(prometheus.contains("skm_selections_by_strategy{strategy=\"trigger\"}"));
        assert!(prometheus.contains("# TYPE skm_selections_total counter"));
        assert!(prometheus.contains("skm_trigger_hit_rate"));
    }

    #[test]
    fn test_json_export() {
        let metrics = SelectionMetrics::new();

        let skill = SkillName::new("test").unwrap();
        metrics.record_selection(&skill, "trigger", Confidence::High, Duration::from_millis(10));

        let json = metrics.to_json();

        assert_eq!(json["total_selections"], 1);
        assert!(json["by_strategy"]["trigger"].as_u64().is_some());
    }

    #[test]
    fn test_cache_metrics() {
        let metrics = SelectionMetrics::new();

        metrics.record_cache_hit();
        metrics.record_cache_hit();
        metrics.record_cache_miss();

        let prometheus = metrics.to_prometheus();

        assert!(prometheus.contains("skm_cache_hits_total 2"));
        assert!(prometheus.contains("skm_cache_misses_total 1"));
    }

    #[test]
    fn test_hit_rates() {
        let metrics = SelectionMetrics::new();

        let skill = SkillName::new("test").unwrap();
        // 2 trigger hits, 1 semantic hit, 1 llm fallback
        metrics.record_selection(&skill, "trigger", Confidence::High, Duration::from_millis(10));
        metrics.record_selection(&skill, "trigger", Confidence::High, Duration::from_millis(10));
        metrics.record_selection(&skill, "semantic", Confidence::Medium, Duration::from_millis(50));
        metrics.record_selection(&skill, "llm", Confidence::Low, Duration::from_millis(500));

        let summary = metrics.summary();

        assert!((summary.trigger_hit_rate - 0.5).abs() < 0.01);
        assert!((summary.semantic_hit_rate - 0.25).abs() < 0.01);
        assert!((summary.llm_fallback_rate - 0.25).abs() < 0.01);
    }
}
