//! 自动验证模块
//!
//! 在文件修改（file_edit / file_write）后自动运行项目级验证，
//! 将编译错误、类型错误、lint 警告、测试失败等反馈注入对话上下文，
//! 形成"生成→编译→诊断→修复"的闭环。
//!
//! 当前支持：Rust (cargo check, cargo test), standalone Python syntax checks
//! 未来可扩展：JavaScript/TypeScript (tsc, jest), Python (mypy/pyright, pytest), Go (go build, go test)

use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tracing::{debug, info, warn};

mod parsers;
use parsers::*;

fn auto_verify_timeout() -> std::time::Duration {
    let secs = std::env::var("PRIORITY_AGENT_AUTO_VERIFY_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(300)
        .clamp(30, 900);
    std::time::Duration::from_secs(secs)
}

async fn command_output_with_timeout(
    mut cmd: Command,
    label: &str,
) -> std::io::Result<std::process::Output> {
    #[cfg(unix)]
    cmd.process_group(0);
    cmd.kill_on_drop(true);
    let timeout = auto_verify_timeout();
    let mut child = cmd.spawn()?;
    let child_pid = child.id();
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let stdout_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        if let Some(ref mut stream) = stdout {
            stream.read_to_end(&mut buffer).await?;
        }
        Ok::<Vec<u8>, std::io::Error>(buffer)
    });
    let stderr_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        if let Some(ref mut stream) = stderr {
            stream.read_to_end(&mut buffer).await?;
        }
        Ok::<Vec<u8>, std::io::Error>(buffer)
    });

    let status = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(result) => result?,
        Err(_) => {
            #[cfg(unix)]
            if let Some(pid) = child_pid {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("{} timed out after {}s", label, timeout.as_secs()),
            ));
        }
    };

    let stdout = stdout_task.await.map_err(std::io::Error::other)??;
    let stderr = stderr_task.await.map_err(std::io::Error::other)??;

    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

/// 单条验证问题
#[derive(Debug, Clone)]
pub struct VerificationIssue {
    pub severity: String, // "error" | "warning"
    pub file: Option<String>,
    pub line: Option<u32>,
    pub message: String,
}

/// 验证结果
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub language: String,
    pub command: String,
    pub success: bool,
    pub issues: Vec<VerificationIssue>,
    pub raw_output: String,
    /// 测试运行的额外摘要（如 "test result: FAILED. 2 passed; 1 failed"）
    pub summary: String,
}

impl VerificationResult {
    /// 格式化为对话友好的摘要
    pub fn to_dialog_text(&self) -> String {
        if self.issues.is_empty() {
            let summary = if self.summary.is_empty() {
                format!("{} passed with no issues.", self.command)
            } else {
                self.summary.clone()
            };
            return format!("[{} verification] {}", self.language, summary);
        }

        let errors = self.issues.iter().filter(|i| i.severity == "error").count();
        let warnings = self
            .issues
            .iter()
            .filter(|i| i.severity == "warning")
            .count();

        let mut lines = vec![format!(
            "[{} verification] {} found {} error(s), {} warning(s):",
            self.language, self.command, errors, warnings
        )];

        for issue in self.issues.iter().take(20) {
            let loc = match (&issue.file, issue.line) {
                (Some(f), Some(l)) => format!("{}:{}", f, l),
                (Some(f), None) => f.clone(),
                _ => "unknown".to_string(),
            };
            lines.push(format!("  [{}] {}: {}", issue.severity, loc, issue.message));
        }

        if self.issues.len() > 20 {
            lines.push(format!("  ... and {} more issues", self.issues.len() - 20));
        }

        if !self.summary.is_empty() {
            lines.push(format!("  [{}]", self.summary));
        }

        lines.join("\n")
    }
}

/// 检测项目类型并执行验证
///
/// 对于 Rust workspace 项目，只检查与修改文件相关的 crate，
/// 避免全量编译，显著减少验证时间。
pub async fn verify_file_changes(
    working_dir: &Path,
    changed_files: &[PathBuf],
) -> Vec<VerificationResult> {
    let mut results = Vec::new();

    // Rust 项目检测。只有 Rust 相关文件变更才跑 cargo，避免 Python/Docs
    // fixture 的小改动在 Rust workspace 根目录触发全量 check。
    if working_dir.join("Cargo.toml").exists() && has_rust_relevant_changes(changed_files) {
        if let Some(result) = verify_rust(working_dir, changed_files).await {
            results.push(result);
        }
    }

    // Python 项目检测
    if is_python_project(working_dir) {
        if let Some(result) = verify_python(working_dir).await {
            results.push(result);
        }
    }
    if let Some(result) = verify_changed_python_files(working_dir, changed_files).await {
        results.push(result);
    }

    // TypeScript/JavaScript 项目检测
    if is_typescript_project(working_dir) {
        if let Some(result) = verify_typescript(working_dir).await {
            results.push(result);
        }
    }

    // Go 项目检测
    if working_dir.join("go.mod").exists() {
        if let Some(result) = verify_go(working_dir).await {
            results.push(result);
        }
    }

    results
}

/// 运行项目测试
///
/// 自动检测项目类型并运行对应的测试框架：
/// - Rust (Cargo.toml) → cargo test --no-fail-fast
///
/// 环境变量开关（默认关闭，因为测试可能很慢）：
/// - `PRIORITY_AGENT_AUTO_TEST=1` 启用
/// - `PRIORITY_AGENT_AUTO_TEST=check_then_test` 仅在 cargo check 通过后运行
pub async fn run_tests(
    working_dir: &Path,
    changed_files: &[PathBuf],
    check_passed: bool,
) -> Vec<VerificationResult> {
    let mode = std::env::var("PRIORITY_AGENT_AUTO_TEST").unwrap_or_default();

    let should_run = match mode.as_str() {
        "1" | "true" | "yes" => true,
        "check_then_test" => check_passed,
        _ => false,
    };

    if !should_run {
        debug!("Auto test disabled. Set PRIORITY_AGENT_AUTO_TEST=1 to enable.");
        return Vec::new();
    }

    let mut results = Vec::new();

    if working_dir.join("Cargo.toml").exists() && has_rust_relevant_changes(changed_files) {
        if let Some(result) = run_rust_tests(working_dir, changed_files).await {
            results.push(result);
        }
    }

    if is_python_project(working_dir) {
        if let Some(result) = run_python_tests(working_dir).await {
            results.push(result);
        }
    }

    if is_typescript_project(working_dir) {
        if let Some(result) = run_typescript_tests(working_dir).await {
            results.push(result);
        }
    }

    if working_dir.join("go.mod").exists() {
        if let Some(result) = run_go_tests(working_dir).await {
            results.push(result);
        }
    }

    results
}

// ────────────────────────────────────────────────
// Workspace 检测
// ────────────────────────────────────────────────

/// 检测 workspace 并返回与修改文件相关的 member Cargo.toml 路径。
///
/// 返回空 Vec 表示：
/// - 不是 workspace 项目（普通单 crate）
/// - 修改了根目录文件（需要全量检查）
/// - 解析失败
fn resolve_workspace_targets(working_dir: &Path, changed_files: &[PathBuf]) -> Vec<PathBuf> {
    let cargo_toml = working_dir.join("Cargo.toml");
    let content = match std::fs::read_to_string(&cargo_toml) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let manifest: toml::Value = match content.parse() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let workspace = match manifest.get("workspace") {
        Some(w) => w,
        None => return Vec::new(),
    };

    let members = match workspace.get("members") {
        Some(m) => match m.as_array() {
            Some(arr) => arr,
            None => return Vec::new(),
        },
        None => return Vec::new(),
    };

    // 收集所有 member 目录
    let mut member_dirs: Vec<PathBuf> = Vec::new();
    for member_val in members {
        if let Some(member) = member_val.as_str() {
            if member.contains('*') {
                let pattern = working_dir.join(member).to_string_lossy().to_string();
                if let Ok(entries) = glob::glob(&pattern) {
                    for entry in entries.flatten() {
                        if entry.is_dir() {
                            member_dirs.push(entry);
                        }
                    }
                }
            } else {
                let p = working_dir.join(member);
                if p.is_dir() {
                    member_dirs.push(p);
                }
            }
        }
    }

    if member_dirs.is_empty() {
        return Vec::new();
    }

    // 检查是否有修改发生在根目录（不属于任何 member）
    let has_root_changes = changed_files.iter().any(|file| {
        let absolute = absolute_changed_path(working_dir, file);
        let parent = absolute.parent().unwrap_or(working_dir);
        // 如果文件在 working_dir 根下（不在任何 member 子目录中）
        !member_dirs.iter().any(|m| parent.starts_with(m))
    });

    if has_root_changes {
        return Vec::new(); // 根目录修改，回退到全量
    }

    // 收集涉及的 member
    let mut targets = std::collections::HashSet::new();
    for file in changed_files {
        let absolute = absolute_changed_path(working_dir, file);
        if let Some(parent) = absolute.parent() {
            for member in &member_dirs {
                if parent.starts_with(member) {
                    targets.insert(member.join("Cargo.toml"));
                    break;
                }
            }
        }
    }

    targets.into_iter().collect()
}

fn absolute_changed_path(working_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        working_dir.join(path)
    }
}

fn has_rust_relevant_changes(changed_files: &[PathBuf]) -> bool {
    changed_files.iter().any(|path| {
        let file_name = path.file_name().and_then(|name| name.to_str());
        matches!(file_name, Some("Cargo.toml" | "Cargo.lock" | "build.rs"))
            || path.extension().and_then(|ext| ext.to_str()) == Some("rs")
            || path
                .components()
                .any(|component| component.as_os_str() == ".cargo")
    })
}

// ────────────────────────────────────────────────
// Rust 验证
// ────────────────────────────────────────────────

/// Rust 验证：cargo check
///
/// 对于 workspace 项目，只 check 与修改文件相关的 crate，
/// 通过 `--manifest-path` 指向各 member 的 Cargo.toml。
async fn verify_rust(working_dir: &Path, changed_files: &[PathBuf]) -> Option<VerificationResult> {
    let targets = resolve_workspace_targets(working_dir, changed_files);

    if targets.is_empty() {
        // 单 crate 或根目录修改：全量 check
        info!("Running cargo check for Rust verification");
        run_cargo_check(working_dir, None).await
    } else {
        // Workspace 增量：逐个 check 涉及的 member
        info!(
            "Running incremental cargo check for {} workspace member(s): {:?}",
            targets.len(),
            targets
        );
        let mut all_issues = Vec::new();
        let mut all_raw = String::new();
        let mut all_success = true;
        let mut commands = Vec::new();

        for manifest in targets {
            if let Some(result) = run_cargo_check(working_dir, Some(&manifest)).await {
                commands.push(result.command.clone());
                all_success &= result.success;
                all_issues.extend(result.issues);
                if !result.raw_output.is_empty() {
                    all_raw.push_str(&format!(
                        "\n--- {} ---\n{}",
                        manifest.display(),
                        result.raw_output
                    ));
                }
            }
        }

        Some(VerificationResult {
            language: "Rust".to_string(),
            command: commands.join(", "),
            success: all_success,
            issues: all_issues,
            raw_output: all_raw,
            summary: String::new(),
        })
    }
}

/// 执行 cargo check
async fn run_cargo_check(
    working_dir: &Path,
    manifest: Option<&Path>,
) -> Option<VerificationResult> {
    let mut cmd = Command::new("cargo");
    cmd.args(["check", "--message-format=short"]);
    if let Some(m) = manifest {
        cmd.arg("--manifest-path").arg(m);
    }
    cmd.current_dir(working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let output = match command_output_with_timeout(cmd, "cargo check").await {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to run cargo check: {}", e);
            return Some(VerificationResult {
                language: "Rust".to_string(),
                command: "cargo check".to_string(),
                success: false,
                issues: vec![VerificationIssue {
                    severity: "error".to_string(),
                    file: None,
                    line: None,
                    message: format!("Failed to run cargo check: {}", e),
                }],
                raw_output: String::new(),
                summary: String::new(),
            });
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = stderr.to_string();
    let issues = parse_cargo_check_output(&stderr);

    let command = if let Some(m) = manifest {
        format!("cargo check --manifest-path {}", m.display())
    } else {
        "cargo check".to_string()
    };

    debug!(
        "cargo check: exit={} issues={}",
        output.status.code().unwrap_or(-1),
        issues.len()
    );

    Some(VerificationResult {
        language: "Rust".to_string(),
        command,
        success: output.status.success() && issues.is_empty(),
        issues,
        raw_output: raw,
        summary: String::new(),
    })
}

/// Rust 测试：cargo test --no-fail-fast
async fn run_rust_tests(
    working_dir: &Path,
    changed_files: &[PathBuf],
) -> Option<VerificationResult> {
    let targets = resolve_workspace_targets(working_dir, changed_files);

    if targets.is_empty() {
        info!("Running cargo test for Rust verification");
        run_cargo_test(working_dir, None).await
    } else {
        info!(
            "Running incremental cargo test for {} workspace member(s)",
            targets.len()
        );
        let mut all_issues = Vec::new();
        let mut all_raw = String::new();
        let mut all_success = true;
        let mut commands = Vec::new();
        let mut summaries = Vec::new();

        for manifest in targets {
            if let Some(result) = run_cargo_test(working_dir, Some(&manifest)).await {
                commands.push(result.command.clone());
                all_success &= result.success;
                all_issues.extend(result.issues);
                if !result.raw_output.is_empty() {
                    all_raw.push_str(&format!(
                        "\n--- {} ---\n{}",
                        manifest.display(),
                        result.raw_output
                    ));
                }
                if !result.summary.is_empty() {
                    summaries.push(result.summary);
                }
            }
        }

        Some(VerificationResult {
            language: "Rust".to_string(),
            command: commands.join(", "),
            success: all_success,
            issues: all_issues,
            raw_output: all_raw,
            summary: summaries.join("; "),
        })
    }
}

/// 执行 cargo test
async fn run_cargo_test(working_dir: &Path, manifest: Option<&Path>) -> Option<VerificationResult> {
    let mut cmd = Command::new("cargo");
    cmd.args(["test", "--no-fail-fast", "--color=never"]);
    if let Some(m) = manifest {
        cmd.arg("--manifest-path").arg(m);
    }
    cmd.current_dir(working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let output = match command_output_with_timeout(cmd, "cargo test").await {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to run cargo test: {}", e);
            return Some(VerificationResult {
                language: "Rust".to_string(),
                command: "cargo test".to_string(),
                success: false,
                issues: vec![VerificationIssue {
                    severity: "error".to_string(),
                    file: None,
                    line: None,
                    message: format!("Failed to run cargo test: {}", e),
                }],
                raw_output: String::new(),
                summary: String::new(),
            });
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    let (issues, summary) = parse_cargo_test_output(&combined);

    let command = if let Some(m) = manifest {
        format!("cargo test --manifest-path {}", m.display())
    } else {
        "cargo test".to_string()
    };

    debug!(
        "cargo test: exit={} issues={} summary={}",
        output.status.code().unwrap_or(-1),
        issues.len(),
        summary
    );

    Some(VerificationResult {
        language: "Rust".to_string(),
        command,
        success: output.status.success() && issues.is_empty(),
        issues,
        raw_output: combined,
        summary,
    })
}

// ────────────────────────────────────────────────
// 输出解析器
// ────────────────────────────────────────────────

// 解析 cargo check --message-format=short 输出
// 格式示例：
// ```text
// error[E0308]: mismatched types
//   --> src/main.rs:42:10
//    |
// 42 |     let x: u32 = "hello";
//    |                  ^^^^^^^ expected u32, found &str
// ```
// ────────────────────────────────────────────────
// TypeScript 验证与测试
// ────────────────────────────────────────────────

async fn verify_typescript(working_dir: &Path) -> Option<VerificationResult> {
    info!("Running TypeScript verification");

    // 优先尝试 tsc
    let tsc_output = Command::new("npx")
        .args(["tsc", "--noEmit"])
        .current_dir(working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;

    if let Ok(output) = tsc_output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let raw = format!("{}{}", stdout, stderr);
        let issues = parse_tsc_output(&raw);

        return Some(VerificationResult {
            language: "TypeScript".to_string(),
            command: "npx tsc --noEmit".to_string(),
            success: output.status.success() && issues.is_empty(),
            issues,
            raw_output: raw,
            summary: String::new(),
        });
    }

    warn!("tsc not available for TypeScript verification");
    None
}

async fn run_typescript_tests(working_dir: &Path) -> Option<VerificationResult> {
    info!("Running TypeScript tests");

    // 检测测试框架
    let package_json = working_dir.join("package.json");
    let test_cmd = if package_json.exists() {
        if let Ok(content) = std::fs::read_to_string(&package_json) {
            if content.contains("jest") {
                Some("npx jest --colors=false")
            } else if content.contains("vitest") {
                Some("npx vitest run")
            } else {
                Some("npm test")
            }
        } else {
            Some("npm test")
        }
    } else {
        Some("npm test")
    };

    let cmd_str = test_cmd?;
    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let mut cmd = Command::new(parts[0]);
    cmd.args(&parts[1..]);
    cmd.current_dir(working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let output = match command_output_with_timeout(cmd, cmd_str).await {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to run TypeScript tests: {}", e);
            return Some(VerificationResult {
                language: "TypeScript".to_string(),
                command: cmd_str.to_string(),
                success: false,
                issues: vec![VerificationIssue {
                    severity: "error".to_string(),
                    file: None,
                    line: None,
                    message: format!("Failed to run tests: {}", e),
                }],
                raw_output: String::new(),
                summary: String::new(),
            });
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    let (issues, summary) = parse_jest_output(&combined);

    Some(VerificationResult {
        language: "TypeScript".to_string(),
        command: cmd_str.to_string(),
        success: output.status.success() && issues.is_empty(),
        issues,
        raw_output: combined,
        summary,
    })
}

// ────────────────────────────────────────────────
// Go 验证与测试
// ────────────────────────────────────────────────

async fn verify_go(working_dir: &Path) -> Option<VerificationResult> {
    info!("Running Go verification");

    let output = match Command::new("go")
        .args(["build", "./..."])
        .current_dir(working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to run go build: {}", e);
            return Some(VerificationResult {
                language: "Go".to_string(),
                command: "go build ./...".to_string(),
                success: false,
                issues: vec![VerificationIssue {
                    severity: "error".to_string(),
                    file: None,
                    line: None,
                    message: format!("Failed to run go build: {}", e),
                }],
                raw_output: String::new(),
                summary: String::new(),
            });
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    let issues = parse_go_build_output(&stderr);

    Some(VerificationResult {
        language: "Go".to_string(),
        command: "go build ./...".to_string(),
        success: output.status.success() && issues.is_empty(),
        issues,
        raw_output: stderr.to_string(),
        summary: String::new(),
    })
}

async fn run_go_tests(working_dir: &Path) -> Option<VerificationResult> {
    info!("Running Go tests");

    let output = match Command::new("go")
        .args(["test", "./...", "-v"])
        .current_dir(working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to run go test: {}", e);
            return Some(VerificationResult {
                language: "Go".to_string(),
                command: "go test ./...".to_string(),
                success: false,
                issues: vec![VerificationIssue {
                    severity: "error".to_string(),
                    file: None,
                    line: None,
                    message: format!("Failed to run go test: {}", e),
                }],
                raw_output: String::new(),
                summary: String::new(),
            });
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    let (issues, summary) = parse_go_test_output(&combined);

    Some(VerificationResult {
        language: "Go".to_string(),
        command: "go test ./...".to_string(),
        success: output.status.success() && issues.is_empty(),
        issues,
        raw_output: combined,
        summary,
    })
}

// ────────────────────────────────────────────────
// 测试
// ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_issue_header_error() {
        let line = "error[E0308]: mismatched types";
        let (sev, msg) = parse_issue_header(line).unwrap();
        assert_eq!(sev, "error");
        assert_eq!(msg, "mismatched types");
    }

    #[test]
    fn test_parse_issue_header_simple_error() {
        let line = "error: something went wrong";
        let (sev, msg) = parse_issue_header(line).unwrap();
        assert_eq!(sev, "error");
        assert_eq!(msg, "something went wrong");
    }

    #[test]
    fn test_parse_issue_header_warning() {
        let line = "warning: unused variable";
        let (sev, msg) = parse_issue_header(line).unwrap();
        assert_eq!(sev, "warning");
        assert_eq!(msg, "unused variable");
    }

    #[test]
    fn test_parse_location_line() {
        let line = "  --> src/main.rs:42:10";
        let (file, line_num) = parse_location_line(line).unwrap();
        assert_eq!(file, "src/main.rs");
        assert_eq!(line_num, 42);
    }

    #[test]
    fn test_parse_cargo_check_output() {
        let stderr = r#"error[E0308]: mismatched types
  --> src/main.rs:42:10
   |
42 |     let x: u32 = "hello";
   |                  ^^^^^^^ expected u32, found &str

warning: unused variable: `foo`
  --> src/lib.rs:10:5
   |
"#;
        let issues = parse_cargo_check_output(stderr);
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].severity, "error");
        assert_eq!(issues[0].message, "mismatched types");
        assert_eq!(issues[0].file, Some("src/main.rs".to_string()));
        assert_eq!(issues[0].line, Some(42));

        assert_eq!(issues[1].severity, "warning");
        assert_eq!(issues[1].file, Some("src/lib.rs".to_string()));
        assert_eq!(issues[1].line, Some(10));
    }

    #[test]
    fn test_verification_result_dialog_text() {
        let result = VerificationResult {
            language: "Rust".to_string(),
            command: "cargo check".to_string(),
            success: false,
            issues: vec![VerificationIssue {
                severity: "error".to_string(),
                file: Some("src/main.rs".to_string()),
                line: Some(42),
                message: "mismatched types".to_string(),
            }],
            raw_output: String::new(),
            summary: String::new(),
        };
        let text = result.to_dialog_text();
        assert!(text.contains("error"));
        assert!(text.contains("mismatched types"));
        assert!(text.contains("src/main.rs:42"));
    }

    #[test]
    fn test_changed_python_files_detects_standalone_script() {
        let root =
            std::env::temp_dir().join(format!("priority-agent-auto-verify-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let script = root.join("snake.py");
        std::fs::write(&script, "print('ok')\n").unwrap();
        let files = changed_python_files(&root, &[script.clone(), root.join("README.md")]);
        assert_eq!(files, vec![script]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_rust_relevance_skips_python_fixture_changes() {
        assert!(!has_rust_relevant_changes(&[PathBuf::from(
            "fixtures/core_quality/simple_edit/settings.py"
        )]));
        assert!(!has_rust_relevant_changes(&[PathBuf::from("README.md")]));
        assert!(has_rust_relevant_changes(&[PathBuf::from("src/main.rs")]));
        assert!(has_rust_relevant_changes(&[PathBuf::from("Cargo.toml")]));
        assert!(has_rust_relevant_changes(&[PathBuf::from(
            ".cargo/config.toml"
        )]));
    }

    #[test]
    fn test_resolve_workspace_targets_handles_relative_member_paths() {
        let root = std::env::temp_dir().join(format!(
            "priority-agent-workspace-targets-{}",
            std::process::id()
        ));
        let member = root.join("crates/demo");
        std::fs::create_dir_all(member.join("src")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        std::fs::write(member.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();

        let targets = resolve_workspace_targets(&root, &[PathBuf::from("crates/demo/src/lib.rs")]);

        assert_eq!(targets, vec![member.join("Cargo.toml")]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_parse_py_compile_output_syntax_error() {
        let raw = r#"  File "snake.py", line 12
    if x ==
          ^
SyntaxError: invalid syntax
"#;
        let issues = parse_py_compile_output(raw);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].file, Some("snake.py".to_string()));
        assert_eq!(issues[0].line, Some(12));
        assert!(issues[0].message.contains("SyntaxError"));
    }

    // ── cargo test 解析测试 ─────────────────────────

    #[test]
    fn test_parse_cargo_test_output_all_pass() {
        let output = r#"running 2 tests
test tests::test_a ... ok
test tests::test_b ... ok

test result: ok. 2 passed; 0 failed; 0 ignored
"#;
        let (issues, summary) = parse_cargo_test_output(output);
        assert!(issues.is_empty());
        assert_eq!(summary, "test result: ok. 2 passed; 0 failed; 0 ignored");
    }

    #[test]
    fn test_parse_cargo_test_output_with_failures() {
        let output = r#"running 3 tests
test tests::test_a ... ok
test tests::test_b ... FAILED
test tests::test_c ... ok

failures:

---- tests::test_b stdout ----
thread 'tests::test_b' panicked at src/main.rs:42:10:
assertion failed: x == y

failures:
    tests::test_b

test result: FAILED. 2 passed; 1 failed; 0 ignored
"#;
        let (issues, summary) = parse_cargo_test_output(output);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, "error");
        assert_eq!(
            issues[0].message,
            "Test 'tests::test_b' failed: assertion failed: x == y"
        );
        assert_eq!(issues[0].file, Some("src/main.rs".to_string()));
        assert_eq!(issues[0].line, Some(42));
        assert_eq!(
            summary,
            "test result: FAILED. 2 passed; 1 failed; 0 ignored"
        );
    }

    #[test]
    fn test_parse_cargo_test_output_multiple_failures() {
        let output = r#"running 3 tests
test tests::test_a ... FAILED
test tests::test_b ... FAILED
test tests::test_c ... ok

failures:

---- tests::test_a stdout ----
thread 'tests::test_a' panicked at src/lib.rs:10:5:
assertion `left == right` failed

---- tests::test_b stdout ----
thread 'tests::test_b' panicked at src/lib.rs:20:8:
expected 42, got 0

failures:
    tests::test_a
    tests::test_b

test result: FAILED. 1 passed; 2 failed; 0 ignored
"#;
        let (issues, summary) = parse_cargo_test_output(output);
        assert_eq!(issues.len(), 2);

        let a = issues
            .iter()
            .find(|i| i.message.contains("test_a"))
            .unwrap();
        assert_eq!(a.file, Some("src/lib.rs".to_string()));
        assert_eq!(a.line, Some(10));
        assert!(a.message.contains("assertion `left == right` failed"));

        let b = issues
            .iter()
            .find(|i| i.message.contains("test_b"))
            .unwrap();
        assert_eq!(b.file, Some("src/lib.rs".to_string()));
        assert_eq!(b.line, Some(20));
        assert!(b.message.contains("expected 42, got 0"));

        assert_eq!(
            summary,
            "test result: FAILED. 1 passed; 2 failed; 0 ignored"
        );
    }

    #[test]
    fn test_extract_panic_location() {
        let line = "thread 'tests::test_b' panicked at src/main.rs:42:10:";
        let loc = extract_panic_location(line).unwrap();
        assert_eq!(loc, "src/main.rs:42:10");
    }

    #[test]
    fn test_extract_panic_location_with_module() {
        let line = "thread 'engine::tests::foo' panicked at src/engine/mod.rs:100:20:";
        let loc = extract_panic_location(line).unwrap();
        assert_eq!(loc, "src/engine/mod.rs:100:20");
    }

    #[test]
    fn test_parse_panic_location_str() {
        let (file, line) = parse_panic_location_str("src/main.rs:42:10");
        assert_eq!(file, Some("src/main.rs".to_string()));
        assert_eq!(line, Some(42));

        let (file, line) = parse_panic_location_str("src/main.rs:42");
        assert_eq!(file, Some("src/main.rs".to_string()));
        assert_eq!(line, Some(42));
    }
}
