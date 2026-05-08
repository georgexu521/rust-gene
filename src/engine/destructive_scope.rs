//! Runtime contract for destructive operation scope.
//!
//! The permission layer answers "is this risky enough to ask?". This module
//! answers a narrower question: "does this destructive command target exactly
//! what the user approved in the latest request?"

use crate::services::api::ToolCall;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestructiveScopeContract {
    approved_targets: Vec<PathBuf>,
    destructive_intent: bool,
    cleanup_requested: bool,
    singular_file_reference: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestructiveScopeCheck {
    pub applies: bool,
    pub allowed: bool,
    pub operation: String,
    pub target: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DestructiveOperation {
    name: String,
    targets: Vec<PathBuf>,
    recursive: bool,
    scope_kind: DestructiveScopeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DestructiveScopeKind {
    Filesystem,
    Repository,
}

impl DestructiveScopeContract {
    pub fn from_user_request(user_request: &str, working_dir: &Path) -> Self {
        let lower = user_request.to_ascii_lowercase();
        let destructive_intent = contains_any(
            &lower,
            &[
                "delete",
                "remove",
                "rm ",
                "trash",
                "move",
                "rename",
                "reset",
                "clean",
                "cleanup",
                "删除",
                "删掉",
                "删了",
                "移除",
                "移动",
                "重命名",
                "重置",
                "清理",
            ],
        );
        let cleanup_requested = contains_any(
            &lower,
            &[
                "folder",
                "directory",
                "dir",
                "parent",
                "recursive",
                "all",
                "everything",
                "cleanup",
                "clean up",
                "文件夹",
                "目录",
                "父目录",
                "整个",
                "全部",
                "所有",
                "清理",
            ],
        );
        let singular_file_reference = contains_any(
            &lower,
            &[
                "this file",
                "that file",
                "current file",
                "这个文件",
                "该文件",
                "当前文件",
                "这份文件",
            ],
        );
        let approved_targets = extract_user_path_targets(user_request)
            .into_iter()
            .map(|target| normalize_path_for_scope(&target, working_dir))
            .collect();

        Self {
            approved_targets,
            destructive_intent,
            cleanup_requested,
            singular_file_reference,
        }
    }

    pub fn check_tool_call(
        &self,
        tool_call: &ToolCall,
        working_dir: &Path,
    ) -> DestructiveScopeCheck {
        let Some(operation) = destructive_operation_from_tool_call(tool_call, working_dir) else {
            return DestructiveScopeCheck {
                applies: false,
                allowed: true,
                operation: "none".to_string(),
                target: None,
                reason: "tool call is not destructive".to_string(),
            };
        };

        let target_preview = operation
            .targets
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let target = (!target_preview.is_empty()).then_some(target_preview.clone());

        if self.cleanup_requested {
            return DestructiveScopeCheck {
                applies: true,
                allowed: true,
                operation: operation.name,
                target,
                reason: "user request explicitly allowed cleanup or broader directory scope"
                    .to_string(),
            };
        }

        if !self.destructive_intent {
            return DestructiveScopeCheck {
                applies: true,
                allowed: false,
                operation: operation.name,
                target,
                reason:
                    "destructive tool call is outside the latest user request; explicit scope is required"
                        .to_string(),
            };
        }

        if operation.scope_kind == DestructiveScopeKind::Repository {
            return DestructiveScopeCheck {
                applies: true,
                allowed: false,
                operation: operation.name,
                target,
                reason:
                    "repository-wide destructive operation requires explicit reset/clean scope from the user"
                        .to_string(),
            };
        }

        if !self.approved_targets.is_empty() {
            let outside = operation
                .targets
                .iter()
                .find(|target| !self.target_is_approved(target));
            if let Some(outside) = outside {
                return DestructiveScopeCheck {
                    applies: true,
                    allowed: false,
                    operation: operation.name,
                    target,
                    reason: format!(
                        "destructive target '{}' is outside the user-approved scope ({})",
                        outside.display(),
                        self.approved_targets_preview()
                    ),
                };
            }
            return DestructiveScopeCheck {
                applies: true,
                allowed: true,
                operation: operation.name,
                target,
                reason: "destructive target matches the user-approved scope".to_string(),
            };
        }

        if self.singular_file_reference {
            let file_like = !operation.recursive
                && operation
                    .targets
                    .iter()
                    .all(|target| path_looks_like_file(target));
            return DestructiveScopeCheck {
                applies: true,
                allowed: file_like,
                operation: operation.name,
                target,
                reason: if file_like {
                    "user referred to a single file and the command targets file-like path(s)"
                        .to_string()
                } else {
                    "user referred to a single file, but the command targets a recursive or directory-like scope"
                        .to_string()
                },
            };
        }

        DestructiveScopeCheck {
            applies: true,
            allowed: true,
            operation: operation.name,
            target,
            reason: "destructive intent detected; exact target not inferred, falling back to permission approval"
                .to_string(),
        }
    }

    pub fn completion_guard_for_results<'a>(
        &self,
        results: impl IntoIterator<Item = (&'a ToolCall, bool)>,
        working_dir: &Path,
    ) -> Option<String> {
        if self.cleanup_requested {
            return None;
        }

        let mut targets = Vec::new();
        for (tool_call, success) in results {
            if !success {
                continue;
            }
            let Some(operation) = destructive_operation_from_tool_call(tool_call, working_dir)
            else {
                continue;
            };
            if operation.scope_kind != DestructiveScopeKind::Filesystem {
                continue;
            }
            for target in operation.targets {
                let label = target.display().to_string();
                if !targets.contains(&label) {
                    targets.push(label);
                }
            }
        }

        if targets.is_empty() {
            return None;
        }

        Some(format!(
            "Destructive scope completed only for: {}. Do not ask or suggest deleting parent directories, sibling files, unrelated folders, or broader cleanup unless the user explicitly requests that scope.",
            targets.join(", ")
        ))
    }

    fn target_is_approved(&self, target: &Path) -> bool {
        self.approved_targets
            .iter()
            .any(|approved| target == approved || path_suffix_matches(target, approved))
    }

    fn approved_targets_preview(&self) -> String {
        if self.approved_targets.is_empty() {
            "none".to_string()
        } else {
            self.approved_targets
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
}

fn destructive_operation_from_tool_call(
    tool_call: &ToolCall,
    working_dir: &Path,
) -> Option<DestructiveOperation> {
    match tool_call.name.as_str() {
        "bash" | "powershell" | "remote_dev" => {
            let command = tool_call.arguments["command"]
                .as_str()
                .or_else(|| tool_call.arguments["cmd"].as_str())?;
            let command_working_dir = tool_call_working_dir(tool_call, working_dir);
            destructive_operation_from_shell(command, &command_working_dir)
        }
        "worktree" if tool_call.arguments["action"].as_str() == Some("remove") => {
            let target = tool_call.arguments["path"]
                .as_str()
                .or_else(|| tool_call.arguments["name"].as_str())
                .map(|raw| normalize_path_for_scope(raw, working_dir))
                .into_iter()
                .collect::<Vec<_>>();
            Some(DestructiveOperation {
                name: "worktree remove".to_string(),
                targets: target,
                recursive: true,
                scope_kind: DestructiveScopeKind::Filesystem,
            })
        }
        _ => None,
    }
}

fn tool_call_working_dir(tool_call: &ToolCall, default_working_dir: &Path) -> PathBuf {
    let Some(raw) = tool_call.arguments["working_dir"].as_str() else {
        return default_working_dir.to_path_buf();
    };
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        normalize_components(&path)
    } else {
        normalize_components(&default_working_dir.join(path))
    }
}

fn destructive_operation_from_shell(
    command: &str,
    working_dir: &Path,
) -> Option<DestructiveOperation> {
    let normalized =
        crate::tools::bash_tool::command_classifier::normalize_command_for_match(command);
    let tokens = split_shell_like(&normalized);
    for idx in 0..tokens.len() {
        let token = command_basename(&tokens[idx]);
        match token.as_str() {
            "rm" | "trash" => {
                let (targets, recursive) = collect_rm_targets(&tokens[idx + 1..], working_dir);
                if !targets.is_empty() {
                    return Some(DestructiveOperation {
                        name: token,
                        targets,
                        recursive,
                        scope_kind: DestructiveScopeKind::Filesystem,
                    });
                }
            }
            "mv" => {
                let targets = collect_mv_targets(&tokens[idx + 1..], working_dir);
                if !targets.is_empty() {
                    return Some(DestructiveOperation {
                        name: "mv".to_string(),
                        targets,
                        recursive: false,
                        scope_kind: DestructiveScopeKind::Filesystem,
                    });
                }
            }
            "git" => {
                if let Some(next) = tokens.get(idx + 1).map(|value| value.as_str()) {
                    if matches!(next, "reset" | "clean") {
                        return Some(DestructiveOperation {
                            name: format!("git {}", next),
                            targets: Vec::new(),
                            recursive: true,
                            scope_kind: DestructiveScopeKind::Repository,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn collect_rm_targets(tokens: &[String], working_dir: &Path) -> (Vec<PathBuf>, bool) {
    let mut targets = Vec::new();
    let mut recursive = false;
    let mut after_double_dash = false;
    for token in tokens {
        if is_shell_separator(token) {
            break;
        }
        if !after_double_dash && token == "--" {
            after_double_dash = true;
            continue;
        }
        if !after_double_dash && token.starts_with('-') {
            if token.contains('r') || token.contains('R') {
                recursive = true;
            }
            continue;
        }
        if token.is_empty() {
            continue;
        }
        targets.push(normalize_path_for_scope(token, working_dir));
    }
    (targets, recursive)
}

fn collect_mv_targets(tokens: &[String], working_dir: &Path) -> Vec<PathBuf> {
    let mut positional = Vec::new();
    let mut after_double_dash = false;
    for token in tokens {
        if is_shell_separator(token) {
            break;
        }
        if !after_double_dash && token == "--" {
            after_double_dash = true;
            continue;
        }
        if !after_double_dash && token.starts_with('-') {
            continue;
        }
        positional.push(token.clone());
    }
    positional
        .into_iter()
        .map(|target| normalize_path_for_scope(&target, working_dir))
        .collect()
}

fn extract_user_path_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut current = String::new();

    for ch in text.chars().chain(std::iter::once(' ')) {
        if is_path_char(ch) {
            current.push(ch);
            continue;
        }
        push_path_candidate(&mut targets, &current);
        current.clear();
    }

    targets
}

fn push_path_candidate(targets: &mut Vec<String>, raw: &str) {
    let candidate = raw
        .trim_matches(|ch: char| {
            matches!(
                ch,
                '`' | '"'
                    | '\''
                    | ','
                    | '，'
                    | '.'
                    | '。'
                    | ':'
                    | '：'
                    | ';'
                    | '；'
                    | ')'
                    | ']'
                    | '}'
            )
        })
        .trim();
    if candidate.is_empty() {
        return;
    }
    if candidate.starts_with('-') {
        return;
    }
    let looks_like_path = candidate.starts_with("~/")
        || candidate.starts_with('/')
        || candidate.starts_with("./")
        || candidate.starts_with("../")
        || candidate.contains('/')
        || candidate.rsplit_once('.').is_some_and(|(_, ext)| {
            !ext.is_empty() && ext.len() <= 12 && ext.chars().all(|ch| ch.is_ascii_alphanumeric())
        });
    if looks_like_path && !targets.iter().any(|existing| existing == candidate) {
        targets.push(candidate.to_string());
    }
}

fn split_shell_like(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }

        match ch {
            '\'' | '"' => quote = Some(ch),
            ';' | '|' | '&' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                let mut separator = ch.to_string();
                if chars.peek().is_some_and(|next| *next == ch) {
                    separator.push(chars.next().unwrap_or(ch));
                }
                tokens.push(separator);
            }
            ch if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn normalize_path_for_scope(raw: &str, working_dir: &Path) -> PathBuf {
    let raw = raw.trim();
    let expanded = if let Some(rest) = raw.strip_prefix("~/") {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(rest)
    } else {
        PathBuf::from(raw)
    };
    let candidate = if expanded.is_absolute() {
        expanded
    } else {
        working_dir.join(expanded)
    };
    normalize_components(&candidate)
}

fn normalize_components(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn path_suffix_matches(target: &Path, approved: &Path) -> bool {
    if approved.components().count() != 1 {
        return false;
    }
    target.file_name() == approved.file_name()
}

fn path_looks_like_file(path: &Path) -> bool {
    path.extension().is_some()
}

fn command_basename(token: &str) -> String {
    Path::new(token)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(token)
        .to_ascii_lowercase()
}

fn is_shell_separator(token: &str) -> bool {
    matches!(token, ";" | "|" | "||" | "&" | "&&")
}

fn is_path_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || matches!(
            ch,
            '_' | '-' | '.' | '/' | '\\' | '~' | '@' | '+' | '=' | ':' | '%'
        )
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bash(command: &str) -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            name: "bash".to_string(),
            arguments: json!({ "command": command }),
        }
    }

    fn bash_in(command: &str, working_dir: &str) -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            name: "bash".to_string(),
            arguments: json!({ "command": command, "working_dir": working_dir }),
        }
    }

    fn root() -> PathBuf {
        PathBuf::from("/tmp/gex")
    }

    #[test]
    fn named_file_delete_allows_exact_target() {
        let contract = DestructiveScopeContract::from_user_request("删除 abc.txt", &root());
        let check = contract.check_tool_call(&bash("rm /tmp/gex/abc.txt"), &root());
        assert!(check.applies);
        assert!(check.allowed, "{}", check.reason);
    }

    #[test]
    fn named_file_delete_respects_bash_working_dir_for_relative_target() {
        let contract = DestructiveScopeContract::from_user_request("删除 abc.txt", &root());
        let check = contract.check_tool_call(&bash_in("rm abc.txt", "/tmp/gex"), Path::new("/tmp"));
        assert!(check.allowed, "{}", check.reason);
    }

    #[test]
    fn named_file_delete_blocks_parent_directory() {
        let contract = DestructiveScopeContract::from_user_request("删除 abc.txt", &root());
        let check = contract.check_tool_call(&bash("rm -rf /tmp/gex"), &root());
        assert!(check.applies);
        assert!(!check.allowed);
        assert!(check.reason.contains("outside the user-approved scope"));
    }

    #[test]
    fn singular_file_reference_allows_file_like_target() {
        let contract = DestructiveScopeContract::from_user_request("帮我把这个文件删了吧", &root());
        let check = contract.check_tool_call(&bash("rm /tmp/gex/abc.txt"), &root());
        assert!(check.allowed, "{}", check.reason);
    }

    #[test]
    fn singular_file_reference_blocks_recursive_directory_target() {
        let contract = DestructiveScopeContract::from_user_request("帮我把这个文件删了吧", &root());
        let check = contract.check_tool_call(&bash("rm -rf /tmp/gex"), &root());
        assert!(!check.allowed);
        assert!(check.reason.contains("single file"));
    }

    #[test]
    fn cleanup_request_allows_directory_delete() {
        let contract = DestructiveScopeContract::from_user_request("清理整个 gex 文件夹", &root());
        let check = contract.check_tool_call(&bash("rm -rf /tmp/gex"), &root());
        assert!(check.allowed, "{}", check.reason);
    }

    #[test]
    fn no_destructive_intent_blocks_rm() {
        let contract = DestructiveScopeContract::from_user_request("运行一下测试", &root());
        let check = contract.check_tool_call(&bash("rm -rf /tmp/gex"), &root());
        assert!(!check.allowed);
        assert!(check.reason.contains("outside the latest user request"));
    }

    #[test]
    fn git_reset_requires_explicit_reset_scope() {
        let contract = DestructiveScopeContract::from_user_request("删除 abc.txt", &root());
        let check = contract.check_tool_call(&bash("git reset --hard HEAD"), &root());
        assert!(!check.allowed);
        assert!(check.reason.contains("repository-wide"));
    }

    #[test]
    fn completion_guard_blocks_broader_follow_up_suggestion() {
        let contract = DestructiveScopeContract::from_user_request("删除 abc.txt", &root());
        let guard = contract
            .completion_guard_for_results([(&bash("rm /tmp/gex/abc.txt"), true)], &root())
            .expect("successful delete should produce a guard");
        assert!(guard.contains("Do not ask or suggest deleting parent directories"));
    }
}
