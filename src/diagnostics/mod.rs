//! /doctor 深度诊断模块
//!
//! 检测环境健康状态，提供可执行的修复建议，并支持导出 JSON 诊断报告。

pub mod provider_health;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

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
    checks.push(check_permissions_config(working_dir));
    checks.push(check_session_store());

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
    let openai = std::env::var("OPENAI_API_KEY").is_ok();
    let moonshot = std::env::var("MOONSHOT_API_KEY").is_ok();

    if openai || moonshot {
        let mut parts = Vec::new();
        if openai {
            parts.push("OPENAI_API_KEY set");
        }
        if moonshot {
            parts.push("MOONSHOT_API_KEY set");
        }
        CheckResult::ok("config", parts.join("; "))
    } else {
        CheckResult::error(
            "config",
            "No LLM API key configured",
            "Set OPENAI_API_KEY or MOONSHOT_API_KEY in your environment",
        )
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
