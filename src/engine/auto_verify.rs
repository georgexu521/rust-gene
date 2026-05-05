//! 自动验证模块
//!
//! 在文件修改（file_edit / file_write）后自动运行项目级验证，
//! 将编译错误、类型错误、lint 警告、测试失败等反馈注入对话上下文，
//! 形成"生成→编译→诊断→修复"的闭环。
//!
//! 当前支持：Rust (cargo check, cargo test)
//! 未来可扩展：JavaScript/TypeScript (tsc, jest), Python (mypy/pyright, pytest), Go (go build, go test)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tracing::{debug, info, warn};

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

    // Rust 项目检测
    if working_dir.join("Cargo.toml").exists() {
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

    if working_dir.join("Cargo.toml").exists() {
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
    let has_root_changes = changed_files.iter().any(|f| {
        let parent = f.parent().unwrap_or(Path::new(""));
        // 如果文件在 working_dir 根下（不在任何 member 子目录中）
        !member_dirs.iter().any(|m| parent.starts_with(m))
    });

    if has_root_changes {
        return Vec::new(); // 根目录修改，回退到全量
    }

    // 收集涉及的 member
    let mut targets = std::collections::HashSet::new();
    for file in changed_files {
        if let Some(parent) = file.parent() {
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

/// 解析 cargo check --message-format=short 输出
/// 格式示例：
/// ```text
/// error[E0308]: mismatched types
///   --> src/main.rs:42:10
///    |
/// 42 |     let x: u32 = "hello";
///    |                  ^^^^^^^ expected u32, found &str
/// ```
fn parse_cargo_check_output(stderr: &str) -> Vec<VerificationIssue> {
    let mut issues = Vec::new();
    let lines: Vec<&str> = stderr.lines().collect();

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];

        // 匹配错误/警告行：
        // error: message
        // error[E0000]: message
        // warning: message
        if let Some((severity, message)) = parse_issue_header(line) {
            let mut issue = VerificationIssue {
                severity,
                file: None,
                line: None,
                message: message.to_string(),
            };

            // 尝试解析下一行的文件位置
            //   --> src/main.rs:42:10
            if i + 1 < lines.len() {
                let next = lines[i + 1].trim_start();
                if let Some(loc) = parse_location_line(next) {
                    issue.file = Some(loc.0);
                    issue.line = Some(loc.1);
                    i += 1; // 跳过位置行
                }
            }

            issues.push(issue);
        }

        i += 1;
    }

    issues
}

/// 解析错误/警告头行
fn parse_issue_header(line: &str) -> Option<(String, &str)> {
    if line.starts_with("error[") {
        // error[E0000]: message
        if let Some(pos) = line.find("]: ") {
            let msg = &line[pos + 3..];
            return Some(("error".to_string(), msg));
        }
    } else if let Some(msg) = line.strip_prefix("error: ") {
        return Some(("error".to_string(), msg));
    } else if let Some(msg) = line.strip_prefix("warning: ") {
        return Some(("warning".to_string(), msg));
    }
    None
}

/// 解析位置行：  --> file.rs:42:10
fn parse_location_line(line: &str) -> Option<(String, u32)> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("-->") {
        return None;
    }

    let after_arrow = trimmed[3..].trim_start();
    // 格式: path:line:col
    if let Some(colon_pos) = after_arrow.rfind(':') {
        let before_last_colon = &after_arrow[..colon_pos];
        if let Some(line_pos) = before_last_colon.rfind(':') {
            let file = before_last_colon[..line_pos].to_string();
            let line_num: u32 = before_last_colon[line_pos + 1..].parse().ok()?;
            return Some((file, line_num));
        }
    }

    None
}

// ────────────────────────────────────────────────
// 测试输出解析
// ────────────────────────────────────────────────

/// 解析 cargo test 输出
///
/// 格式示例：
///   running 3 tests
///   test tests::test_a ... ok
///   test tests::test_b ... FAILED
///
///   failures:
///
///   ---- tests::test_b stdout ----
///   thread 'tests::test_b' panicked at src/main.rs:42:10:
///   assertion failed: x == y
///
///   failures:
///       tests::test_b
///
///   test result: FAILED. 2 passed; 1 failed; 0 ignored; finished in 0.01s
fn parse_cargo_test_output(raw: &str) -> (Vec<VerificationIssue>, String) {
    let mut issues = Vec::new();
    let lines: Vec<&str> = raw.lines().collect();

    // 第一步：收集所有失败的测试名
    let mut failed_tests: HashMap<String, TestFailureInfo> = HashMap::new();

    for line in &lines {
        if line.starts_with("test ") && line.contains(" ... ") {
            let rest = &line[5..];
            if let Some(pos) = rest.find(" ... ") {
                let name = rest[..pos].to_string();
                let status = rest[pos + 5..].trim();
                if status == "FAILED" {
                    failed_tests.insert(name, TestFailureInfo::default());
                }
            }
        }
    }

    let summary = extract_test_summary(raw);

    if failed_tests.is_empty() {
        return (issues, summary);
    }

    // 第二步：解析 failures 区域，提取 panic 位置和错误消息
    let mut in_failure_detail = false;
    let mut current_test: Option<String> = None;

    for line in &lines {
        if line.trim() == "failures:" {
            if in_failure_detail {
                // 第二个 failures: 是底部的列表，结束解析
                break;
            }
            in_failure_detail = true;
            continue;
        }

        if !in_failure_detail {
            continue;
        }

        // ---- test_name stdout ----
        if line.starts_with("---- ") && line.contains(" stdout ----") {
            let name = line[5..]
                .split(" stdout ----")
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if failed_tests.contains_key(&name) {
                current_test = Some(name);
            }
            continue;
        }

        // 解析 panic 位置：thread 'X' panicked at src/file.rs:42:10:
        if let Some(ref test) = current_test {
            if line.starts_with("thread '") && line.contains("panicked at ") {
                if let Some(loc) = extract_panic_location(line) {
                    if let Some(info) = failed_tests.get_mut(test) {
                        info.location = Some(loc);
                    }
                }
            }
            // 收集错误消息（跳过 thread/panicked/note/backtrace 行）
            if !line.starts_with("thread '")
                && !line.contains("panicked at ")
                && !line.starts_with("note:")
                && !line.starts_with("stack backtrace:")
                && !line.trim().starts_with("at ")
                && !line.trim().is_empty()
            {
                if let Some(info) = failed_tests.get_mut(test) {
                    if info.message.is_empty() {
                        info.message = line.trim().to_string();
                    }
                }
            }
        }
    }

    // 第三步：构建 VerificationIssue
    for (name, info) in failed_tests {
        let (file, line_num) = info
            .location
            .as_ref()
            .map(|loc| parse_panic_location_str(loc))
            .unwrap_or((None, None));

        let msg = if info.message.is_empty() {
            format!("Test '{}' failed", name)
        } else {
            format!("Test '{}' failed: {}", name, info.message)
        };

        issues.push(VerificationIssue {
            severity: "error".to_string(),
            file,
            line: line_num,
            message: msg,
        });
    }

    (issues, summary)
}

#[derive(Default)]
struct TestFailureInfo {
    location: Option<String>,
    message: String,
}

/// 从 panic 行提取位置字符串
/// "thread 'tests::test_b' panicked at src/main.rs:42:10:" -> "src/main.rs:42:10"
fn extract_panic_location(line: &str) -> Option<String> {
    let after = line.strip_prefix("thread '")?;
    // 跳过测试名直到 panicked at
    let pos = after.find("panicked at ")?;
    let loc_part = &after[pos + 12..]; // after "panicked at "

    // 提取到行尾，去掉末尾的冒号
    let loc = loc_part.trim_end_matches(':').trim();
    if loc.is_empty() {
        return None;
    }
    Some(loc.to_string())
}

/// 解析位置字符串如 "src/main.rs:42:10" 或 "src/main.rs:42"
fn parse_panic_location_str(loc: &str) -> (Option<String>, Option<u32>) {
    // 格式: file.rs:line:col 或 file.rs:line
    if let Some(last_colon) = loc.rfind(':') {
        let before = &loc[..last_colon];
        if let Some(prev_colon) = before.rfind(':') {
            // file.rs:line:col
            let file = loc[..prev_colon].to_string();
            if let Ok(line) = loc[prev_colon + 1..last_colon].parse::<u32>() {
                return (Some(file), Some(line));
            }
        } else {
            // file.rs:line
            let file = before.to_string();
            if let Ok(line) = loc[last_colon + 1..].parse::<u32>() {
                return (Some(file), Some(line));
            }
        }
    }
    (None, None)
}

/// 提取 cargo test 总结行
fn extract_test_summary(raw: &str) -> String {
    for line in raw.lines() {
        if line.starts_with("test result:") {
            return line.to_string();
        }
    }
    String::new()
}

// ────────────────────────────────────────────────
// 多语言项目检测
// ────────────────────────────────────────────────

fn is_python_project(working_dir: &Path) -> bool {
    working_dir.join("pyproject.toml").exists()
        || working_dir.join("setup.py").exists()
        || working_dir.join("requirements.txt").exists()
}

fn is_typescript_project(working_dir: &Path) -> bool {
    working_dir.join("package.json").exists() || working_dir.join("tsconfig.json").exists()
}

// ────────────────────────────────────────────────
// Python 验证与测试
// ────────────────────────────────────────────────

async fn verify_python(working_dir: &Path) -> Option<VerificationResult> {
    info!("Running Python verification");

    // 优先尝试 mypy
    let mypy_output = Command::new("mypy")
        .arg(".")
        .current_dir(working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;

    if let Ok(output) = mypy_output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let raw = format!("{}{}", stdout, stderr);
        let issues = parse_python_mypy_output(&raw);

        return Some(VerificationResult {
            language: "Python".to_string(),
            command: "mypy .".to_string(),
            success: output.status.success() && issues.is_empty(),
            issues,
            raw_output: raw,
            summary: String::new(),
        });
    }

    // mypy 不可用，尝试 pyright
    let pyright_output = Command::new("pyright")
        .current_dir(working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;

    if let Ok(output) = pyright_output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let raw = format!("{}{}", stdout, stderr);
        let issues = parse_pyright_output(&raw);

        return Some(VerificationResult {
            language: "Python".to_string(),
            command: "pyright".to_string(),
            success: output.status.success() && issues.is_empty(),
            issues,
            raw_output: raw,
            summary: String::new(),
        });
    }

    warn!("Neither mypy nor pyright available for Python verification");
    None
}

async fn run_python_tests(working_dir: &Path) -> Option<VerificationResult> {
    info!("Running Python tests");

    let output = match Command::new("pytest")
        .args(["-v", "--color=no"])
        .current_dir(working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to run pytest: {}", e);
            return Some(VerificationResult {
                language: "Python".to_string(),
                command: "pytest".to_string(),
                success: false,
                issues: vec![VerificationIssue {
                    severity: "error".to_string(),
                    file: None,
                    line: None,
                    message: format!("Failed to run pytest: {}", e),
                }],
                raw_output: String::new(),
                summary: String::new(),
            });
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    let (issues, summary) = parse_pytest_output(&combined);

    Some(VerificationResult {
        language: "Python".to_string(),
        command: "pytest".to_string(),
        success: output.status.success() && issues.is_empty(),
        issues,
        raw_output: combined,
        summary,
    })
}

fn parse_python_mypy_output(raw: &str) -> Vec<VerificationIssue> {
    let mut issues = Vec::new();
    for line in raw.lines() {
        // mypy 格式: file.py:42: error: message
        // 或: file.py:42: note: message
        if let Some((file, line_num, severity, msg)) = parse_python_error_line(line) {
            issues.push(VerificationIssue {
                severity,
                file: Some(file),
                line: Some(line_num),
                message: msg,
            });
        }
    }
    issues
}

fn parse_pyright_output(raw: &str) -> Vec<VerificationIssue> {
    let mut issues = Vec::new();
    for line in raw.lines() {
        // pyright 格式: file.py:42:10 - error: message
        if let Some((file, line_num, severity, msg)) = parse_python_error_line(line) {
            issues.push(VerificationIssue {
                severity,
                file: Some(file),
                line: Some(line_num),
                message: msg,
            });
        }
    }
    issues
}

/// 解析 Python 风格错误行：file.py:42: error: message
fn parse_python_error_line(line: &str) -> Option<(String, u32, String, String)> {
    let trimmed = line.trim();
    // 格式: path:line: severity: message
    // 或: path:line:col - severity: message
    let first_colon = trimmed.find(':')?;
    let file = trimmed[..first_colon].to_string();

    let rest = &trimmed[first_colon + 1..];
    let line_end = rest.find(|c: char| !c.is_ascii_digit())?;
    let line_num: u32 = rest[..line_end].parse().ok()?;

    let after_line = &rest[line_end..];
    // 跳过可选的列号 :col
    let after_line = if let Some(after_colon) = after_line.strip_prefix(':') {
        if let Some(end) = after_colon.find(|c: char| !c.is_ascii_digit()) {
            &after_colon[end..]
        } else {
            after_colon
        }
    } else {
        after_line
    };

    // 跳过空格/破折号
    let after_line = after_line.trim_start_matches([' ', '-']);

    // 提取 severity
    let severity = if after_line.starts_with("error:") {
        "error"
    } else if after_line.starts_with("warning:") {
        "warning"
    } else if after_line.starts_with("note:") {
        return None; // 跳过 note
    } else {
        return None;
    };

    let msg_start = after_line.find(':').map(|i| i + 1).unwrap_or(0);
    let msg = after_line[msg_start..].trim().to_string();

    Some((file, line_num, severity.to_string(), msg))
}

fn parse_pytest_output(raw: &str) -> (Vec<VerificationIssue>, String) {
    let mut issues = Vec::new();
    let mut summary = String::new();

    for line in raw.lines() {
        if line.contains("FAILED") && line.contains("::") {
            // test_module.py::test_name FAILED
            if let Some(pos) = line.find("::") {
                let file = line[..pos].trim().to_string();
                let rest = &line[pos + 2..];
                let test_name = rest.split_whitespace().next().unwrap_or("");
                issues.push(VerificationIssue {
                    severity: "error".to_string(),
                    file: Some(file),
                    line: None,
                    message: format!("Test '{}' failed", test_name),
                });
            }
        }
        if line.contains("passed") && line.contains("failed") {
            summary = line.trim().to_string();
        }
    }

    (issues, summary)
}

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

fn parse_tsc_output(raw: &str) -> Vec<VerificationIssue> {
    let mut issues = Vec::new();
    for line in raw.lines() {
        // tsc 格式: file.ts(42,10): error TS0000: message
        if let Some(paren_pos) = line.find('(') {
            let file = line[..paren_pos].to_string();
            let after_paren = &line[paren_pos + 1..];
            if let Some(close_paren) = after_paren.find(')') {
                let loc = &after_paren[..close_paren];
                let parts: Vec<&str> = loc.split(',').collect();
                let line_num: u32 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);

                let after_loc = &after_paren[close_paren + 1..];
                if after_loc.starts_with(": error") {
                    let msg_start = after_loc.find("error").map(|i| i + 5).unwrap_or(0);
                    let msg = after_loc[msg_start..]
                        .trim_start_matches([' ', ':'])
                        .to_string();
                    issues.push(VerificationIssue {
                        severity: "error".to_string(),
                        file: Some(file),
                        line: Some(line_num),
                        message: msg,
                    });
                } else if after_loc.starts_with(": warning") {
                    let msg_start = after_loc.find("warning").map(|i| i + 7).unwrap_or(0);
                    let msg = after_loc[msg_start..]
                        .trim_start_matches([' ', ':'])
                        .to_string();
                    issues.push(VerificationIssue {
                        severity: "warning".to_string(),
                        file: Some(file),
                        line: Some(line_num),
                        message: msg,
                    });
                }
            }
        }
    }
    issues
}

fn parse_jest_output(raw: &str) -> (Vec<VerificationIssue>, String) {
    let mut issues = Vec::new();
    let mut summary = String::new();

    for line in raw.lines() {
        if line.starts_with("  ✕") || line.starts_with("  ×") {
            // Jest failure: ✕ test name (42 ms)
            let test_name = line[3..].trim().to_string();
            issues.push(VerificationIssue {
                severity: "error".to_string(),
                file: None,
                line: None,
                message: format!("Test failed: {}", test_name),
            });
        }
        if line.contains("Tests:") && (line.contains("failed") || line.contains("passed")) {
            summary = line.trim().to_string();
        }
    }

    (issues, summary)
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

fn parse_go_build_output(stderr: &str) -> Vec<VerificationIssue> {
    let mut issues = Vec::new();
    for line in stderr.lines() {
        // go build 格式: file.go:42:10: error message
        if let Some((file, line_num, msg)) = parse_go_error_line(line) {
            issues.push(VerificationIssue {
                severity: "error".to_string(),
                file: Some(file),
                line: Some(line_num),
                message: msg,
            });
        }
    }
    issues
}

fn parse_go_test_output(raw: &str) -> (Vec<VerificationIssue>, String) {
    let mut issues = Vec::new();
    let mut summary = String::new();

    for line in raw.lines() {
        // go test 失败: --- FAIL: TestName (0.01s)
        if let Some(rest) = line.strip_prefix("--- FAIL:") {
            let test_name = rest.trim().to_string();
            issues.push(VerificationIssue {
                severity: "error".to_string(),
                file: None,
                line: None,
                message: format!("Test failed: {}", test_name),
            });
        }
        // go test 格式: file.go:42: TestName: error message
        if let Some((file, line_num, msg)) = parse_go_error_line(line) {
            issues.push(VerificationIssue {
                severity: "error".to_string(),
                file: Some(file),
                line: Some(line_num),
                message: msg,
            });
        }
        if line.starts_with("FAIL") || line.starts_with("ok") {
            summary = line.trim().to_string();
        }
    }

    (issues, summary)
}

/// 解析 Go 风格错误行：file.go:42:10: message
fn parse_go_error_line(line: &str) -> Option<(String, u32, String)> {
    let trimmed = line.trim();
    // 格式: path:line:col: message 或 path:line: message
    let first_colon = trimmed.find(':')?;
    let file = trimmed[..first_colon].to_string();
    if !file.ends_with(".go") {
        return None;
    }

    let rest = &trimmed[first_colon + 1..];
    let line_end = rest.find(|c: char| !c.is_ascii_digit())?;
    let line_num: u32 = rest[..line_end].parse().ok()?;

    let after_line = &rest[line_end..];
    // 跳过可选的列号
    let after_line = if let Some(after_colon) = after_line.strip_prefix(':') {
        if let Some(end) = after_colon.find(|c: char| !c.is_ascii_digit()) {
            &after_colon[end..]
        } else {
            after_colon
        }
    } else {
        after_line
    };

    let msg = after_line.trim_start_matches(':').trim().to_string();
    if msg.is_empty() {
        return None;
    }

    Some((file, line_num, msg))
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
