//! In-memory skill registry with lazy loading and filesystem watching.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{mpsc, OnceCell, RwLock};
use tracing::{debug, info, warn};

use crate::error::CoreError;
use crate::parser::SkillParser;
use crate::schema::{Skill, SkillMetadata, SkillName, SkillStats};
use crate::watcher::WatchEvent;

/// Internal entry tracking loading state for a skill.
struct SkillEntry {
    /// Metadata is always loaded (Level 0).
    metadata: SkillMetadata,

    /// Full skill is lazily loaded on first activation.
    full: OnceCell<Skill>,

    /// Selection/usage statistics.
    stats: SkillStats,
}

/// Report from a refresh operation.
#[derive(Debug, Clone, Default)]
pub struct RefreshReport {
    /// Skills that were added.
    pub added: Vec<SkillName>,

    /// Skills that were updated.
    pub updated: Vec<SkillName>,

    /// Skills that were removed.
    pub removed: Vec<SkillName>,

    /// Errors encountered during refresh.
    pub errors: Vec<(PathBuf, String)>,
}

impl RefreshReport {
    /// Check if any changes occurred.
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.updated.is_empty() || !self.removed.is_empty()
    }
}

/// In-memory skill registry with lazy loading and optional filesystem watching.
pub struct SkillRegistry {
    /// Registered skills by name.
    skills: RwLock<HashMap<SkillName, SkillEntry>>,

    /// Directories being watched.
    directories: Vec<PathBuf>,

    /// Parser instance.
    parser: SkillParser,

    /// Filesystem watcher (if enabled).
    watcher: Option<RecommendedWatcher>,

    /// Channel for watch events.
    watch_rx: Option<mpsc::Receiver<WatchEvent>>,
}

impl SkillRegistry {
    /// Create a new registry scanning the given directories.
    ///
    /// This performs an initial scan of all directories, loading metadata
    /// for all found SKILL.md files. Full skill content is loaded lazily.
    pub async fn new<P: AsRef<Path>>(dirs: &[P]) -> Result<Self, CoreError> {
        let directories: Vec<PathBuf> = dirs.iter().map(|p| p.as_ref().to_path_buf()).collect();

        let mut registry = Self {
            skills: RwLock::new(HashMap::new()),
            directories: directories.clone(),
            parser: SkillParser::new(),
            watcher: None,
            watch_rx: None,
        };

        // Initial scan
        for dir in &directories {
            if dir.exists() {
                registry.scan_directory(dir).await?;
            } else {
                warn!("Skill directory does not exist: {:?}", dir);
            }
        }

        Ok(registry)
    }

    /// Create a registry with filesystem watching enabled.
    pub async fn with_watch<P: AsRef<Path>>(dirs: &[P]) -> Result<Self, CoreError> {
        let mut registry = Self::new(dirs).await?;
        registry.enable_watch()?;
        Ok(registry)
    }

    /// Enable filesystem watching for auto-refresh.
    pub fn enable_watch(&mut self) -> Result<(), CoreError> {
        let (tx, rx) = mpsc::channel(100);

        let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
            if let Ok(event) = res {
                if let Some(watch_event) = WatchEvent::from_notify(&event) {
                    let _ = tx.blocking_send(watch_event);
                }
            }
        })?;

        // Watch all directories
        for dir in &self.directories {
            if dir.exists() {
                watcher.watch(dir, RecursiveMode::Recursive)?;
                debug!("Watching directory: {:?}", dir);
            }
        }

        self.watcher = Some(watcher);
        self.watch_rx = Some(rx);

        Ok(())
    }

    /// Process any pending watch events.
    /// Returns a RefreshReport if any changes were processed.
    pub async fn process_watch_events(&mut self) -> Option<RefreshReport> {
        let rx = self.watch_rx.as_mut()?;
        let mut events = Vec::new();

        // Drain all available events
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        if events.is_empty() {
            return None;
        }

        // Process events
        let mut report = RefreshReport::default();

        for event in events {
            match event {
                WatchEvent::Created(path) | WatchEvent::Modified(path) => {
                    if path.file_name().map(|n| n == "SKILL.md").unwrap_or(false) {
                        match self.load_skill_file(&path).await {
                            Ok(Some(name)) => {
                                if report.added.contains(&name) || report.updated.contains(&name) {
                                    // Already processed
                                } else {
                                    let skills = self.skills.read().await;
                                    if skills.contains_key(&name) {
                                        report.updated.push(name);
                                    } else {
                                        drop(skills);
                                        report.added.push(name);
                                    }
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                report.errors.push((path, e.to_string()));
                            }
                        }
                    }
                }
                WatchEvent::Deleted(path) => {
                    if path.file_name().map(|n| n == "SKILL.md").unwrap_or(false) {
                        let mut skills = self.skills.write().await;
                        let to_remove: Vec<SkillName> = skills
                            .iter()
                            .filter(|(_, e)| e.metadata.source_path == path)
                            .map(|(n, _)| n.clone())
                            .collect();

                        for name in to_remove {
                            skills.remove(&name);
                            report.removed.push(name);
                        }
                    }
                }
            }
        }

        if report.has_changes() {
            info!(
                "Watch refresh: {} added, {} updated, {} removed",
                report.added.len(),
                report.updated.len(),
                report.removed.len()
            );
            Some(report)
        } else {
            None
        }
    }

    /// Reload skills from disk. Called on filesystem change or manually.
    pub async fn refresh(&mut self) -> Result<RefreshReport, CoreError> {
        let mut report = RefreshReport::default();

        // Track which skills we've seen
        let mut seen_skills = std::collections::HashSet::new();

        // Scan all directories
        for dir in self.directories.clone() {
            if !dir.exists() {
                continue;
            }

            let skill_files = find_skill_files(&dir);

            for path in skill_files {
                match self.parser.parse_metadata(&path) {
                    Ok(metadata) => {
                        let name = metadata.name.clone();
                        seen_skills.insert(name.clone());

                        let mut skills = self.skills.write().await;

                        // Check if skill exists and get old stats if content changed
                        let existing_stats = skills.get(&name).and_then(|existing| {
                            if existing.metadata.content_hash != metadata.content_hash {
                                Some(existing.stats.clone())
                            } else {
                                None
                            }
                        });

                        if let Some(stats) = existing_stats {
                            // Update entry (reset lazy-loaded content)
                            skills.insert(
                                name.clone(),
                                SkillEntry {
                                    metadata,
                                    full: OnceCell::new(),
                                    stats,
                                },
                            );
                            report.updated.push(name);
                        } else if !skills.contains_key(&name) {
                            // New skill
                            skills.insert(
                                name.clone(),
                                SkillEntry {
                                    metadata,
                                    full: OnceCell::new(),
                                    stats: SkillStats::default(),
                                },
                            );
                            report.added.push(name);
                        }
                    }
                    Err(e) => {
                        report.errors.push((path, e.to_string()));
                    }
                }
            }
        }

        // Remove skills that no longer exist
        let mut skills = self.skills.write().await;
        let to_remove: Vec<SkillName> = skills
            .keys()
            .filter(|name| !seen_skills.contains(*name))
            .cloned()
            .collect();

        for name in to_remove {
            skills.remove(&name);
            report.removed.push(name);
        }

        if report.has_changes() {
            info!(
                "Refresh complete: {} added, {} updated, {} removed, {} errors",
                report.added.len(),
                report.updated.len(),
                report.removed.len(),
                report.errors.len()
            );
        }

        Ok(report)
    }

    /// Get metadata for all registered skills.
    pub async fn catalog(&self) -> Vec<SkillMetadata> {
        let skills = self.skills.read().await;
        skills.values().map(|e| e.metadata.clone()).collect()
    }

    /// Get metadata for a specific skill.
    pub async fn get_metadata(&self, name: &SkillName) -> Option<SkillMetadata> {
        let skills = self.skills.read().await;
        skills.get(name).map(|e| e.metadata.clone())
    }

    /// Get a fully-loaded skill by name (triggers progressive loading).
    pub async fn get(&self, name: &SkillName) -> Result<Arc<Skill>, CoreError> {
        let skills = self.skills.read().await;

        let entry = skills.get(name).ok_or_else(|| CoreError::NotFound(name.clone()))?;

        // Lazy load the full skill
        let skill = entry
            .full
            .get_or_try_init(|| async {
                self.parser.parse_file(&entry.metadata.source_path).map_err(CoreError::from)
            })
            .await?;

        Ok(Arc::new(skill.clone()))
    }

    /// Register a skill programmatically (not from filesystem).
    pub async fn register(&self, skill: Skill) -> Result<(), CoreError> {
        let mut skills = self.skills.write().await;

        if skills.contains_key(&skill.name) {
            return Err(CoreError::Duplicate(skill.name.clone()));
        }

        let content = format!(
            "---\nname: {}\ndescription: {}\n---\n\n{}",
            skill.name, skill.description, skill.instructions
        );
        let content_hash = xxhash_rust::xxh64::xxh64(content.as_bytes(), 0);

        let metadata = SkillMetadata::from_skill(&skill, content_hash);
        let name = skill.name.clone();

        let full = OnceCell::new();
        full.set(skill).ok(); // Set the already-loaded skill

        skills.insert(
            name,
            SkillEntry {
                metadata,
                full,
                stats: SkillStats::default(),
            },
        );

        Ok(())
    }

    /// Deregister a skill.
    pub async fn deregister(&self, name: &SkillName) -> Result<(), CoreError> {
        let mut skills = self.skills.write().await;

        if skills.remove(name).is_none() {
            return Err(CoreError::NotFound(name.clone()));
        }

        Ok(())
    }

    /// Number of registered skills.
    pub async fn len(&self) -> usize {
        self.skills.read().await.len()
    }

    /// Check if registry is empty.
    pub async fn is_empty(&self) -> bool {
        self.skills.read().await.is_empty()
    }

    /// Get skill names.
    pub async fn names(&self) -> Vec<SkillName> {
        self.skills.read().await.keys().cloned().collect()
    }

    /// Get statistics for a skill.
    pub async fn get_stats(&self, name: &SkillName) -> Option<SkillStats> {
        let skills = self.skills.read().await;
        skills.get(name).map(|e| e.stats.clone())
    }

    /// Update statistics for a skill.
    pub async fn update_stats<F>(&self, name: &SkillName, f: F)
    where
        F: FnOnce(&mut SkillStats),
    {
        let mut skills = self.skills.write().await;
        if let Some(entry) = skills.get_mut(name) {
            f(&mut entry.stats);
        }
    }

    /// Scan a directory for SKILL.md files.
    async fn scan_directory(&mut self, dir: &Path) -> Result<(), CoreError> {
        let skill_files = find_skill_files(dir);

        for path in skill_files {
            if let Err(e) = self.load_skill_file(&path).await {
                warn!("Failed to load {:?}: {}", path, e);
            }
        }

        Ok(())
    }

    /// Load a single skill file.
    async fn load_skill_file(&self, path: &Path) -> Result<Option<SkillName>, CoreError> {
        match self.parser.parse_metadata(path) {
            Ok(metadata) => {
                let name = metadata.name.clone();
                let mut skills = self.skills.write().await;

                skills.insert(
                    name.clone(),
                    SkillEntry {
                        metadata,
                        full: OnceCell::new(),
                        stats: SkillStats::default(),
                    },
                );

                debug!("Loaded skill: {} from {:?}", name, path);
                Ok(Some(name))
            }
            Err(e) => {
                warn!("Failed to parse {:?}: {}", path, e);
                Err(e.into())
            }
        }
    }
}

/// Find all SKILL.md files in a directory (recursive).
fn find_skill_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                // Recurse into subdirectories
                files.extend(find_skill_files(&path));
            } else if path.file_name().map(|n| n == "SKILL.md").unwrap_or(false) {
                files.push(path);
            }
        }
    }

    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const TEST_SKILL: &str = r#"---
name: test-skill
description: A test skill for testing
metadata:
  triggers: "test, testing"
---

# Test Skill

This is a test skill.
"#;

    fn create_skill_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let skill_dir = dir.join(name);
        fs::create_dir_all(&skill_dir).unwrap();

        let skill_file = skill_dir.join("SKILL.md");
        fs::write(&skill_file, content).unwrap();

        skill_file
    }

    #[tokio::test]
    async fn test_registry_new_empty() {
        let temp = TempDir::new().unwrap();
        let registry = SkillRegistry::new(&[temp.path()]).await.unwrap();

        assert!(registry.is_empty().await);
        assert_eq!(registry.len().await, 0);
    }

    #[tokio::test]
    async fn test_registry_scan() {
        let temp = TempDir::new().unwrap();
        create_skill_file(temp.path(), "test-skill", TEST_SKILL);

        let registry = SkillRegistry::new(&[temp.path()]).await.unwrap();

        assert_eq!(registry.len().await, 1);

        let names = registry.names().await;
        assert!(names.iter().any(|n| n.as_str() == "test-skill"));
    }

    #[tokio::test]
    async fn test_registry_catalog() {
        let temp = TempDir::new().unwrap();
        create_skill_file(temp.path(), "test-skill", TEST_SKILL);

        let registry = SkillRegistry::new(&[temp.path()]).await.unwrap();
        let catalog = registry.catalog().await;

        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].name.as_str(), "test-skill");
        assert_eq!(catalog[0].description, "A test skill for testing");
        assert_eq!(catalog[0].triggers, vec!["test", "testing"]);
    }

    #[tokio::test]
    async fn test_registry_get() {
        let temp = TempDir::new().unwrap();
        create_skill_file(temp.path(), "test-skill", TEST_SKILL);

        let registry = SkillRegistry::new(&[temp.path()]).await.unwrap();

        let name = SkillName::new("test-skill").unwrap();
        let skill = registry.get(&name).await.unwrap();

        assert_eq!(skill.name.as_str(), "test-skill");
        assert!(skill.instructions.contains("This is a test skill"));
    }

    #[tokio::test]
    async fn test_registry_get_not_found() {
        let temp = TempDir::new().unwrap();
        let registry = SkillRegistry::new(&[temp.path()]).await.unwrap();

        let name = SkillName::new("nonexistent").unwrap();
        let result = registry.get(&name).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CoreError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_registry_register() {
        let temp = TempDir::new().unwrap();
        let registry = SkillRegistry::new(&[temp.path()]).await.unwrap();

        let skill = Skill {
            name: SkillName::new("custom-skill").unwrap(),
            description: "A custom registered skill".to_string(),
            license: None,
            compatibility: None,
            metadata: HashMap::new(),
            instructions: "Do something custom".to_string(),
            source_path: PathBuf::new(),
        };

        registry.register(skill).await.unwrap();

        assert_eq!(registry.len().await, 1);

        let name = SkillName::new("custom-skill").unwrap();
        let loaded = registry.get(&name).await.unwrap();
        assert_eq!(loaded.description, "A custom registered skill");
    }

    #[tokio::test]
    async fn test_registry_register_duplicate() {
        let temp = TempDir::new().unwrap();
        let registry = SkillRegistry::new(&[temp.path()]).await.unwrap();

        let skill = Skill {
            name: SkillName::new("dup-skill").unwrap(),
            description: "First".to_string(),
            license: None,
            compatibility: None,
            metadata: HashMap::new(),
            instructions: String::new(),
            source_path: PathBuf::new(),
        };

        registry.register(skill.clone()).await.unwrap();

        let result = registry.register(skill).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CoreError::Duplicate(_)));
    }

    #[tokio::test]
    async fn test_registry_deregister() {
        let temp = TempDir::new().unwrap();
        create_skill_file(temp.path(), "test-skill", TEST_SKILL);

        let registry = SkillRegistry::new(&[temp.path()]).await.unwrap();
        assert_eq!(registry.len().await, 1);

        let name = SkillName::new("test-skill").unwrap();
        registry.deregister(&name).await.unwrap();

        assert_eq!(registry.len().await, 0);
    }

    #[tokio::test]
    async fn test_registry_refresh() {
        let temp = TempDir::new().unwrap();
        create_skill_file(temp.path(), "skill-1", TEST_SKILL);

        let mut registry = SkillRegistry::new(&[temp.path()]).await.unwrap();
        assert_eq!(registry.len().await, 1);

        // Add another skill
        let skill2 = TEST_SKILL.replace("test-skill", "skill-2");
        create_skill_file(temp.path(), "skill-2", &skill2);

        let report = registry.refresh().await.unwrap();
        assert!(report.added.iter().any(|n| n.as_str() == "skill-2"));
        assert_eq!(registry.len().await, 2);
    }

    #[tokio::test]
    async fn test_registry_stats() {
        let temp = TempDir::new().unwrap();
        create_skill_file(temp.path(), "test-skill", TEST_SKILL);

        let registry = SkillRegistry::new(&[temp.path()]).await.unwrap();
        let name = SkillName::new("test-skill").unwrap();

        // Initial stats
        let stats = registry.get_stats(&name).await.unwrap();
        assert_eq!(stats.selection_count, 0);

        // Update stats
        registry
            .update_stats(&name, |s| {
                s.selection_count = 5;
                s.activation_count = 2;
            })
            .await;

        let stats = registry.get_stats(&name).await.unwrap();
        assert_eq!(stats.selection_count, 5);
        assert_eq!(stats.activation_count, 2);
    }

    #[tokio::test]
    async fn test_registry_multiple_directories() {
        let temp1 = TempDir::new().unwrap();
        let temp2 = TempDir::new().unwrap();

        create_skill_file(temp1.path(), "skill-1", TEST_SKILL);
        let skill2 = TEST_SKILL.replace("test-skill", "skill-2");
        create_skill_file(temp2.path(), "skill-2", &skill2);

        let registry = SkillRegistry::new(&[temp1.path(), temp2.path()]).await.unwrap();

        assert_eq!(registry.len().await, 2);

        let names = registry.names().await;
        assert!(names.iter().any(|n| n.as_str() == "test-skill"));
        assert!(names.iter().any(|n| n.as_str() == "skill-2"));
    }

    #[tokio::test]
    async fn test_registry_nested_directories() {
        let temp = TempDir::new().unwrap();

        // Create nested structure: skills/category/skill-name/SKILL.md
        let nested_path = temp.path().join("category").join("nested-skill");
        fs::create_dir_all(&nested_path).unwrap();
        fs::write(nested_path.join("SKILL.md"), TEST_SKILL).unwrap();

        let registry = SkillRegistry::new(&[temp.path()]).await.unwrap();

        assert_eq!(registry.len().await, 1);
    }
}
