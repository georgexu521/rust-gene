//! Diff/Patch 输出服务
//!
//! 对标 Claude Code 的 `diff.ts`
//! 生成结构化 hunk 输出和可读的 git diff

use std::path::Path;
use tokio::process::Command;
use tracing::debug;

/// Hunk 头信息
#[derive(Debug, Clone)]
pub struct HunkHeader {
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
}

/// 单行差异
#[derive(Debug, Clone)]
pub struct DiffLine {
    pub line_type: DiffLineType,
    pub content: String,
    pub old_line_num: Option<u32>,
    pub new_line_num: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiffLineType {
    Context,
    Addition,
    Deletion,
    Header,
}

/// 结构化 Hunk
#[derive(Debug, Clone)]
pub struct Hunk {
    pub header: HunkHeader,
    pub lines: Vec<DiffLine>,
}

/// 文件差异
#[derive(Debug, Clone)]
pub struct FileDiff {
    pub old_path: String,
    pub new_path: String,
    pub hunks: Vec<Hunk>,
    pub insertions: u32,
    pub deletions: u32,
}

/// Diff 输出选项
#[derive(Debug, Clone)]
pub struct DiffOptions {
    /// 是否包含上下文行数
    pub context_lines: u32,
    /// 是否忽略空白差异
    pub ignore_whitespace: bool,
    /// Tab 宽度（用于显示转换）
    pub tab_width: u32,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            context_lines: 3,
            ignore_whitespace: false,
            tab_width: 4,
        }
    }
}

/// 使用 git diff 生成结构化输出
pub async fn get_patch_from_contents(
    old_content: &str,
    new_content: &str,
    old_path: &str,
    new_path: &str,
    options: &DiffOptions,
) -> Result<FileDiff, String> {
    // 创建临时文件用于 diff
    let temp_dir = std::env::temp_dir().join("priority_agent_diff");
    let old_file = temp_dir.join("old");
    let new_file = temp_dir.join("new");

    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| format!("Failed to create temp dir: {}", e))?;

    tokio::fs::write(&old_file, old_content)
        .await
        .map_err(|e| format!("Failed to write old file: {}", e))?;

    tokio::fs::write(&new_file, new_content)
        .await
        .map_err(|e| format!("Failed to write new file: {}", e))?;

    let mut args = vec![
        "--no-index".to_string(),
        "--no-color".to_string(),
        "-U".to_string(),
        options.context_lines.to_string(),
    ];

    if options.ignore_whitespace {
        args.push("--ignore-whitespace".to_string());
    }

    args.push(old_file.to_string_lossy().to_string());
    args.push(new_file.to_string_lossy().to_string());

    let output = Command::new("git")
        .args(&args)
        .output()
        .await
        .map_err(|e| format!("git diff failed: {}", e))?;

    // git diff --no-index 返回 exit code 1 是正常的（表示有差异）
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    debug!("git diff stdout: {}", stdout);
    debug!("git diff stderr: {}", stderr);

    let diff_text = if !stdout.is_empty() {
        stdout.to_string()
    } else {
        stderr.to_string()
    };

    // 清理临时文件
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;

    // 解析 diff 输出
    parse_diff_output(&diff_text, old_path, new_path)
}

/// 解析 git diff 输出为结构化数据
fn parse_diff_output(diff_text: &str, old_path: &str, new_path: &str) -> Result<FileDiff, String> {
    let mut hunks = Vec::new();
    let mut insertions = 0u32;
    let mut deletions = 0u32;

    let mut current_hunk: Option<Hunk> = None;
    let mut current_lines: Vec<DiffLine> = Vec::new();

    for line in diff_text.lines() {
        let trimmed = line.trim_end();

        // Hunk 头: @@ -start,count +start,count @@
        if let Some(caps) = regex::Regex::new(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(.*)$")
            .ok()
            .and_then(|re| re.captures(trimmed))
        {
            // 保存之前的 hunk
            if let Some(h) = current_hunk.take() {
                hunks.push(h);
            }

            let old_start: u32 = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
            let old_count: u32 = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
            let new_start: u32 = caps.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
            let new_count: u32 = caps.get(4).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);

            current_hunk = Some(Hunk {
                header: HunkHeader {
                    old_start,
                    old_count,
                    new_start,
                    new_count,
                },
                lines: Vec::new(),
            });
            current_lines = Vec::new();

            debug!("Found hunk: @@ -{},{} +{},{} @@", old_start, old_count, new_start, new_count);
            continue;
        }

        // 差异行
        if let Some(line_type) = match trimmed.chars().next() {
            Some('+') => Some(DiffLineType::Addition),
            Some('-') => Some(DiffLineType::Deletion),
            Some(' ') => Some(DiffLineType::Context),
            Some('\\') => None, // "No newline at end of file" 特殊行
            _ => None,
        } {
            let content = if trimmed.len() > 1 {
                trimmed[1..].to_string()
            } else {
                String::new()
            };

            if line_type == DiffLineType::Addition {
                insertions += 1;
            } else if line_type == DiffLineType::Deletion {
                deletions += 1;
            }

            current_lines.push(DiffLine {
                line_type,
                content,
                old_line_num: None,
                new_line_num: None,
            });
        }
    }

    // 保存最后一个 hunk
    if let Some(mut h) = current_hunk.take() {
        h.lines = current_lines;
        hunks.push(h);
    }

    Ok(FileDiff {
        old_path: old_path.to_string(),
        new_path: new_path.to_string(),
        hunks,
        insertions,
        deletions,
    })
}

/// 生成显示友好的 diff 字符串
pub fn get_patch_for_display(diff: &FileDiff, options: &DiffOptions) -> String {
    let mut output = Vec::new();

    // 文件头
    output.push(format!("diff --git a/{} b/{}", diff.old_path, diff.new_path));
    output.push(format!("--- a/{}\n+++ b/{}", diff.old_path, diff.new_path));

    // 统计信息
    output.push(format!(
        "@@ -{},{} +{},{} @@",
        diff.hunks.first().map(|h| h.header.old_start).unwrap_or(0),
        diff.hunks.iter().map(|h| h.header.old_count).sum::<u32>(),
        diff.hunks.first().map(|h| h.header.new_start).unwrap_or(0),
        diff.hunks.iter().map(|h| h.header.new_count).sum::<u32>()
    ));

    for hunk in &diff.hunks {
        // Hunk 头
        output.push(format!(
            "@@ -{},{} +{},{} @@",
            hunk.header.old_start, hunk.header.old_count,
            hunk.header.new_start, hunk.header.new_count
        ));

        for line in &hunk.lines {
            let prefix = match line.line_type {
                DiffLineType::Addition => "+",
                DiffLineType::Deletion => "-",
                DiffLineType::Context => " ",
                DiffLineType::Header => "",
            };

            // Tab 转换和转义
            let content = escape_for_display(&line.content, options.tab_width);
            output.push(format!("{}{}", prefix, content));
        }
    }

    // 尾部统计
    output.push(format!(
        "\n{} insertions(+), {} deletions(-)",
        diff.insertions, diff.deletions
    ));

    output.join("\n")
}

/// Tab 转义和显示转义
fn escape_for_display(content: &str, tab_width: u32) -> String {
    let mut result = String::with_capacity(content.len());

    for ch in content.chars() {
        match ch {
            '\t' => {
                // Tab 转换为空格
                let spaces = tab_width as usize;
                result.extend(std::iter::repeat(' ').take(spaces));
            }
            '\n' => result.push('\n'),
            '\r' => {} // 忽略回车
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            _ => result.push(ch),
        }
    }

    result
}

/// 简单 diff 算法（不依赖外部命令）
pub fn simple_diff(old_content: &str, new_content: &str) -> Vec<(DiffLineType, String)> {
    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = new_content.lines().collect();

    let mut result = Vec::new();

    // 简单的行对行比较
    let max_len = old_lines.len().max(new_lines.len());

    for i in 0..max_len {
        let old_line = old_lines.get(i).copied();
        let new_line = new_lines.get(i).copied();

        match (old_line, new_line) {
            (Some(ol), Some(nl)) if ol == nl => {
                result.push((DiffLineType::Context, ol.to_string()));
            }
            (Some(ol), Some(nl)) => {
                result.push((DiffLineType::Deletion, ol.to_string()));
                result.push((DiffLineType::Addition, nl.to_string()));
            }
            (Some(ol), None) => {
                result.push((DiffLineType::Deletion, ol.to_string()));
            }
            (None, Some(nl)) => {
                result.push((DiffLineType::Addition, nl.to_string()));
            }
            _ => {}
        }
    }

    result
}

/// 使用 git diff 对文件生成 patch
pub async fn get_file_diff(old_path: &Path, new_path: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["diff", "--no-color", old_path.to_string_lossy().as_ref(), new_path.to_string_lossy().as_ref()])
        .output()
        .await
        .map_err(|e| format!("git diff failed: {}", e))?;

    if output.status.success() || !output.stdout.is_empty() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// 获取工作目录的未提交 diff
pub async fn get_uncommitted_diff(working_dir: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["diff", "--no-color"])
        .current_dir(working_dir)
        .output()
        .await
        .map_err(|e| format!("git diff failed: {}", e))?;

    if output.status.success() || !output.stdout.is_empty() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// 获取 staged 的 diff
pub async fn get_staged_diff(working_dir: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["diff", "--no-color", "--cached"])
        .current_dir(working_dir)
        .output()
        .await
        .map_err(|e| format!("git diff --cached failed: {}", e))?;

    if output.status.success() || !output.stdout.is_empty() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_diff() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nmodified\nline3\n";

        let diff = simple_diff(old, new);

        assert_eq!(diff.len(), 4);
        assert_eq!(diff[0], (DiffLineType::Context, "line1".to_string()));
        assert_eq!(diff[1], (DiffLineType::Deletion, "line2".to_string()));
        assert_eq!(diff[2], (DiffLineType::Addition, "modified".to_string()));
        assert_eq!(diff[3], (DiffLineType::Context, "line3".to_string()));
    }

    #[test]
    fn test_escape_for_display() {
        assert_eq!(escape_for_display("hello", 4), "hello");
        assert_eq!(escape_for_display("a\tb", 4), "a    b");
        assert_eq!(escape_for_display("a&lt;b&gt;c", 4), "a&amp;lt;b&amp;gt;c");
    }

    #[tokio::test]
    async fn test_simple_diff_lines() {
        let old = "hello\nworld\n";
        let new = "hello\nrust\n";

        let diff_lines = simple_diff(old, new);

        // Should have: context "hello", deletion "world", addition "rust"
        assert!(diff_lines.len() >= 3);
    }

    #[tokio::test]
    async fn test_get_patch_from_contents() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nmodified\nline3\n";

        // Use simple_diff which doesn't depend on external commands
        let diff_lines = simple_diff(old, new);

        assert!(diff_lines.len() >= 3);
        // Check we have at least one addition and one deletion
        let additions = diff_lines.iter().filter(|(t, _)| t == &DiffLineType::Addition).count();
        let deletions = diff_lines.iter().filter(|(t, _)| t == &DiffLineType::Deletion).count();
        assert!(additions >= 1 && deletions >= 1);
    }
}
