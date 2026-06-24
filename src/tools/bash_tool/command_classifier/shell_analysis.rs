//! Shell command classification support.
//!
//! Keeps deterministic command-risk analysis separate from shell execution.

use super::*;

pub(super) fn extract_path_patterns(command: &str) -> Vec<String> {
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

pub(super) fn shell_tokens(command: &str) -> Vec<String> {
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

pub(super) fn absolute_path_patterns(path_patterns: &[String]) -> Vec<String> {
    let mut paths = path_patterns
        .iter()
        .filter(|path| path.starts_with('/') || path.starts_with("~/"))
        .cloned()
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

pub(super) fn external_path_pattern(path: &str) -> bool {
    path.starts_with('/') || path.starts_with("~/") || path.starts_with("../")
}

pub(super) fn shell_control_operators(command: &str) -> Vec<String> {
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

pub(super) fn shell_subcommand_facts(command: &str) -> Vec<ShellSubcommandFact> {
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

pub(super) fn split_shell_subcommands(command: &str) -> Vec<String> {
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

pub(super) fn push_shell_subcommand(parts: &mut Vec<String>, current: &mut String) {
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }
    current.clear();
}

pub(super) fn shell_redirection_facts(command: &str) -> Vec<ShellRedirectionFact> {
    let tokens = shell_tokens(command);
    let mut facts = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        let token = tokens[index].as_str();
        if let Some((operator, inline_target, writes)) = redirection_operator(token) {
            let raw_target = inline_target
                .map(str::to_string)
                .or_else(|| tokens.get(index + 1).cloned());
            let duplicates_fd = raw_target
                .as_deref()
                .is_some_and(|target| target.starts_with('&'));
            let target = raw_target.filter(|target| !target.starts_with('&'));
            let writes = writes
                && !duplicates_fd
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

pub(super) fn redirection_operator(token: &str) -> Option<(&'static str, Option<&str>, bool)> {
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

pub(super) fn shell_mutation_paths(
    command: &str,
    redirections: &[ShellRedirectionFact],
) -> Vec<String> {
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

pub(super) fn shell_control_token(token: &str) -> bool {
    matches!(token, "|" | "||" | "&&" | ";")
}

pub(super) fn shell_mutation_indicators(command: &str) -> Vec<String> {
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
