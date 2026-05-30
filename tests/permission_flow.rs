//! Integration test: permission flow.
//!
//! Validates PermissionRules priority (deny > allow > ask) and PermissionMode behavior.

use priority_agent::permissions::{
    PermissionContext, PermissionDecision, PermissionMode, PermissionRules,
};

#[test]
fn permission_rules_deny_overrides_allow() {
    let rules = PermissionRules::new().allow("file_read").deny("file_read");

    let decision = rules.check("file_read");
    assert_eq!(
        decision,
        PermissionDecision::Deny,
        "deny should override allow"
    );
}

#[test]
fn permission_rules_allow_takes_priority_over_ask() {
    let rules = PermissionRules::new().ask("file_read").allow("file_read");

    let decision = rules.check("file_read");
    assert_eq!(
        decision,
        PermissionDecision::Allow,
        "allow should override ask"
    );
}

#[test]
fn permission_rules_unmatched_returns_ask() {
    let rules = PermissionRules::new().allow("file_read").deny("bash");

    let decision = rules.check("unknown_tool");
    assert_eq!(
        decision,
        PermissionDecision::Ask,
        "unmatched tools should default to Ask"
    );
}

#[test]
fn permission_wildcard_matches() {
    let rules = PermissionRules::new().allow("file_*");

    assert_eq!(rules.check("file_read"), PermissionDecision::Allow);
    assert_eq!(rules.check("file_write"), PermissionDecision::Allow);
    assert_eq!(
        rules.check("bash"),
        PermissionDecision::Ask,
        "non-matching tool should not be affected"
    );
}

#[test]
fn permission_mode_read_only_blocks_writes() {
    let mut ctx = PermissionContext::new("/tmp/test");
    ctx.mode = PermissionMode::ReadOnly;

    // In ReadOnly mode, file_write should require confirmation.
    let needs_confirmation = ctx.requires_confirmation(
        "file_write",
        &serde_json::json!({"path": "/tmp/test/foo.txt"}),
    );
    assert!(
        needs_confirmation,
        "ReadOnly mode should require confirmation for file_write"
    );

    // In ReadOnly mode, file_read should not be exposed (or should be allowed).
    let read_exposed = ctx.should_expose_tool("file_read");
    assert!(read_exposed, "ReadOnly mode should expose file_read");
}

#[test]
fn permission_mode_auto_all_approves_low_risk() {
    let mut ctx = PermissionContext::new("/tmp/test");
    ctx.mode = PermissionMode::AutoAll;

    // AutoAll should approve low-risk tools without confirmation.
    let approved = ctx.auto_approves_tool_confirmation(
        "file_read",
        &serde_json::json!({"path": "/tmp/test/foo.txt"}),
    );
    assert!(approved, "AutoAll should approve file_read");
}

#[test]
fn permission_allowlist_respected() {
    let rules = PermissionRules::new()
        .allow("file_read")
        .allow("grep")
        .deny("bash");

    assert_eq!(rules.check("file_read"), PermissionDecision::Allow);
    assert_eq!(rules.check("grep"), PermissionDecision::Allow);
    assert_eq!(rules.check("bash"), PermissionDecision::Deny);
    assert_eq!(rules.check("file_write"), PermissionDecision::Ask);
}
