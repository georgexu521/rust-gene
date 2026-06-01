//! Read-before-edit guard — tracks which files the model has read so
//! edit_file / multi_edit / file_write can validate that SEARCH text is
//! grounded in on-disk bytes the model has actually seen.
//!
//! Mirrors Reasonix's `ReadTracker` in `src/tools/read-tracker.ts`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Per-file read record.
#[derive(Debug, Clone)]
struct ReadRecord {
    read_count: u64,
}

/// Tracks files the model has read this session.
///
/// The tracker is cleared on fold / mechanical truncation — the model's
/// byte-level view of the elided history is gone, so edit safety can't be
/// guaranteed without a fresh read.
#[derive(Debug, Default)]
pub struct ReadTracker {
    files: Mutex<HashMap<PathBuf, ReadRecord>>,
}

impl ReadTracker {
    pub fn new() -> Self {
        Self {
            files: Mutex::new(HashMap::new()),
        }
    }

    /// Record that a file was successfully read. Canonicalizes the path
    /// so `./README.md` and `/abs/path/README.md` resolve to the same key.
    pub fn mark_read(&self, path: &Path) {
        let Ok(canonical) = path.canonicalize() else {
            return;
        };
        let mut files = self.files.lock().unwrap_or_else(|poisoned| {
            // Mutex poisoning means a panic occurred in another thread.
            // The HashMap should still be consistent; recover it.
            poisoned.into_inner()
        });
        let entry = files
            .entry(canonical.clone())
            .or_insert_with(|| ReadRecord { read_count: 0 });
        entry.read_count += 1;
    }

    /// Check whether a file has been read by the model in this session.
    /// Returns `true` if the file is known (read at least once).
    pub fn was_read(&self, path: &Path) -> bool {
        let Ok(canonical) = path.canonicalize() else {
            // If the path doesn't exist on disk, it's a new-file create —
            // that's always allowed (no SEARCH text to match).
            return true;
        };
        let files = self.files.lock().unwrap_or_else(|p| p.into_inner());
        files.contains_key(&canonical)
    }

    /// Validate that an edit is safe: a non-empty SEARCH block requires a
    /// prior read of the target file. Creating new files (empty SEARCH) or
    /// files that don't exist yet are always allowed.
    ///
    /// Returns `Ok(())` if the edit should proceed, or `Err(message)` to
    /// return to the model.
    pub fn check_edit(&self, path: &Path, search_text: &str) -> Result<(), String> {
        // Empty SEARCH → new file creation, always safe.
        if search_text.is_empty() {
            return Ok(());
        }

        // File doesn't exist yet → SEARCH won't match, but don't block —
        // the edit gate itself will produce a clear "file-missing" status.
        if !path.exists() {
            return Ok(());
        }

        if !self.was_read(path) {
            let display = path.display();
            return Err(format!(
                "Read-before-edit guard: `{display}` was not read in this session. \
                 Read the file first with file_read so the SEARCH text has matching \
                 on-disk bytes; otherwise the edit will fail with 'not-found'. \
                 If you intend to create a new file, use an empty SEARCH block."
            ));
        }

        Ok(())
    }

    /// Number of tracked files.
    #[cfg(test)]
    pub fn tracked_count(&self) -> usize {
        self.files.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    /// Clear all tracked reads. Called on fold / mechanical truncation.
    pub fn reset(&self) {
        self.files
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn mark_read_tracks_file() {
        let tracker = ReadTracker::new();
        let dir = std::env::temp_dir().join("read_tracker_test");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.md");
        fs::write(&file, "# Hello").unwrap();

        tracker.mark_read(&file);
        assert!(tracker.was_read(&file));
        assert_eq!(tracker.tracked_count(), 1);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unread_file_is_blocked() {
        let tracker = ReadTracker::new();
        let dir = std::env::temp_dir().join("read_tracker_unread");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("unread.md");
        fs::write(&file, "# Never seen").unwrap();

        let err = tracker
            .check_edit(&file, "# Never seen")
            .unwrap_err();
        assert!(err.contains("was not read"));
        assert!(err.contains("file_read"));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn empty_search_is_always_allowed() {
        let tracker = ReadTracker::new();
        let dir = std::env::temp_dir().join("read_tracker_empty");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("new.md");

        // Empty SEARCH → new file creation, always OK.
        assert!(tracker.check_edit(&file, "").is_ok());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn read_then_edit_is_allowed() {
        let tracker = ReadTracker::new();
        let dir = std::env::temp_dir().join("read_tracker_allowed");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("allowed.md");
        fs::write(&file, "# Read first").unwrap();

        tracker.mark_read(&file);
        assert!(tracker
            .check_edit(&file, "# Read first")
            .is_ok());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reset_clears_all() {
        let tracker = ReadTracker::new();
        let dir = std::env::temp_dir().join("read_tracker_reset");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("reset.md");
        fs::write(&file, "# Will reset").unwrap();

        tracker.mark_read(&file);
        assert_eq!(tracker.tracked_count(), 1);
        tracker.reset();
        assert_eq!(tracker.tracked_count(), 0);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn nonexistent_file_edit_is_allowed() {
        let tracker = ReadTracker::new();
        let dir = std::env::temp_dir().join("read_tracker_noexist");
        let file = dir.join("ghost.md");

        // File doesn't exist — let the edit gate handle it.
        assert!(tracker
            .check_edit(&file, "some search text")
            .is_ok());
    }
}
