//! Real-time selection metrics.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use hdrhistogram::Histogram;

use ase_core::SkillName;
use ase_select::Confidence;

/// Summary of metrics.
#[derive(Debug, Clone)]
pub struct MetricsSummary {
    /// Total selections.
    pub total_selections: u64,

    /// Total timeouts.
    pub total_timeouts: u64,

    /// Selections by confidence level.
    pub by_confidence: HashMap<String, u64>,

    /// Latency percentiles (in milliseconds).
    pub latency_p50: f64,
    pub latency_p95: f64,
    pub latency_p99: f64,

    /// Per-strategy selection counts.
    pub by_strategy: HashMap<String, u64>,

    /// Per-skill selection counts.
    pub by_skill: HashMap<SkillName, u64>,
}

/// Real-time metrics collector.
pub struct SelectionMetrics {
    /// Total selection count.
    total_selections: Mutex<u64>,

    /// Timeout count.
    total_timeouts: Mutex<u64>,

    /// Selections by confidence level.
    by_confidence: Mutex<HashMap<String, u64>>,

    /// Selections by strategy.
    by_strategy: Mutex<HashMap<String, u64>>,

    /// Selections by skill.
    by_skill: Mutex<HashMap<SkillName, u64>>,

    /// Latency histogram (in microseconds).
    latency_histogram: Mutex<Histogram<u64>>,
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
            total_selections: Mutex::new(0),
            total_timeouts: Mutex::new(0),
            by_confidence: Mutex::new(HashMap::new()),
            by_strategy: Mutex::new(HashMap::new()),
            by_skill: Mutex::new(HashMap::new()),
            latency_histogram: Mutex::new(Histogram::new(3).unwrap()),
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
        *self.total_selections.lock().unwrap() += 1;

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
        *self.total_timeouts.lock().unwrap() += 1;

        // Still record latency
        let mut hist = self.latency_histogram.lock().unwrap();
        let micros = latency.as_micros() as u64;
        let high = hist.high();
        let _ = hist.record(micros.min(high));
    }

    /// Record when no skill was selected.
    pub fn record_no_match(&self, latency: Duration) {
        *self.total_selections.lock().unwrap() += 1;

        let mut by_conf = self.by_confidence.lock().unwrap();
        *by_conf.entry("None".to_string()).or_default() += 1;

        let mut hist = self.latency_histogram.lock().unwrap();
        let micros = latency.as_micros() as u64;
        let high = hist.high();
        let _ = hist.record(micros.min(high));
    }

    /// Get a summary of metrics.
    pub fn summary(&self) -> MetricsSummary {
        let hist = self.latency_histogram.lock().unwrap();

        MetricsSummary {
            total_selections: *self.total_selections.lock().unwrap(),
            total_timeouts: *self.total_timeouts.lock().unwrap(),
            by_confidence: self.by_confidence.lock().unwrap().clone(),
            latency_p50: hist.value_at_quantile(0.5) as f64 / 1000.0, // Convert to ms
            latency_p95: hist.value_at_quantile(0.95) as f64 / 1000.0,
            latency_p99: hist.value_at_quantile(0.99) as f64 / 1000.0,
            by_strategy: self.by_strategy.lock().unwrap().clone(),
            by_skill: self.by_skill.lock().unwrap().clone(),
        }
    }

    /// Reset all metrics.
    pub fn reset(&self) {
        *self.total_selections.lock().unwrap() = 0;
        *self.total_timeouts.lock().unwrap() = 0;
        self.by_confidence.lock().unwrap().clear();
        self.by_strategy.lock().unwrap().clear();
        self.by_skill.lock().unwrap().clear();
        *self.latency_histogram.lock().unwrap() = Histogram::new(3).unwrap();
    }

    /// Export metrics in Prometheus format.
    pub fn to_prometheus(&self) -> String {
        let summary = self.summary();
        let mut lines = Vec::new();

        lines.push(format!(
            "ase_selections_total {}",
            summary.total_selections
        ));
        lines.push(format!("ase_timeouts_total {}", summary.total_timeouts));

        for (conf, count) in &summary.by_confidence {
            lines.push(format!(
                "ase_selections_by_confidence{{confidence=\"{}\"}} {}",
                conf, count
            ));
        }

        for (strategy, count) in &summary.by_strategy {
            lines.push(format!(
                "ase_selections_by_strategy{{strategy=\"{}\"}} {}",
                strategy, count
            ));
        }

        lines.push(format!("ase_latency_p50_ms {:.3}", summary.latency_p50));
        lines.push(format!("ase_latency_p95_ms {:.3}", summary.latency_p95));
        lines.push(format!("ase_latency_p99_ms {:.3}", summary.latency_p99));

        lines.join("\n")
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

        assert!(prometheus.contains("ase_selections_total 1"));
        assert!(prometheus.contains("ase_selections_by_strategy{strategy=\"trigger\"}"));
    }
}
