use crate::engine::conversation_loop::ToolApprovalRequest;
use crate::tools::bash_tool::command_classifier::{classify_command, ShellCommandCategory};

/// 计算待审批工具的 Diff 预览。
///
/// 该函数前端无关，可被 TUI 和 CLI 同时复用。
pub fn compute_permission_diff(req: &ToolApprovalRequest) -> Option<(String, String)> {
    let name = req.tool_call.name.as_str();
    let args = &req.tool_call.arguments;

    match name {
        "file_write" => {
            let path = args["path"].as_str().unwrap_or("unknown");
            let content = args["content"].as_str().unwrap_or("");
            let line_count = content.lines().count();
            let mut lines = vec![
                format!("--- /dev/null"),
                format!("+++ b/{}", path),
                format!("@@ -0,0 +1,{} @@", line_count),
            ];
            for line in content.lines() {
                lines.push(format!("+{}", line));
            }
            Some((format!("Preview: {}", path), lines.join("\n")))
        }
        "file_edit" => {
            let path = args["path"].as_str().unwrap_or("unknown");
            // 尝试读取原始文件并生成真实的 unified diff
            if let Ok(original) = std::fs::read_to_string(path) {
                if let Ok(new_content) =
                    crate::tools::file_tool::FileEditTool::preview_edit(args, &original)
                {
                    if let Some(diff) = generate_unified_diff(&original, &new_content, path) {
                        return Some((format!("Diff: {}", path), diff));
                    }
                }
            }
            // 回退：显示旧版本的参数展示
            let old_string = args["old_string"].as_str().unwrap_or("");
            let new_string = args["new_string"].as_str().unwrap_or("");
            let insert_after = args["insert_after"].as_str();
            let insert_before = args["insert_before"].as_str();

            let mut lines = vec![format!("File: {}", path), "".to_string()];

            if let Some(after) = insert_after {
                lines.push("Insert after:".to_string());
                lines.push(format!("  {}", after));
                lines.push("New text:".to_string());
                for line in new_string.lines() {
                    lines.push(format!("  {}", line));
                }
            } else if let Some(before) = insert_before {
                lines.push("Insert before:".to_string());
                lines.push(format!("  {}", before));
                lines.push("New text:".to_string());
                for line in new_string.lines() {
                    lines.push(format!("  {}", line));
                }
            } else {
                lines.push("--- old_string ---".to_string());
                for line in old_string.lines() {
                    lines.push(format!("-{}", line));
                }
                lines.push("".to_string());
                lines.push("+++ new_string +++".to_string());
                for line in new_string.lines() {
                    lines.push(format!("+{}", line));
                }
            }
            Some((format!("Preview: {}", path), lines.join("\n")))
        }
        "file_patch" => {
            let ops = args["operations"].as_array().map_or(0, |a| a.len());
            let mut lines = vec![format!("Patch operations: {}", ops), "".to_string()];
            if let Some(ops) = args["operations"].as_array() {
                for (i, op) in ops.iter().enumerate() {
                    let path = op["path"].as_str().unwrap_or("?");
                    let rep = op["replacements"].as_array().map_or(0, |a| a.len());
                    lines.push(format!(
                        "[{}] {} ({} replacement{})",
                        i + 1,
                        path,
                        rep,
                        if rep == 1 { "" } else { "s" }
                    ));
                    if let Some(reps) = op["replacements"].as_array() {
                        for (j, r) in reps.iter().enumerate() {
                            let old = r["old_string"].as_str().unwrap_or("");
                            let new = r["new_string"].as_str().unwrap_or("");
                            lines.push(format!(
                                "  {}.{}: -{} → +{}",
                                i + 1,
                                j + 1,
                                old.chars().take(60).collect::<String>(),
                                new.chars().take(60).collect::<String>()
                            ));
                        }
                    }
                }
            }
            Some((format!("Patch: {} file(s)", ops), lines.join("\n")))
        }
        "format" => {
            let path = args["file_path"].as_str().unwrap_or("unknown");
            let formatter = args["formatter"].as_str().unwrap_or("default");
            Some((
                format!("Format: {}", path),
                format!("Formatter: {}\nFile: {}", formatter, path),
            ))
        }
        "bash" => {
            let command = args["command"].as_str().unwrap_or("");
            let working_dir = args["working_dir"].as_str().unwrap_or("current directory");
            let mut lines = vec![
                format!("Command: {}", command),
                format!("Working directory: {}", working_dir),
            ];
            if let Some(timeout) = args["timeout"].as_u64() {
                lines.push(format!("Timeout: {}s", timeout));
            }

            // 检测 bash 命令是否包含文件修改操作，给出替代工具建议
            let classification = classify_command(command);
            if classification.category == ShellCommandCategory::FileMutation
                || classification.category == ShellCommandCategory::GitMutation
            {
                lines.push("".to_string());
                lines.push("--- File mutation detected ---".to_string());
                lines.push(
                    "Prefer file_write, file_edit, or file_patch for file changes.".to_string(),
                );
                lines.push(
                    "bash should be used for: running tests, starting services, reading state."
                        .to_string(),
                );
            }

            Some(("Preview: bash command".to_string(), lines.join("\n")))
        }
        _ => None,
    }
}

/// 生成 unified diff（通过 diff -u 命令）
fn generate_unified_diff(old_content: &str, new_content: &str, path: &str) -> Option<String> {
    let old_file = std::env::temp_dir().join(format!("diff_old_{}", uuid::Uuid::new_v4()));
    let new_file = std::env::temp_dir().join(format!("diff_new_{}", uuid::Uuid::new_v4()));

    std::fs::write(&old_file, old_content).ok()?;
    std::fs::write(&new_file, new_content).ok()?;

    let output = std::process::Command::new("diff")
        .args(["-u", old_file.to_str()?, new_file.to_str()?])
        .output()
        .ok()?;

    let _ = std::fs::remove_file(&old_file).ok();
    let _ = std::fs::remove_file(&new_file).ok();

    let diff = String::from_utf8_lossy(&output.stdout);
    if diff.is_empty() {
        Some(format!("No differences in {}", path))
    } else {
        Some(diff.to_string())
    }
}
