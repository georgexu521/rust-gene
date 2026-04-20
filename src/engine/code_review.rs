//! 代码自审查模块
//!
//! 在文件修改后自动扫描新增代码中的风险模式，
//! 无需额外 LLM 调用，轻量、快速、覆盖常见代码质量问题。
//!
//! 审查规则：
//! - unwrap() / expect() — 可能导致 panic
//! - panic!() — 硬崩溃
//! - unsafe { } — 需要人工审查
//! - todo!() / unimplemented!() — 未完成代码
//! - 命令注入风险（Command + 拼接）
//! - 破坏性文件操作（remove_dir_all / remove_file）
//! - 硬编码敏感信息（密码/token）
//! - 调试残留（println! / dbg!）
//! - TODO / FIXME / HACK 注释
//!
//! 环境变量开关：
//! - `PRIORITY_AGENT_AUTO_REVIEW=1` 启用（默认关闭）

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// 单条审查意见
#[derive(Debug, Clone)]
pub struct ReviewIssue {
    pub severity: String, // "critical" | "warning" | "suggestion"
    pub rule: String,
    pub file: String,
    pub line: u32,
    pub message: String,
    pub snippet: String,
}

/// 审查结果
#[derive(Debug, Clone)]
pub struct ReviewResult {
    pub success: bool, // true = 无问题
    pub issues: Vec<ReviewIssue>,
}

impl ReviewResult {
    pub fn to_dialog_text(&self) -> String {
        if self.issues.is_empty() {
            return "[Code review] No issues found in changed files.".to_string();
        }

        let critical = self
            .issues
            .iter()
            .filter(|i| i.severity == "critical")
            .count();
        let warning = self
            .issues
            .iter()
            .filter(|i| i.severity == "warning")
            .count();
        let suggestion = self
            .issues
            .iter()
            .filter(|i| i.severity == "suggestion")
            .count();

        let mut lines = vec![format!(
            "[Code review] Found {} critical, {} warning, {} suggestion:",
            critical, warning, suggestion
        )];

        for issue in self.issues.iter().take(15) {
            lines.push(format!(
                "  [{}] {}:{} | {}: {}",
                issue.severity, issue.file, issue.line, issue.rule, issue.message
            ));
            if !issue.snippet.is_empty() {
                lines.push(format!("    > {}", issue.snippet));
            }
        }

        if self.issues.len() > 15 {
            lines.push(format!(
                "  ... and {} more issues",
                self.issues.len() - 15
            ));
        }

        lines.join("\n")
    }
}

/// 对修改的文件执行代码审查
pub fn review_changed_files(working_dir: &Path, changed_files: &[PathBuf]) -> ReviewResult {
    let enabled = std::env::var("PRIORITY_AGENT_AUTO_REVIEW")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);

    if !enabled {
        debug!("Auto review disabled. Set PRIORITY_AGENT_AUTO_REVIEW=1 to enable.");
        return ReviewResult {
            success: true,
            issues: Vec::new(),
        };
    }

    if changed_files.is_empty() {
        return ReviewResult {
            success: true,
            issues: Vec::new(),
        };
    }

    info!("Running code review on {} changed file(s)", changed_files.len());

    let mut all_issues = Vec::new();
    let mut seen = HashSet::new(); // 去重：(file, line, rule)

    for file in changed_files {
        let path = if file.is_absolute() {
            file.clone()
        } else {
            working_dir.join(file)
        };

        // 只审查源码文件
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !is_source_file(ext) {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                debug!("Cannot read {} for review: {}", path.display(), e);
                continue;
            }
        };

        let file_name = file.to_string_lossy().to_string();
        let issues = review_file_content(&file_name, &content);

        for issue in issues {
            let key = format!("{}:{}:{}", issue.file, issue.line, issue.rule);
            if seen.insert(key) {
                all_issues.push(issue);
            }
        }
    }

    // 按严重程度排序
    all_issues.sort_by_key(|i| match i.severity.as_str() {
        "critical" => 0,
        "warning" => 1,
        _ => 2,
    });

    ReviewResult {
        success: all_issues.is_empty(),
        issues: all_issues,
    }
}

fn is_source_file(ext: &str) -> bool {
    matches!(
        ext,
        "rs" | "py" | "js" | "ts" | "go" | "java" | "c" | "cpp" | "h" | "hpp" | "swift" | "kt"
    )
}

/// 审查单个文件内容（根据扩展名分发到语言特定审查器）
fn review_file_content(file: &str, content: &str) -> Vec<ReviewIssue> {
    let ext = std::path::Path::new(file)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "rs" => review_rust(file, content),
        "py" => review_python(file, content),
        "js" | "ts" | "jsx" | "tsx" => review_javascript(file, content),
        "go" => review_go(file, content),
        _ => review_generic(file, content),
    }
}

// ────────────────────────────────────────────────
// Rust 审查规则
// ────────────────────────────────────────────────

fn review_rust(file: &str, content: &str) -> Vec<ReviewIssue> {
    let mut issues = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        let line_num = (idx + 1) as u32;
        let trimmed = line.trim();
        let is_comment = trimmed.starts_with("//") || trimmed.starts_with("/*");

        // unwrap() / expect()
        if !is_comment && (line.contains("unwrap()") || line.contains(".expect(")) {
            let severity = if line.contains("unwrap()") { "critical" } else { "warning" };
            issues.push(ReviewIssue {
                severity: severity.to_string(),
                rule: "unwrap_or_expect".to_string(),
                file: file.to_string(),
                line: line_num,
                message: format!(
                    "Use of {} may cause panic. Consider using `?`, `match`, or `if let`.",
                    if line.contains("unwrap()") { "unwrap()" } else { "expect()" }
                ),
                snippet: trim_snippet(line),
            });
        }

        // panic!()
        if !is_comment && line.contains("panic!") {
            issues.push(ReviewIssue {
                severity: "critical".to_string(),
                rule: "panic".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "Hard panic. Consider returning Result or using a typed error.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // unsafe
        if !is_comment && line.contains("unsafe") && !line.contains("unsafe_") {
            issues.push(ReviewIssue {
                severity: "critical".to_string(),
                rule: "unsafe_block".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "Unsafe code block requires careful manual review.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // todo!() / unimplemented!()
        if !is_comment && (line.contains("todo!") || line.contains("unimplemented!")) {
            issues.push(ReviewIssue {
                severity: "warning".to_string(),
                rule: "incomplete_code".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "Incomplete implementation marker found.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // Command 拼接风险
        if !is_comment && line.contains("Command")
            && (line.contains("format!") || line.contains("+") || line.contains("push"))
        {
            issues.push(ReviewIssue {
                severity: "critical".to_string(),
                rule: "command_injection".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "Potential command injection: Command argument may be constructed dynamically.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // 破坏性文件操作
        if !is_comment && (line.contains("remove_dir_all") || line.contains("remove_file")) {
            issues.push(ReviewIssue {
                severity: "warning".to_string(),
                rule: "destructive_fs".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "Destructive file operation. Ensure path is validated and confirmed.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // println! / dbg! 残留
        if !is_comment && (line.contains("println!") || line.contains("dbg!")) {
            issues.push(ReviewIssue {
                severity: "suggestion".to_string(),
                rule: "debug_residual".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "Debug print statement. Consider using tracing::debug! or removing.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        check_generic_rules(file, line, line_num, &mut issues);
    }

    issues
}

// ────────────────────────────────────────────────
// Python 审查规则
// ────────────────────────────────────────────────

fn review_python(file: &str, content: &str) -> Vec<ReviewIssue> {
    let mut issues = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        let line_num = (idx + 1) as u32;
        let trimmed = line.trim();
        let is_comment = trimmed.starts_with('#');

        // eval() / exec()
        if !is_comment && (line.contains("eval(") || line.contains("exec(")) {
            issues.push(ReviewIssue {
                severity: "critical".to_string(),
                rule: "dangerous_eval".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "eval()/exec() can execute arbitrary code. Use ast.literal_eval or json.loads for safe parsing.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // os.system() / subprocess with shell=True
        if !is_comment
            && (line.contains("os.system(") || (line.contains("subprocess") && line.contains("shell=True")))
        {
            issues.push(ReviewIssue {
                severity: "critical".to_string(),
                rule: "command_injection".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "Executing shell commands with user input is dangerous. Use subprocess.run with a list and shell=False.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // bare except:
        if !is_comment && (trimmed == "except:" || trimmed.starts_with("except :")) {
            issues.push(ReviewIssue {
                severity: "warning".to_string(),
                rule: "bare_except".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "Bare `except:` catches KeyboardInterrupt and SystemExit. Use `except Exception:` instead.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // pickle.loads with untrusted input
        if !is_comment && line.contains("pickle.loads") {
            issues.push(ReviewIssue {
                severity: "warning".to_string(),
                rule: "unsafe_deserialization".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "pickle.loads can execute arbitrary code on untrusted data. Use json or msgpack instead.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // yaml.load without Loader
        if !is_comment && line.contains("yaml.load") && !line.contains("Loader=") && !line.contains("SafeLoader") {
            issues.push(ReviewIssue {
                severity: "warning".to_string(),
                rule: "unsafe_yaml".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "yaml.load without SafeLoader can execute arbitrary code. Use yaml.safe_load() instead.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // print() 残留
        if !is_comment && line.contains("print(") && !line.contains("_print") {
            issues.push(ReviewIssue {
                severity: "suggestion".to_string(),
                rule: "debug_residual".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "Debug print statement. Consider using logging or removing.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        check_generic_rules(file, line, line_num, &mut issues);
    }

    issues
}

// ────────────────────────────────────────────────
// JavaScript / TypeScript 审查规则
// ────────────────────────────────────────────────

fn review_javascript(file: &str, content: &str) -> Vec<ReviewIssue> {
    let mut issues = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        let line_num = (idx + 1) as u32;
        let trimmed = line.trim();
        let is_comment = trimmed.starts_with("//") || trimmed.starts_with("/*");

        // eval() / new Function() / setTimeout string
        if !is_comment
            && (line.contains("eval(") || line.contains("new Function(") || line.contains("setTimeout(") && line.contains("\""))
        {
            issues.push(ReviewIssue {
                severity: "critical".to_string(),
                rule: "dangerous_eval".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "eval() and new Function() can execute arbitrary code. Avoid or sanitize input.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // == instead of ===
        if !is_comment && (line.contains(" == ") || line.contains(" ==") || line.contains("== ")) && !line.contains("===") {
            issues.push(ReviewIssue {
                severity: "warning".to_string(),
                rule: "loose_equality".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "Loose equality (==) can cause unexpected type coercion. Use strict equality (===).".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // var instead of let/const
        if !is_comment && (trimmed.starts_with("var ") || trimmed.contains(" var ")) {
            issues.push(ReviewIssue {
                severity: "warning".to_string(),
                rule: "var_usage".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "var has function scope and hoisting issues. Use let or const instead.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // console.log 残留
        if !is_comment && line.contains("console.log") {
            issues.push(ReviewIssue {
                severity: "suggestion".to_string(),
                rule: "debug_residual".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "Debug console.log statement. Remove before committing.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // document.write
        if !is_comment && line.contains("document.write") {
            issues.push(ReviewIssue {
                severity: "warning".to_string(),
                rule: "document_write".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "document.write can overwrite the entire document and is discouraged.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        check_generic_rules(file, line, line_num, &mut issues);
    }

    issues
}

// ────────────────────────────────────────────────
// Go 审查规则
// ────────────────────────────────────────────────

fn review_go(file: &str, content: &str) -> Vec<ReviewIssue> {
    let mut issues = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        let line_num = (idx + 1) as u32;
        let trimmed = line.trim();
        let is_comment = trimmed.starts_with("//") || trimmed.starts_with("/*");

        // panic()
        if !is_comment && line.contains("panic(") {
            issues.push(ReviewIssue {
                severity: "critical".to_string(),
                rule: "panic".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "panic() causes hard crash. Return an error instead.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // unsafe
        if !is_comment && line.contains("unsafe.") {
            issues.push(ReviewIssue {
                severity: "critical".to_string(),
                rule: "unsafe_block".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "Unsafe code requires careful manual review.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // goto
        if !is_comment && (trimmed.starts_with("goto ") || trimmed.contains(" goto ")) {
            issues.push(ReviewIssue {
                severity: "warning".to_string(),
                rule: "goto".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "goto can make code hard to follow. Consider refactoring.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        // fmt.Println 残留
        if !is_comment && line.contains("fmt.Println") {
            issues.push(ReviewIssue {
                severity: "suggestion".to_string(),
                rule: "debug_residual".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "Debug print statement. Consider using a proper logger or removing.".to_string(),
                snippet: trim_snippet(line),
            });
        }

        check_generic_rules(file, line, line_num, &mut issues);
    }

    issues
}

// ────────────────────────────────────────────────
// 通用规则（所有语言）
// ────────────────────────────────────────────────

fn review_generic(file: &str, content: &str) -> Vec<ReviewIssue> {
    let mut issues = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        let line_num = (idx + 1) as u32;
        check_generic_rules(file, line, line_num, &mut issues);
    }

    issues
}

/// 通用规则检查
fn check_generic_rules(file: &str, line: &str, line_num: u32, issues: &mut Vec<ReviewIssue>) {
    let trimmed = line.trim();

    // 破坏性文件操作
    let destructive = ["remove_dir_all", "remove_file", "os.remove", "fs.unlink", "fs.rmdir"];
    for op in &destructive {
        if line.contains(op) {
            issues.push(ReviewIssue {
                severity: "warning".to_string(),
                rule: "destructive_fs".to_string(),
                file: file.to_string(),
                line: line_num,
                message: "Destructive file operation. Ensure path is validated and confirmed.".to_string(),
                snippet: trim_snippet(line),
            });
            break;
        }
    }

    // 硬编码敏感信息
    let lower = line.to_lowercase();
    if (lower.contains("password") || lower.contains("secret") || lower.contains("api_key") || lower.contains("token"))
        && (line.contains('"') || line.contains('\''))
    {
        issues.push(ReviewIssue {
            severity: "warning".to_string(),
            rule: "hardcoded_secret".to_string(),
            file: file.to_string(),
            line: line_num,
            message: "Possible hardcoded sensitive value. Use environment variables or a secret manager.".to_string(),
            snippet: trim_snippet(line),
        });
    }

    // TODO / FIXME / HACK / XXX 注释
    let upper = trimmed.to_uppercase();
    if (upper.contains("TODO") || upper.contains("FIXME") || upper.contains("HACK") || upper.contains("XXX"))
        && (trimmed.starts_with("//") || trimmed.starts_with("#") || trimmed.starts_with("/*") || trimmed.starts_with("*"))
    {
        issues.push(ReviewIssue {
            severity: "suggestion".to_string(),
            rule: "tech_debt".to_string(),
            file: file.to_string(),
            line: line_num,
            message: "Technical debt marker found. Consider resolving before merging.".to_string(),
            snippet: trim_snippet(line),
        });
    }
}

fn trim_snippet(line: &str) -> String {
    let s = line.trim();
    if s.len() > 80 {
        format!("{}...", &s[..80])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_review_rust_file() {
        let content = r#"
fn risky() {
    let x = data.unwrap();
    let y = data.expect("must exist");
    panic!("should not reach here");
    unsafe { *ptr = 42; }
    todo!("implement this");
    println!("debug");
    // TODO: handle error case
}
"#;
        let issues = review_file_content("src/lib.rs", content);
        assert!(!issues.is_empty());

        let has_unwrap = issues.iter().any(|i| i.rule == "unwrap_or_expect" && i.severity == "critical");
        let has_panic = issues.iter().any(|i| i.rule == "panic");
        let has_unsafe = issues.iter().any(|i| i.rule == "unsafe_block");
        let has_todo = issues.iter().any(|i| i.rule == "incomplete_code");
        let has_debug = issues.iter().any(|i| i.rule == "debug_residual");
        let has_tech_debt = issues.iter().any(|i| i.rule == "tech_debt");

        assert!(has_unwrap, "should detect unwrap()");
        assert!(has_panic, "should detect panic!");
        assert!(has_unsafe, "should detect unsafe");
        assert!(has_todo, "should detect todo!");
        assert!(has_debug, "should detect println!");
        assert!(has_tech_debt, "should detect TODO comment");
    }

    #[test]
    fn test_review_no_issues() {
        let content = r#"
fn safe() -> Result<T, E> {
    let x = data?;
    tracing::info!("done");
    Ok(x)
}
"#;
        let issues = review_file_content("src/lib.rs", content);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_review_destructive_fs() {
        let content = r#"std::fs::remove_dir_all(path);"#;
        let issues = review_file_content("src/lib.rs", content);
        assert!(issues.iter().any(|i| i.rule == "destructive_fs"));
    }

    #[test]
    fn test_review_command_injection() {
        let content = r#"Command::new("sh").arg(format!("-c {}", input));"#;
        let issues = review_file_content("src/lib.rs", content);
        assert!(issues.iter().any(|i| i.rule == "command_injection"));
    }

    #[test]
    fn test_review_hardcoded_secret() {
        let content = r#"let api_key = "sk-1234567890abcdef";"#;
        let issues = review_file_content("src/lib.rs", content);
        assert!(issues.iter().any(|i| i.rule == "hardcoded_secret"));
    }

    #[test]
    fn test_review_result_empty_when_disabled() {
        // 默认不启用
        let result = review_changed_files(
            Path::new("."),
            &[PathBuf::from("src/main.rs")],
        );
        assert!(result.success);
        assert!(result.issues.is_empty());
    }
}
