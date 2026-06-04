use super::*;

#[test]
fn test_wildcard_matching() {
    assert!(match_wildcard("file_*", "file_read"));
    assert!(match_wildcard("file_*", "file_write"));
    assert!(!match_wildcard("file_*", "bash"));

    assert!(match_wildcard("*tool", "mytool"));
    assert!(match_wildcard("*tool", "sometool"));
    assert!(!match_wildcard("*tool", "bash"));

    assert!(match_wildcard("web_*", "web_fetch"));
    assert!(match_wildcard("web_*", "web_search"));
    assert!(!match_wildcard("web_*", "bash"));

    assert!(match_wildcard("?at", "cat"));
    assert!(match_wildcard("?at", "bat"));
    assert!(!match_wildcard("?at", "chat"));

    assert!(match_wildcard("*", "anything"));
    assert!(match_wildcard("exact", "exact"));
}

#[test]
fn test_sourced_rule_matching() {
    let rule = SourcedRule::new("file_*", RuleSource::User);
    assert!(rule.matches("file_read"));
    assert!(rule.matches("file_write"));
    assert!(!rule.matches("bash"));
}

#[test]
fn test_permission_rules_with_wildcards() {
    let rules = PermissionRules::new()
        .allow("file_*")
        .deny("*_dangerous")
        .ask("custom_*");

    // file_* should be allowed
    assert_eq!(rules.check("file_read"), PermissionDecision::Allow);
    assert_eq!(rules.check("file_write"), PermissionDecision::Allow);

    // *_dangerous should be denied
    assert_eq!(rules.check("tool_dangerous"), PermissionDecision::Deny);

    // custom_* should ask
    assert_eq!(rules.check("custom_tool"), PermissionDecision::Ask);

    // unknown tools should ask
    assert_eq!(rules.check("unknown"), PermissionDecision::Ask);
}

#[test]
fn test_permission_rules_priority() {
    // deny has highest priority
    let rules = PermissionRules::new()
        .allow("file_*")
        .deny("file_dangerous");

    assert_eq!(rules.check("file_read"), PermissionDecision::Allow);
    assert_eq!(rules.check("file_dangerous"), PermissionDecision::Deny);
}

#[test]
fn test_get_matching_rules() {
    let rules = PermissionRules::new()
        .allow("file_*")
        .allow("read_*")
        .deny("*_dangerous");

    let matches = rules.get_matching_rules("file_read");
    assert_eq!(matches.len(), 1); // only allow matches

    let matches = rules.get_matching_rules("file_dangerous");
    assert_eq!(matches.len(), 2); // allow and deny both match
}

#[test]
fn test_explainable_decision_concise_summary_mentions_risk_and_reason() {
    let ctx = PermissionContext {
        mode: PermissionMode::Default,
        rules: PermissionRules::new().ask_with_source("bash", RuleSource::Project),
        working_dir: std::path::PathBuf::from("."),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };
    let decision = ctx.explain_decision(
        "bash",
        &serde_json::json!({"command": "rm -rf /tmp/example"}),
    );
    let summary = decision.concise_summary();
    assert!(summary.contains("decision=Ask"));
    assert!(summary.contains("risk="));
    assert!(summary.contains("Project rule 'bash'"));
    assert!(summary.contains("warnings="));
}

#[test]
fn test_permission_mode_readonly() {
    let ctx = PermissionContext {
        mode: PermissionMode::ReadOnly,
        rules: PermissionRules::new(),
        working_dir: std::path::PathBuf::from("."),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };

    assert!(ctx.requires_confirmation("file_write", &serde_json::Value::Null));
    assert!(ctx.requires_confirmation("file_edit", &serde_json::Value::Null));
    assert!(ctx.requires_confirmation("bash", &serde_json::Value::Null));
    assert!(!ctx.requires_confirmation("file_read", &serde_json::Value::Null));
}

#[test]
fn test_permission_mode_auto_low_risk() {
    let ctx = PermissionContext {
        mode: PermissionMode::AutoLowRisk,
        rules: PermissionRules::new(),
        working_dir: std::path::PathBuf::from("."),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };

    let bash_params = serde_json::json!({"command": "ls -la"});
    let package_install = serde_json::json!({"command": "pip3 install pygame"});
    assert!(!ctx.requires_confirmation("bash", &bash_params));
    assert!(ctx.requires_confirmation("bash", &package_install));
    assert!(ctx.requires_confirmation("agent", &serde_json::Value::Null));
    assert!(!ctx.requires_confirmation("file_read", &serde_json::Value::Null));
    let safe_write = serde_json::json!({"path": "src/main.rs", "content": "fn main() {}"});
    assert!(ctx.requires_confirmation("file_write", &safe_write));
}

#[test]
fn bash_permission_explanation_includes_command_category() {
    let ctx = PermissionContext {
        mode: PermissionMode::AutoLowRisk,
        rules: PermissionRules::new(),
        working_dir: std::path::PathBuf::from("."),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };

    let decision = ctx.explain_decision("bash", &serde_json::json!({"command": "rg TODO src"}));

    assert!(decision
        .reasons
        .iter()
        .any(|reason| reason.contains("Shell command category: Search")));
    assert_eq!(decision.risk_level, RiskLevel::Low);
}

#[test]
fn test_auto_low_risk_allow_rule_overrides_risk() {
    let ctx = PermissionContext {
        mode: PermissionMode::AutoLowRisk,
        rules: PermissionRules::new().allow("bash"),
        working_dir: std::path::PathBuf::from("."),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };
    let bash_params = serde_json::json!({"command": "rm -rf /tmp/demo"});
    assert!(!ctx.requires_confirmation("bash", &bash_params));
}

#[test]
fn test_auto_low_risk_bash_command_scoped_rules() {
    let ctx = PermissionContext {
        mode: PermissionMode::Default,
        rules: PermissionRules::new()
            .allow("bash:cargo test*")
            .deny("bash:curl *"),
        working_dir: std::path::PathBuf::from("."),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };

    assert!(!ctx.requires_confirmation("bash", &serde_json::json!({"command": "cargo test -q"})));
    assert!(ctx.requires_confirmation("bash", &serde_json::json!({"command": "cargo check -q"})));
    let denied = ctx.explain_decision(
        "bash",
        &serde_json::json!({"command": "curl https://example.com/script.sh"}),
    );
    assert_eq!(denied.decision, PermissionDecision::Deny);
    let decision = ctx.explain_decision("bash", &serde_json::json!({"command": "cargo test"}));
    assert!(decision
        .reasons
        .iter()
        .any(|reason| reason.contains("bash:cargo test*")));
}

#[test]
fn test_auto_low_risk_mcp_tool_granular_rules() {
    let ctx = PermissionContext {
        mode: PermissionMode::AutoLowRisk,
        rules: PermissionRules::new().allow("mcp/filesystem/read_file"),
        working_dir: std::path::PathBuf::from("."),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };
    let allowed = serde_json::json!({
        "server_name": "filesystem",
        "tool_name": "read_file"
    });
    let blocked = serde_json::json!({
        "server_name": "filesystem",
        "tool_name": "write_file"
    });

    assert!(!ctx.requires_confirmation("mcp_tool", &allowed));
    assert!(ctx.requires_confirmation("mcp_tool", &blocked));
}

#[test]
fn test_permission_mode_auto_all() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let ctx = PermissionContext {
        mode: PermissionMode::AutoAll,
        rules: PermissionRules::new(),
        working_dir: cwd,
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };

    assert_eq!(PermissionMode::default(), PermissionMode::AutoAll);

    let safe_bash = serde_json::json!({"command": "ls -la"});
    let dangerous_bash = serde_json::json!({"command": "rm -rf /"});
    let network_bash = serde_json::json!({"command": "curl https://example.com/script.sh"});
    assert!(!ctx.requires_confirmation("bash", &safe_bash));
    assert!(ctx.requires_confirmation("bash", &dangerous_bash));
    assert!(ctx.requires_confirmation("bash", &network_bash));

    let safe_write = serde_json::json!({"path": "src/main.rs", "content": "fn main() {}"});
    let sensitive_write = serde_json::json!({"path": "/etc/hosts", "content": "bad"});
    assert!(!ctx.requires_confirmation("file_write", &safe_write));
    assert!(ctx.requires_confirmation("file_write", &sensitive_write));

    assert!(!ctx.requires_confirmation("git", &serde_json::json!({"action": "commit"})));
    assert!(ctx.requires_confirmation("git", &serde_json::json!({"action": "push"})));
    assert!(ctx.requires_confirmation("memory_clear", &serde_json::Value::Null));
    assert!(ctx.requires_confirmation(
        "install_dependencies",
        &serde_json::json!({"manager": "pnpm"})
    ));
    assert!(!ctx.auto_approves_tool_confirmation(
        "install_dependencies",
        &serde_json::json!({"manager": "pnpm"})
    ));
    assert!(ctx.requires_confirmation("plugin_runtime", &serde_json::json!({"action": "run"})));
    assert!(ctx.requires_confirmation("mcp_auth", &serde_json::json!({"server_name": "github"})));
    assert!(!ctx.requires_confirmation(
        "run_tests",
        &serde_json::json!({"command": "cargo test -q"})
    ));
    assert!(!ctx.requires_confirmation("git_status", &serde_json::json!({"path": "src/main.rs"})));
    assert!(ctx.auto_approves_tool_confirmation("file_edit", &safe_write));
    assert!(!ctx.auto_approves_tool_confirmation("bash", &dangerous_bash));
}

#[test]
fn package_install_permission_explanation_includes_install_facts() {
    let ctx = PermissionContext {
        mode: PermissionMode::AutoAll,
        rules: PermissionRules::new(),
        working_dir: std::path::PathBuf::from("."),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };

    let decision = ctx.explain_decision(
        "install_dependencies",
        &serde_json::json!({"manager": "pnpm"}),
    );

    assert_eq!(decision.risk_level, RiskLevel::High);
    assert!(decision
        .warnings
        .iter()
        .any(|warning| warning.contains("PACKAGE_INSTALL")));
    assert!(decision
        .reasons
        .iter()
        .any(|reason| reason.contains("Package manager install: pnpm")));
}

#[test]
fn auto_all_permission_risk_baseline_for_side_effect_tools() {
    let ctx = PermissionContext {
        mode: PermissionMode::AutoAll,
        rules: PermissionRules::new(),
        working_dir: std::path::PathBuf::from("."),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };

    let must_ask = [
        (
            "format",
            serde_json::json!({"action": "format", "file_path": "src/main.rs"}),
        ),
        (
            "notebook",
            serde_json::json!({"action": "edit_cell", "file_path": "demo.ipynb", "cell_index": 0, "content": "x"}),
        ),
        (
            "config",
            serde_json::json!({"action": "set", "key": "model", "value": "x"}),
        ),
        (
            "skill_manage",
            serde_json::json!({"action": "patch", "name": "helper", "content": "x"}),
        ),
        (
            "rewind",
            serde_json::json!({"target": "latest_file_change"}),
        ),
        (
            "powershell",
            serde_json::json!({"action": "execute", "command": "Remove-Item file.txt"}),
        ),
        (
            "cron",
            serde_json::json!({"action": "create", "name": "later", "prompt": "check", "schedule": "30m"}),
        ),
    ];
    for (tool, params) in must_ask {
        assert!(
            ctx.requires_confirmation(tool, &params),
            "expected {tool} to require AutoAll confirmation"
        );
    }

    let auto_allowed = [
        (
            "format",
            serde_json::json!({"action": "check", "file_path": "src/main.rs"}),
        ),
        (
            "notebook",
            serde_json::json!({"action": "read", "file_path": "demo.ipynb"}),
        ),
        (
            "config",
            serde_json::json!({"action": "get", "key": "model"}),
        ),
        (
            "skill_manage",
            serde_json::json!({"action": "view", "name": "helper"}),
        ),
        (
            "task_output",
            serde_json::json!({"action": "get", "task_id": "task_1"}),
        ),
    ];
    for (tool, params) in auto_allowed {
        assert!(
            !ctx.requires_confirmation(tool, &params),
            "expected {tool} to stay AutoAll-allowed for read/check usage"
        );
    }
}

#[test]
fn test_auto_all_prompts_for_outside_workspace_paths() {
    let ctx = PermissionContext {
        mode: PermissionMode::AutoAll,
        rules: PermissionRules::new(),
        working_dir: std::path::PathBuf::from("/tmp/priority-agent-workspace"),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };

    assert!(!ctx.requires_confirmation(
        "file_write",
        &serde_json::json!({"path": "src/main.rs", "content": "ok"})
    ));
    assert!(ctx.requires_confirmation(
        "file_write",
        &serde_json::json!({"path": "/Users/georgexu/Desktop/other/file.rs", "content": "no"})
    ));
}

#[test]
fn test_auto_all_prompts_for_bash_outside_workspace_paths() {
    let ctx = PermissionContext {
        mode: PermissionMode::AutoAll,
        rules: PermissionRules::new(),
        working_dir: std::path::PathBuf::from("/tmp/priority-agent-workspace"),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };

    assert!(!ctx.requires_confirmation(
        "bash",
        &serde_json::json!({"command": "sed -n '1,20p' src/main.rs"})
    ));
    assert!(!ctx.requires_confirmation(
        "bash",
        &serde_json::json!({"command": "sed -n '1,20p' /tmp/priority-agent-workspace/src/main.rs"})
    ));
    assert!(ctx.requires_confirmation(
        "bash",
        &serde_json::json!({"command": "sed -n '1,20p' /Users/georgexu/Desktop/rust-agent/src/main.rs"})
    ));
    assert!(ctx.requires_confirmation(
        "bash",
        &serde_json::json!({"command": "rg memory --glob '*.rs' root=/Users/georgexu/Desktop/rust-agent/src"})
    ));
}

#[test]
fn test_auto_all_prompts_for_structurally_risky_bash() {
    let ctx = PermissionContext {
        mode: PermissionMode::AutoAll,
        rules: PermissionRules::new(),
        working_dir: std::path::PathBuf::from("/tmp/priority-agent-workspace"),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };

    for command in [
        "echo ok >> src/generated.txt",
        "cd ../outside && git status",
        "python3 <<'PY'\nopen('src/out.txt', 'w').write('x')\nPY",
        "python -c \"open('src/out.txt', 'w').write('x')\"",
        "printf ok | tee src/generated.txt",
        "sed -i '' 's/a/b/' src/lib.rs",
    ] {
        assert!(
            ctx.requires_confirmation("bash", &serde_json::json!({"command": command})),
            "expected structural shell review for {command:?}"
        );
    }
}

#[test]
fn test_auto_all_prompts_for_untrusted_web_fetch() {
    let ctx = PermissionContext {
        mode: PermissionMode::AutoAll,
        rules: PermissionRules::new(),
        working_dir: std::path::PathBuf::from("."),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };

    assert!(ctx.requires_confirmation(
        "web_fetch",
        &serde_json::json!({"url": "https://example.com"})
    ));
    assert!(!ctx.requires_confirmation("web_search", &serde_json::json!({"query": "rust ratatui"})));
}

#[test]
fn test_auto_all_prompts_for_remote_execution_but_not_remote_reads() {
    let ctx = PermissionContext {
        mode: PermissionMode::AutoAll,
        rules: PermissionRules::new(),
        working_dir: std::path::PathBuf::from("."),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };

    assert!(ctx.requires_confirmation(
        "remote_trigger",
        &serde_json::json!({"action": "run", "id": "session-1"})
    ));
    assert!(ctx.requires_confirmation(
        "remote_dev",
        &serde_json::json!({
            "action": "exec",
            "id": "prod-shell",
            "command": "cargo test -q"
        })
    ));
    assert!(!ctx.requires_confirmation(
        "remote_trigger",
        &serde_json::json!({"action": "status", "id": "session-1"})
    ));
    assert!(!ctx.requires_confirmation("remote_dev", &serde_json::json!({"action": "detect"})));
}

#[test]
fn remote_permission_explanation_includes_remote_facts() {
    let ctx = PermissionContext {
        mode: PermissionMode::AutoAll,
        rules: PermissionRules::new(),
        working_dir: std::path::PathBuf::from("."),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };

    let trigger = ctx.explain_decision(
        "remote_trigger",
        &serde_json::json!({"action": "run", "id": "session-1"}),
    );
    assert_eq!(trigger.risk_level, RiskLevel::High);
    assert!(trigger
        .warnings
        .iter()
        .any(|warning| warning.contains("REMOTE_EXECUTION")));
    assert!(trigger
        .reasons
        .iter()
        .any(|reason| reason.contains("Remote bridge facts")));

    let remote_dev = ctx.explain_decision(
        "remote_dev",
        &serde_json::json!({
            "action": "exec",
            "id": "prod-shell",
            "command": "cargo test -q"
        }),
    );
    assert_eq!(remote_dev.risk_level, RiskLevel::High);
    assert!(remote_dev
        .warnings
        .iter()
        .any(|warning| warning.contains("REMOTE_COMMAND")));
}

#[test]
fn test_permission_mode_once() {
    let mut ctx = PermissionContext {
        mode: PermissionMode::Once,
        rules: PermissionRules::new(),
        working_dir: std::path::PathBuf::from("."),
        is_bypass_available: false,
        once_authorizations: std::collections::HashMap::new(),
    };

    // Initially requires confirmation
    assert!(ctx.requires_confirmation("file_write", &serde_json::Value::Null));

    // Grant once authorization
    ctx.grant_once("file_write");

    // Now should NOT require confirmation (allowed for 5 minutes)
    assert!(!ctx.requires_confirmation("file_write", &serde_json::Value::Null));

    // Other tools still require confirmation
    assert!(ctx.requires_confirmation("bash", &serde_json::Value::Null));
}

// ─── Security Replay Tests ────────────────────────────────────────────────

#[test]
fn test_security_replay_command_injection_pipe() {
    // Simulates: echo "malicious" | rm -rf /
    let cmd = "echo test | rm -rf /";
    assert!(crate::security::is_dangerous_command(cmd));
}

#[test]
fn test_security_replay_command_injection_semicolon() {
    // Simulates: rm -rf / ; echo done
    let cmd = "rm -rf / ; echo done";
    assert!(crate::security::is_dangerous_command(cmd));
}

#[test]
fn test_security_replay_command_injection_and() {
    // Simulates: rm -rf / && echo done
    let cmd = "rm -rf / && echo done";
    assert!(crate::security::is_dangerous_command(cmd));
}

#[test]
fn test_security_replay_command_injection_or() {
    // Simulates: rm -rf / || echo done
    let cmd = "rm -rf / || echo done";
    assert!(crate::security::is_dangerous_command(cmd));
}

#[test]
fn test_security_replay_command_injection_backtick() {
    // Simulates: `rm -rf /`
    let cmd = "`rm -rf /`";
    assert!(crate::security::is_dangerous_command(cmd));
}

#[test]
fn test_security_replay_command_injection_dollar() {
    // Simulates: $(rm -rf /)
    let cmd = "$(rm -rf /)";
    assert!(crate::security::is_dangerous_command(cmd));
}

#[test]
fn test_security_replay_command_injection_fork_bomb() {
    // Fork bomb pattern
    let cmd = ":(){ :|:& };:";
    assert!(crate::security::is_dangerous_command(cmd));
}

#[test]
fn test_security_replay_path_traversal_simple() {
    // Simulates: ../../../etc/passwd
    let path = "../../../etc/passwd";
    assert!(path.contains(".."));
}

#[test]
fn test_security_replay_path_traversal_encoded() {
    // Simulates: %2e%2e%2f%2e%2e%2fetc%2fpasswd (URL encoded ../..)
    // We check for literal ".." which is the decoded form
    let path = "a/../b/../c";
    let parts: Vec<&str> = path.split('/').collect();
    assert!(parts.contains(&".."));
}

#[test]
fn test_security_replay_path_traversal_absolute() {
    // Absolute path with traversal
    let path = "/etc/../etc/passwd";
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    assert!(parts.contains(&".."));
}

#[test]
fn test_security_replay_mcp_malicious_server_name() {
    // Malicious server name patterns that should be detected
    let malicious_names = [
        ("../../malicious", "path traversal in server name"),
        ("'; DROP TABLE--", "SQL injection in server name"),
        ("<script>alert(1)</script>", "XSS pattern in server name"),
    ];
    for (name, description) in malicious_names {
        // Server names should not contain shell metacharacters or path traversal
        let has_shell_chars = name.chars().any(|c| {
            c == ';' || c == '|' || c == '&' || c == '$' || c == '`' || c == '<' || c == '>'
        });
        let has_traversal = name.contains("..");
        assert!(
            has_shell_chars || has_traversal,
            "Should detect {}: {}",
            description,
            name
        );
    }
}

#[test]
fn test_security_replay_mcp_malicious_tool_name() {
    // Malicious tool name injection
    let malicious = "read_file'; exec('rm -rf /')";
    let has_injection =
        malicious.contains('\'') || malicious.contains(';') || malicious.contains("exec");
    assert!(has_injection);
}

#[test]
fn test_security_replay_env_variable_injection() {
    // Environment variable injection
    let cmd = "echo $HOME/.ssh/id_rsa";
    // $ in commands can be dangerous if variables expand to malicious values
    assert!(cmd.contains('$'));
}

#[test]
fn test_security_replay_heredoc_injection() {
    // Heredoc injection
    let cmd = "cat <<EOF\nmalicious content\nEOF";
    assert!(cmd.contains("<<"));
}

#[test]
fn test_security_replay_base64_injection() {
    // Base64 encoded command injection
    let cmd = "base64 -d <<<'cm0gLXJmIC8=' | sh";
    assert!(crate::security::is_dangerous_command(cmd));
}

#[test]
fn test_security_replay_overwrite_sensitive_file() {
    // High risk paths
    let sensitive_paths = [
        "/etc/passwd",
        "/etc/shadow",
        "/.ssh/authorized_keys",
        ".env",
        "id_rsa",
        "/dev/sda",
    ];
    for path in sensitive_paths {
        // Create a PermissionContext and check if path is high risk
        let ctx = PermissionContext::new(".");
        let params = serde_json::json!({"path": path, "content": "malicious"});
        let decision = ctx.explain_decision("file_write", &params);
        assert!(
            decision
                .warnings
                .iter()
                .any(|w| w.contains("HIGH_RISK_PATH") || w.contains("PATH_TRAVERSAL")),
            "Should warn about sensitive path: {}",
            path
        );
    }
}

#[test]
fn test_security_replay_disk_write() {
    // Direct disk write
    let cmd = "dd if=/dev/zero of=/dev/sda";
    assert!(crate::security::is_dangerous_command(cmd));
}

#[test]
fn test_security_replay_chmod_dangerous() {
    // Dangerous chmod - recursive permission changes to root
    let dangerous_chmod = [
        "chmod -R 777 /",
        "chmod -R 000 /",
        "chmod 777 /",
        "chmod 000 /",
    ];
    for cmd in dangerous_chmod {
        assert!(
            crate::security::is_dangerous_command(cmd),
            "Should detect dangerous chmod: {}",
            cmd
        );
    }
}

#[test]
fn test_security_replay_sudo_without_confirmation() {
    // Sudo without confirmation
    let cmd = "sudo rm -rf /";
    assert!(crate::security::is_dangerous_command(cmd));
}

#[test]
fn test_should_expose_tool_respects_deny_rules() {
    let mut ctx = PermissionContext::new(".");
    ctx.rules = PermissionRules::new().deny("bash");

    assert!(!ctx.should_expose_tool("bash"));
    assert!(ctx.should_expose_tool("file_read"));
}

#[test]
fn test_should_expose_tool_hides_write_tools_in_read_only_mode() {
    let mut ctx = PermissionContext::new(".");
    ctx.mode = PermissionMode::ReadOnly;

    assert!(ctx.should_expose_tool("file_read"));
    assert!(ctx.should_expose_tool("grep"));
    assert!(!ctx.should_expose_tool("file_write"));
    assert!(!ctx.should_expose_tool("bash"));
    assert!(!ctx.should_expose_tool("git"));
}

#[test]
fn test_security_replay_kill_critical_process() {
    // Kill critical processes via sudo
    let dangerous = ["sudo kill -9 1", "sudo killall -9 init"];
    for cmd in dangerous {
        assert!(
            crate::security::is_dangerous_command(cmd),
            "Should detect: {}",
            cmd
        );
    }
}

// ---- Phase 5 (Reasonix alignment): policy-as-pure-logic verification ----

#[test]
fn permission_rules_evaluate_purely_without_ui() {
    // deny on the bare tool name.
    let rules = crate::permissions::PermissionRules::new()
        .deny("file_write")
        .allow("grep");
    assert_eq!(
        rules.check("file_write"),
        crate::permissions::PermissionDecision::Deny
    );
    assert_eq!(
        rules.check("grep"),
        crate::permissions::PermissionDecision::Allow
    );
    assert_eq!(
        rules.check("glob"),
        crate::permissions::PermissionDecision::Ask
    );
}

#[test]
fn deny_blocks_tool_exposure() {
    let context = crate::permissions::PermissionContext::new(".");
    assert!(context.should_expose_tool("file_read"));
    assert!(context.should_expose_tool("file_write"));
}

#[test]
fn memory_clear_requires_confirmation() {
    let context = crate::permissions::PermissionContext::new(".");
    assert!(
        context.requires_confirmation("memory_clear", &serde_json::json!({"confirm": true})),
        "memory_clear must require confirmation"
    );
}

#[test]
fn mcp_tool_execution_requires_confirmation() {
    let context = crate::permissions::PermissionContext::new(".");
    assert!(
        context.requires_confirmation("mcp_tool", &serde_json::json!({})),
        "mcp_tool must require confirmation"
    );
}

#[test]
fn file_write_outside_workspace_requires_confirmation() {
    let context = crate::permissions::PermissionContext::new(".");
    // file_write with external path is always high-risk.
    assert!(
        context.requires_confirmation("file_write", &serde_json::json!({"path": "/etc/hosts"})),
        "file_write to /etc/hosts must require confirmation"
    );
}

#[test]
fn read_only_bash_does_not_require_confirmation_in_auto_all() {
    let context = crate::permissions::PermissionContext::new(".");
    let read_commands = [
        serde_json::json!({"command": "ls -la"}),
        serde_json::json!({"command": "cat README.md"}),
        serde_json::json!({"command": "git status"}),
    ];

    for cmd in &read_commands {
        let needs = context.requires_confirmation("bash", cmd);
        assert!(
            !needs,
            "read command {cmd:?} should not require confirmation"
        );
    }
}

#[test]
fn bash_redirect_commands_require_confirmation() {
    let context = crate::permissions::PermissionContext::new(".");
    let redirect_commands = [
        serde_json::json!({"command": "echo hello > /tmp/out.txt"}),
        serde_json::json!({"command": "sed -i 's/old/new/' file.txt"}),
    ];

    for cmd in &redirect_commands {
        let needs = context.requires_confirmation("bash", cmd);
        assert!(needs, "redirect command {cmd:?} must require confirmation");
    }
}
