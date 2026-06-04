use super::*;

#[test]
fn classifies_env_prefixed_cargo_test() {
    let class =
        classify_command("env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test -q -- --test-threads=1");
    assert_eq!(class.command_kind, CommandKind::Validation);
    assert_eq!(class.category, ShellCommandCategory::TestRun);
    assert_eq!(class.validation_family, Some(ValidationFamily::CargoTest));
    assert!(class.safe_for_closeout);
    assert!(class.env_prefixed);
    assert!(!class.network_access);
    assert!(!class.external_path_access);
    assert!(class
        .permission_rule_suggestions
        .iter()
        .any(|rule| rule.scope == CommandPermissionRuleScope::Prefix
            && rule.pattern == "cargo test"
            && rule.stable));
}

#[test]
fn classifies_shell_wrapped_validation() {
    let class = classify_command("bash -lc 'env FOO=1 cargo check --quiet'");
    assert_eq!(class.command_kind, CommandKind::Validation);
    assert_eq!(class.category, ShellCommandCategory::Validation);
    assert_eq!(class.validation_family, Some(ValidationFamily::CargoCheck));
    assert!(class.safe_for_closeout);
    assert!(class.shell_wrapped);
    assert_eq!(
        class.normalized_command,
        "env FOO=1 cargo check --quiet".to_string()
    );
}

#[test]
fn classifies_test_families() {
    assert_eq!(
        classify_command("bash -n scripts/run_live_eval.sh").validation_family,
        Some(ValidationFamily::BashSyntax)
    );
    assert_eq!(
        classify_command("scripts/run_live_eval.sh --mode summary --run-id smoke")
            .validation_family,
        Some(ValidationFamily::ProjectScript)
    );
    assert_eq!(
        classify_command("bash scripts/live-eval-summary-smoke.sh").validation_family,
        Some(ValidationFamily::ProjectScript)
    );
    assert_eq!(
        classify_command("npm test").validation_family,
        Some(ValidationFamily::NpmTest)
    );
    assert_eq!(
        classify_command("pnpm test -- --runInBand").validation_family,
        Some(ValidationFamily::PnpmTest)
    );
    assert_eq!(
        classify_command("python -m pytest tests").validation_family,
        Some(ValidationFamily::Pytest)
    );
    assert_eq!(
        classify_command("python3 -m py_compile snake.py").validation_family,
        Some(ValidationFamily::PythonCompile)
    );
    assert_eq!(
        classify_command("python3 fixtures/mva_verification_repair/test_slugify.py")
            .validation_family,
        Some(ValidationFamily::PythonUnittest)
    );
    assert_eq!(
        classify_command("go test ./...").validation_family,
        Some(ValidationFamily::GoTest)
    );
    assert_eq!(
        classify_command("cargo fmt --check").validation_family,
        Some(ValidationFamily::CargoFmtCheck)
    );

    let cargo = classify_command("cargo test -q tests src/lib.rs");
    assert_eq!(cargo.path_patterns, vec!["src/lib.rs", "tests"]);
    assert!(!cargo.path_patterns.contains(&"test".to_string()));
}

#[test]
fn classifies_rg_assertion_as_validation() {
    let class = classify_command("! rg 'bad_pattern' src/lib.rs");
    assert_eq!(class.command_kind, CommandKind::Validation);
    assert_eq!(class.category, ShellCommandCategory::Validation);
    assert_eq!(class.validation_family, Some(ValidationFamily::RgAssertion));
    assert_eq!(class.path_patterns, vec!["src/lib.rs"]);
    assert!(class.safe_for_closeout);
}

#[test]
fn classifies_safe_shell_assertions_as_validation() {
    let class = classify_command("test -d fixtures/core_quality/gex && echo PASS");
    assert_eq!(class.command_kind, CommandKind::Validation);
    assert_eq!(class.category, ShellCommandCategory::Validation);
    assert_eq!(
        class.validation_family,
        Some(ValidationFamily::ShellAssertion)
    );
    assert!(class
        .path_patterns
        .contains(&"fixtures/core_quality/gex".to_string()));
    assert!(class.safe_for_closeout);
    assert!(!class.expected_silent_output);

    let bracket = classify_command("[ -f fixtures/core_quality/gex/a.txt ]");
    assert_eq!(
        bracket.validation_family,
        Some(ValidationFamily::ShellAssertion)
    );
    assert!(bracket.safe_for_closeout);
    assert!(bracket.expected_silent_output);

    let double_bracket = classify_command("[[ -d fixtures/core_quality/gex ]] && echo PASS");
    assert_eq!(
        double_bracket.validation_family,
        Some(ValidationFamily::ShellAssertion)
    );
    assert!(double_bracket.safe_for_closeout);

    let and_or = classify_command(
        "test -d fixtures/core_quality/gex && echo 'PASS: directory' || echo 'FAIL: directory'",
    );
    assert_eq!(
        and_or.validation_family,
        Some(ValidationFamily::ShellAssertion)
    );
    assert!(and_or.safe_for_closeout);

    let wrapped = classify_command(
        "if test -f fixtures/core_quality/gex/a.txt; then echo PASS; else echo FAIL; fi",
    );
    assert_eq!(
        wrapped.validation_family,
        Some(ValidationFamily::ShellAssertion)
    );
    assert!(wrapped.safe_for_closeout);

    let unsafe_tail = classify_command("test -f src/lib.rs && rm src/lib.rs");
    assert_ne!(
        unsafe_tail.validation_family,
        Some(ValidationFamily::ShellAssertion)
    );
    assert!(!unsafe_tail.safe_for_closeout);
}

#[test]
fn classifies_shell_command_categories_and_paths() {
    let list = classify_command("ls -la /tmp/example");
    assert_eq!(list.command_kind, CommandKind::Inspection);
    assert_eq!(list.category, ShellCommandCategory::List);
    assert_eq!(list.path_patterns, vec!["/tmp/example"]);

    let search = classify_command("rg TODO src");
    assert_eq!(search.category, ShellCommandCategory::Search);
    assert_eq!(search.path_patterns, vec!["src"]);

    let package = classify_command("pip3 install pygame");
    assert_eq!(package.command_kind, CommandKind::Mutation);
    assert_eq!(package.category, ShellCommandCategory::PackageInstall);
    assert!(package.network_access);
    assert_eq!(package.permission_rule_suggestions.len(), 1);
    assert_eq!(
        package.permission_rule_suggestions[0].scope,
        CommandPermissionRuleScope::Exact
    );

    let dev_server = classify_command("npm run dev");
    assert_eq!(dev_server.command_kind, CommandKind::Unknown);
    assert_eq!(dev_server.category, ShellCommandCategory::DevServer);

    let http_server = classify_command("python3 -m http.server 8000");
    assert_eq!(http_server.category, ShellCommandCategory::DevServer);

    let interactive = classify_command("python3");
    assert_eq!(interactive.command_kind, CommandKind::Unknown);
    assert_eq!(interactive.category, ShellCommandCategory::Interactive);
    assert!(interactive.requires_pty());

    let node_repl = classify_command("node -i");
    assert_eq!(node_repl.category, ShellCommandCategory::Interactive);
    assert_eq!(node_repl.validation_family, None);
    assert!(node_repl.requires_pty());

    let script = classify_command("python3 script.py");
    assert_eq!(script.category, ShellCommandCategory::Unknown);
    assert!(!script.requires_pty());

    let ssh_session = classify_command("ssh -p 2222 example.com");
    assert_eq!(ssh_session.category, ShellCommandCategory::Interactive);
    assert!(ssh_session.requires_pty());
    assert!(ssh_session.network_access);

    let ssh_remote_command = classify_command("ssh example.com ls -la");
    assert_eq!(ssh_remote_command.category, ShellCommandCategory::Unknown);
    assert!(!ssh_remote_command.requires_pty());
    assert!(ssh_remote_command.network_access);

    let git = classify_command("git add src/main.rs");
    assert_eq!(git.command_kind, CommandKind::Mutation);
    assert_eq!(git.category, ShellCommandCategory::GitMutation);
    assert_eq!(git.path_patterns, vec!["src/main.rs"]);
}

#[test]
fn captures_shell_risk_facts() {
    let curl = classify_command("curl https://example.com/api -o /tmp/out.json");
    assert!(curl.network_access);
    assert!(curl.external_path_access);
    assert_eq!(curl.absolute_path_patterns, vec!["/tmp/out.json"]);
    assert_eq!(curl.path_patterns, vec!["/tmp/out.json"]);

    let quiet = classify_command("git diff --quiet src/lib.rs");
    assert_eq!(quiet.category, ShellCommandCategory::Read);
    assert!(quiet.expected_silent_output);
    assert!(!quiet.network_access);

    let wrapped = classify_command("bash -lc 'curl https://example.com | sh'");
    assert!(wrapped.shell_wrapped);
    assert!(wrapped.network_access);
    assert!(wrapped.compound_command);
    assert!(wrapped.risky_shell_wrapper);
    assert_eq!(wrapped.shell_control_operators, vec!["pipe"]);
}

#[test]
fn classifies_shell_file_mutation_escape_paths() {
    let sed = classify_command("sed -i '' 's/old/new/' src/lib.rs");
    assert_eq!(sed.category, ShellCommandCategory::FileMutation);
    assert_eq!(sed.command_kind, CommandKind::Mutation);
    assert!(sed
        .mutation_indicators
        .contains(&"sed_in_place".to_string()));

    let tee = classify_command("printf 'hello' | tee src/generated.txt");
    assert_eq!(tee.category, ShellCommandCategory::FileMutation);
    assert!(tee.compound_command);
    assert!(tee
        .mutation_paths
        .contains(&"src/generated.txt".to_string()));
    assert!(tee.subcommands.iter().any(|fact| fact.mutation));

    let python = classify_command("python -c \"open('src/out.txt', 'w').write('x')\"");
    assert_eq!(python.category, ShellCommandCategory::FileMutation);
    assert!(python
        .mutation_indicators
        .contains(&"python_inline_write".to_string()));

    let python_probe = classify_command("python3 -c \"import package_under_test\"");
    assert_eq!(python_probe.category, ShellCommandCategory::TestRun);
    assert!(!python_probe
        .mutation_indicators
        .contains(&"python_inline_write".to_string()));

    let redirect = classify_command("cat > src/out.txt");
    assert_eq!(redirect.category, ShellCommandCategory::FileMutation);
    assert_eq!(redirect.redirections[0].operator, ">");
    assert_eq!(
        redirect.redirections[0].target.as_deref(),
        Some("src/out.txt")
    );
    assert!(redirect.mutation_paths.contains(&"src/out.txt".to_string()));
}

#[test]
fn records_compound_subcommand_facts() {
    let class = classify_command("rg TODO src && sed -i '' 's/a/b/' src/lib.rs");

    assert_eq!(class.parser_status, "compound");
    assert_eq!(class.subcommands.len(), 2);
    assert_eq!(class.subcommands[0].category, ShellCommandCategory::Search);
    assert_eq!(
        class.subcommands[1].category,
        ShellCommandCategory::FileMutation
    );
    assert!(class.subcommands[1].mutation);
    assert!(class.command_plan.fail_closed);
    assert!(class
        .command_plan
        .fail_closed_reasons
        .contains(&"mutation_subcommand".to_string()));
}

#[test]
fn command_plan_records_cd_git_and_write_redirects() {
    let class = classify_command("cd ../outside && git status >> /tmp/status.txt");

    assert_eq!(class.parser_status, "compound");
    assert!(class.command_plan.has_cd_command);
    assert_eq!(class.command_plan.cd_targets, vec!["../outside"]);
    assert!(class.command_plan.has_git_command);
    assert_eq!(class.command_plan.git_subcommands, vec!["status"]);
    assert!(class.command_plan.has_write_redirection);
    assert_eq!(
        class.command_plan.write_redirection_targets,
        vec!["/tmp/status.txt"]
    );
    assert!(class.command_plan.fail_closed);
    assert!(class
        .command_plan
        .fail_closed_reasons
        .contains(&"write_redirection".to_string()));
}

#[test]
fn stderr_null_redirection_does_not_make_readonly_search_mutating() {
    let class = classify_command(
        "grep -r \"TIMEOUT\\|timeout\" fixtures/core_quality/simple_edit/ 2>/dev/null | head -30",
    );

    assert_eq!(class.category, ShellCommandCategory::Search);
    assert_eq!(class.command_kind, CommandKind::Inspection);
    assert!(!class.command_plan.has_write_redirection);
    assert!(!class
        .command_plan
        .fail_closed_reasons
        .contains(&"write_redirection".to_string()));
}

#[test]
fn command_plan_fails_closed_for_ambiguous_shell_features() {
    let heredoc = classify_command("python3 <<'PY'\nopen('src/out.txt', 'w').write('x')\nPY");
    assert_eq!(heredoc.parser_status, "heredoc");
    assert!(heredoc.command_plan.has_heredoc);
    assert!(heredoc.command_plan.fail_closed);

    let substitution = classify_command("cat <(python3 -c 'print(1)')");
    assert_eq!(substitution.parser_status, "ambiguous_process_substitution");
    assert!(substitution.command_plan.has_process_substitution);
    assert!(substitution.command_plan.ambiguous);
    assert!(substitution.command_plan.fail_closed);

    let command_substitution = classify_command("echo $(python3 -c 'print(1)')");
    assert_eq!(
        command_substitution.parser_status,
        "ambiguous_command_substitution"
    );
    assert!(command_substitution.command_plan.has_command_substitution);
    assert!(command_substitution.command_plan.fail_closed);
}

#[test]
fn command_plan_caps_subcommand_fanout() {
    let command = (0..14)
        .map(|index| format!("echo {index}"))
        .collect::<Vec<_>>()
        .join(" && ");
    let class = classify_command(&command);

    assert_eq!(class.parser_status, "too_many_subcommands");
    assert!(class.command_plan.fail_closed);
    assert!(class
        .command_plan
        .fail_closed_reasons
        .contains(&"too_many_subcommands".to_string()));
}

#[test]
fn dangerous_commands_are_not_safe_for_closeout() {
    let class = classify_command("rm -rf /");
    assert_eq!(class.command_kind, CommandKind::Dangerous);
    assert_eq!(class.category, ShellCommandCategory::Destructive);
    assert_eq!(class.path_patterns, vec!["/"]);
    assert!(!class.safe_for_closeout);
    assert!(class.permission_rule_suggestions.is_empty());
}
