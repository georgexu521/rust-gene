//! File operation tools.
//!
//! File tools own read-before-write evidence, bounded previews, patch/edit
//! application, and mutation metadata. Any write-like operation must preserve
//! checkpoint and permission expectations before closeout can be verified.

use crate::engine::context_ledger::{record_file_read, FileReadLedgerInput};
use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult, ToolSearchOrReadSemantics};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use serde_json::json;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, warn};

mod diagnostics;
mod edit_match;
mod edit_tool;
pub(crate) mod history;
pub mod mutation_result;
mod patch;
mod path_policy;
mod read;
mod state;
mod text_codec;
mod write;

use diagnostics::{
    collect_file_edit_diagnostics, file_edit_diagnostics_content_line, file_edit_diagnostics_delta,
};
use edit_match::*;
pub use edit_tool::FileEditTool;
pub use edit_tool::InsertMode;
use history::{
    checkpoint_metadata_json, create_file_checkpoint, record_file_change, FileChangeRequest,
};
pub use patch::FilePatchTool;
pub(crate) use path_policy::is_unc_or_network_path;
pub use path_policy::{
    canonicalize_or_normalize, is_allowed_absolute_path, is_allowed_read_absolute_path,
    normalize_path, resolve_path, resolve_read_path,
};
pub use read::FileReadTool;
use state::*;
pub use state::{
    check_read_before_write, clear_read_files, is_file_modified_since_read, is_file_read,
    mark_file_read, mark_file_read_with_state,
};
pub(crate) use state::{edit_diff_summary, edit_diff_summary_json};
use text_codec::{
    detect_line_ending, normalize_text_line_endings, read_text_file, split_leading_text_bom,
    text_format_json, text_write_format_json, write_text_file, TextFileEncoding,
};
pub use write::FileWriteTool;

#[cfg(test)]
use text_codec::{decode_text_file, encode_text_content, LineEndingStyle};

const MAX_EDITABLE_FILE_SIZE_BYTES: u64 = 64 * 1024 * 1024; // 64 MiB
const DEFAULT_MAX_FILE_EDIT_REPLACEMENTS: usize = 20;
const DEFAULT_DIRECTORY_READ_ENTRY_LIMIT: usize = 200;
const FILE_READ_PREVIEW_MAX_LINES: usize = 5;
const FILE_READ_PREVIEW_MAX_CHARS: usize = 280;

fn check_file_size_limit(path: &Path, operation: &str) -> Result<(), String> {
    let metadata = std::fs::metadata(path).map_err(|e| {
        format!(
            "Failed to read file metadata for {} '{}': {}",
            operation,
            path.display(),
            e
        )
    })?;
    if metadata.len() > MAX_EDITABLE_FILE_SIZE_BYTES {
        return Err(format!(
            "Refusing to {} file '{}': {} bytes exceeds limit {} bytes",
            operation,
            path.display(),
            metadata.len(),
            MAX_EDITABLE_FILE_SIZE_BYTES
        ));
    }
    Ok(())
}

fn max_file_edit_replacements() -> usize {
    std::env::var("PRIORITY_AGENT_MAX_FILE_EDIT_REPLACEMENTS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MAX_FILE_EDIT_REPLACEMENTS)
}

fn allow_bulk_code_edit() -> bool {
    std::env::var("PRIORITY_AGENT_ALLOW_BULK_CODE_EDIT").as_deref() == Ok("1")
}

fn allow_high_risk_file_mutation() -> bool {
    std::env::var("PRIORITY_AGENT_ALLOW_HIGH_RISK_FILE_MUTATION").as_deref() == Ok("1")
}

fn allow_edit_without_read() -> bool {
    std::env::var("PRIORITY_AGENT_ALLOW_EDIT_WITHOUT_READ").as_deref() == Ok("1")
}

fn is_code_like_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext,
                "rs" | "ts"
                    | "tsx"
                    | "js"
                    | "jsx"
                    | "py"
                    | "go"
                    | "java"
                    | "kt"
                    | "swift"
                    | "c"
                    | "cc"
                    | "cpp"
                    | "h"
                    | "hpp"
                    | "cs"
                    | "rb"
                    | "php"
                    | "scala"
                    | "sh"
                    | "zsh"
                    | "fish"
            )
        })
        .unwrap_or(false)
}

fn path_component_names(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(part) => Some(part.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect()
}

fn high_risk_file_target_diagnostic(
    path: &Path,
    identity: &FilePathIdentity,
    working_dir: &Path,
    operation: &str,
) -> Option<(String, serde_json::Value)> {
    if allow_high_risk_file_mutation() {
        return None;
    }

    let components = path_component_names(path);
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let classification = if extension == "ipynb" {
        Some((
            "wrong_tool_notebook",
            "notebook files should be changed through notebook-aware tooling, not raw file mutation",
            "use_notebook_tool",
        ))
    } else if file_name == ".env"
        || file_name.starts_with(".env.")
        || file_name.ends_with(".env")
        || matches!(
            file_name.as_str(),
            "id_rsa" | "id_dsa" | "id_ecdsa" | "id_ed25519" | "authorized_keys" | "known_hosts"
        )
        || matches!(
            extension.as_str(),
            "pem" | "key" | "p12" | "pfx" | "crt" | "cer"
        )
    {
        Some((
            "secret_or_credential_target",
            "target looks like an environment, credential, certificate, or SSH key file",
            "ask_user_for_explicit_secret_file_plan",
        ))
    } else if components.iter().any(|component| component == ".git") {
        Some((
            "vcs_metadata_target",
            "target is inside .git metadata",
            "use_git_tool_or_choose_project_file",
        ))
    } else if is_generated_dir_worktree_path(path, working_dir) {
        None
    } else if components.iter().any(|component| {
        matches!(
            component.as_str(),
            "target"
                | "node_modules"
                | "dist"
                | "build"
                | ".next"
                | ".nuxt"
                | ".cache"
                | "coverage"
        )
    }) {
        Some((
            "generated_or_dependency_target",
            "target is inside a generated, build, cache, coverage, or dependency directory",
            "edit_source_file_instead",
        ))
    } else {
        let canonical_path = canonicalize_or_normalize(path);
        let canonical_working_dir = canonicalize_or_normalize(working_dir);
        let home_config = std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| {
                canonical_path.starts_with(canonicalize_or_normalize(&home.join(".config")))
            })
            .unwrap_or(false);
        if home_config && !canonical_path.starts_with(&canonical_working_dir) {
            Some((
                "home_config_outside_project",
                "target is a home configuration file outside the selected project",
                "open_config_setup_or_switch_project",
            ))
        } else {
            None
        }
    }?;

    let (failure, reason, recommended_action) = classification;
    let message = format!(
        "Refusing {operation} for '{}': {reason}. Set PRIORITY_AGENT_ALLOW_HIGH_RISK_FILE_MUTATION=1 only after an explicit user-approved plan.",
        identity.lexical_path
    );
    Some((
        message,
        json!({
            "failure": failure,
            "operation": operation,
            "path_identity": path_identity_json(identity),
            "guardrail": {
                "reason": reason,
                "override_env": "PRIORITY_AGENT_ALLOW_HIGH_RISK_FILE_MUTATION",
                "override_required_value": "1",
            },
            "recovery": {
                "recommended_action": recommended_action,
                "next_actions": ["choose_safer_target", "ask_user_for_explicit_approval", "retry_with_source_file"],
            }
        }),
    ))
}

fn is_generated_dir_worktree_path(path: &Path, working_dir: &Path) -> bool {
    let normalized_path = normalize_path(path);
    let normalized_working_dir = normalize_path(working_dir);
    let canonical_path = canonicalize_or_normalize(path);
    let canonical_working_dir = canonicalize_or_normalize(working_dir);

    let inside_working_dir = normalized_path.starts_with(&normalized_working_dir)
        || normalized_path.starts_with(&canonical_working_dir)
        || canonical_path.starts_with(&normalized_working_dir)
        || canonical_path.starts_with(&canonical_working_dir);
    if !inside_working_dir {
        return false;
    }

    let is_worktree = [&normalized_working_dir, &canonical_working_dir]
        .into_iter()
        .map(|path| path.to_string_lossy().to_ascii_lowercase())
        .any(|lower| {
            (lower.contains("/target/live-evals/") && lower.ends_with("/worktree"))
                || lower.contains("/.claude/worktrees/")
        });
    is_worktree
}

fn high_risk_file_target_result(
    path: &Path,
    identity: &FilePathIdentity,
    working_dir: &Path,
    operation: &str,
) -> Option<ToolResult> {
    high_risk_file_target_diagnostic(path, identity, working_dir, operation)
        .map(|(message, data)| file_edit_error_with_data(message, data))
}

/// 智能引号归一化（Claude Code 模式）
/// 处理文件中的智能引号 vs 模型输出的直引号差异
#[derive(Clone, Debug)]
struct FilePathIdentity {
    lexical_path: String,
    resolved_path: String,
    canonical_path: String,
    display_path: String,
    state_key: String,
}

#[derive(Clone, Debug)]
pub(crate) struct EditDiffSummary {
    pub(crate) additions: usize,
    pub(crate) deletions: usize,
    pub(crate) changed_line_start: usize,
    pub(crate) changed_line_end: usize,
    pub(crate) unified_diff: String,
    pub(crate) preview_truncated: bool,
}

fn file_edit_error_with_data(error: impl Into<String>, data: serde_json::Value) -> ToolResult {
    let error = error.into();
    let mut result = ToolResult::error_with_content(error, data.to_string());
    result.data = Some(data);
    result
}

fn priority_agent_settings_validation_error(
    identity: &FilePathIdentity,
    content: &str,
    stage: &str,
) -> Option<ToolResult> {
    let path = std::path::Path::new(&identity.resolved_path);
    if !is_priority_agent_settings_path(path) {
        return None;
    }

    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let validation = if filename == "permissions.toml" {
        validate_permissions_toml(content)
    } else if filename == "config.toml"
        || path.extension().and_then(|ext| ext.to_str()) == Some("toml")
    {
        toml::from_str::<toml::Value>(content)
            .map(|_| ())
            .map_err(|err| format!("invalid TOML: {err}"))
    } else if filename.ends_with(".json") {
        serde_json::from_str::<serde_json::Value>(content)
            .map(|_| ())
            .map_err(|err| format!("invalid JSON: {err}"))
    } else {
        Ok(())
    };

    validation.err().map(|error| {
        file_edit_error_with_data(
            format!(
                "Refusing to write Priority Agent settings file '{}': {}",
                identity.display_path, error
            ),
            json!({
                "failure": "settings_schema_validation",
                "stage": stage,
                "path_identity": path_identity_json(identity),
                "schema_error": error,
            }),
        )
    })
}

fn checkpoint_creation_failed_result(
    tool_name: &str,
    path_str: &str,
    identity: &FilePathIdentity,
) -> ToolResult {
    file_edit_error_with_data(
        format!(
            "Refusing {tool_name} for '{}': checkpoint creation failed before write, so rollback would be unavailable.",
            path_str
        ),
        json!({
            "failure": "checkpoint_creation_failed",
            "stage": "checkpoint_guard",
            "tool": tool_name,
            "path_identity": path_identity_json(identity),
        }),
    )
}

fn is_priority_agent_settings_path(path: &std::path::Path) -> bool {
    let components = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    components.windows(2).any(|window| {
        matches!(
            window,
            [".priority-agent", _] | ["priority-agent", "config.toml"]
        )
    })
}

fn validate_permissions_toml(content: &str) -> Result<(), String> {
    let value =
        toml::from_str::<toml::Value>(content).map_err(|err| format!("invalid TOML: {err}"))?;
    let table = value
        .as_table()
        .ok_or_else(|| "permissions.toml must be a TOML table".to_string())?;

    for key in table.keys() {
        if !matches!(key.as_str(), "always_allow" | "always_deny" | "always_ask") {
            return Err(format!(
                "unsupported permissions key '{}'; expected always_allow, always_deny, or always_ask",
                key
            ));
        }
    }

    for key in ["always_allow", "always_deny", "always_ask"] {
        let Some(value) = table.get(key) else {
            continue;
        };
        let Some(rules) = value.as_array() else {
            return Err(format!("{key} must be an array of rule tables"));
        };
        for (index, rule) in rules.iter().enumerate() {
            let Some(rule_table) = rule.as_table() else {
                return Err(format!("{key}[{index}] must be a table with a pattern"));
            };
            let pattern = rule_table
                .get("pattern")
                .and_then(toml::Value::as_str)
                .unwrap_or_default()
                .trim();
            if pattern.is_empty() {
                return Err(format!("{key}[{index}].pattern must be a non-empty string"));
            }
            if let Some(source) = rule_table.get("source") {
                let source = source
                    .as_str()
                    .ok_or_else(|| format!("{key}[{index}].source must be a string"))?;
                if !matches!(
                    source,
                    "global"
                        | "project"
                        | "user"
                        | "system"
                        | "Global"
                        | "Project"
                        | "User"
                        | "System"
                ) {
                    return Err(format!(
                        "{key}[{index}].source has unsupported value '{source}'"
                    ));
                }
            }
        }
    }

    Ok(())
}

fn stale_conflict_json(
    identity: &FilePathIdentity,
    session_id: &str,
    current_content: &str,
    current_mtime: std::time::SystemTime,
    stage: &str,
) -> serde_json::Value {
    let read_state = file_state_snapshot(session_id, &identity.state_key);
    let read_hash = read_state
        .as_ref()
        .map(|state| format!("{:016x}", state.content_hash));
    let mtime_changed = read_state
        .as_ref()
        .map(|state| state.mtime != current_mtime)
        .unwrap_or(false);
    let content_changed = read_state
        .as_ref()
        .map(|state| compute_content_hash(current_content) != state.content_hash)
        .unwrap_or(false);
    json!({
        "failure": "stale_read_conflict",
        "stage": stage,
        "path_identity": path_identity_json(identity),
        "conflict": {
            "read_hash": read_hash,
            "current_hash": content_hash_hex(current_content),
            "mtime_changed": mtime_changed,
            "content_changed": content_changed,
        },
        "recovery": {
            "recommended_action": "re_read_file",
            "next_actions": ["file_read", "regenerate_patch", "retry_file_edit"],
            "allow_stale_read_available": true,
            "allow_stale_read_warning": "Use allow_stale_read=true only for intentional overwrites after reviewing the current file content."
        }
    })
}

#[cfg(test)]
mod tests;
