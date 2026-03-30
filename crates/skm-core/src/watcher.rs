//! Filesystem watcher for SKILL.md file changes.

use std::path::PathBuf;

use notify::event::{CreateKind, ModifyKind, RemoveKind};
use notify::EventKind;

/// Watch events for skill files.
#[derive(Debug, Clone)]
pub enum WatchEvent {
    /// A SKILL.md file was created.
    Created(PathBuf),

    /// A SKILL.md file was modified.
    Modified(PathBuf),

    /// A SKILL.md file was deleted.
    Deleted(PathBuf),
}

impl WatchEvent {
    /// Convert a notify event to a WatchEvent, if applicable.
    pub fn from_notify(event: &notify::Event) -> Option<Self> {
        // Filter to only SKILL.md files
        let skill_paths: Vec<_> = event
            .paths
            .iter()
            .filter(|p| p.file_name().map(|n| n == "SKILL.md").unwrap_or(false))
            .cloned()
            .collect();

        if skill_paths.is_empty() {
            return None;
        }

        // Take the first matching path
        let path = skill_paths.into_iter().next()?;

        match event.kind {
            // File created
            EventKind::Create(CreateKind::File) => Some(WatchEvent::Created(path)),

            // File modified
            EventKind::Modify(ModifyKind::Data(_)) | EventKind::Modify(ModifyKind::Any) => {
                Some(WatchEvent::Modified(path))
            }

            // File removed
            EventKind::Remove(RemoveKind::File) => Some(WatchEvent::Deleted(path)),

            // Ignore other events
            _ => None,
        }
    }

    /// Get the path associated with this event.
    pub fn path(&self) -> &PathBuf {
        match self {
            WatchEvent::Created(p) => p,
            WatchEvent::Modified(p) => p,
            WatchEvent::Deleted(p) => p,
        }
    }

    /// Check if this is a create event.
    pub fn is_created(&self) -> bool {
        matches!(self, WatchEvent::Created(_))
    }

    /// Check if this is a modify event.
    pub fn is_modified(&self) -> bool {
        matches!(self, WatchEvent::Modified(_))
    }

    /// Check if this is a delete event.
    pub fn is_deleted(&self) -> bool {
        matches!(self, WatchEvent::Deleted(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, AccessMode};

    fn make_event(kind: EventKind, path: &str) -> notify::Event {
        notify::Event {
            kind,
            paths: vec![PathBuf::from(path)],
            attrs: Default::default(),
        }
    }

    #[test]
    fn test_watch_event_create() {
        let event = make_event(
            EventKind::Create(CreateKind::File),
            "/skills/test/SKILL.md",
        );

        let watch_event = WatchEvent::from_notify(&event).unwrap();
        assert!(watch_event.is_created());
        assert_eq!(watch_event.path(), &PathBuf::from("/skills/test/SKILL.md"));
    }

    #[test]
    fn test_watch_event_modify() {
        let event = make_event(
            EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            "/skills/test/SKILL.md",
        );

        let watch_event = WatchEvent::from_notify(&event).unwrap();
        assert!(watch_event.is_modified());
    }

    #[test]
    fn test_watch_event_delete() {
        let event = make_event(
            EventKind::Remove(RemoveKind::File),
            "/skills/test/SKILL.md",
        );

        let watch_event = WatchEvent::from_notify(&event).unwrap();
        assert!(watch_event.is_deleted());
    }

    #[test]
    fn test_watch_event_ignore_non_skill() {
        let event = make_event(
            EventKind::Create(CreateKind::File),
            "/skills/test/README.md",
        );

        let watch_event = WatchEvent::from_notify(&event);
        assert!(watch_event.is_none());
    }

    #[test]
    fn test_watch_event_ignore_access() {
        let event = make_event(
            EventKind::Access(AccessKind::Read),
            "/skills/test/SKILL.md",
        );

        let watch_event = WatchEvent::from_notify(&event);
        assert!(watch_event.is_none());
    }

    #[test]
    fn test_watch_event_multiple_paths() {
        let event = notify::Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![
                PathBuf::from("/skills/a/README.md"),
                PathBuf::from("/skills/b/SKILL.md"),
            ],
            attrs: Default::default(),
        };

        let watch_event = WatchEvent::from_notify(&event).unwrap();
        assert!(watch_event.is_created());
        assert_eq!(watch_event.path(), &PathBuf::from("/skills/b/SKILL.md"));
    }
}
