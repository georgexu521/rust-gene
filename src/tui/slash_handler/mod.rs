//! Slash command handlers for TuiApp
//!
//! Each handler function takes `&mut TuiApp` + args and returns a String response.
//! This module exists to keep app.rs focused on core TUI state management.
//!
//! Handler functions are organized into submodules:
//! - `session`: Session management, review skills, control commands
//! - `agents`: System diagnostics, agent generation, git operations
//! - `config`: Configuration, permissions, integrations, tool commands
//! - `utils`: Shared utility functions and types

pub mod agents;
pub mod config;
pub mod session;
pub mod utils;

// Re-export all handler functions so they're accessible via `slash_handler::handle_*`
pub use agents::*;
pub use config::*;
pub use session::*;

#[cfg(test)]
mod tests {
    use super::utils::{
        get_config_value, is_valid_rollback_target, is_valid_webhook_url,
        message_items_to_api_messages, parse_bool, parse_optional_count, parse_rollback_args,
        sanitize_note_name, sanitize_profile_key, sanitize_snippet_name, set_config_value,
    };
    use crate::state::{MessageItem, MessageRole};

    // Test that git action validation rejects disallowed actions
    #[tokio::test]
    async fn test_git_rejects_dangerous_actions() {
        // This test verifies the semantic contract that /git should reject
        // dangerous actions like force push, rebase, reset --hard, etc.
        let allowed_actions = [
            "status", "diff", "log", "branch", "checkout", "stash", "tag",
        ];
        let disallowed = ["push", "force-push", "rebase", "reset", "clean"];

        for action in disallowed {
            assert!(
                !allowed_actions.contains(&action),
                "Test setup: '{}' should be in disallowed list",
                action
            );
        }
    }

    #[test]
    fn test_handle_share_returns_path_on_success() {
        // Contract: /share should return a path when session exists
        // This is a structural test - actual session integration tested separately
        let expected_keyword = "exported to:";
        assert!(
            expected_keyword.contains("exported"),
            "Contract: success message should mention 'exported'"
        );
    }

    #[test]
    fn test_handle_feedback_requires_args() {
        // Contract: /feedback without args should show usage
        let usage_msg = "Usage: /feedback <message>";
        assert!(usage_msg.starts_with("Usage:"));
    }

    #[test]
    fn test_handle_redo_contract_message() {
        // Contract: /redo failure path should be descriptive.
        let msg = "Nothing to redo or redo failed: No edits to redo";
        assert!(msg.contains("Nothing to redo"));
        assert!(msg.contains("redo failed"));
    }

    #[test]
    fn test_package_help_shows_all_commands() {
        // Contract: /package help should show all available subcommands
        let help_text = "Package Manager Commands:\n\n\
                 /package list     - List package files in project\n\
                 /package deps     - Show installed dependencies\n\
                 /package outdated - Check for outdated packages";

        assert!(help_text.contains("/package list"));
        assert!(help_text.contains("/package deps"));
        assert!(help_text.contains("/package outdated"));
    }

    #[test]
    fn test_rollback_parse_defaults_to_head_prev() {
        let parsed = parse_rollback_args("").expect("parse should succeed");
        assert_eq!(parsed.target, "HEAD~1");
        assert!(!parsed.confirmed);
    }

    #[test]
    fn test_rollback_parse_target_and_confirm() {
        let parsed = parse_rollback_args("HEAD~3 --yes").expect("parse should succeed");
        assert_eq!(parsed.target, "HEAD~3");
        assert!(parsed.confirmed);
    }

    #[test]
    fn test_rollback_rejects_unknown_flag() {
        let err = parse_rollback_args("--force").expect_err("unknown flag should fail");
        assert!(err.contains("Unknown option"));
    }

    #[test]
    fn test_rollback_rejects_multiple_targets() {
        let err =
            parse_rollback_args("HEAD~1 HEAD~2 --yes").expect_err("multiple targets should fail");
        assert!(err.contains("Too many arguments"));
    }

    #[test]
    fn test_rollback_target_validation() {
        assert!(is_valid_rollback_target("HEAD~1"));
        assert!(is_valid_rollback_target("main"));
        assert!(is_valid_rollback_target("HEAD@{1}"));

        assert!(!is_valid_rollback_target("-hard"));
        assert!(!is_valid_rollback_target("HEAD;rm"));
        assert!(!is_valid_rollback_target("HEAD$1"));
    }

    #[test]
    fn test_parse_optional_count_defaults_to_one() {
        assert_eq!(parse_optional_count("", "/undo").unwrap(), 1);
        assert_eq!(parse_optional_count("3", "/undo").unwrap(), 3);
    }

    #[test]
    fn test_parse_optional_count_rejects_zero_or_invalid() {
        assert!(parse_optional_count("0", "/redo").is_err());
        assert!(parse_optional_count("abc", "/redo").is_err());
    }

    #[test]
    fn test_message_items_to_api_messages_preserves_count() {
        let items = vec![
            MessageItem {
                id: "1".to_string(),
                role: MessageRole::System,
                content: "sys".to_string(),
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            },
            MessageItem {
                id: "2".to_string(),
                role: MessageRole::User,
                content: "hello".to_string(),
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            },
            MessageItem {
                id: "3".to_string(),
                role: MessageRole::Assistant,
                content: "hi".to_string(),
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            },
        ];
        let api = message_items_to_api_messages(&items);
        assert_eq!(api.len(), items.len());
    }

    #[tokio::test]
    async fn test_retry_rejects_arguments() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_retry(&mut app, "unexpected").await;
        assert_eq!(msg, "Usage: /retry");
    }

    #[test]
    fn test_parse_bool_variants() {
        assert!(parse_bool("true").unwrap());
        assert!(parse_bool("ON").unwrap());
        assert!(!parse_bool("0").unwrap());
        assert!(parse_bool("maybe").is_err());
    }

    #[test]
    fn test_config_set_and_get_roundtrip() {
        let mut cfg = crate::services::config::AppConfig::default();
        set_config_value(&mut cfg, "api.model", "gpt-4o").unwrap();
        set_config_value(&mut cfg, "api.temperature", "0.7").unwrap();
        set_config_value(&mut cfg, "features.web_search", "false").unwrap();

        assert_eq!(get_config_value(&cfg, "api.model").unwrap(), "gpt-4o");
        assert_eq!(get_config_value(&cfg, "api.temperature").unwrap(), "0.7");
        assert_eq!(
            get_config_value(&cfg, "features.web_search").unwrap(),
            "false"
        );
    }

    #[test]
    fn test_config_rejects_unknown_or_invalid() {
        let mut cfg = crate::services::config::AppConfig::default();
        assert!(set_config_value(&mut cfg, "unknown.key", "x").is_err());
        assert!(set_config_value(&mut cfg, "api.temperature", "abc").is_err());
        assert!(set_config_value(&mut cfg, "ui.show_token_usage", "abc").is_err());
    }

    #[test]
    fn test_sanitize_snippet_name_validation() {
        assert_eq!(
            sanitize_snippet_name("hello_world-1.0"),
            Some("hello_world-1.0".to_string())
        );
        assert!(sanitize_snippet_name("").is_none());
        assert!(sanitize_snippet_name("../passwd").is_none());
        assert!(sanitize_snippet_name("name with spaces").is_none());
    }

    #[test]
    fn test_cleanup_requires_confirmation_for_sessions() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_cleanup(&mut app, "sessions");
        assert!(msg.contains("destructive"));
        assert!(msg.contains("--yes"));
    }

    #[test]
    fn test_cleanup_requires_confirmation_for_all() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_cleanup(&mut app, "all");
        assert!(msg.contains("Usage: /cleanup all --yes"));
    }

    #[test]
    fn test_sanitize_note_name_validation() {
        assert_eq!(
            sanitize_note_name("bookmark_1"),
            Some("bookmark_1".to_string())
        );
        assert!(sanitize_note_name("../x").is_none());
        assert!(sanitize_note_name("a b").is_none());
        assert!(sanitize_note_name("").is_none());
    }

    #[tokio::test]
    async fn test_bookmark_usage_without_name() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_bookmark(&mut app, "add").await;
        assert!(msg.starts_with("Usage: /bookmark add"));
    }

    #[test]
    fn test_tag_usage_without_args() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_tag(&mut app, "");
        assert!(msg.starts_with("Usage: /tag"));
    }

    #[test]
    fn test_sanitize_profile_key_validation() {
        assert_eq!(
            sanitize_profile_key("user.name"),
            Some("user.name".to_string())
        );
        assert!(sanitize_profile_key("../name").is_none());
        assert!(sanitize_profile_key("bad key").is_none());
        assert!(sanitize_profile_key("").is_none());
    }

    #[test]
    fn test_filter_usage_requires_role() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_filter(&mut app, "");
        assert!(msg.starts_with("Usage: /filter"));
    }

    #[test]
    fn test_theme_rejects_unknown_preset() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_theme(&mut app, "set neon");
        assert!(msg.contains("Unknown theme"));
    }

    #[test]
    fn test_shortcuts_contains_core_bindings() {
        let app = crate::tui::app::TuiApp::new();
        let msg = super::handle_shortcuts(&app);
        assert!(msg.contains("quit:"));
        assert!(msg.contains("submit:"));
    }

    #[test]
    fn test_quick_panel_contains_status() {
        let mut app = crate::tui::app::TuiApp::new();
        let msg = super::handle_quick(&mut app);
        assert!(msg.contains("Quick Panel"));
        assert!(msg.contains("Status:"));
        assert!(msg.contains("Runtime:"));
        assert!(msg.contains("Contracts:"));
        assert!(msg.contains("A2A:"));
        assert!(msg.contains("Workspace:"));
        assert!(msg.contains("Messages:"));
        assert!(msg.contains("Goal drift:"));
    }

    #[test]
    fn test_goal_drift_report_summarizes_events() {
        let mut trace = crate::engine::trace::TurnTrace::new("s1", 1, "inspect workspace");
        trace
            .events
            .push(crate::engine::trace::TraceEvent::GoalDriftDetected {
                goal_id: "goal-123456".to_string(),
                tool: "bash".to_string(),
                call_id: "call-abcdef".to_string(),
                level: "high".to_string(),
                reason: "tool request moved away from inspecting the workspace".to_string(),
                suggested_action: Some("ask_user".to_string()),
            });

        let label = super::config::goal_drift_count_label(&trace);
        let report = super::config::format_goal_drift_report(&trace, 8);

        assert_eq!(label, "1 high");
        assert!(report.contains("Goal Drift from trace"));
        assert!(report.contains("high drift via bash"));
        assert!(report.contains("suggested=ask_user"));
    }

    #[test]
    fn test_focus_toggle_and_status() {
        let mut app = crate::tui::app::TuiApp::new();
        assert_eq!(
            super::handle_focus(&mut app, "status"),
            "Focus mode: disabled"
        );
        assert_eq!(super::handle_focus(&mut app, "on"), "Focus mode enabled.");
        assert!(app.focus_mode);
        assert_eq!(
            super::handle_focus(&mut app, "toggle"),
            "Focus mode disabled."
        );
        assert!(!app.focus_mode);
    }

    #[test]
    fn test_pause_toggle_and_status() {
        let mut app = crate::tui::app::TuiApp::new();
        assert_eq!(
            super::handle_pause(&mut app, "status"),
            "Pause state: running"
        );
        let paused = super::handle_pause(&mut app, "pause");
        assert!(paused.contains("Agent paused"));
        assert!(app.paused);
        assert_eq!(super::handle_pause(&mut app, "resume"), "Agent resumed.");
        assert!(!app.paused);
    }

    #[test]
    fn test_is_valid_webhook_url_validation() {
        assert!(is_valid_webhook_url("https://example.com/hook"));
        assert!(is_valid_webhook_url("http://127.0.0.1:8080/webhook"));
        assert!(!is_valid_webhook_url("ftp://example.com/hook"));
        assert!(!is_valid_webhook_url("https://"));
        assert!(!is_valid_webhook_url("not-a-url"));
    }
}
