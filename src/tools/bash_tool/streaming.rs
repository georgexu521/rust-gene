//! Streaming output buffer for bash commands.
//!
//! Accumulates stdout/stderr during command execution.  When the combined
//! in-memory buffer exceeds `max_chars`, the accumulated prefix is written
//! to a timestamped artifact file and subsequent chunks are appended
//! directly to disk.  At the end, the caller gets a tail preview of the
//! last N characters plus the artifact path.

use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

/// Configuration for the streaming output buffer.
pub struct StreamingConfig {
    /// Max chars to keep in memory before switching to disk mode.
    pub max_chars: usize,
    /// Number of tail chars to preserve in memory for preview.
    pub tail_preview_chars: usize,
    /// Directory where artifact files are written.
    pub artifact_dir: PathBuf,
    /// Filename stem (timestamp and hash are appended automatically).
    pub file_stem: String,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            max_chars: 10_000,
            tail_preview_chars: 2_000,
            artifact_dir: PathBuf::from(".priority-agent/tool-results"),
            file_stem: "bash-stream".to_string(),
        }
    }
}

/// Accumulates output during command execution with automatic spill-to-disk.
pub struct StreamingOutput {
    buffer: String,
    file: Option<File>,
    artifact_path: Option<PathBuf>,
    artifact_dir: PathBuf,
    file_stem: String,
    max_chars: usize,
    tail_chars: usize,
    total_bytes: usize,
    truncated: bool,
}

impl StreamingOutput {
    pub fn new(cfg: StreamingConfig) -> Self {
        Self {
            buffer: String::with_capacity(cfg.max_chars),
            file: None,
            artifact_path: None,
            artifact_dir: cfg.artifact_dir,
            file_stem: cfg.file_stem,
            max_chars: cfg.max_chars,
            tail_chars: cfg.tail_preview_chars,
            total_bytes: 0,
            truncated: false,
        }
    }

    /// Append a chunk of output.
    ///
    /// When the in-memory buffer exceeds `max_chars`, the accumulated prefix
    /// is flushed to a file and subsequent writes go directly to disk.  The
    /// tail buffer is kept in memory for the final preview.
    pub fn write(&mut self, chunk: &str) -> io::Result<()> {
        if chunk.is_empty() {
            return Ok(());
        }
        self.total_bytes += chunk.len();

        if !self.truncated && self.buffer.len() + chunk.len() > self.max_chars {
            // Trigger disk spill.
            self.ensure_file_open()?;
            // Now self.file is guaranteed Some.
            self.file
                .as_mut()
                .expect("file must be open after spill")
                .write_all(self.buffer.as_bytes())?;
            self.truncated = true;
        }

        if self.truncated {
            self.file
                .as_mut()
                .expect("file must be open after spill")
                .write_all(chunk.as_bytes())?;

            // Keep tail preview in memory.
            self.buffer.push_str(chunk);
            if self.buffer.len() > self.tail_chars {
                let trim = self.buffer.len() - self.tail_chars;
                // Find a safe UTF-8 boundary.
                let mut boundary = trim;
                while boundary < self.buffer.len() && !self.buffer.is_char_boundary(boundary) {
                    boundary += 1;
                }
                self.buffer = self.buffer[boundary..].to_string();
            }
        } else {
            self.buffer.push_str(chunk);
        }
        Ok(())
    }

    /// Whether the output was truncated (spilled to disk).
    pub fn was_truncated(&self) -> bool {
        self.truncated
    }

    /// Total bytes accumulated.
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Get the artifact path, if output was written to disk.
    pub fn artifact_path(&self) -> Option<&PathBuf> {
        self.artifact_path.as_ref()
    }

    /// Get a preview of the output suitable for the LLM.
    ///
    /// Returns the last `tail_chars` from the buffer.
    pub fn preview(&self) -> &str {
        &self.buffer
    }

    /// Build the full LLM-facing preview message.
    pub fn build_preview_message(&self) -> String {
        if !self.truncated {
            return self.buffer.clone();
        }

        let mut msg = format!(
            "... [output truncated: {} bytes total] ...\n\n",
            self.total_bytes
        );

        if let Some(path) = &self.artifact_path {
            msg.push_str(&format!(
                "Full output saved to: {}\n\
                 Use grep to search or file_read with offset/limit to inspect specific sections.\n\
                 Do NOT read the entire file into context.\n\n",
                path.display()
            ));
        }

        msg.push_str("--- Tail preview ---\n");
        msg.push_str(&self.buffer);
        msg
    }

    fn ensure_file_open(&mut self) -> io::Result<()> {
        if self.file.is_none() {
            std::fs::create_dir_all(&self.artifact_dir)?;
            let millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let path = self
                .artifact_dir
                .join(format!("{}-{}.log", self.file_stem, millis));
            let file = std::fs::File::create(&path)?;
            self.artifact_path = Some(path);
            self.file = Some(file);
        }
        Ok(())
    }
}

impl Drop for StreamingOutput {
    fn drop(&mut self) {
        // Flush any remaining buffered data to the file on drop.
        if let Some(ref mut f) = self.file {
            let _ = f.write_all(self.buffer.as_bytes());
            let _ = f.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> StreamingConfig {
        let tmp = std::env::temp_dir().join(format!(
            "priority-agent-stream-test-{}",
            uuid::Uuid::new_v4()
        ));
        StreamingConfig {
            max_chars: 50,
            tail_preview_chars: 20,
            artifact_dir: tmp,
            file_stem: "test-stream".to_string(),
        }
    }

    #[test]
    fn small_output_stays_in_memory() {
        let mut out = StreamingOutput::new(test_config());
        out.write("hello").unwrap();
        assert!(!out.was_truncated());
        assert_eq!(out.preview(), "hello");
    }

    #[test]
    fn large_output_spills_to_disk() {
        let mut out = StreamingOutput::new(test_config());
        let big = "x".repeat(60);
        out.write(&big).unwrap();
        assert!(out.was_truncated());
        assert!(out.artifact_path().is_some());
        assert!(out.total_bytes() >= 60);
        // Tail preview should only show last ~20 chars.
        assert!(out.preview().len() <= 25);
    }

    #[test]
    fn tail_preview_shows_end_of_output() {
        let mut out = StreamingOutput::new(test_config());
        out.write("AAAAA BBBBB CCCCC DDDDD EEEEE FFFFF GGGGG HHHHH IIIII JJJJJ KKKKK")
            .unwrap();
        let preview = out.preview().to_string();
        // Last chunk should be in the tail.
        assert!(preview.contains("KKKKK") || preview.contains("JJJJJ"));
    }

    #[test]
    fn preview_message_includes_artifact_path() {
        let mut out = StreamingOutput::new(test_config());
        let big = "x".repeat(100);
        out.write(&big).unwrap();
        let _ = out.artifact_path().unwrap();
        let msg = out.build_preview_message();
        assert!(msg.contains("truncated"));
        assert!(msg.contains("Full output saved to"));
        assert!(msg.contains("grep"));
        assert!(msg.contains("file_read"));
    }

    #[test]
    fn empty_chunks_are_no_ops() {
        let mut out = StreamingOutput::new(test_config());
        out.write("").unwrap();
        out.write("").unwrap();
        assert_eq!(out.total_bytes(), 0);
    }
}
