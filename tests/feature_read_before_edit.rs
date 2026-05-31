//! Integration test: Read-before-Edit enforcement (Phase A #2).
//!
//! Verifies that file_write and file_edit are blocked when the target
//! file has not been read in the current session.

use priority_agent::tools::file_tool::{check_read_before_write, mark_file_read};
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct ReadBeforeEditEnv {
    previous: Option<String>,
}

impl ReadBeforeEditEnv {
    fn set(value: &str) -> Self {
        let previous = std::env::var("PRIORITY_AGENT_READ_BEFORE_EDIT").ok();
        std::env::set_var("PRIORITY_AGENT_READ_BEFORE_EDIT", value);
        Self { previous }
    }
}

impl Drop for ReadBeforeEditEnv {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            std::env::set_var("PRIORITY_AGENT_READ_BEFORE_EDIT", previous);
        } else {
            std::env::remove_var("PRIORITY_AGENT_READ_BEFORE_EDIT");
        }
    }
}

#[test]
fn write_without_read_is_blocked() {
    let _lock = ENV_LOCK.lock().unwrap();
    // Ensure the feature is enabled (other tests may have disabled it).
    let _env = ReadBeforeEditEnv::set("1");

    // No prior read — should be blocked.
    let result = check_read_before_write("test-session-1", "/tmp/test-file-1.txt");
    assert!(
        result.is_some(),
        "should block write without prior read, got None (check was skipped)"
    );
    assert!(
        !result.unwrap().success,
        "result should be an error (success=false)"
    );
}

#[test]
fn write_after_read_is_allowed() {
    let _lock = ENV_LOCK.lock().unwrap();
    let _env = ReadBeforeEditEnv::set("1");

    let session = "test-session-read-2";
    let path = "/tmp/test-file-read-2.txt";

    // Mark file as read first.
    mark_file_read(session, path);

    // Now write should be allowed.
    let result = check_read_before_write(session, path);
    assert!(
        result.is_none(),
        "should allow write after read, got: {:?}",
        result
    );
}

#[test]
fn read_before_edit_disabled_by_env() {
    let _lock = ENV_LOCK.lock().unwrap();
    // Set env to disable the check.
    let _env = ReadBeforeEditEnv::set("0");

    let result = check_read_before_write("no-env-session", "/tmp/no-env.txt");
    assert!(result.is_none(), "should skip check when env var is 0");
}
