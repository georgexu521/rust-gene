//! Integration test: Read-before-Edit enforcement (Phase A #2).
//!
//! Verifies that file_write and file_edit are blocked when the target
//! file has not been read in the current session.

use priority_agent::tools::file_tool::{check_read_before_write, mark_file_read};

#[test]
fn write_without_read_is_blocked() {
    // Ensure the feature is enabled (other tests may have disabled it).
    std::env::set_var("PRIORITY_AGENT_READ_BEFORE_EDIT", "1");

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
    std::env::set_var("PRIORITY_AGENT_READ_BEFORE_EDIT", "1");

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
    // Set env to disable the check.
    std::env::set_var("PRIORITY_AGENT_READ_BEFORE_EDIT", "0");

    let result = check_read_before_write("no-env-session", "/tmp/no-env.txt");
    assert!(result.is_none(), "should skip check when env var is 0");

    std::env::remove_var("PRIORITY_AGENT_READ_BEFORE_EDIT");
}
