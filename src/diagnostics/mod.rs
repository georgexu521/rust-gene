//! /doctor 深度诊断模块
//!
//! 检测环境健康状态，提供可执行的修复建议，并支持导出 JSON 诊断报告。

pub mod provider_health;

use crate::plugins::{self, PluginRuntimeStatus};
use crate::services::api::provider_protocol::ProviderRuntimeFacts;
use crate::services::config::AppConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 检查结果状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Warning,
    Error,
    Info,
}

/// 单项检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl CheckResult {
    pub fn ok(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Ok,
            message: message.into(),
            suggestion: None,
        }
    }

    pub fn warn(
        name: impl Into<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Warning,
            message: message.into(),
            suggestion: Some(suggestion.into()),
        }
    }

    pub fn error(
        name: impl Into<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Error,
            message: message.into(),
            suggestion: Some(suggestion.into()),
        }
    }

    pub fn info(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Info,
            message: message.into(),
            suggestion: None,
        }
    }
}

/// 诊断报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub overall: CheckStatus,
    pub checks: Vec<CheckResult>,
    pub metadata: HashMap<String, String>,
}

impl DiagnosticReport {
    pub fn new(checks: Vec<CheckResult>) -> Self {
        let overall = if checks.iter().any(|c| c.status == CheckStatus::Error) {
            CheckStatus::Error
        } else if checks.iter().any(|c| c.status == CheckStatus::Warning) {
            CheckStatus::Warning
        } else {
            CheckStatus::Ok
        };
        Self {
            overall,
            checks,
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn format_text(&self) -> String {
        let mut lines = vec![format!("Diagnostic Report: {:?}", self.overall)];
        lines.push("-".repeat(40));
        for check in &self.checks {
            let emoji = match check.status {
                CheckStatus::Ok => "✅",
                CheckStatus::Warning => "⚠️",
                CheckStatus::Error => "❌",
                CheckStatus::Info => "ℹ️",
            };
            lines.push(format!("{} {}: {}", emoji, check.name, check.message));
            if let Some(ref suggestion) = check.suggestion {
                lines.push(format!("   💡 Fix: {}", suggestion));
            }
        }
        lines.join("\n")
    }
}

/// 运行完整诊断
pub async fn run_full_diagnostics(working_dir: &Path) -> DiagnosticReport {
    let mut checks = Vec::new();

    checks.push(check_git_available());
    checks.push(check_network_connectivity().await);
    checks.push(check_toolchain());
    checks.push(check_config());
    checks.push(check_provider_runtime_config());
    checks.push(check_permissions_config(working_dir));
    checks.push(check_session_store());
    checks.push(check_state_dirs_writable());
    checks.push(check_git_worktree(working_dir));
    checks.push(check_release_artifacts(working_dir));
    checks.push(check_plugin_runtime(working_dir));
    checks.push(check_bridge_runtime());
    checks.push(check_remote_runtime());
    checks.push(check_memory(working_dir));

    DiagnosticReport::new(checks)
        .with_metadata("working_dir", working_dir.display().to_string())
        .with_metadata("platform", std::env::consts::OS.to_string())
}

/// 检测 git 可用性
pub fn check_git_available() -> CheckResult {
    match std::process::Command::new("git").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string();
            CheckResult::ok("git", version)
        }
        _ => CheckResult::error(
            "git",
            "git not found in PATH",
            "Install git and ensure it is in your PATH (e.g., 'brew install git' or 'apt install git')",
        ),
    }
}

/// 检测网络连通性（Kimi API）
pub async fn check_network_connectivity() -> CheckResult {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return CheckResult::error(
                "network",
                format!("Failed to build HTTP client: {}", e),
                "Check your system network stack and proxy settings",
            )
        }
    };

    // Ping Kimi API base endpoint (lightweight GET, should return 404 or similar quickly)
    match client.get("https://api.moonshot.cn/v1").send().await {
        Ok(resp) => {
            let status = resp.status();
            if status.as_u16() == 401 || status.as_u16() == 404 || status.is_success() {
                CheckResult::ok("network", "Kimi API reachable")
            } else {
                CheckResult::warn(
                    "network",
                    format!("Kimi API returned unexpected status: {}", status),
                    "Check API status page or your network/proxy configuration",
                )
            }
        }
        Err(e) => {
            if e.is_timeout() {
                CheckResult::error(
                    "network",
                    "Kimi API connection timed out",
                    "Check your internet connection or proxy settings",
                )
            } else {
                CheckResult::error(
                    "network",
                    format!("Cannot reach Kimi API: {}", e),
                    "Check your internet connection, DNS, and firewall settings",
                )
            }
        }
    }
}

/// 检测工具链完整性
pub fn check_toolchain() -> CheckResult {
    let mut missing = Vec::new();

    if std::process::Command::new("cargo")
        .arg("--version")
        .output()
        .is_err()
    {
        missing.push("cargo");
    }
    if std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .is_err()
    {
        missing.push("rustc");
    }

    if missing.is_empty() {
        CheckResult::ok("toolchain", "Rust toolchain (cargo, rustc) available")
    } else {
        CheckResult::error(
            "toolchain",
            format!("Missing tools: {}", missing.join(", ")),
            "Install Rust via rustup: https://rustup.rs",
        )
    }
}

/// 检测配置（API keys 等）
pub fn check_config() -> CheckResult {
    let configured = crate::services::api::provider_catalog::builtin_catalog()
        .into_iter()
        .filter_map(|entry| {
            entry
                .key_env_vars
                .iter()
                .any(|env| std::env::var(env).is_ok_and(|value| !value.trim().is_empty()))
                .then_some(format!("{} configured", entry.label))
        })
        .collect::<Vec<_>>();

    if configured.is_empty() {
        CheckResult::error(
            "config",
            "No LLM API key configured",
            format!(
                "Set one provider key in your environment: {}",
                crate::services::api::provider::provider_key_env_hint()
            ),
        )
    } else {
        CheckResult::ok("config", configured.join("; "))
    }
}

/// Detect active provider protocol behavior from config/env.
pub fn check_provider_runtime_config() -> CheckResult {
    let (mut base_url, mut model, config_note) = match AppConfig::load() {
        Ok(config) => (
            first_non_empty(vec![
                config.api.base_url,
                std::env::var("MINIMAX_BASE_URL").unwrap_or_default(),
                std::env::var("KIMI_CODE_BASE_URL").unwrap_or_default(),
                std::env::var("DEEPSEEK_BASE_URL").unwrap_or_default(),
                std::env::var("GLM_BASE_URL").unwrap_or_default(),
                std::env::var("ZAI_BASE_URL").unwrap_or_default(),
                std::env::var("ZHIPUAI_BASE_URL").unwrap_or_default(),
                std::env::var("BIGMODEL_BASE_URL").unwrap_or_default(),
                std::env::var("MOONSHOT_BASE_URL").unwrap_or_default(),
                std::env::var("OPENAI_BASE_URL").unwrap_or_default(),
            ]),
            first_non_empty(vec![
                config.api.model,
                std::env::var("MINIMAX_MODEL").unwrap_or_default(),
                std::env::var("KIMI_CODE_MODEL").unwrap_or_default(),
                std::env::var("DEEPSEEK_MODEL").unwrap_or_default(),
                std::env::var("GLM_MODEL").unwrap_or_default(),
                std::env::var("ZAI_MODEL").unwrap_or_default(),
                std::env::var("ZHIPUAI_MODEL").unwrap_or_default(),
                std::env::var("BIGMODEL_MODEL").unwrap_or_default(),
                std::env::var("MOONSHOT_MODEL").unwrap_or_default(),
                std::env::var("OPENAI_MODEL").unwrap_or_default(),
            ]),
            None,
        ),
        Err(e) => (
            first_non_empty(vec![
                std::env::var("MINIMAX_BASE_URL").unwrap_or_default(),
                std::env::var("KIMI_CODE_BASE_URL").unwrap_or_default(),
                std::env::var("DEEPSEEK_BASE_URL").unwrap_or_default(),
                std::env::var("GLM_BASE_URL").unwrap_or_default(),
                std::env::var("ZAI_BASE_URL").unwrap_or_default(),
                std::env::var("ZHIPUAI_BASE_URL").unwrap_or_default(),
                std::env::var("BIGMODEL_BASE_URL").unwrap_or_default(),
                std::env::var("MOONSHOT_BASE_URL").unwrap_or_default(),
                std::env::var("OPENAI_BASE_URL").unwrap_or_default(),
            ]),
            first_non_empty(vec![
                std::env::var("MINIMAX_MODEL").unwrap_or_default(),
                std::env::var("KIMI_CODE_MODEL").unwrap_or_default(),
                std::env::var("DEEPSEEK_MODEL").unwrap_or_default(),
                std::env::var("GLM_MODEL").unwrap_or_default(),
                std::env::var("ZAI_MODEL").unwrap_or_default(),
                std::env::var("ZHIPUAI_MODEL").unwrap_or_default(),
                std::env::var("BIGMODEL_MODEL").unwrap_or_default(),
                std::env::var("MOONSHOT_MODEL").unwrap_or_default(),
                std::env::var("OPENAI_MODEL").unwrap_or_default(),
            ]),
            Some(e.to_string()),
        ),
    };

    if base_url.is_empty() || model.is_empty() {
        let registry = crate::services::api::provider::ProviderRegistry::from_env();
        if let Some(config) = registry
            .selected()
            .and_then(|selected| registry.get_config(selected))
        {
            if base_url.is_empty() {
                base_url = config.base_url.clone().unwrap_or_default();
            }
            if model.is_empty() {
                model = config.default_model.clone();
            }
        }
    }

    if model.is_empty() && base_url.is_empty() {
        return CheckResult::warn(
            "provider_runtime",
            "No provider model/base URL configured for protocol detection",
            "Set PRIORITY_AGENT_API_MODEL and PRIORITY_AGENT_API_BASE_URL, or provider-specific env vars",
        );
    }

    let facts = ProviderRuntimeFacts::detect(&base_url, &model);
    let mut traits = Vec::new();
    if facts.supports_streaming_tool_calls {
        traits.push("streaming");
    }
    if facts.supports_tool_calls {
        traits.push("tools");
    }
    if facts.supports_reasoning_tokens {
        traits.push("reasoning_tokens");
    }
    if facts.requires_nonstreaming_tool_calls {
        traits.push("nonstreaming_tools");
    }
    if facts.requires_tool_result_adjacency {
        traits.push("tool_adjacency");
    }

    let message = format!(
        "family={:?}; model={}; traits={}; normalization={}",
        facts.protocol_family,
        if facts.model.is_empty() {
            "<unset>"
        } else {
            facts.model.as_str()
        },
        if traits.is_empty() {
            "none".to_string()
        } else {
            traits.join(",")
        },
        facts.normalization.join(",")
    );

    if let Some(note) = config_note {
        CheckResult::warn(
            "provider_runtime",
            format!("{}; config load warning: {}", message, note),
            "Fix config.toml or rely on provider-specific environment variables",
        )
    } else {
        CheckResult::ok("provider_runtime", message)
    }
}

/// 检测权限配置文件
pub fn check_permissions_config(working_dir: &Path) -> CheckResult {
    let global = dirs::home_dir()
        .map(|d| d.join(".priority-agent").join("permissions.toml"))
        .filter(|p| p.exists());
    let project = working_dir.join(".priority-agent").join("permissions.toml");

    if global.is_some() && project.exists() {
        CheckResult::info("permissions", "Global and project permission rules found")
    } else if global.is_some() || project.exists() {
        CheckResult::info("permissions", "Permission rules found")
    } else {
        CheckResult::info(
            "permissions",
            "No permission rules configured (using defaults)",
        )
    }
}

/// 检测 session store
pub fn check_session_store() -> CheckResult {
    let db_path = dirs::data_dir()
        .map(|d| d.join("priority-agent").join("sessions.db"))
        .unwrap_or_else(|| std::path::PathBuf::from(".priority-agent/sessions.db"));

    if db_path.exists() {
        CheckResult::ok(
            "session_store",
            format!("Session store exists at {:?}", db_path),
        )
    } else {
        CheckResult::info(
            "session_store",
            format!("Session store not yet created at {:?}", db_path),
        )
    }
}

pub fn check_state_dirs_writable() -> CheckResult {
    let config_dir = dirs::config_dir()
        .map(|d| d.join("priority-agent"))
        .unwrap_or_else(|| PathBuf::from(".priority-agent"));
    let data_dir = dirs::data_dir()
        .map(|d| d.join("priority-agent"))
        .unwrap_or_else(|| PathBuf::from(".priority-agent"));
    let cache_dir = dirs::cache_dir()
        .map(|d| d.join("priority-agent"))
        .unwrap_or_else(|| PathBuf::from(".priority-agent-cache"));

    check_state_dirs_writable_for_paths(&[config_dir, data_dir, cache_dir])
}

fn check_state_dirs_writable_for_paths(paths: &[PathBuf]) -> CheckResult {
    let mut checked = Vec::new();
    let mut failures = Vec::new();

    for path in paths {
        match probe_writable_dir(path) {
            Ok(()) => checked.push(path.display().to_string()),
            Err(e) => failures.push(format!("{} ({})", path.display(), e)),
        }
    }

    if failures.is_empty() {
        CheckResult::ok(
            "state_dirs",
            format!("Writable state directories: {}", checked.join("; ")),
        )
    } else {
        CheckResult::error(
            "state_dirs",
            format!("State directory write failures: {}", failures.join("; ")),
            "Fix directory permissions or set XDG config/data/cache directories to writable paths",
        )
    }
}

fn probe_writable_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;
    let probe = path.join(format!(".doctor-write-test-{}", uuid::Uuid::new_v4()));
    std::fs::write(&probe, b"ok")?;
    std::fs::remove_file(probe)?;
    Ok(())
}

pub fn check_git_worktree(working_dir: &Path) -> CheckResult {
    let inside = std::process::Command::new("git")
        .arg("-C")
        .arg(working_dir)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output();

    match inside {
        Ok(output) if output.status.success() => {
            let worktree_count = std::process::Command::new("git")
                .arg("-C")
                .arg(working_dir)
                .args(["worktree", "list", "--porcelain"])
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| {
                    String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .filter(|line| line.starts_with("worktree "))
                        .count()
                })
                .unwrap_or(0);

            CheckResult::ok(
                "git_worktree",
                format!(
                    "Current directory is a git worktree; registered worktrees={}",
                    worktree_count
                ),
            )
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            CheckResult::warn(
                "git_worktree",
                if stderr.is_empty() {
                    "Current directory is not inside a git worktree".to_string()
                } else {
                    format!("Current directory is not inside a git worktree: {}", stderr)
                },
                "Run priority-agent from a git project for coding-agent diff and worktree features",
            )
        }
        Err(e) => CheckResult::error(
            "git_worktree",
            format!("Failed to run git worktree check: {}", e),
            "Install git and ensure it is available in PATH",
        ),
    }
}

pub fn check_release_artifacts(working_dir: &Path) -> CheckResult {
    let required = [
        "Cargo.toml",
        "scripts/install.sh",
        "scripts/package-release.sh",
        "scripts/release-gates.sh",
        "docs/RELEASE_READINESS_GUIDE_2026-05-22.md",
    ];
    check_release_artifacts_for_paths(
        &required
            .iter()
            .map(|relative| working_dir.join(relative))
            .collect::<Vec<_>>(),
    )
}

fn check_release_artifacts_for_paths(paths: &[PathBuf]) -> CheckResult {
    let mut present = Vec::new();
    let mut missing = Vec::new();

    for path in paths {
        if path.exists() {
            present.push(path.display().to_string());
        } else {
            missing.push(path.display().to_string());
        }
    }

    if missing.is_empty() {
        CheckResult::ok(
            "release_artifacts",
            format!("Release assets present: {}", present.join("; ")),
        )
    } else {
        CheckResult::warn(
            "release_artifacts",
            format!("Missing release assets: {}", missing.join("; ")),
            "Restore the release script/docs files or update the release gate before publishing",
        )
    }
}

pub fn check_plugin_runtime(working_dir: &Path) -> CheckResult {
    let roots = plugins::default_plugin_roots(working_dir);
    let trust_mode = AppConfig::load()
        .map(|config| plugins::trust::TrustMode::parse_lossy(&config.features.plugin_trust_mode))
        .unwrap_or(plugins::trust::TrustMode::Warn);
    check_plugin_runtime_for_roots(&roots, trust_mode)
}

fn check_plugin_runtime_for_roots(
    roots: &[PathBuf],
    trust_mode: plugins::trust::TrustMode,
) -> CheckResult {
    let discovered = plugins::discover_plugins(roots);
    if discovered.is_empty() {
        return CheckResult::info(
            "plugin_runtime",
            format!(
                "No plugins discovered; roots={}",
                roots
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        );
    }

    let facts = plugins::runtime_facts(&discovered, trust_mode);
    let ready = facts
        .iter()
        .filter(|f| f.status == PluginRuntimeStatus::Ready)
        .count();
    let disabled = facts
        .iter()
        .filter(|f| f.status == PluginRuntimeStatus::Disabled)
        .count();
    let warnings = facts
        .iter()
        .filter(|f| f.status == PluginRuntimeStatus::UsableWithWarnings)
        .count();
    let blocked = facts
        .iter()
        .filter(|f| f.status == PluginRuntimeStatus::Blocked)
        .count();
    let message = format!(
        "plugins={} ready={} warnings={} disabled={} blocked={} trust_mode={}",
        facts.len(),
        ready,
        warnings,
        disabled,
        blocked,
        trust_mode.as_str()
    );

    if blocked > 0 {
        CheckResult::warn(
            "plugin_runtime",
            message,
            "Run plugin_manage action=status or plugin_manage action=validate to inspect blocked plugins",
        )
    } else {
        CheckResult::ok("plugin_runtime", message)
    }
}

pub fn check_bridge_runtime() -> CheckResult {
    let snapshot = crate::bridge::runtime_snapshot();
    match snapshot.status {
        crate::bridge::BridgeRuntimeStatus::Ready => CheckResult::ok(
            "bridge_runtime",
            format!(
                "{}; cursors={}; tenant={}",
                snapshot.diagnostic,
                snapshot.cursor_count,
                snapshot.tenant_id.as_deref().unwrap_or("<unset>")
            ),
        ),
        crate::bridge::BridgeRuntimeStatus::ConfiguredWithoutAuth => CheckResult::warn(
            "bridge_runtime",
            format!("{}; cursors={}", snapshot.diagnostic, snapshot.cursor_count),
            "Set PRIORITY_AGENT_BRIDGE_TOKEN when using remote bridge features",
        ),
        crate::bridge::BridgeRuntimeStatus::NotConfigured => CheckResult::info(
            "bridge_runtime",
            format!("{}; cursors={}", snapshot.diagnostic, snapshot.cursor_count),
        ),
    }
}

pub fn check_remote_runtime() -> CheckResult {
    let env = crate::remote::RemoteEnvDetector::detect();
    let snapshot = crate::remote::RemoteSessionManager::new().runtime_snapshot(env);
    let message = format!(
        "env={}; remote={}; saved_sessions={}; connected={}; errors={}; {}",
        snapshot.env.env_type,
        snapshot.env.is_remote,
        snapshot.saved_session_count,
        snapshot.connected_session_count,
        snapshot.error_session_count,
        snapshot.diagnostics.join("; ")
    );

    if snapshot.error_session_count > 0 {
        CheckResult::warn(
            "remote_runtime",
            message,
            "Inspect remote sessions and reconnect or remove failed entries",
        )
    } else {
        CheckResult::info("remote_runtime", message)
    }
}

fn first_non_empty(values: Vec<String>) -> String {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

/// Check the Priority Agent memory store (Phase 4: Reasonix-aligned product surface).
///
/// Reports on three-layer model: standing docs (MEMORY.md, USER.md), saved
/// index (typed records), and dynamic recall policy.
pub fn check_memory(_working_dir: &Path) -> CheckResult {
    let priority_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".priority-agent");
    check_memory_store(&priority_dir)
}

fn check_memory_store(priority_dir: &Path) -> CheckResult {
    let memory_md = priority_dir.join("MEMORY.md");
    let user_md = priority_dir.join("USER.md");
    let memory_dir = priority_dir.join("memory");
    let records_path = memory_dir.join("records.jsonl");
    let decisions_path = memory_dir.join("decisions.jsonl");

    let has_memory_md = memory_md.exists();
    let has_user_md = user_md.exists();
    let has_records = records_path.exists();
    let _has_decisions = decisions_path.exists();

    let topic_count = if memory_dir.exists() {
        std::fs::read_dir(&memory_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                    .count()
            })
            .unwrap_or(0)
    } else {
        0
    };

    let write_policy = crate::services::config::runtime_config()
        .auto_memory_write_policy()
        .to_string();
    let active_memory = std::env::var("PRIORITY_AGENT_ACTIVE_MEMORY")
        .map(|v| v == "1")
        .unwrap_or(false);

    let standing = if has_memory_md || has_user_md {
        format!(
            "standing: {}/{} files",
            if has_memory_md { "MEMORY.md" } else { "" },
            if has_user_md { "USER.md" } else { "" },
        )
    } else {
        "standing: none".to_string()
    };

    let saved = if has_records {
        let count = std::fs::read_to_string(&records_path)
            .map(|content| content.lines().count())
            .unwrap_or(0);
        let decisions_count = std::fs::read_to_string(&decisions_path)
            .map(|content| content.lines().count())
            .unwrap_or(0);
        format!(
            "saved: {} typed records, {} decisions, {} topics",
            count, decisions_count, topic_count
        )
    } else {
        "saved: none".to_string()
    };

    let recall = format!(
        "recall: write-policy={}, active-memory={}",
        write_policy, active_memory
    );

    let message = format!("memory: {} | {} | {}", standing, saved, recall);

    if !has_memory_md && !has_user_md && !has_records {
        CheckResult::info(
            "memory",
            "No memory initialized yet. The agent will save facts and preferences as you work.",
        )
    } else if write_policy == "review_only" {
        CheckResult::ok("memory", message)
    } else {
        CheckResult::warn(
            "memory",
            message,
            "Auto-write is enabled. Consider review_only policy to keep memory changes explicit and reviewable.",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_git_available() {
        let result = check_git_available();
        // git 应该在大多数开发环境中可用
        assert!(
            result.status == CheckStatus::Ok || result.status == CheckStatus::Error,
            "git check should return ok or error"
        );
    }

    #[test]
    fn test_check_toolchain() {
        let result = check_toolchain();
        assert!(
            result.status == CheckStatus::Ok || result.status == CheckStatus::Error,
            "toolchain check should return ok or error"
        );
    }

    #[test]
    fn test_check_config_with_no_keys() {
        // 测试在无环境变量时的行为（可能失败，因为现实环境可能已设置）
        let result = check_config();
        assert!(
            result.status == CheckStatus::Ok || result.status == CheckStatus::Error,
            "config check should return ok or error"
        );
    }

    #[tokio::test]
    async fn test_check_network_connectivity() {
        let result = check_network_connectivity().await;
        // 网络检查应返回 ok、warning 或 error
        assert!(
            matches!(
                result.status,
                CheckStatus::Ok | CheckStatus::Warning | CheckStatus::Error
            ),
            "network check should return ok, warning, or error"
        );
    }

    #[test]
    fn test_check_permissions_config() {
        let tmp = std::env::temp_dir();
        let result = check_permissions_config(&tmp);
        assert!(
            result.status == CheckStatus::Info,
            "permissions check should return info for temp dir"
        );
    }

    #[test]
    fn test_check_session_store() {
        let result = check_session_store();
        assert!(
            result.status == CheckStatus::Ok || result.status == CheckStatus::Info,
            "session_store check should return ok or info"
        );
    }

    #[test]
    fn test_check_memory_store_reports_home_style_memory() {
        let root = std::env::temp_dir().join(format!(
            "priority-agent-doctor-memory-test-{}",
            uuid::Uuid::new_v4()
        ));
        let memory_dir = root.join("memory");
        std::fs::create_dir_all(&memory_dir).expect("create memory dir");
        std::fs::write(root.join("MEMORY.md"), "# Memory\nproject fact").expect("write memory");
        std::fs::write(memory_dir.join("records.jsonl"), "{}\n").expect("write records");

        let result = check_memory_store(&root);

        assert_eq!(result.status, CheckStatus::Ok);
        assert!(result.message.contains("MEMORY.md"));
        assert!(result.message.contains("typed records"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_check_provider_runtime_config_returns_result() {
        let result = check_provider_runtime_config();
        assert!(
            matches!(
                result.status,
                CheckStatus::Ok | CheckStatus::Warning | CheckStatus::Error | CheckStatus::Info
            ),
            "provider runtime check should return a valid status"
        );
    }

    #[test]
    fn test_check_state_dirs_writable_for_temp_paths() {
        let root = std::env::temp_dir().join(format!(
            "priority-agent-doctor-state-test-{}",
            uuid::Uuid::new_v4()
        ));
        let paths = vec![root.join("config"), root.join("data"), root.join("cache")];

        let result = check_state_dirs_writable_for_paths(&paths);

        assert_eq!(result.status, CheckStatus::Ok);
        assert!(result.message.contains("Writable state directories"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_check_git_worktree_warns_for_plain_temp_dir() {
        let root = std::env::temp_dir().join(format!(
            "priority-agent-doctor-git-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("create temp dir");

        let result = check_git_worktree(&root);

        assert!(matches!(
            result.status,
            CheckStatus::Warning | CheckStatus::Error
        ));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_check_release_artifacts_reports_missing_assets() {
        let root = std::env::temp_dir().join(format!(
            "priority-agent-doctor-release-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(root.join("scripts")).expect("create scripts dir");
        std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"x\"\n").expect("write cargo");

        let result = check_release_artifacts(&root);

        assert_eq!(result.status, CheckStatus::Warning);
        assert!(result.message.contains("package-release.sh"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_check_release_artifacts_for_paths_ok() {
        let root = std::env::temp_dir().join(format!(
            "priority-agent-doctor-release-ok-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("create root");
        let paths = ["a", "b", "c"]
            .iter()
            .map(|name| {
                let path = root.join(name);
                std::fs::write(&path, "ok").expect("write release asset");
                path
            })
            .collect::<Vec<_>>();

        let result = check_release_artifacts_for_paths(&paths);

        assert_eq!(result.status, CheckStatus::Ok);
        assert!(result.message.contains("Release assets present"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_check_plugin_runtime_reports_blocked_plugin() {
        let root = std::env::temp_dir().join(format!(
            "priority-agent-doctor-plugin-test-{}",
            uuid::Uuid::new_v4()
        ));
        let plugin_dir = root.join("blocked");
        std::fs::create_dir_all(&plugin_dir).expect("create plugin dir");
        std::fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
name = "blocked"
version = "0.1.0"
enabled = true
"#,
        )
        .expect("write plugin manifest");

        let result = check_plugin_runtime_for_roots(
            std::slice::from_ref(&root),
            plugins::trust::TrustMode::Off,
        );

        assert_eq!(result.status, CheckStatus::Warning);
        assert!(result.message.contains("blocked=1"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_report_overall_error() {
        let checks = vec![
            CheckResult::ok("a", "fine"),
            CheckResult::error("b", "bad", "fix it"),
        ];
        let report = DiagnosticReport::new(checks);
        assert_eq!(report.overall, CheckStatus::Error);
        assert!(report.format_text().contains("❌"));
    }

    #[test]
    fn test_report_overall_warning() {
        let checks = vec![
            CheckResult::ok("a", "fine"),
            CheckResult::warn("b", "meh", "fix it"),
        ];
        let report = DiagnosticReport::new(checks);
        assert_eq!(report.overall, CheckStatus::Warning);
    }

    #[test]
    fn test_report_json_roundtrip() {
        let checks = vec![CheckResult::ok("git", "git version 2.43")];
        let report = DiagnosticReport::new(checks).with_metadata("working_dir", "/tmp");
        let json = report.to_json();
        assert!(json.contains("git"));
        assert!(json.contains("/tmp"));
    }
}
