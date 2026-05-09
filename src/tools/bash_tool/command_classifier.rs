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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandClassification {
    pub normalized_command: String,
    pub command_kind: CommandKind,
    pub validation_family: Option<ValidationFamily>,
    pub safe_for_closeout: bool,
    pub shell_wrapped: bool,
    pub env_prefixed: bool,
}

impl CommandClassification {
    pub fn is_safe_validation(&self) -> bool {
        self.command_kind == CommandKind::Validation && self.safe_for_closeout
    }
}

pub fn classify_command(command: &str) -> CommandClassification {
    let normalized = normalize_command_for_match(command);
    let shell_wrapped = normalized.trim() != command.trim();
    let (base_command, env_prefixed) = strip_env_prefix(&normalized);

    if crate::security::is_dangerous_command(command) {
        return CommandClassification {
            normalized_command: normalized,
            command_kind: CommandKind::Dangerous,
            validation_family: None,
            safe_for_closeout: false,
            shell_wrapped,
            env_prefixed,
        };
    }

    if let Some(family) = validation_family(base_command) {
        return CommandClassification {
            normalized_command: normalized,
            command_kind: CommandKind::Validation,
            validation_family: Some(family),
            safe_for_closeout: true,
            shell_wrapped,
            env_prefixed,
        };
    }

    let command_kind = if is_inspection_command(base_command) {
        CommandKind::Inspection
    } else if is_mutation_command(base_command) {
        CommandKind::Mutation
    } else {
        CommandKind::Unknown
    };

    CommandClassification {
        normalized_command: normalized,
        command_kind,
        validation_family: None,
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
    } else if lower.starts_with("node ") {
        Some(ValidationFamily::NodeScript)
    } else if lower.starts_with("python3 -c ") || lower.starts_with("python -c ") {
        Some(ValidationFamily::Pytest)
    } else {
        None
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
    let command = command.strip_prefix("! ").unwrap_or(command).trim();
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

fn is_inspection_command(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    matches!(
        lower.split_whitespace().next(),
        Some("ls" | "cat" | "head" | "tail" | "sed" | "awk" | "rg" | "grep" | "find" | "pwd")
    ) || lower.starts_with("git status")
        || lower.starts_with("git diff")
        || lower.starts_with("git log")
        || lower.starts_with("git show")
}

fn is_mutation_command(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    matches!(
        lower.split_whitespace().next(),
        Some("touch" | "mkdir" | "cp" | "mv" | "rm" | "chmod" | "chown" | "ln")
    ) || lower.contains(" > ")
        || lower.contains(" >> ")
        || lower.starts_with("git add")
        || lower.starts_with("git commit")
        || lower.starts_with("git checkout")
        || lower.starts_with("git switch")
        || lower.starts_with("git reset")
        || lower.starts_with("git clean")
        || lower.starts_with("git merge")
        || lower.starts_with("git rebase")
        || lower.starts_with("git apply")
        || lower.starts_with("patch ")
        || lower.contains("sed -i")
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
        assert_eq!(class.validation_family, Some(ValidationFamily::CargoTest));
        assert!(class.safe_for_closeout);
        assert!(class.env_prefixed);
    }

    #[test]
    fn classifies_shell_wrapped_validation() {
        let class = classify_command("bash -lc 'env FOO=1 cargo check --quiet'");
        assert_eq!(class.command_kind, CommandKind::Validation);
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
    }

    #[test]
    fn classifies_rg_assertion_as_validation() {
        let class = classify_command("! rg 'bad_pattern' src/lib.rs");
        assert_eq!(class.command_kind, CommandKind::Validation);
        assert_eq!(class.validation_family, Some(ValidationFamily::RgAssertion));
        assert!(class.safe_for_closeout);
    }

    #[test]
    fn dangerous_commands_are_not_safe_for_closeout() {
        let class = classify_command("rm -rf /");
        assert_eq!(class.command_kind, CommandKind::Dangerous);
        assert!(!class.safe_for_closeout);
    }
}
