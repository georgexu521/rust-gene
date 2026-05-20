use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    Validation,
    Inspection,
    Mutation,
    Dangerous,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellCommandCategory {
    Read,
    List,
    Search,
    Validation,
    PackageInstall,
    DevServer,
    Interactive,
    TestRun,
    FileMutation,
    GitMutation,
    Destructive,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationFamily {
    BashSyntax,
    CargoTest,
    CargoCheck,
    CargoClippy,
    NpmTest,
    PnpmTest,
    YarnTest,
    Pytest,
    PythonCompile,
    PythonUnittest,
    GoTest,
    NodeScript,
    ProjectScript,
    RgAssertion,
    ShellAssertion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandClassification {
    pub normalized_command: String,
    pub command_kind: CommandKind,
    pub category: ShellCommandCategory,
    pub validation_family: Option<ValidationFamily>,
    pub path_patterns: Vec<String>,
    pub safe_for_closeout: bool,
    pub shell_wrapped: bool,
    pub env_prefixed: bool,
}

impl CommandClassification {
    pub fn is_safe_validation(&self) -> bool {
        self.command_kind == CommandKind::Validation && self.safe_for_closeout
    }

    pub fn requires_pty(&self) -> bool {
        self.category == ShellCommandCategory::Interactive
    }
}

pub fn classify_command(command: &str) -> CommandClassification {
    let normalized = normalize_command_for_match(command);
    let shell_wrapped = normalized.trim() != command.trim();
    let (base_command, env_prefixed) = strip_env_prefix(&normalized);
    let base_command = base_command.to_string();

    if crate::security::is_dangerous_command(command) {
        return CommandClassification {
            normalized_command: normalized,
            command_kind: CommandKind::Dangerous,
            category: ShellCommandCategory::Destructive,
            validation_family: None,
            path_patterns: extract_path_patterns(&base_command),
            safe_for_closeout: false,
            shell_wrapped,
            env_prefixed,
        };
    }

    if let Some(family) = validation_family(&base_command) {
        let category = if validation_family_is_test_run(family) {
            ShellCommandCategory::TestRun
        } else {
            ShellCommandCategory::Validation
        };
        return CommandClassification {
            normalized_command: normalized,
            command_kind: CommandKind::Validation,
            category,
            validation_family: Some(family),
            path_patterns: extract_path_patterns(&base_command),
            safe_for_closeout: true,
            shell_wrapped,
            env_prefixed,
        };
    }

    let category = shell_command_category(&base_command);
    let command_kind = command_kind_for_category(category);

    CommandClassification {
        normalized_command: normalized,
        command_kind,
        category,
        validation_family: None,
        path_patterns: extract_path_patterns(&base_command),
        safe_for_closeout: false,
        shell_wrapped,
        env_prefixed,
    }
}

pub fn normalize_command_for_match(command: &str) -> String {
    let mut command = command.trim();
    if let Some(inner) = strip_shell_lc_wrapper(command) {
        command = inner;
    }
    command.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_shell_lc_wrapper(command: &str) -> Option<&str> {
    let command = command.trim();
    for prefix in [
        "bash -lc ",
        "sh -lc ",
        "zsh -lc ",
        "bash -c ",
        "sh -c ",
        "zsh -c ",
    ] {
        let Some(rest) = command.strip_prefix(prefix) else {
            continue;
        };
        let rest = rest.trim();
        if rest.len() < 2 {
            return None;
        }
        let bytes = rest.as_bytes();
        let quote = bytes[0];
        if quote != b'\'' && quote != b'"' {
            return None;
        }
        if bytes[bytes.len() - 1] != quote {
            return None;
        }
        return Some(&rest[1..rest.len() - 1]);
    }
    None
}

fn strip_env_prefix(command: &str) -> (&str, bool) {
    let command = command.trim();
    let Some(rest) = command.strip_prefix("env ") else {
        return (command, false);
    };
    let mut consumed = 4usize;
    let mut saw_assignment = false;
    for token in rest.split_whitespace() {
        if token.starts_with('-') {
            consumed += token.len() + 1;
            continue;
        }
        if is_env_assignment(token) {
            saw_assignment = true;
            consumed += token.len() + 1;
            continue;
        }
        return (command.get(consumed..).unwrap_or("").trim(), saw_assignment);
    }
    (command, saw_assignment)
}

fn is_env_assignment(token: &str) -> bool {
    let Some((key, _)) = token.split_once('=') else {
        return false;
    };
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn validation_family(command: &str) -> Option<ValidationFamily> {
    let command = command.trim();
    let lower = command.to_ascii_lowercase();
    if is_safe_rg_assertion(command) {
        return Some(ValidationFamily::RgAssertion);
    }
    if is_safe_shell_assertion(command) {
        return Some(ValidationFamily::ShellAssertion);
    }
    if lower.starts_with("bash -n ") || lower.starts_with("sh -n ") {
        Some(ValidationFamily::BashSyntax)
    } else if is_project_validation_script(command) {
        Some(ValidationFamily::ProjectScript)
    } else if lower.contains("cargo test") {
        Some(ValidationFamily::CargoTest)
    } else if lower.contains("cargo check") {
        Some(ValidationFamily::CargoCheck)
    } else if lower.contains("cargo clippy") {
        Some(ValidationFamily::CargoClippy)
    } else if lower == "npm test"
        || lower.starts_with("npm test ")
        || lower.contains("npm run test")
    {
        Some(ValidationFamily::NpmTest)
    } else if lower == "pnpm test" || lower.starts_with("pnpm test ") {
        Some(ValidationFamily::PnpmTest)
    } else if lower == "yarn test" || lower.starts_with("yarn test ") {
        Some(ValidationFamily::YarnTest)
    } else if lower == "pytest"
        || lower.starts_with("pytest ")
        || lower.contains("python -m pytest")
    {
        Some(ValidationFamily::Pytest)
    } else if lower.starts_with("python3 -m py_compile")
        || lower.starts_with("python -m py_compile")
    {
        Some(ValidationFamily::PythonCompile)
    } else if lower.starts_with("python3 -m unittest") || lower.starts_with("python -m unittest") {
        Some(ValidationFamily::PythonUnittest)
    } else if lower == "go test" || lower.starts_with("go test ") {
        Some(ValidationFamily::GoTest)
    } else if lower.starts_with("node ") && !lower.starts_with("node -i") {
        Some(ValidationFamily::NodeScript)
    } else if lower.starts_with("python3 -c ") || lower.starts_with("python -c ") {
        Some(ValidationFamily::Pytest)
    } else {
        None
    }
}

fn validation_family_is_test_run(family: ValidationFamily) -> bool {
    matches!(
        family,
        ValidationFamily::CargoTest
            | ValidationFamily::NpmTest
            | ValidationFamily::PnpmTest
            | ValidationFamily::YarnTest
            | ValidationFamily::Pytest
            | ValidationFamily::PythonUnittest
            | ValidationFamily::GoTest
    )
}

fn command_kind_for_category(category: ShellCommandCategory) -> CommandKind {
    match category {
        ShellCommandCategory::Read | ShellCommandCategory::List | ShellCommandCategory::Search => {
            CommandKind::Inspection
        }
        ShellCommandCategory::Validation | ShellCommandCategory::TestRun => CommandKind::Validation,
        ShellCommandCategory::PackageInstall
        | ShellCommandCategory::FileMutation
        | ShellCommandCategory::GitMutation => CommandKind::Mutation,
        ShellCommandCategory::Destructive => CommandKind::Dangerous,
        ShellCommandCategory::DevServer
        | ShellCommandCategory::Interactive
        | ShellCommandCategory::Unknown => CommandKind::Unknown,
    }
}

fn is_project_validation_script(command: &str) -> bool {
    let command = command.trim();
    if command.is_empty()
        || command.contains('\n')
        || command.contains(';')
        || command.contains('|')
        || command.contains("&&")
        || command.contains("||")
        || command.contains('`')
        || command.contains("$(")
        || command.contains('>')
        || command.contains('<')
    {
        return false;
    }

    let tokens = command.split_whitespace().collect::<Vec<_>>();
    let Some(first) = tokens.first().copied() else {
        return false;
    };
    let script = match first {
        "bash" | "sh" => tokens.get(1).copied().unwrap_or_default(),
        _ => first,
    };
    let script = script.strip_prefix("./").unwrap_or(script);
    script.starts_with("scripts/") && script.ends_with(".sh")
}

fn is_safe_rg_assertion(command: &str) -> bool {
    let command = command.trim();
    let Some(command) = command.strip_prefix("! ") else {
        return false;
    };
    let command = command.trim();
    if !command.starts_with("rg ") {
        return false;
    }
    !command.contains('\n')
        && !command.contains(';')
        && !command.contains('|')
        && !command.contains("&&")
        && !command.ends_with('&')
        && !command.contains(" & ")
        && !command.contains('`')
        && !command.contains("$(")
        && !command.contains('>')
        && !command.contains('<')
}

fn is_safe_shell_assertion(command: &str) -> bool {
    let command = command.trim();
    if is_safe_if_shell_assertion(command) {
        return true;
    }
    if is_safe_and_or_shell_assertion(command) {
        return true;
    }
    if command.is_empty()
        || command.contains('\n')
        || command.contains(';')
        || command.contains('|')
        || command.contains("||")
        || command.ends_with('&')
        || command.contains(" & ")
        || command.contains('`')
        || command.contains("$(")
        || command.contains('>')
        || command.contains('<')
    {
        return false;
    }

    let (assertion, tail) = command
        .split_once("&&")
        .map(|(assertion, tail)| (assertion.trim(), Some(tail.trim())))
        .unwrap_or((command, None));
    if let Some(tail) = tail {
        if !tail.starts_with("echo ") {
            return false;
        }
    }

    let tokens = assertion.split_whitespace().collect::<Vec<_>>();
    is_safe_test_assertion_tokens(&tokens)
}

fn is_safe_and_or_shell_assertion(command: &str) -> bool {
    if command.is_empty()
        || command.contains('\n')
        || command.contains(';')
        || command.contains('|') && !command.contains("||")
        || command.ends_with('&')
        || command.contains(" & ")
        || command.contains('`')
        || command.contains("$(")
        || command.contains('>')
        || command.contains('<')
    {
        return false;
    }

    let Some((assertion, after_then)) = command.split_once("&&") else {
        return false;
    };
    let assertion_tokens = assertion.split_whitespace().collect::<Vec<_>>();
    if !is_safe_test_assertion_tokens(&assertion_tokens) {
        return false;
    }
    if let Some((then_part, else_part)) = after_then.split_once("||") {
        is_safe_echo(then_part) && is_safe_echo(else_part)
    } else {
        is_safe_echo(after_then)
    }
}

fn is_safe_if_shell_assertion(command: &str) -> bool {
    if command.is_empty()
        || command.contains('\n')
        || command.contains('|')
        || command.contains("&&")
        || command.contains("||")
        || command.ends_with('&')
        || command.contains(" & ")
        || command.contains('`')
        || command.contains("$(")
        || command.contains('>')
        || command.contains('<')
    {
        return false;
    }

    let Some(rest) = command.strip_prefix("if ") else {
        return false;
    };
    let Some((assertion, after_assertion)) = rest.split_once("; then ") else {
        return false;
    };
    let assertion_tokens = assertion.split_whitespace().collect::<Vec<_>>();
    if !is_safe_test_assertion_tokens(&assertion_tokens) {
        return false;
    }

    if let Some((then_part, after_else)) = after_assertion.split_once("; else ") {
        let Some((else_part, tail)) = after_else.rsplit_once("; fi") else {
            return false;
        };
        tail.trim().is_empty() && is_safe_echo(then_part) && is_safe_echo(else_part)
    } else {
        let Some((then_part, tail)) = after_assertion.rsplit_once("; fi") else {
            return false;
        };
        tail.trim().is_empty() && is_safe_echo(then_part)
    }
}

fn is_safe_test_assertion_tokens(tokens: &[&str]) -> bool {
    match tokens {
        ["test", flag, path] => is_safe_test_flag(flag) && is_safe_assertion_path(path),
        ["[", flag, path, "]"] => is_safe_test_flag(flag) && is_safe_assertion_path(path),
        ["[[", flag, path, "]]"] => is_safe_test_flag(flag) && is_safe_assertion_path(path),
        _ => false,
    }
}

fn is_safe_echo(command: &str) -> bool {
    let command = command.trim();
    command.starts_with("echo ") && !command.contains(';')
}

fn is_safe_test_flag(flag: &str) -> bool {
    matches!(flag, "-d" | "-e" | "-f" | "-s" | "-x" | "-r" | "-w")
}

fn is_safe_assertion_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('-')
        && !path.contains('*')
        && !path.contains('?')
        && !path.contains('[')
        && !path.contains(']')
        && !path.contains('{')
        && !path.contains('}')
}

fn shell_command_category(command: &str) -> ShellCommandCategory {
    let lower = command.to_ascii_lowercase();
    let first = lower.split_whitespace().next();
    if matches!(first, Some("ls" | "find")) {
        return ShellCommandCategory::List;
    }
    if matches!(first, Some("rg" | "grep")) {
        return ShellCommandCategory::Search;
    }
    if matches!(first, Some("cat" | "head" | "tail" | "sed" | "awk" | "pwd"))
        || lower.starts_with("git status")
        || lower.starts_with("git diff")
        || lower.starts_with("git log")
        || lower.starts_with("git show")
    {
        return ShellCommandCategory::Read;
    }
    if is_package_install_command(&lower) {
        return ShellCommandCategory::PackageInstall;
    }
    if is_dev_server_command(&lower) {
        return ShellCommandCategory::DevServer;
    }
    if is_interactive_command(&lower) {
        return ShellCommandCategory::Interactive;
    }
    if is_git_mutation_command(&lower) {
        return ShellCommandCategory::GitMutation;
    }
    if is_legacy_mutation_command(&lower) {
        return ShellCommandCategory::FileMutation;
    }
    ShellCommandCategory::Unknown
}

fn is_package_install_command(lower: &str) -> bool {
    lower.starts_with("pip install ")
        || lower.starts_with("pip3 install ")
        || lower.starts_with("python -m pip install ")
        || lower.starts_with("python3 -m pip install ")
        || lower.starts_with("uv pip install ")
        || lower == "npm install"
        || lower.starts_with("npm install ")
        || lower.starts_with("npm i ")
        || lower.starts_with("npm add ")
        || lower.starts_with("pnpm install")
        || lower.starts_with("pnpm add ")
        || lower.starts_with("yarn install")
        || lower.starts_with("yarn add ")
        || lower.starts_with("cargo add ")
        || lower.starts_with("cargo install ")
        || lower.starts_with("brew install ")
}

fn is_dev_server_command(lower: &str) -> bool {
    lower == "npm start"
        || lower.starts_with("npm start ")
        || lower == "npm run dev"
        || lower.starts_with("npm run dev ")
        || lower == "pnpm dev"
        || lower.starts_with("pnpm dev ")
        || lower == "pnpm start"
        || lower.starts_with("pnpm start ")
        || lower == "yarn dev"
        || lower.starts_with("yarn dev ")
        || lower == "yarn start"
        || lower.starts_with("yarn start ")
        || lower == "vite"
        || lower.starts_with("vite ")
        || lower == "next dev"
        || lower.starts_with("next dev ")
}

fn is_interactive_command(lower: &str) -> bool {
    let lower = lower.trim();
    let first = lower.split_whitespace().next();
    if matches!(
        first,
        Some(
            "bash"
                | "sh"
                | "zsh"
                | "fish"
                | "python"
                | "python3"
                | "node"
                | "irb"
                | "psql"
                | "mysql"
                | "sqlite3"
                | "redis-cli"
        )
    ) && lower.split_whitespace().count() == 1
    {
        return true;
    }

    lower.starts_with("python -i")
        || lower.starts_with("python3 -i")
        || lower.starts_with("node -i")
        || is_interactive_ssh_command(lower)
        || lower == "npm init"
        || lower.starts_with("npm init ")
        || lower.starts_with("pnpm create ")
        || lower.starts_with("yarn create ")
}

fn is_interactive_ssh_command(lower: &str) -> bool {
    let tokens = lower.split_whitespace().collect::<Vec<_>>();
    if tokens.first() != Some(&"ssh") {
        return false;
    }
    if tokens
        .iter()
        .skip(1)
        .any(|token| matches!(*token, "-t" | "-tt"))
    {
        return true;
    }

    let mut index = 1usize;
    while index < tokens.len() {
        let token = tokens[index];
        if matches!(
            token,
            "-b" | "-c" | "-e" | "-f" | "-i" | "-j" | "-l" | "-m" | "-o" | "-p" | "-s" | "-w"
        ) {
            index += 2;
            continue;
        }
        if token.starts_with('-') {
            index += 1;
            continue;
        }

        let remote_command_tokens = tokens.len().saturating_sub(index + 1);
        return remote_command_tokens == 0;
    }

    false
}

fn is_git_mutation_command(lower: &str) -> bool {
    lower.starts_with("git add")
        || lower.starts_with("git commit")
        || lower.starts_with("git checkout")
        || lower.starts_with("git switch")
        || lower.starts_with("git reset")
        || lower.starts_with("git clean")
        || lower.starts_with("git merge")
        || lower.starts_with("git rebase")
        || lower.starts_with("git apply")
        || lower.starts_with("git push")
        || lower.starts_with("git pull")
}

fn is_legacy_mutation_command(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    matches!(
        lower.split_whitespace().next(),
        Some("touch" | "mkdir" | "cp" | "mv" | "rm" | "chmod" | "chown" | "ln")
    ) || lower.contains(" > ")
        || lower.contains(" >> ")
        || lower.starts_with("patch ")
        || lower.contains("sed -i")
}

fn extract_path_patterns(command: &str) -> Vec<String> {
    let tokens = command
        .split_whitespace()
        .map(clean_shell_token)
        .collect::<Vec<_>>();
    let mut paths = tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            if command_token_should_not_be_path(&tokens, index)
                || !likely_path_token(token.as_str())
            {
                None
            } else {
                Some(token.clone())
            }
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn clean_shell_token(token: &str) -> String {
    token
        .trim_matches(|ch| matches!(ch, '\'' | '"' | ',' | ';' | '(' | ')'))
        .to_string()
}

fn command_token_should_not_be_path(tokens: &[String], index: usize) -> bool {
    let Some(token) = tokens.get(index).map(String::as_str) else {
        return false;
    };

    if index == 0
        && matches!(
            token,
            "test"
                | "["
                | "[["
                | "cargo"
                | "npm"
                | "pnpm"
                | "yarn"
                | "go"
                | "python"
                | "python3"
                | "pip"
                | "pip3"
                | "git"
                | "rg"
                | "grep"
                | "find"
                | "ls"
        )
    {
        return true;
    }

    let Some(previous) = tokens.get(index.saturating_sub(1)).map(String::as_str) else {
        return false;
    };
    matches!(
        (previous, token),
        (
            "cargo",
            "test" | "check" | "clippy" | "fmt" | "build" | "run" | "doc" | "clean"
        ) | ("go", "test" | "run" | "build" | "vet" | "fmt")
            | (
                "npm" | "pnpm" | "yarn",
                "test" | "install" | "run" | "exec" | "dlx"
            )
            | ("python" | "python3", "-m")
            | ("pip" | "pip3", "install" | "uninstall")
            | (
                "git",
                "add" | "diff" | "status" | "checkout" | "restore" | "commit"
            )
    )
}

fn likely_path_token(token: &str) -> bool {
    if token.is_empty()
        || token.starts_with('-')
        || token.contains('=')
        || matches!(
            token,
            "bash"
                | "sh"
                | "zsh"
                | "env"
                | "git"
                | "cargo"
                | "npm"
                | "pnpm"
                | "yarn"
                | "python"
                | "python3"
                | "pip"
                | "pip3"
        )
    {
        return false;
    }
    token == "."
        || matches!(
            token,
            "src" | "tests" | "test" | "docs" | "scripts" | "fixtures"
        )
        || token.starts_with("./")
        || token.starts_with("../")
        || token.starts_with('/')
        || token.starts_with("~/")
        || token.contains('/')
        || [
            ".rs", ".py", ".js", ".ts", ".tsx", ".jsx", ".json", ".toml", ".yaml", ".yml", ".md",
            ".txt", ".sh", ".html", ".css",
        ]
        .iter()
        .any(|suffix| token.ends_with(suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_env_prefixed_cargo_test() {
        let class = classify_command(
            "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test -q -- --test-threads=1",
        );
        assert_eq!(class.command_kind, CommandKind::Validation);
        assert_eq!(class.category, ShellCommandCategory::TestRun);
        assert_eq!(class.validation_family, Some(ValidationFamily::CargoTest));
        assert!(class.safe_for_closeout);
        assert!(class.env_prefixed);
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
            classify_command("go test ./...").validation_family,
            Some(ValidationFamily::GoTest)
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

        let bracket = classify_command("[ -f fixtures/core_quality/gex/a.txt ]");
        assert_eq!(
            bracket.validation_family,
            Some(ValidationFamily::ShellAssertion)
        );
        assert!(bracket.safe_for_closeout);

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

        let dev_server = classify_command("npm run dev");
        assert_eq!(dev_server.command_kind, CommandKind::Unknown);
        assert_eq!(dev_server.category, ShellCommandCategory::DevServer);

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

        let ssh_remote_command = classify_command("ssh example.com ls -la");
        assert_eq!(ssh_remote_command.category, ShellCommandCategory::Unknown);
        assert!(!ssh_remote_command.requires_pty());

        let git = classify_command("git add src/main.rs");
        assert_eq!(git.command_kind, CommandKind::Mutation);
        assert_eq!(git.category, ShellCommandCategory::GitMutation);
        assert_eq!(git.path_patterns, vec!["src/main.rs"]);
    }

    #[test]
    fn dangerous_commands_are_not_safe_for_closeout() {
        let class = classify_command("rm -rf /");
        assert_eq!(class.command_kind, CommandKind::Dangerous);
        assert_eq!(class.category, ShellCommandCategory::Destructive);
        assert_eq!(class.path_patterns, vec!["/"]);
        assert!(!class.safe_for_closeout);
    }
}
