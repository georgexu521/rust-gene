use serde::{Deserialize, Serialize};

const MAX_SUBCOMMAND_FACTS: usize = 12;

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
    CargoFmtCheck,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandPermissionRuleScope {
    Exact,
    Prefix,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandPermissionRuleSuggestion {
    pub pattern: String,
    pub scope: CommandPermissionRuleScope,
    pub stable: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellSubcommandFact {
    pub index: usize,
    pub command: String,
    pub category: ShellCommandCategory,
    pub command_kind: CommandKind,
    pub mutation: bool,
    pub redirection: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellRedirectionFact {
    pub operator: String,
    pub target: Option<String>,
    pub writes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BashCommandPlan {
    pub parser_status: String,
    pub fail_closed: bool,
    pub fail_closed_reasons: Vec<String>,
    pub subcommand_count: usize,
    pub subcommand_cap: usize,
    pub has_cd_command: bool,
    pub cd_targets: Vec<String>,
    pub has_git_command: bool,
    pub git_subcommands: Vec<String>,
    pub has_process_substitution: bool,
    pub has_command_substitution: bool,
    pub has_heredoc: bool,
    pub has_write_redirection: bool,
    pub write_redirection_targets: Vec<String>,
    pub ambiguous: bool,
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
    pub network_access: bool,
    pub external_path_access: bool,
    pub absolute_path_patterns: Vec<String>,
    pub compound_command: bool,
    pub shell_control_operators: Vec<String>,
    pub risky_shell_wrapper: bool,
    pub expected_silent_output: bool,
    pub parser_status: String,
    pub subcommands: Vec<ShellSubcommandFact>,
    pub redirections: Vec<ShellRedirectionFact>,
    pub mutation_paths: Vec<String>,
    pub mutation_indicators: Vec<String>,
    pub command_plan: BashCommandPlan,
    pub permission_rule_suggestions: Vec<CommandPermissionRuleSuggestion>,
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
        return build_command_classification(CommandClassificationInput {
            normalized_command: normalized,
            base_command: &base_command,
            command_kind: CommandKind::Dangerous,
            category: ShellCommandCategory::Destructive,
            validation_family: None,
            safe_for_closeout: false,
            shell_wrapped,
            env_prefixed,
        });
    }

    if let Some(family) = validation_family(&base_command) {
        let category = if validation_family_is_test_run(family) {
            ShellCommandCategory::TestRun
        } else {
            ShellCommandCategory::Validation
        };
        return build_command_classification(CommandClassificationInput {
            normalized_command: normalized,
            base_command: &base_command,
            command_kind: CommandKind::Validation,
            category,
            validation_family: Some(family),
            safe_for_closeout: true,
            shell_wrapped,
            env_prefixed,
        });
    }

    let category = shell_command_category(&base_command);
    let command_kind = command_kind_for_category(category);
    let safe_for_closeout = matches!(
        category,
        ShellCommandCategory::Validation | ShellCommandCategory::TestRun
    );

    build_command_classification(CommandClassificationInput {
        normalized_command: normalized,
        base_command: &base_command,
        command_kind,
        category,
        validation_family: None,
        safe_for_closeout,
        shell_wrapped,
        env_prefixed,
    })
}

struct CommandClassificationInput<'a> {
    normalized_command: String,
    base_command: &'a str,
    command_kind: CommandKind,
    category: ShellCommandCategory,
    validation_family: Option<ValidationFamily>,
    safe_for_closeout: bool,
    shell_wrapped: bool,
    env_prefixed: bool,
}

fn build_command_classification(input: CommandClassificationInput<'_>) -> CommandClassification {
    let path_patterns = extract_path_patterns(input.base_command);
    let absolute_path_patterns = absolute_path_patterns(&path_patterns);
    let external_path_access = path_patterns.iter().any(|path| external_path_pattern(path));
    let network_access = command_has_network_access(input.base_command, input.category);
    let shell_control_operators = shell_control_operators(input.base_command);
    let compound_command = !shell_control_operators.is_empty();
    let subcommands = shell_subcommand_facts(input.base_command);
    let redirections = shell_redirection_facts(input.base_command);
    let mutation_indicators = shell_mutation_indicators(input.base_command);
    let mutation_paths = shell_mutation_paths(input.base_command, &redirections);
    let command_plan = bash_command_plan(
        input.base_command,
        compound_command,
        &subcommands,
        &redirections,
    );
    let parser_status = command_plan.parser_status.clone();
    let risky_shell_wrapper = input.shell_wrapped
        && (compound_command
            || network_access
            || external_path_access
            || matches!(
                input.category,
                ShellCommandCategory::Destructive
                    | ShellCommandCategory::FileMutation
                    | ShellCommandCategory::GitMutation
                    | ShellCommandCategory::PackageInstall
            ));
    let expected_silent_output =
        command_expected_silent_output(input.base_command, input.validation_family);
    let permission_rule_suggestions = command_permission_rule_suggestions(
        input.base_command,
        input.category,
        input.validation_family,
        input.safe_for_closeout,
        network_access,
        external_path_access,
        compound_command,
    );

    CommandClassification {
        normalized_command: input.normalized_command,
        command_kind: input.command_kind,
        category: input.category,
        validation_family: input.validation_family,
        path_patterns,
        safe_for_closeout: input.safe_for_closeout,
        shell_wrapped: input.shell_wrapped,
        env_prefixed: input.env_prefixed,
        network_access,
        external_path_access,
        absolute_path_patterns,
        compound_command,
        shell_control_operators,
        risky_shell_wrapper,
        expected_silent_output,
        parser_status: parser_status.to_string(),
        subcommands,
        redirections,
        mutation_paths,
        mutation_indicators,
        command_plan,
        permission_rule_suggestions,
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
    } else if lower.contains("cargo fmt") && lower.contains("--check") {
        Some(ValidationFamily::CargoFmtCheck)
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
    } else if lower.starts_with("python3 -m unittest")
        || lower.starts_with("python -m unittest")
        || is_python_test_script(command)
    {
        Some(ValidationFamily::PythonUnittest)
    } else if lower == "go test" || lower.starts_with("go test ") {
        Some(ValidationFamily::GoTest)
    } else if lower.starts_with("node ") && !lower.starts_with("node -i") {
        Some(ValidationFamily::NodeScript)
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

fn is_python_test_script(command: &str) -> bool {
    let tokens = command.split_whitespace().collect::<Vec<_>>();
    let [python, script] = tokens.as_slice() else {
        return false;
    };
    if !matches!(*python, "python" | "python3") {
        return false;
    }
    let script = script.strip_prefix("./").unwrap_or(script);
    if !script.ends_with(".py") {
        return false;
    }
    let file_name = script.rsplit('/').next().unwrap_or(script);
    file_name.starts_with("test_")
        || file_name.ends_with("_test.py")
        || script.starts_with("tests/")
        || script.contains("/tests/")
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
    if is_git_mutation_command(&lower) {
        return ShellCommandCategory::GitMutation;
    }
    if is_legacy_mutation_command(&lower) {
        return ShellCommandCategory::FileMutation;
    }
    if is_python_inline_probe(&lower) {
        return ShellCommandCategory::TestRun;
    }
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
        || lower == "npm ci"
        || lower.starts_with("npm ci ")
        || lower.starts_with("npm i ")
        || lower.starts_with("npm add ")
        || lower.starts_with("pnpm install")
        || lower.starts_with("pnpm add ")
        || lower.starts_with("yarn install")
        || lower.starts_with("yarn add ")
        || lower.starts_with("cargo add ")
        || lower.starts_with("cargo install ")
        || lower.starts_with("go get ")
        || lower.starts_with("go install ")
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
        || lower == "cargo watch"
        || lower.starts_with("cargo watch ")
        || lower == "watchexec"
        || lower.starts_with("watchexec ")
        || lower == "trunk serve"
        || lower.starts_with("trunk serve ")
        || lower == "python -m http.server"
        || lower.starts_with("python -m http.server ")
        || lower == "python3 -m http.server"
        || lower.starts_with("python3 -m http.server ")
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
        || lower.starts_with("git clone")
        || lower.starts_with("git fetch")
        || lower.starts_with("git push")
        || lower.starts_with("git pull")
        || lower.starts_with("git submodule")
}

fn is_legacy_mutation_command(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    matches!(
        lower.split_whitespace().next(),
        Some("touch" | "mkdir" | "cp" | "mv" | "rm" | "chmod" | "chown" | "ln")
    ) || lower.contains(" > ")
        || lower.contains(" >> ")
        || shell_redirection_facts(command)
            .iter()
            .any(|fact| fact.writes)
        || lower.contains("tee ")
        || lower.starts_with("patch ")
        || lower.starts_with("apply_patch")
        || lower == "cargo fmt"
        || lower.starts_with("cargo fmt ")
        || lower.contains("sed -i")
        || lower.contains("perl -pi")
        || python_inline_mutates_files(&lower)
}

fn is_python_inline_probe(lower: &str) -> bool {
    lower.starts_with("python -c ") || lower.starts_with("python3 -c ")
}

fn extract_path_patterns(command: &str) -> Vec<String> {
    let tokens = shell_tokens(command)
        .into_iter()
        .map(|token| clean_shell_token(&token))
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

fn shell_tokens(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for ch in command.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if quote != Some('\'') && ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            continue;
        }
        if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(ch);
    }

    if escaped {
        current.push('\\');
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn absolute_path_patterns(path_patterns: &[String]) -> Vec<String> {
    let mut paths = path_patterns
        .iter()
        .filter(|path| path.starts_with('/') || path.starts_with("~/"))
        .cloned()
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn external_path_pattern(path: &str) -> bool {
    path.starts_with('/') || path.starts_with("~/") || path.starts_with("../")
}

fn shell_control_operators(command: &str) -> Vec<String> {
    let mut operators = Vec::new();
    for (label, found) in [
        ("and", command.contains("&&")),
        ("or", command.contains("||")),
        ("semicolon", command.contains(';')),
        ("pipe", contains_single_pipe(command)),
        ("redirect", contains_shell_redirection(command)),
        (
            "background",
            command.trim_end().ends_with('&') || command.contains(" & "),
        ),
        (
            "command_substitution",
            command.contains("$(") || command.contains('`'),
        ),
    ] {
        if found {
            operators.push(label.to_string());
        }
    }
    operators
}

fn shell_subcommand_facts(command: &str) -> Vec<ShellSubcommandFact> {
    split_shell_subcommands(command)
        .into_iter()
        .take(MAX_SUBCOMMAND_FACTS + 1)
        .enumerate()
        .map(|(index, subcommand)| {
            let category = shell_command_category(&subcommand);
            let command_kind = command_kind_for_category(category);
            let redirection = contains_shell_redirection(&subcommand);
            ShellSubcommandFact {
                index,
                command: subcommand,
                category,
                command_kind,
                mutation: matches!(
                    category,
                    ShellCommandCategory::FileMutation | ShellCommandCategory::GitMutation
                ),
                redirection,
            }
        })
        .collect()
}

fn split_shell_subcommands(command: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let chars = command.chars().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < chars.len() {
        let ch = chars[index];
        if escaped {
            current.push(ch);
            escaped = false;
            index += 1;
            continue;
        }
        if quote != Some('\'') && ch == '\\' {
            current.push(ch);
            escaped = true;
            index += 1;
            continue;
        }
        if let Some(active_quote) = quote {
            current.push(ch);
            if ch == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            current.push(ch);
            index += 1;
            continue;
        }

        let next = chars.get(index + 1).copied();
        let is_separator = ch == ';'
            || ch == '\n'
            || (ch == '&' && next == Some('&'))
            || (ch == '|' && next == Some('|'))
            || (ch == '|' && next != Some('|'));
        if is_separator {
            push_shell_subcommand(&mut parts, &mut current);
            if matches!((ch, next), ('&', Some('&')) | ('|', Some('|'))) {
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }

        current.push(ch);
        index += 1;
    }

    push_shell_subcommand(&mut parts, &mut current);
    parts
}

fn push_shell_subcommand(parts: &mut Vec<String>, current: &mut String) {
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }
    current.clear();
}

fn shell_redirection_facts(command: &str) -> Vec<ShellRedirectionFact> {
    let tokens = shell_tokens(command);
    let mut facts = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        let token = tokens[index].as_str();
        if let Some((operator, inline_target, writes)) = redirection_operator(token) {
            let target = inline_target
                .map(str::to_string)
                .or_else(|| tokens.get(index + 1).cloned())
                .filter(|target| !target.starts_with('&'));
            let writes = writes
                && target
                    .as_deref()
                    .map(|target| target != "/dev/null")
                    .unwrap_or(true);
            facts.push(ShellRedirectionFact {
                operator: operator.to_string(),
                target,
                writes,
            });
        }
        index += 1;
    }
    facts
}

fn redirection_operator(token: &str) -> Option<(&'static str, Option<&str>, bool)> {
    for operator in [">>", "2>>", "2>", "&>", ">", "<<"] {
        if token == operator {
            return Some((operator, None, operator != "<<"));
        }
        if let Some(rest) = token.strip_prefix(operator).filter(|rest| !rest.is_empty()) {
            return Some((operator, Some(rest), operator != "<<"));
        }
    }
    None
}

fn shell_mutation_paths(command: &str, redirections: &[ShellRedirectionFact]) -> Vec<String> {
    let tokens = shell_tokens(command)
        .into_iter()
        .map(|token| clean_shell_token(&token))
        .collect::<Vec<_>>();
    let mut paths = redirections
        .iter()
        .filter(|fact| fact.writes)
        .filter_map(|fact| fact.target.clone())
        .collect::<Vec<_>>();

    for (index, token) in tokens.iter().enumerate() {
        if token == "tee" {
            paths.extend(
                tokens
                    .iter()
                    .skip(index + 1)
                    .take_while(|value| !shell_control_token(value))
                    .filter(|value| !value.starts_with('-'))
                    .cloned(),
            );
        }
        if matches!(token.as_str(), "touch" | "mkdir" | "cp" | "mv" | "rm") {
            paths.extend(
                tokens
                    .iter()
                    .skip(index + 1)
                    .filter(|value| likely_path_token(value))
                    .cloned(),
            );
        }
    }

    paths.sort();
    paths.dedup();
    paths
}

fn shell_control_token(token: &str) -> bool {
    matches!(token, "|" | "||" | "&&" | ";")
}

fn shell_mutation_indicators(command: &str) -> Vec<String> {
    let lower = command.to_ascii_lowercase();
    let mut indicators = Vec::new();
    for (name, detected) in [
        (
            "redirection_write",
            shell_redirection_facts(command)
                .iter()
                .any(|fact| fact.writes),
        ),
        ("sed_in_place", lower.contains("sed -i")),
        ("perl_in_place", lower.contains("perl -pi")),
        ("python_inline_write", python_inline_mutates_files(&lower)),
        (
            "tee_write",
            lower.split_whitespace().any(|token| token == "tee"),
        ),
        ("apply_patch", lower.contains("apply_patch")),
        (
            "filesystem_mutation",
            matches!(
                lower.split_whitespace().next(),
                Some("touch" | "mkdir" | "cp" | "mv" | "rm" | "chmod" | "chown" | "ln")
            ),
        ),
    ] {
        if detected {
            indicators.push(name.to_string());
        }
    }
    indicators
}

fn bash_command_plan(
    command: &str,
    compound_command: bool,
    subcommands: &[ShellSubcommandFact],
    redirections: &[ShellRedirectionFact],
) -> BashCommandPlan {
    let has_unclosed_quote = shell_has_unclosed_quote(command);
    let has_process_substitution = command.contains("<(") || command.contains(">(");
    let has_command_substitution = command.contains("$(") || command.contains('`');
    let has_heredoc = redirections.iter().any(|fact| fact.operator == "<<");
    let write_redirection_targets = redirections
        .iter()
        .filter(|fact| fact.writes)
        .filter_map(|fact| fact.target.clone())
        .collect::<Vec<_>>();
    let has_write_redirection = !write_redirection_targets.is_empty();
    let cd_targets = shell_cd_targets(subcommands);
    let git_subcommands = shell_git_subcommands(subcommands);
    let ambiguous = has_unclosed_quote || has_process_substitution || has_command_substitution;

    let parser_status = if has_unclosed_quote {
        "ambiguous_unclosed_quote"
    } else if subcommands.len() > MAX_SUBCOMMAND_FACTS {
        "too_many_subcommands"
    } else if has_process_substitution {
        "ambiguous_process_substitution"
    } else if has_command_substitution {
        "ambiguous_command_substitution"
    } else if has_heredoc {
        "heredoc"
    } else if compound_command {
        "compound"
    } else {
        "simple"
    }
    .to_string();

    let mut fail_closed_reasons = Vec::new();
    if has_unclosed_quote {
        fail_closed_reasons.push("ambiguous_unclosed_quote".to_string());
    }
    if subcommands.len() > MAX_SUBCOMMAND_FACTS {
        fail_closed_reasons.push("too_many_subcommands".to_string());
    }
    if has_process_substitution {
        fail_closed_reasons.push("process_substitution".to_string());
    }
    if has_command_substitution {
        fail_closed_reasons.push("command_substitution".to_string());
    }
    if has_heredoc {
        fail_closed_reasons.push("heredoc".to_string());
    }
    if has_write_redirection {
        fail_closed_reasons.push("write_redirection".to_string());
    }
    if !cd_targets.is_empty() && compound_command {
        fail_closed_reasons.push("cd_context_shift".to_string());
    }
    if subcommands.iter().any(|fact| fact.mutation) {
        fail_closed_reasons.push("mutation_subcommand".to_string());
    }
    if subcommands
        .iter()
        .any(|fact| fact.category == ShellCommandCategory::Destructive)
    {
        fail_closed_reasons.push("destructive_subcommand".to_string());
    }

    fail_closed_reasons.sort();
    fail_closed_reasons.dedup();

    BashCommandPlan {
        parser_status,
        fail_closed: !fail_closed_reasons.is_empty(),
        fail_closed_reasons,
        subcommand_count: subcommands.len(),
        subcommand_cap: MAX_SUBCOMMAND_FACTS,
        has_cd_command: !cd_targets.is_empty(),
        cd_targets,
        has_git_command: !git_subcommands.is_empty(),
        git_subcommands,
        has_process_substitution,
        has_command_substitution,
        has_heredoc,
        has_write_redirection,
        write_redirection_targets,
        ambiguous,
    }
}

fn shell_has_unclosed_quote(command: &str) -> bool {
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for ch in command.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote != Some('\'') && ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
        }
    }

    quote.is_some()
}

fn shell_cd_targets(subcommands: &[ShellSubcommandFact]) -> Vec<String> {
    subcommands
        .iter()
        .filter_map(|fact| {
            let tokens = shell_tokens(&fact.command);
            if tokens.first().map(String::as_str) == Some("cd") {
                tokens
                    .get(1)
                    .map(|target| clean_shell_token(target))
                    .filter(|target| !target.is_empty())
            } else {
                None
            }
        })
        .collect()
}

fn shell_git_subcommands(subcommands: &[ShellSubcommandFact]) -> Vec<String> {
    subcommands
        .iter()
        .filter_map(|fact| {
            let tokens = shell_tokens(&fact.command);
            if tokens.first().map(String::as_str) == Some("git") {
                tokens
                    .get(1)
                    .map(|subcommand| clean_shell_token(subcommand))
                    .filter(|subcommand| !subcommand.is_empty())
            } else {
                None
            }
        })
        .collect()
}

fn python_inline_mutates_files(lower: &str) -> bool {
    (lower.starts_with("python -c ")
        || lower.starts_with("python3 -c ")
        || lower.starts_with("python <<")
        || lower.starts_with("python3 <<"))
        && (lower.contains(".write(")
            || lower.contains("write_text(")
            || lower.contains("write_bytes(")
            || lower.contains("open(") && (lower.contains(", 'w") || lower.contains(", \"w")))
}

fn contains_single_pipe(command: &str) -> bool {
    let bytes = command.as_bytes();
    bytes.iter().enumerate().any(|(index, byte)| {
        *byte == b'|'
            && bytes.get(index.wrapping_sub(1)) != Some(&b'|')
            && bytes.get(index + 1) != Some(&b'|')
    })
}

fn contains_shell_redirection(command: &str) -> bool {
    command.contains(" >")
        || command.contains("> ")
        || command.contains(" <")
        || command.contains("< ")
        || command.contains("<<")
        || command.contains("2>")
        || command.contains("&>")
}

fn command_has_network_access(command: &str, category: ShellCommandCategory) -> bool {
    if category == ShellCommandCategory::PackageInstall {
        return true;
    }
    let lower = command.to_ascii_lowercase();
    if lower.contains("://") || lower.contains("git@") {
        return true;
    }
    let tokens = shell_tokens(&lower);
    tokens
        .iter()
        .enumerate()
        .any(|(index, token)| is_network_executable(token) || is_network_subcommand(&tokens, index))
}

fn is_network_executable(token: &str) -> bool {
    matches!(
        token,
        "curl"
            | "wget"
            | "ssh"
            | "scp"
            | "sftp"
            | "rsync"
            | "nc"
            | "ncat"
            | "netcat"
            | "telnet"
            | "ftp"
    )
}

fn is_network_subcommand(tokens: &[String], index: usize) -> bool {
    let Some(token) = tokens.get(index).map(String::as_str) else {
        return false;
    };
    let next = tokens.get(index + 1).map(String::as_str);
    match token {
        "git" => matches!(
            next,
            Some("clone" | "fetch" | "pull" | "push" | "ls-remote")
        ),
        "gh" => true,
        "brew" => matches!(next, Some("install" | "update" | "upgrade" | "tap")),
        "cargo" => matches!(next, Some("add" | "fetch" | "install" | "update")),
        "go" => matches!(next, Some("get" | "install")),
        "npm" => matches!(next, Some("install" | "i" | "ci" | "add" | "publish")),
        "pnpm" | "yarn" => matches!(next, Some("install" | "add" | "publish" | "dlx")),
        "pip" | "pip3" => matches!(next, Some("install" | "download")),
        _ => false,
    }
}

fn command_expected_silent_output(
    command: &str,
    validation_family: Option<ValidationFamily>,
) -> bool {
    let lower = command.trim().to_ascii_lowercase();
    let shell_assertion_prints = lower.contains(" echo ") || lower.starts_with("echo ");
    if matches!(validation_family, Some(ValidationFamily::ShellAssertion))
        && !shell_assertion_prints
    {
        return true;
    }
    lower.starts_with("git diff --quiet")
        || lower.starts_with("git diff --exit-code --quiet")
        || lower.starts_with("cmp -s ")
        || lower.starts_with("rg -q ")
        || lower.starts_with("grep -q ")
        || ((lower.starts_with("test ") || lower.starts_with("[ ") || lower.starts_with("[[ "))
            && !shell_assertion_prints)
        || (lower.contains("cargo fmt") && lower.contains("--check"))
}

fn command_permission_rule_suggestions(
    command: &str,
    category: ShellCommandCategory,
    validation_family: Option<ValidationFamily>,
    safe_for_closeout: bool,
    network_access: bool,
    external_path_access: bool,
    compound_command: bool,
) -> Vec<CommandPermissionRuleSuggestion> {
    if category == ShellCommandCategory::Destructive {
        return Vec::new();
    }

    let command = command.trim();
    if command.is_empty() {
        return Vec::new();
    }

    let mut suggestions = vec![CommandPermissionRuleSuggestion {
        pattern: command.to_string(),
        scope: CommandPermissionRuleScope::Exact,
        stable: false,
        reason: "exact command for this permission review".to_string(),
    }];

    if !safe_for_closeout || network_access || external_path_access || compound_command {
        return suggestions;
    }

    if let Some(prefix) = stable_validation_permission_prefix(command, validation_family) {
        suggestions.push(CommandPermissionRuleSuggestion {
            pattern: prefix.to_string(),
            scope: CommandPermissionRuleScope::Prefix,
            stable: true,
            reason: "stable validation prefix with no network or external path access".to_string(),
        });
    }

    suggestions
}

fn stable_validation_permission_prefix(
    command: &str,
    validation_family: Option<ValidationFamily>,
) -> Option<&'static str> {
    let lower = command.trim().to_ascii_lowercase();
    match validation_family {
        Some(ValidationFamily::CargoTest) => Some("cargo test"),
        Some(ValidationFamily::CargoCheck) => Some("cargo check"),
        Some(ValidationFamily::CargoClippy) => Some("cargo clippy"),
        Some(ValidationFamily::CargoFmtCheck) => Some("cargo fmt --check"),
        Some(ValidationFamily::NpmTest) => {
            if lower.starts_with("npm run test") {
                Some("npm run test")
            } else {
                Some("npm test")
            }
        }
        Some(ValidationFamily::PnpmTest) => Some("pnpm test"),
        Some(ValidationFamily::YarnTest) => Some("yarn test"),
        Some(ValidationFamily::Pytest) => {
            if lower.starts_with("python -m pytest") {
                Some("python -m pytest")
            } else if lower.starts_with("python3 -m pytest") {
                Some("python3 -m pytest")
            } else {
                Some("pytest")
            }
        }
        Some(ValidationFamily::PythonCompile) => {
            if lower.starts_with("python -m py_compile") {
                Some("python -m py_compile")
            } else {
                Some("python3 -m py_compile")
            }
        }
        Some(ValidationFamily::PythonUnittest) => {
            if lower.starts_with("python -m unittest") {
                Some("python -m unittest")
            } else {
                Some("python3 -m unittest")
            }
        }
        Some(ValidationFamily::GoTest) => Some("go test"),
        Some(ValidationFamily::BashSyntax) => {
            if lower.starts_with("sh -n ") {
                Some("sh -n")
            } else {
                Some("bash -n")
            }
        }
        Some(ValidationFamily::ProjectScript) => None,
        Some(ValidationFamily::RgAssertion) => None,
        Some(ValidationFamily::ShellAssertion) => None,
        Some(ValidationFamily::NodeScript) => None,
        None => None,
    }
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
        || token.contains("://")
        || token.starts_with("git@")
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
}
