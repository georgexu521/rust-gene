use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
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

fn legacy_agent_memory_dir() -> PathBuf {
    memory_root().join("agent_memories")
}

#[derive(Debug, Clone)]
struct MemoryDocument {
    namespace: String,
    path: String,
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentMemoryJsonEntry {
    key: String,
    value: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct MemoryKeyValue {
    namespace: String,
    key: String,
    value: String,
}

fn load_memory_dir_files() -> Vec<(String, String)> {
    let root = memory_dir();
    let mut files = Vec::new();
    collect_memory_dir_files(&root, &root, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn load_memory_documents() -> Vec<MemoryDocument> {
    let mut docs = Vec::new();
    push_text_document(&mut docs, "project", "MEMORY.md", &memory_path());
    push_text_document(&mut docs, "user", "USER.md", &user_path());

    for (path, content) in load_memory_dir_files() {
        docs.push(MemoryDocument {
            namespace: "topic".to_string(),
            path: format!("memory/{}", path),
            content,
        });
    }

    collect_agent_memory_documents(&memory_dir().join("agents"), "agent", &mut docs);
    collect_agent_memory_documents(&legacy_agent_memory_dir(), "agent_legacy", &mut docs);
    docs.sort_by(|a, b| {
        a.namespace
            .cmp(&b.namespace)
            .then_with(|| a.path.cmp(&b.path))
    });
    docs
}

fn push_text_document(docs: &mut Vec<MemoryDocument>, namespace: &str, label: &str, path: &Path) {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return,
    };
    if content.trim().is_empty() {
        return;
    }
    docs.push(MemoryDocument {
        namespace: namespace.to_string(),
        path: label.to_string(),
        content,
    });
}

fn collect_agent_memory_documents(dir: &Path, namespace: &str, docs: &mut Vec<MemoryDocument>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        if content.trim().is_empty() {
            continue;
        }
        let display_content = format_agent_memory_content(&content);
        if display_content.trim().is_empty() {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown.json");
        docs.push(MemoryDocument {
            namespace: namespace.to_string(),
            path: format!("memory/agents/{}", file_name),
            content: display_content,
        });
    }
}

fn format_agent_memory_content(content: &str) -> String {
    match serde_json::from_str::<Vec<AgentMemoryJsonEntry>>(content) {
        Ok(entries) => entries
            .into_iter()
            .map(|entry| {
                let tags = if entry.tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", entry.tags.join(","))
                };
                format!("{}: {}{}", entry.key, entry.value, tags)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Err(_) => content.to_string(),
    }
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

fn search_memory_documents(docs: &[MemoryDocument], query: &str) -> Vec<String> {
    let query_lower = query.to_lowercase();
    let mut matching = Vec::new();

    for doc in docs {
        for line in doc.content.lines() {
            if line.to_lowercase().contains(&query_lower) {
                matching.push(format!("[{}:{}] {}", doc.namespace, doc.path, line.trim()));
            }
        }
    }

    matching
}

fn memory_conflicts(docs: &[MemoryDocument], max_conflicts: usize) -> Vec<String> {
    let mut by_key: HashMap<String, Vec<MemoryKeyValue>> = HashMap::new();
    for doc in docs {
        for entry in extract_key_values(doc) {
            by_key
                .entry(entry.key.to_lowercase())
                .or_default()
                .push(entry);
        }
    }

    let mut conflicts = by_key
        .into_iter()
        .filter_map(|(key, entries)| {
            if entries.len() < 2 {
                return None;
            }
            let mut values = entries
                .iter()
                .map(|entry| normalize_value(&entry.value))
                .collect::<Vec<_>>();
            values.sort();
            values.dedup();
            if values.len() < 2 {
                return None;
            }
            let locations = entries
                .iter()
                .take(4)
                .map(|entry| {
                    format!(
                        "{}={} ({})",
                        entry.namespace,
                        compact_line(&entry.value, 70),
                        entry.key
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
            Some(format!(
                "- key '{}' has conflicting values: {}",
                key, locations
            ))
        })
        .collect::<Vec<_>>();

    conflicts.sort();
    conflicts.truncate(max_conflicts);
    conflicts
}

fn extract_key_values(doc: &MemoryDocument) -> Vec<MemoryKeyValue> {
    doc.content
        .lines()
        .filter_map(|line| {
            let trimmed = line
                .trim()
                .trim_start_matches("- ")
                .trim_start_matches("* ");
            let (key, value) = trimmed.split_once(':')?;
            let key = key.trim().trim_matches('`');
            let value = value.trim();
            if key.is_empty()
                || value.is_empty()
                || key.starts_with('#')
                || key.chars().count() > 80
                || key.contains("://")
            {
                return None;
            }
            Some(MemoryKeyValue {
                namespace: format!("{}:{}", doc.namespace, doc.path),
                key: key.to_string(),
                value: value.to_string(),
            })
        })
        .collect()
}

fn normalize_value(value: &str) -> String {
    value
        .trim()
        .trim_end_matches('.')
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn compact_line(text: &str, max_chars: usize) -> String {
    let mut value = text.replace('\n', " ");
    if value.chars().count() > max_chars {
        value = value.chars().take(max_chars).collect::<String>();
        value.push_str("...");
    }
    value
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
                },
                "include_conflicts": {
                    "type": "boolean",
                    "description": "Whether to include duplicate/conflicting key hints across memory namespaces.",
                    "default": true
                }
            }
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let docs = load_memory_documents();
        let include_conflicts = params["include_conflicts"].as_bool().unwrap_or(true);

        if docs.is_empty() {
            return ToolResult::success("Memory is empty.");
        }

        let query = params["query"].as_str().unwrap_or("");
        let conflicts = if include_conflicts {
            memory_conflicts(&docs, 8)
        } else {
            Vec::new()
        };

        if query.is_empty() {
            // 返回全部（限制大小）
            let mut output = String::new();
            for doc in &docs {
                output.push_str(&format!("# [{}] {}\n", doc.namespace, doc.path));
                output.push_str(doc.content.trim());
                output.push_str("\n\n");
            }
            if !conflicts.is_empty() {
                output.push_str("# Conflicts\n");
                output.push_str(&conflicts.join("\n"));
                output.push('\n');
            }
            let truncated: String = output.chars().take(5000).collect();
            ToolResult::success(truncated)
        } else {
            let mut matching = search_memory_documents(&docs, query);

            if matching.is_empty() {
                ToolResult::success(format!("No memories matching '{}'", query))
            } else {
                if !conflicts.is_empty() {
                    matching.push(String::new());
                    matching.push("Conflicts:".to_string());
                    matching.extend(conflicts);
                }
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

    #[test]
    fn test_memory_document_search_includes_namespaces() {
        let docs = vec![
            MemoryDocument {
                namespace: "user".to_string(),
                path: "USER.md".to_string(),
                content: "language: Chinese".to_string(),
            },
            MemoryDocument {
                namespace: "agent".to_string(),
                path: "memory/agents/reviewer.json".to_string(),
                content: "review_style: strict".to_string(),
            },
        ];

        let results = search_memory_documents(&docs, "strict");
        assert_eq!(results.len(), 1);
        assert!(results[0].starts_with("[agent:memory/agents/reviewer.json]"));
    }

    #[test]
    fn test_memory_conflicts_detect_duplicate_keys() {
        let docs = vec![
            MemoryDocument {
                namespace: "user".to_string(),
                path: "USER.md".to_string(),
                content: "language: Chinese".to_string(),
            },
            MemoryDocument {
                namespace: "topic".to_string(),
                path: "memory/preferences.md".to_string(),
                content: "language: English".to_string(),
            },
        ];

        let conflicts = memory_conflicts(&docs, 8);
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0].contains("key 'language'"));
    }

    #[test]
    fn test_agent_memory_json_formats_as_key_values() {
        let content = r#"[{"key":"review_style","value":"strict","created_at":1,"updated_at":1,"tags":["review"]}]"#;
        let formatted = format_agent_memory_content(content);
        assert!(formatted.contains("review_style: strict [review]"));
    }
}
