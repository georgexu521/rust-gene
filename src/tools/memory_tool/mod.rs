use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::{Path, PathBuf};

fn memory_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".priority-agent")
}

/// MEMORY.md 文件路径
fn memory_path() -> PathBuf {
    memory_root().join("MEMORY.md")
}

fn user_path() -> PathBuf {
    memory_root().join("USER.md")
}

fn memory_dir() -> PathBuf {
    memory_root().join("memory")
}

fn load_memory_dir_files() -> Vec<(String, String)> {
    let root = memory_dir();
    let mut files = Vec::new();
    collect_memory_dir_files(&root, &root, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn collect_memory_dir_files(root: &Path, dir: &Path, files: &mut Vec<(String, String)>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }

        if path.is_dir() {
            collect_memory_dir_files(root, path.as_path(), files);
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        if content.trim().is_empty() {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        files.push((relative, content));
    }
}

fn topic_file_path(topic: &str) -> Option<PathBuf> {
    let file_stem = sanitize_topic(topic)?;
    Some(memory_dir().join(format!("{}.md", file_stem)))
}

fn infer_topic(content: &str, category: &str) -> Option<&'static str> {
    let lower = content.to_lowercase();
    let category = category.to_lowercase();

    if category == "preference" || lower.contains("user preference") || lower.contains("偏好") {
        return None;
    }
    if contains_any(
        &lower,
        &[
            "tui", "terminal", "ui", "claude", "scroll", "界面", "设计", "滚动",
        ],
    ) {
        return Some("tui-design");
    }
    if contains_any(
        &lower,
        &[
            "context",
            "prompt",
            "token",
            "memory",
            "compression",
            "上下文",
            "提示词",
            "记忆",
        ],
    ) {
        return Some("context-management");
    }
    if contains_any(
        &lower,
        &["permission", "approval", "allow", "deny", "权限", "授权"],
    ) {
        return Some("permissions");
    }
    if contains_any(&lower, &["tool", "bash", "mcp", "工具"]) {
        return Some("tools");
    }
    if contains_any(&lower, &["rust", "cargo", ".rs", "crate"]) {
        return Some("rust-workflow");
    }
    if category == "decision" {
        return Some("decisions");
    }
    if category == "convention" {
        return Some("conventions");
    }
    None
}

fn contains_any(content: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| content.contains(needle))
}

fn sanitize_topic(topic: &str) -> Option<String> {
    let mut output = String::new();
    let mut last_dash = false;

    for ch in topic.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            output.push(ch);
            last_dash = false;
        } else if ch.is_alphanumeric() {
            output.push(ch);
            last_dash = false;
        } else if !last_dash {
            output.push('-');
            last_dash = true;
        }
    }

    let output = output
        .trim_matches('-')
        .chars()
        .take(80)
        .collect::<String>();
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

/// Memory Save 工具 - 保存信息到持久记忆
pub struct MemorySaveTool;

#[async_trait]
impl Tool for MemorySaveTool {
    fn name(&self) -> &str {
        "memory_save"
    }

    fn description(&self) -> &str {
        "Save important information to persistent memory. By default it auto-routes to USER.md or memory/<topic>.md; use target=index to force MEMORY.md."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The information to save to memory"
                },
                "category": {
                    "type": "string",
                    "description": "Category: preference, convention, decision, note",
                    "enum": ["preference", "convention", "decision", "note"],
                    "default": "note"
                },
                "target": {
                    "type": "string",
                    "description": "Optional target: auto infers destination, index writes MEMORY.md, user writes USER.md, topic writes memory/<topic>.md",
                    "enum": ["auto", "index", "user", "topic"],
                    "default": "auto"
                },
                "topic": {
                    "type": "string",
                    "description": "Optional topic filename for memory/<topic>.md. Example: tui-design, context-management, rust-workflow"
                }
            },
            "required": ["content"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let content = params["content"].as_str().unwrap_or("");
        if content.is_empty() {
            return ToolResult::error("Content cannot be empty");
        }

        let category = params["category"].as_str().unwrap_or("note");
        let target = params["target"].as_str().unwrap_or("auto");
        let topic = params["topic"].as_str().unwrap_or("").trim();

        let path = if target == "user" || category == "preference" {
            user_path()
        } else if target == "topic" || !topic.is_empty() {
            match topic_file_path(if topic.is_empty() { category } else { topic }) {
                Some(path) => path,
                None => {
                    return ToolResult::error("Topic must contain at least one valid character")
                }
            }
        } else if target == "auto" {
            if let Some(inferred) = infer_topic(content, category) {
                topic_file_path(inferred).unwrap_or_else(memory_path)
            } else {
                memory_path()
            }
        } else {
            memory_path()
        };

        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        // 读取现有内容
        let existing = std::fs::read_to_string(&path).unwrap_or_default();

        // 追加新记忆
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M");
        let entry = format!(
            "\n## [{}] {}\n{}\n",
            category.to_uppercase(),
            timestamp,
            content
        );

        let new_content = if existing.trim().is_empty() {
            let title = if path == user_path() {
                "# User Preferences"
            } else if path.starts_with(memory_dir()) {
                "# Priority Agent Topic Memory"
            } else {
                "# Priority Agent Memory"
            };
            format!("{}\n{}", title, entry)
        } else {
            format!("{}{}", existing, entry)
        };

        match std::fs::write(&path, &new_content) {
            Ok(_) => ToolResult::success(format!(
                "Saved to {}: [{}] {}",
                path.display(),
                category,
                content
            )),
            Err(e) => ToolResult::error(format!("Failed to save memory: {}", e)),
        }
    }
}

/// Memory Load 工具 - 读取持久记忆
pub struct MemoryLoadTool;

#[async_trait]
impl Tool for MemoryLoadTool {
    fn name(&self) -> &str {
        "memory_load"
    }

    fn description(&self) -> &str {
        "Load persistent memory from MEMORY.md and memory/*.md. Use this to recall user preferences, project conventions, and past decisions."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Optional: search query to filter memories. If empty, returns all memories."
                }
            }
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let path = memory_path();
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let memory_files = load_memory_dir_files();

        if content.trim().is_empty() && memory_files.is_empty() {
            return ToolResult::success("Memory is empty.");
        }

        let query = params["query"].as_str().unwrap_or("");

        if query.is_empty() {
            // 返回全部（限制大小）
            let mut output = String::new();
            if !content.trim().is_empty() {
                output.push_str("# MEMORY.md\n");
                output.push_str(content.trim());
                output.push_str("\n\n");
            }
            for (path, file_content) in memory_files {
                output.push_str(&format!("# memory/{}\n", path));
                output.push_str(file_content.trim());
                output.push_str("\n\n");
            }
            let truncated: String = output.chars().take(5000).collect();
            ToolResult::success(truncated)
        } else {
            // 简单关键词搜索
            let query_lower = query.to_lowercase();
            let mut matching: Vec<String> = content
                .lines()
                .filter(|l| l.to_lowercase().contains(&query_lower))
                .map(|line| format!("[MEMORY.md] {}", line))
                .collect();

            for (path, file_content) in memory_files {
                matching.extend(
                    file_content
                        .lines()
                        .filter(|l| l.to_lowercase().contains(&query_lower))
                        .map(|line| format!("[memory/{}] {}", path, line)),
                );
            }

            if matching.is_empty() {
                ToolResult::success(format!("No memories matching '{}'", query))
            } else {
                let result = matching.join("\n");
                let truncated: String = result.chars().take(3000).collect();
                ToolResult::success(truncated)
            }
        }
    }
}

/// Memory Clear 工具 - 清空记忆
pub struct MemoryClearTool;

#[async_trait]
impl Tool for MemoryClearTool {
    fn name(&self) -> &str {
        "memory_clear"
    }

    fn description(&self) -> &str {
        "Clear all persistent memory. Use with caution - this will delete all saved preferences and notes."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "confirm": {
                    "type": "boolean",
                    "description": "Must be true to confirm deletion"
                }
            },
            "required": ["confirm"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        if !params["confirm"].as_bool().unwrap_or(false) {
            return ToolResult::error("Set confirm=true to clear memory");
        }

        let path = memory_path();
        let memory_dir = memory_dir();
        let write_result = std::fs::write(&path, "# Priority Agent Memory\n");
        if memory_dir.exists() {
            let _ = std::fs::remove_dir_all(&memory_dir);
        }
        let _ = std::fs::create_dir_all(&memory_dir);

        match write_result {
            Ok(_) => ToolResult::success("Memory cleared"),
            Err(e) => ToolResult::error(format!("Failed to clear memory: {}", e)),
        }
    }

    fn requires_confirmation(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn confirmation_prompt(&self, _params: &serde_json::Value) -> Option<String> {
        Some("This will delete all saved memory. Continue?".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_path() {
        let path = memory_path();
        assert!(path.to_string_lossy().contains("MEMORY.md"));
    }

    #[test]
    fn test_sanitize_topic() {
        assert_eq!(sanitize_topic("TUI Design").as_deref(), Some("tui-design"));
        assert_eq!(
            sanitize_topic("../Context 管理.md").as_deref(),
            Some("context-管理-md")
        );
        assert_eq!(sanitize_topic("!!!"), None);
    }

    #[test]
    fn test_infer_topic() {
        assert_eq!(
            infer_topic("The TUI should keep Claude-style scroll anchoring.", "note"),
            Some("tui-design")
        );
        assert_eq!(
            infer_topic(
                "Prompt token budget should include memory snapshots.",
                "note"
            ),
            Some("context-management")
        );
        assert_eq!(
            infer_topic("User preference: respond in Chinese", "preference"),
            None
        );
    }
}
