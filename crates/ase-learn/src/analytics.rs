//! Usage analytics for tracking selection patterns.

use std::collections::HashMap;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use ase_core::SkillName;
use ase_select::Confidence;

use crate::error::LearnError;

/// A recorded selection event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionEvent {
    /// Timestamp.
    pub timestamp: SystemTime,

    /// User query.
    pub query: String,

    /// Selected skill (if any).
    pub selected_skill: Option<SkillName>,

    /// Selection score.
    pub score: Option<f32>,

    /// Confidence level.
    pub confidence: Option<Confidence>,

    /// Strategy that produced the result.
    pub strategy: Option<String>,

    /// Latency in milliseconds.
    pub latency_ms: u64,

    /// User feedback (if any).
    pub feedback: Option<Feedback>,

    /// Custom metadata.
    pub metadata: HashMap<String, String>,
}

/// User feedback on a selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Feedback {
    /// Selection was correct.
    Correct,

    /// Selection was incorrect.
    Incorrect {
        /// What skill should have been selected.
        expected: Option<SkillName>,
    },

    /// Selection was partially correct.
    Partial { reason: String },
}

/// Trait for analytics storage backends.
pub trait AnalyticsStore: Send + Sync {
    /// Record a selection event.
    fn record(&self, event: SelectionEvent) -> Result<(), LearnError>;

    /// Query recent events.
    fn query_recent(&self, limit: usize) -> Result<Vec<SelectionEvent>, LearnError>;

    /// Query events for a specific skill.
    fn query_by_skill(
        &self,
        skill: &SkillName,
        limit: usize,
    ) -> Result<Vec<SelectionEvent>, LearnError>;

    /// Get event count.
    fn count(&self) -> Result<usize, LearnError>;
}

/// In-memory analytics store (for testing/development).
pub struct InMemoryAnalyticsStore {
    events: std::sync::Mutex<Vec<SelectionEvent>>,
    max_events: usize,
}

impl InMemoryAnalyticsStore {
    /// Create a new in-memory store.
    pub fn new(max_events: usize) -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
            max_events,
        }
    }
}

impl Default for InMemoryAnalyticsStore {
    fn default() -> Self {
        Self::new(10000)
    }
}

impl AnalyticsStore for InMemoryAnalyticsStore {
    fn record(&self, event: SelectionEvent) -> Result<(), LearnError> {
        let mut events = self.events.lock().unwrap();

        if events.len() >= self.max_events {
            events.remove(0);
        }

        events.push(event);
        Ok(())
    }

    fn query_recent(&self, limit: usize) -> Result<Vec<SelectionEvent>, LearnError> {
        let events = self.events.lock().unwrap();
        let start = events.len().saturating_sub(limit);
        Ok(events[start..].to_vec())
    }

    fn query_by_skill(
        &self,
        skill: &SkillName,
        limit: usize,
    ) -> Result<Vec<SelectionEvent>, LearnError> {
        let events = self.events.lock().unwrap();
        let filtered: Vec<_> = events
            .iter()
            .filter(|e| e.selected_skill.as_ref() == Some(skill))
            .cloned()
            .collect();

        let start = filtered.len().saturating_sub(limit);
        Ok(filtered[start..].to_vec())
    }

    fn count(&self) -> Result<usize, LearnError> {
        Ok(self.events.lock().unwrap().len())
    }
}

/// SQLite-based analytics store.
#[cfg(feature = "analytics-sqlite")]
pub struct SqliteAnalyticsStore {
    conn: std::sync::Mutex<rusqlite::Connection>,
}

#[cfg(feature = "analytics-sqlite")]
impl SqliteAnalyticsStore {
    /// Create a new SQLite store.
    pub fn new(path: &std::path::Path) -> Result<Self, LearnError> {
        let conn = rusqlite::Connection::open(path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS selection_events (
                id INTEGER PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                query TEXT NOT NULL,
                selected_skill TEXT,
                score REAL,
                confidence TEXT,
                strategy TEXT,
                latency_ms INTEGER NOT NULL,
                feedback TEXT,
                metadata TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_timestamp ON selection_events(timestamp)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_skill ON selection_events(selected_skill)",
            [],
        )?;

        Ok(Self {
            conn: std::sync::Mutex::new(conn),
        })
    }

    /// Create an in-memory store (for testing).
    pub fn in_memory() -> Result<Self, LearnError> {
        let conn = rusqlite::Connection::open_in_memory()?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS selection_events (
                id INTEGER PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                query TEXT NOT NULL,
                selected_skill TEXT,
                score REAL,
                confidence TEXT,
                strategy TEXT,
                latency_ms INTEGER NOT NULL,
                feedback TEXT,
                metadata TEXT
            )",
            [],
        )?;

        Ok(Self {
            conn: std::sync::Mutex::new(conn),
        })
    }
}

#[cfg(feature = "analytics-sqlite")]
impl AnalyticsStore for SqliteAnalyticsStore {
    fn record(&self, event: SelectionEvent) -> Result<(), LearnError> {
        let conn = self.conn.lock().unwrap();

        let timestamp = event
            .timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO selection_events 
            (timestamp, query, selected_skill, score, confidence, strategy, latency_ms, feedback, metadata)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                timestamp,
                event.query,
                event.selected_skill.as_ref().map(|s| s.as_str()),
                event.score,
                event.confidence.map(|c| format!("{:?}", c)),
                event.strategy,
                event.latency_ms as i64,
                event.feedback.as_ref().map(|f| serde_json::to_string(f).unwrap()),
                serde_json::to_string(&event.metadata).ok(),
            ],
        )?;

        Ok(())
    }

    fn query_recent(&self, limit: usize) -> Result<Vec<SelectionEvent>, LearnError> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT timestamp, query, selected_skill, score, confidence, strategy, latency_ms, feedback, metadata
            FROM selection_events
            ORDER BY timestamp DESC
            LIMIT ?1",
        )?;

        let events: Vec<SelectionEvent> = stmt
            .query_map([limit], |row| {
                let timestamp_secs: i64 = row.get(0)?;
                let query: String = row.get(1)?;
                let selected_skill: Option<String> = row.get(2)?;
                let score: Option<f64> = row.get(3)?;
                let confidence: Option<String> = row.get(4)?;
                let strategy: Option<String> = row.get(5)?;
                let latency_ms: i64 = row.get(6)?;
                let feedback: Option<String> = row.get(7)?;
                let metadata: Option<String> = row.get(8)?;

                Ok(SelectionEvent {
                    timestamp: SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(timestamp_secs as u64),
                    query,
                    selected_skill: selected_skill.and_then(|s| SkillName::new(&s).ok()),
                    score: score.map(|s| s as f32),
                    confidence: confidence.and_then(|c| match c.as_str() {
                        "None" => Some(Confidence::None),
                        "Low" => Some(Confidence::Low),
                        "Medium" => Some(Confidence::Medium),
                        "High" => Some(Confidence::High),
                        "Definite" => Some(Confidence::Definite),
                        _ => None,
                    }),
                    strategy,
                    latency_ms: latency_ms as u64,
                    feedback: feedback.and_then(|f| serde_json::from_str(&f).ok()),
                    metadata: metadata
                        .and_then(|m| serde_json::from_str(&m).ok())
                        .unwrap_or_default(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(events)
    }

    fn query_by_skill(
        &self,
        skill: &SkillName,
        limit: usize,
    ) -> Result<Vec<SelectionEvent>, LearnError> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT timestamp, query, selected_skill, score, confidence, strategy, latency_ms, feedback, metadata
            FROM selection_events
            WHERE selected_skill = ?1
            ORDER BY timestamp DESC
            LIMIT ?2",
        )?;

        let events: Vec<SelectionEvent> = stmt
            .query_map(rusqlite::params![skill.as_str(), limit], |row| {
                let timestamp_secs: i64 = row.get(0)?;
                let query: String = row.get(1)?;
                let selected_skill: Option<String> = row.get(2)?;
                let score: Option<f64> = row.get(3)?;
                let confidence: Option<String> = row.get(4)?;
                let strategy: Option<String> = row.get(5)?;
                let latency_ms: i64 = row.get(6)?;
                let feedback: Option<String> = row.get(7)?;
                let metadata: Option<String> = row.get(8)?;

                Ok(SelectionEvent {
                    timestamp: SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_secs(timestamp_secs as u64),
                    query,
                    selected_skill: selected_skill.and_then(|s| SkillName::new(&s).ok()),
                    score: score.map(|s| s as f32),
                    confidence: confidence.and_then(|c| match c.as_str() {
                        "None" => Some(Confidence::None),
                        "Low" => Some(Confidence::Low),
                        "Medium" => Some(Confidence::Medium),
                        "High" => Some(Confidence::High),
                        "Definite" => Some(Confidence::Definite),
                        _ => None,
                    }),
                    strategy,
                    latency_ms: latency_ms as u64,
                    feedback: feedback.and_then(|f| serde_json::from_str(&f).ok()),
                    metadata: metadata
                        .and_then(|m| serde_json::from_str(&m).ok())
                        .unwrap_or_default(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(events)
    }

    fn count(&self) -> Result<usize, LearnError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM selection_events", [], |row| {
            row.get(0)
        })?;
        Ok(count as usize)
    }
}

/// High-level usage analytics interface.
pub struct UsageAnalytics {
    store: Box<dyn AnalyticsStore>,
}

impl UsageAnalytics {
    /// Create with a specific store.
    pub fn new(store: Box<dyn AnalyticsStore>) -> Self {
        Self { store }
    }

    /// Create with in-memory store.
    pub fn in_memory() -> Self {
        Self::new(Box::new(InMemoryAnalyticsStore::default()))
    }

    /// Record an event.
    pub fn record(&self, event: SelectionEvent) -> Result<(), LearnError> {
        self.store.record(event)
    }

    /// Get recent events.
    pub fn recent(&self, limit: usize) -> Result<Vec<SelectionEvent>, LearnError> {
        self.store.query_recent(limit)
    }

    /// Get events for a skill.
    pub fn by_skill(&self, skill: &SkillName, limit: usize) -> Result<Vec<SelectionEvent>, LearnError> {
        self.store.query_by_skill(skill, limit)
    }

    /// Get total event count.
    pub fn count(&self) -> Result<usize, LearnError> {
        self.store.count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_store() {
        let store = InMemoryAnalyticsStore::new(100);

        let event = SelectionEvent {
            timestamp: SystemTime::now(),
            query: "test query".to_string(),
            selected_skill: Some(SkillName::new("test-skill").unwrap()),
            score: Some(0.9),
            confidence: Some(Confidence::High),
            strategy: Some("trigger".to_string()),
            latency_ms: 10,
            feedback: None,
            metadata: HashMap::new(),
        };

        store.record(event).unwrap();

        assert_eq!(store.count().unwrap(), 1);

        let events = store.query_recent(10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].query, "test query");
    }

    #[test]
    fn test_in_memory_store_limit() {
        let store = InMemoryAnalyticsStore::new(5);

        for i in 0..10 {
            let event = SelectionEvent {
                timestamp: SystemTime::now(),
                query: format!("query {}", i),
                selected_skill: None,
                score: None,
                confidence: None,
                strategy: None,
                latency_ms: 10,
                feedback: None,
                metadata: HashMap::new(),
            };
            store.record(event).unwrap();
        }

        // Should only keep last 5
        assert_eq!(store.count().unwrap(), 5);
    }

    #[test]
    fn test_query_by_skill() {
        let store = InMemoryAnalyticsStore::default();

        let skill_a = SkillName::new("skill-a").unwrap();
        let skill_b = SkillName::new("skill-b").unwrap();

        // Add events for skill_a
        for _ in 0..3 {
            store
                .record(SelectionEvent {
                    timestamp: SystemTime::now(),
                    query: "query".to_string(),
                    selected_skill: Some(skill_a.clone()),
                    score: None,
                    confidence: None,
                    strategy: None,
                    latency_ms: 10,
                    feedback: None,
                    metadata: HashMap::new(),
                })
                .unwrap();
        }

        // Add event for skill_b
        store
            .record(SelectionEvent {
                timestamp: SystemTime::now(),
                query: "query".to_string(),
                selected_skill: Some(skill_b.clone()),
                score: None,
                confidence: None,
                strategy: None,
                latency_ms: 10,
                feedback: None,
                metadata: HashMap::new(),
            })
            .unwrap();

        let events = store.query_by_skill(&skill_a, 10).unwrap();
        assert_eq!(events.len(), 3);
    }

    #[cfg(feature = "analytics-sqlite")]
    #[test]
    fn test_sqlite_store() {
        let store = SqliteAnalyticsStore::in_memory().unwrap();

        let event = SelectionEvent {
            timestamp: SystemTime::now(),
            query: "test query".to_string(),
            selected_skill: Some(SkillName::new("test-skill").unwrap()),
            score: Some(0.9),
            confidence: Some(Confidence::High),
            strategy: Some("trigger".to_string()),
            latency_ms: 10,
            feedback: Some(Feedback::Correct),
            metadata: HashMap::new(),
        };

        store.record(event).unwrap();

        assert_eq!(store.count().unwrap(), 1);

        let events = store.query_recent(10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].query, "test query");
    }

    #[test]
    fn test_usage_analytics() {
        let analytics = UsageAnalytics::in_memory();

        let event = SelectionEvent {
            timestamp: SystemTime::now(),
            query: "test".to_string(),
            selected_skill: None,
            score: None,
            confidence: None,
            strategy: None,
            latency_ms: 10,
            feedback: None,
            metadata: HashMap::new(),
        };

        analytics.record(event).unwrap();

        assert_eq!(analytics.count().unwrap(), 1);
    }
}
