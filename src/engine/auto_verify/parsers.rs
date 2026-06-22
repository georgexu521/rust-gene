//! Validation output parsers for auto-verify.
//!
//! Parsers normalize common test/check/lint outputs into structured validation
//! issues. They should preserve evidence rather than hiding failing commands.

use super::{command_output_with_timeout, VerificationIssue, VerificationResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::{info, warn};

pub(super) fn parse_cargo_check_output(stderr: &str) -> Vec<VerificationIssue> {
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
pub(super) fn parse_issue_header(line: &str) -> Option<(String, &str)> {
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
pub(super) fn parse_location_line(line: &str) -> Option<(String, u32)> {
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
pub(super) fn parse_cargo_test_output(raw: &str) -> (Vec<VerificationIssue>, String) {
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
pub(super) struct TestFailureInfo {
    location: Option<String>,
    message: String,
}

/// 从 panic 行提取位置字符串
/// "thread 'tests::test_b' panicked at src/main.rs:42:10:" -> "src/main.rs:42:10"
pub(super) fn extract_panic_location(line: &str) -> Option<String> {
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
pub(super) fn parse_panic_location_str(loc: &str) -> (Option<String>, Option<u32>) {
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
pub(super) fn extract_test_summary(raw: &str) -> String {
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

pub(super) fn is_python_project(working_dir: &Path) -> bool {
    working_dir.join("pyproject.toml").exists()
        || working_dir.join("setup.py").exists()
        || working_dir.join("requirements.txt").exists()
}

pub(super) fn is_typescript_project(working_dir: &Path) -> bool {
    working_dir.join("package.json").exists() || working_dir.join("tsconfig.json").exists()
}

pub(super) fn changed_python_files(working_dir: &Path, changed_files: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = changed_files
        .iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("py"))
        .filter(|path| {
            if path.is_absolute() {
                path.exists()
            } else {
                working_dir.join(path).exists()
            }
        })
        .map(|path| {
            if path.is_absolute() {
                path.clone()
            } else {
                working_dir.join(path)
            }
        })
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    files
}

pub(super) fn path_arg_for_display(working_dir: &Path, path: &Path) -> String {
    path.strip_prefix(working_dir)
        .unwrap_or(path)
        .display()
        .to_string()
}

// ────────────────────────────────────────────────
// Python 验证与测试
// ────────────────────────────────────────────────

pub(super) async fn verify_python(working_dir: &Path) -> Option<VerificationResult> {
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

pub(super) async fn verify_changed_python_files(
    working_dir: &Path,
    changed_files: &[PathBuf],
) -> Option<VerificationResult> {
    let files = changed_python_files(working_dir, changed_files);
    if files.is_empty() {
        return None;
    }

    info!("Running Python syntax verification for changed files");
    let display_files = files
        .iter()
        .map(|path| path_arg_for_display(working_dir, path))
        .collect::<Vec<_>>();
    let mut cmd = Command::new("python3");
    cmd.args(["-m", "py_compile"]);
    cmd.args(&files);
    cmd.current_dir(working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let output = match command_output_with_timeout(cmd, "python3 -m py_compile").await {
        Ok(output) => output,
        Err(e) => {
            warn!("Failed to run python3 -m py_compile: {}", e);
            return Some(VerificationResult {
                language: "Python".to_string(),
                command: format!("python3 -m py_compile {}", display_files.join(" ")),
                success: false,
                issues: vec![VerificationIssue {
                    severity: "error".to_string(),
                    file: None,
                    line: None,
                    message: format!("Failed to run python3 -m py_compile: {}", e),
                }],
                raw_output: String::new(),
                summary: String::new(),
            });
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = format!("{}{}", stdout, stderr);
    let issues = parse_py_compile_output(&raw);
    let summary = if output.status.success() && issues.is_empty() {
        format!("py_compile passed for {} file(s).", display_files.len())
    } else {
        String::new()
    };

    Some(VerificationResult {
        language: "Python".to_string(),
        command: format!("python3 -m py_compile {}", display_files.join(" ")),
        success: output.status.success() && issues.is_empty(),
        issues,
        raw_output: raw,
        summary,
    })
}

pub(super) async fn run_python_tests(working_dir: &Path) -> Option<VerificationResult> {
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

pub(super) fn parse_python_mypy_output(raw: &str) -> Vec<VerificationIssue> {
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

pub(super) fn parse_pyright_output(raw: &str) -> Vec<VerificationIssue> {
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
pub(super) fn parse_python_error_line(line: &str) -> Option<(String, u32, String, String)> {
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

pub(super) fn parse_pytest_output(raw: &str) -> (Vec<VerificationIssue>, String) {
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

pub(super) fn parse_py_compile_output(raw: &str) -> Vec<VerificationIssue> {
    let mut issues = Vec::new();
    let mut current_file = None;
    let mut current_line = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("File \"") {
            if let Some((file, after_file)) = rest.split_once('"') {
                current_file = Some(file.to_string());
                current_line = after_file.split_once("line ").and_then(|(_, line_part)| {
                    line_part
                        .split(|ch: char| !ch.is_ascii_digit())
                        .next()
                        .and_then(|num| num.parse::<u32>().ok())
                });
            }
            continue;
        }

        if trimmed.starts_with("SyntaxError:")
            || trimmed.starts_with("IndentationError:")
            || trimmed.starts_with("TabError:")
        {
            issues.push(VerificationIssue {
                severity: "error".to_string(),
                file: current_file.clone(),
                line: current_line,
                message: trimmed.to_string(),
            });
        }
    }

    if issues.is_empty() && !raw.trim().is_empty() {
        issues.push(VerificationIssue {
            severity: "error".to_string(),
            file: current_file,
            line: current_line,
            message: raw
                .lines()
                .next()
                .unwrap_or("python compile failed")
                .to_string(),
        });
    }

    issues
}

pub(super) fn parse_tsc_output(raw: &str) -> Vec<VerificationIssue> {
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

pub(super) fn parse_jest_output(raw: &str) -> (Vec<VerificationIssue>, String) {
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

pub(super) fn parse_go_build_output(stderr: &str) -> Vec<VerificationIssue> {
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

pub(super) fn parse_go_test_output(raw: &str) -> (Vec<VerificationIssue>, String) {
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
pub(super) fn parse_go_error_line(line: &str) -> Option<(String, u32, String)> {
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
