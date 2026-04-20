//! LSP 工具 - 查询语言服务器
//!
//! 支持获取诊断、悬停信息、定义、引用、符号等。

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// LSP 工具
pub struct LSPTool;

#[async_trait]
impl Tool for LSPTool {
    fn name(&self) -> &str {
        "lsp"
    }

    fn description(&self) -> &str {
        "Query a Language Server Protocol (LSP) server for code intelligence. \
         Actions: 'diagnostics', 'hover', 'definition', 'references', 'symbols', \
         'implementation', 'call_hierarchy', 'incoming_calls', 'outgoing_calls', \
         'servers' (list registered servers), 'register_server' (add new server), \
         'unregister_server' (remove server)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["diagnostics", "hover", "definition", "references", "symbols", "implementation", "call_hierarchy", "incoming_calls", "outgoing_calls", "servers", "register_server", "unregister_server"],
                    "description": "The LSP action to perform"
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to the file (required for diagnostics, hover, definition, references, document symbols)"
                },
                "line": {
                    "type": "integer",
                    "description": "0-indexed line number (required for hover, definition, references)",
                    "minimum": 0
                },
                "character": {
                    "type": "integer",
                    "description": "0-indexed character position (required for hover, definition, references)",
                    "minimum": 0
                },
                "include_declaration": {
                    "type": "boolean",
                    "default": true,
                    "description": "Whether to include declarations in references"
                },
                "query": {
                    "type": "string",
                    "description": "Query string for workspace symbols (required for workspace symbols if file_path not given)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");

        let lsp_manager = match &context.lsp_manager {
            Some(manager) => manager.clone(),
            None => {
                return ToolResult::error(
                    "LSP manager not available. No language servers are configured or auto-detected.".to_string(),
                );
            }
        };

        if lsp_manager.client_count() == 0 {
            return ToolResult::error(
                "No LSP servers are running. Auto-detection may have failed or the project language is not supported yet.".to_string(),
            );
        }

        let client = lsp_manager.first_client();
        let client = match client {
            Some(c) => c,
            None => {
                return ToolResult::error("No LSP client available.".to_string());
            }
        };

        match action {
            "diagnostics" => {
                let file_path = params["file_path"].as_str().unwrap_or("");
                if file_path.is_empty() {
                    return ToolResult::error("file_path is required for diagnostics".to_string());
                }

                let path =
                    match crate::tools::file_tool::resolve_path(file_path, &context.working_dir) {
                        Ok(p) => p,
                        Err(e) => return ToolResult::error(e),
                    };

                let uri = crate::engine::lsp::path_to_uri(&path);
                let lang_id = crate::engine::lsp::language_id_from_path(&path);

                // 读取文件内容并发送 didOpen
                match tokio::fs::read_to_string(&path).await {
                    Ok(text) => {
                        if let Err(e) = client.text_document_did_open(&uri, lang_id, &text).await {
                            return ToolResult::error(format!(
                                "Failed to open document in LSP: {}",
                                e
                            ));
                        }
                    }
                    Err(e) => {
                        return ToolResult::error(format!("Failed to read file: {}", e));
                    }
                }

                // 轮询诊断，最多等待 3 秒
                let mut diagnostics = Vec::new();
                let max_wait = std::time::Duration::from_secs(3);
                let start = std::time::Instant::now();
                while start.elapsed() < max_wait {
                    diagnostics = client.get_diagnostics(&uri).await;
                    if !diagnostics.is_empty() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                if diagnostics.is_empty() {
                    ToolResult::success(format!("No diagnostics found for {}", file_path))
                } else {
                    let mut lines = vec![format!(
                        "Diagnostics for {} ({} found):",
                        file_path,
                        diagnostics.len()
                    )];
                    for d in diagnostics {
                        let severity = d
                            .severity
                            .map(|s| match s {
                                1 => "Error",
                                2 => "Warning",
                                3 => "Info",
                                4 => "Hint",
                                _ => "Unknown",
                            })
                            .unwrap_or("Unknown");
                        lines.push(format!(
                            "  [{}] Line {}:{} - {}",
                            severity,
                            d.range.start.line + 1,
                            d.range.start.character + 1,
                            d.message
                        ));
                    }
                    ToolResult::success(lines.join("\n"))
                }
            }
            "hover" => {
                let file_path = params["file_path"].as_str().unwrap_or("");
                let line = params["line"].as_u64().unwrap_or(0) as u32;
                let character = params["character"].as_u64().unwrap_or(0) as u32;

                if file_path.is_empty() {
                    return ToolResult::error("file_path is required for hover".to_string());
                }

                let path =
                    match crate::tools::file_tool::resolve_path(file_path, &context.working_dir) {
                        Ok(p) => p,
                        Err(e) => return ToolResult::error(e),
                    };
                let uri = crate::engine::lsp::path_to_uri(&path);

                match client.text_document_hover(&uri, line, character).await {
                    Ok(result) => {
                        let content = result["contents"].clone();
                        let text = extract_marked_string(content);
                        ToolResult::success_with_data(
                            text.clone(),
                            json!({ "hover": text, "uri": uri, "line": line, "character": character }),
                        )
                    }
                    Err(e) => ToolResult::error(format!("Hover request failed: {}", e)),
                }
            }
            "definition" => {
                let file_path = params["file_path"].as_str().unwrap_or("");
                let line = params["line"].as_u64().unwrap_or(0) as u32;
                let character = params["character"].as_u64().unwrap_or(0) as u32;

                if file_path.is_empty() {
                    return ToolResult::error("file_path is required for definition".to_string());
                }

                let path =
                    match crate::tools::file_tool::resolve_path(file_path, &context.working_dir) {
                        Ok(p) => p,
                        Err(e) => return ToolResult::error(e.to_string()),
                    };
                let uri = crate::engine::lsp::path_to_uri(&path);

                match client.text_document_definition(&uri, line, character).await {
                    Ok(result) => {
                        let formatted = format_locations(&result);
                        ToolResult::success_with_data(
                            formatted.clone(),
                            json!({ "locations": result, "uri": uri, "line": line, "character": character }),
                        )
                    }
                    Err(e) => ToolResult::error(format!("Definition request failed: {}", e)),
                }
            }
            "references" => {
                let file_path = params["file_path"].as_str().unwrap_or("");
                let line = params["line"].as_u64().unwrap_or(0) as u32;
                let character = params["character"].as_u64().unwrap_or(0) as u32;

                if file_path.is_empty() {
                    return ToolResult::error("file_path is required for references".to_string());
                }

                let path =
                    match crate::tools::file_tool::resolve_path(file_path, &context.working_dir) {
                        Ok(p) => p,
                        Err(e) => return ToolResult::error(e.to_string()),
                    };
                let uri = crate::engine::lsp::path_to_uri(&path);
                let include_declaration = params["include_declaration"].as_bool().unwrap_or(true);

                match client
                    .text_document_references(&uri, line, character, include_declaration)
                    .await
                {
                    Ok(result) => {
                        let formatted = format_locations(&result);
                        ToolResult::success_with_data(
                            formatted.clone(),
                            json!({ "locations": result, "uri": uri, "line": line, "character": character }),
                        )
                    }
                    Err(e) => ToolResult::error(format!("References request failed: {}", e)),
                }
            }
            "symbols" => {
                let file_path = params["file_path"].as_str().unwrap_or("");
                if !file_path.is_empty() {
                    let path = match crate::tools::file_tool::resolve_path(
                        file_path,
                        &context.working_dir,
                    ) {
                        Ok(p) => p,
                        Err(e) => return ToolResult::error(e.to_string()),
                    };
                    let uri = crate::engine::lsp::path_to_uri(&path);

                    match client.text_document_document_symbol(&uri).await {
                        Ok(result) => {
                            let formatted = format_symbols(&result);
                            ToolResult::success_with_data(
                                formatted.clone(),
                                json!({ "symbols": result, "uri": uri }),
                            )
                        }
                        Err(e) => {
                            ToolResult::error(format!("Document symbol request failed: {}", e))
                        }
                    }
                } else {
                    let query = params["query"].as_str().unwrap_or("");
                    match client.workspace_symbol(query).await {
                        Ok(result) => {
                            let formatted = format_symbols(&result);
                            ToolResult::success_with_data(
                                formatted.clone(),
                                json!({ "symbols": result, "query": query }),
                            )
                        }
                        Err(e) => {
                            ToolResult::error(format!("Workspace symbol request failed: {}", e))
                        }
                    }
                }
            }
            "implementation" => {
                let file_path = params["file_path"].as_str().unwrap_or("");
                let line = params["line"].as_u64().unwrap_or(0) as u32;
                let character = params["character"].as_u64().unwrap_or(0) as u32;

                if file_path.is_empty() {
                    return ToolResult::error("file_path is required for implementation".to_string());
                }

                let path =
                    match crate::tools::file_tool::resolve_path(file_path, &context.working_dir) {
                        Ok(p) => p,
                        Err(e) => return ToolResult::error(e.to_string()),
                    };
                let uri = crate::engine::lsp::path_to_uri(&path);

                match client.text_document_implementation(&uri, line, character).await {
                    Ok(result) => {
                        let formatted = format_locations(&result);
                        ToolResult::success_with_data(
                            formatted.clone(),
                            json!({ "locations": result, "uri": uri, "line": line, "character": character }),
                        )
                    }
                    Err(e) => ToolResult::error(format!("Implementation request failed: {}", e)),
                }
            }
            "call_hierarchy" => {
                let file_path = params["file_path"].as_str().unwrap_or("");
                let line = params["line"].as_u64().unwrap_or(0) as u32;
                let character = params["character"].as_u64().unwrap_or(0) as u32;

                if file_path.is_empty() {
                    return ToolResult::error("file_path is required for call_hierarchy".to_string());
                }

                let path =
                    match crate::tools::file_tool::resolve_path(file_path, &context.working_dir) {
                        Ok(p) => p,
                        Err(e) => return ToolResult::error(e.to_string()),
                    };
                let uri = crate::engine::lsp::path_to_uri(&path);

                match client
                    .text_document_prepare_call_hierarchy(&uri, line, character)
                    .await
                {
                    Ok(result) => {
                        let formatted = format_call_hierarchy_items(&result);
                        ToolResult::success_with_data(
                            formatted.clone(),
                            json!({ "items": result, "uri": uri, "line": line, "character": character }),
                        )
                    }
                    Err(e) => {
                        ToolResult::error(format!("Call hierarchy request failed: {}", e))
                    }
                }
            }
            "incoming_calls" => {
                let item = params["item"].clone();
                if item.is_null() {
                    ToolResult::error("item is required for incoming_calls (use call_hierarchy first)".to_string())
                } else {
                    match client.call_hierarchy_incoming_calls(&item).await {
                        Ok(result) => {
                            let formatted = format_incoming_calls(&result);
                            ToolResult::success_with_data(
                                formatted.clone(),
                                json!({ "calls": result }),
                            )
                        }
                        Err(e) => {
                            ToolResult::error(format!("Incoming calls request failed: {}", e))
                        }
                    }
                }
            }
            "outgoing_calls" => {
                let item = params["item"].clone();
                if item.is_null() {
                    ToolResult::error(
                        "item is required for outgoing_calls (use call_hierarchy first)"
                            .to_string(),
                    )
                } else {
                    match client.call_hierarchy_outgoing_calls(&item).await {
                        Ok(result) => {
                            let formatted = format_outgoing_calls(&result);
                            ToolResult::success_with_data(formatted.clone(), json!({ "calls": result }))
                        }
                        Err(e) => {
                            ToolResult::error(format!("Outgoing calls request failed: {}", e))
                        }
                    }
                }
            }
            "servers" => {
                let status = lsp_manager.server_status();
                if status.is_empty() {
                    ToolResult::success("No LSP servers registered.".to_string())
                } else {
                    let lines: Vec<String> = status
                        .iter()
                        .map(|s| {
                            format!(
                                "- {} (connected: {})",
                                s.name,
                                if s.connected { "yes" } else { "no" }
                            )
                        })
                        .collect();
                    ToolResult::success_with_data(
                        format!("Registered LSP servers ({}):\n{}", status.len(), lines.join("\n")),
                        json!({ "servers": status.iter().map(|s| json!({
                            "name": s.name,
                            "connected": s.connected
                        })).collect::<Vec<_>>() }),
                    )
                }
            }
            "register_server" => {
                let name = params["name"].as_str().unwrap_or("");
                let command = params["command"].as_str().unwrap_or("");
                if name.is_empty() || command.is_empty() {
                    return ToolResult::error(
                        "name and command are required for register_server".to_string(),
                    );
                }
                let args: Vec<String> = params["args"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let root_uri = params["root_uri"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        format!(
                            "file://{}",
                            context.working_dir.canonicalize()
                                .unwrap_or_else(|_| context.working_dir.clone())
                                .display()
                        )
                    });

                let config = crate::engine::lsp::LspServerConfig {
                    name: name.to_string(),
                    command: command.to_string(),
                    args,
                    root_uri,
                };

                // 需要可变引用，但 ToolContext 不提供
                // 这里返回配置，让用户手动注册
                ToolResult::success_with_data(
                    format!(
                        "LSP server config created: {} ({}).\nNote: Dynamic registration requires mutable access to LspManager.",
                        name, command
                    ),
                    json!({
                        "name": config.name,
                        "command": config.command,
                        "args": config.args,
                        "root_uri": config.root_uri
                    }),
                )
            }
            "unregister_server" => {
                let name = params["name"].as_str().unwrap_or("");
                if name.is_empty() {
                    return ToolResult::error("name is required for unregister_server".to_string());
                }
                if lsp_manager.is_registered(name) {
                    // 同样需要可变引用
                    ToolResult::success(format!(
                        "Server '{}' is registered. Use LspManager::unregister_server to remove.",
                        name
                    ))
                } else {
                    ToolResult::error(format!("Server '{}' is not registered", name))
                }
            }
            _ => ToolResult::error(format!("Unknown LSP action: {}", action)),
        }
    }
}

/// 从 MarkedString / MarkupContent 提取纯文本
fn extract_marked_string(content: serde_json::Value) -> String {
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    if let Some(obj) = content.as_object() {
        if let (Some(language), Some(value)) = (
            obj.get("language").and_then(|v| v.as_str()),
            obj.get("value").and_then(|v| v.as_str()),
        ) {
            return format!("```{language}\n{}\n```", value);
        }
        if let Some(value) = obj.get("value").and_then(|v| v.as_str()) {
            return value.to_string();
        }
    }
    if let Some(arr) = content.as_array() {
        return arr
            .iter()
            .map(|v| extract_marked_string(v.clone()))
            .collect::<Vec<_>>()
            .join("\n---\n");
    }
    content.to_string()
}

/// 格式化 LSP Location / LocationLink 数组为可读文本
fn format_locations(value: &serde_json::Value) -> String {
    if let Some(arr) = value.as_array() {
        if arr.is_empty() {
            return "No locations found.".to_string();
        }
        let mut lines = vec![format!("Found {} location(s):", arr.len())];
        for (i, item) in arr.iter().enumerate() {
            let uri = item["uri"]
                .as_str()
                .or_else(|| item["targetUri"].as_str())
                .unwrap_or("unknown");
            let range = item["range"]
                .as_object()
                .or_else(|| item["targetRange"].as_object());
            let line = range
                .and_then(|r| r["start"]["line"].as_u64())
                .map(|l| l + 1)
                .unwrap_or(0);
            let character = range
                .and_then(|r| r["start"]["character"].as_u64())
                .map(|c| c + 1)
                .unwrap_or(0);
            let path = crate::engine::lsp::uri_to_path(uri);
            lines.push(format!(
                "  {}. {}:{}:{}",
                i + 1,
                path.display(),
                line,
                character
            ));
        }
        lines.join("\n")
    } else {
        "No locations found.".to_string()
    }
}

/// 格式化 SymbolInformation / DocumentSymbol 数组为可读文本
fn format_symbols(value: &serde_json::Value) -> String {
    if let Some(arr) = value.as_array() {
        if arr.is_empty() {
            return "No symbols found.".to_string();
        }
        let mut lines = vec![format!("Found {} symbol(s):", arr.len())];
        for item in arr {
            let name = item["name"].as_str().unwrap_or("unknown");
            let kind = item["kind"]
                .as_u64()
                .map(symbol_kind_name)
                .unwrap_or("Unknown");
            let detail = item["detail"].as_str().unwrap_or("");
            let loc = if let Some(uri) = item["location"]["uri"].as_str() {
                let line = item["location"]["range"]["start"]["line"]
                    .as_u64()
                    .map(|l| l + 1)
                    .unwrap_or(0);
                format!(
                    " ({}:{})",
                    crate::engine::lsp::uri_to_path(uri).display(),
                    line
                )
            } else {
                String::new()
            };
            if detail.is_empty() {
                lines.push(format!("  [{}] {}{}", kind, name, loc));
            } else {
                lines.push(format!("  [{}] {} - {}{}", kind, name, detail, loc));
            }
        }
        lines.join("\n")
    } else {
        "No symbols found.".to_string()
    }
}

/// SymbolKind 编号转名称
fn symbol_kind_name(kind: u64) -> &'static str {
    match kind {
        1 => "File",
        2 => "Module",
        3 => "Namespace",
        4 => "Package",
        5 => "Class",
        6 => "Method",
        7 => "Property",
        8 => "Field",
        9 => "Constructor",
        10 => "Enum",
        11 => "Interface",
        12 => "Function",
        13 => "Variable",
        14 => "Constant",
        15 => "String",
        16 => "Number",
        17 => "Boolean",
        18 => "Array",
        19 => "Object",
        20 => "Key",
        21 => "Null",
        22 => "EnumMember",
        23 => "Struct",
        24 => "Event",
        25 => "Operator",
        26 => "TypeParameter",
        _ => "Unknown",
    }
}

/// 格式化 CallHierarchyItem 数组为可读文本
fn format_call_hierarchy_items(value: &serde_json::Value) -> String {
    if let Some(arr) = value.as_array() {
        if arr.is_empty() {
            return "No call hierarchy items found.".to_string();
        }
        let mut lines = vec![format!("Found {} call hierarchy item(s):", arr.len())];
        for (i, item) in arr.iter().enumerate() {
            let name = item["name"].as_str().unwrap_or("unknown");
            let kind = item["kind"]
                .as_u64()
                .map(symbol_kind_name)
                .unwrap_or("Unknown");
            let uri = item["uri"].as_str().unwrap_or("unknown");
            let range = &item["range"];
            let start_line = range["start"]["line"].as_u64().map(|l| l + 1).unwrap_or(0);
            let start_char = range["start"]["character"].as_u64().map(|c| c + 1).unwrap_or(0);
            let path = crate::engine::lsp::uri_to_path(uri);
            lines.push(format!(
                "  {}. [{}] {} at {}:{}:{}",
                i + 1,
                kind,
                name,
                path.display(),
                start_line,
                start_char
            ));
        }
        lines.join("\n")
    } else {
        "No call hierarchy items found.".to_string()
    }
}

/// 格式化 IncomingCall 数组为可读文本
fn format_incoming_calls(value: &serde_json::Value) -> String {
    if let Some(arr) = value.as_array() {
        if arr.is_empty() {
            return "No incoming calls found.".to_string();
        }
        let mut lines = vec![format!("Found {} incoming call(s):", arr.len())];
        for (i, call) in arr.iter().enumerate() {
            let from = &call["from"];
            let from_name = from["name"].as_str().unwrap_or("unknown");
            let from_kind = from["kind"]
                .as_u64()
                .map(symbol_kind_name)
                .unwrap_or("Unknown");
            let from_uri = from["uri"].as_str().unwrap_or("unknown");
            let from_path = crate::engine::lsp::uri_to_path(from_uri);
            let from_range = &call["fromRanges"];
            let ranges = from_range.as_array().map(|r| r.len()).unwrap_or(0);
            lines.push(format!(
                "  {}. [{}] {} at {} ({} occurrence(s))",
                i + 1,
                from_kind,
                from_name,
                from_path.display(),
                ranges
            ));
        }
        lines.join("\n")
    } else {
        "No incoming calls found.".to_string()
    }
}

/// 格式化 OutgoingCall 数组为可读文本
fn format_outgoing_calls(value: &serde_json::Value) -> String {
    if let Some(arr) = value.as_array() {
        if arr.is_empty() {
            return "No outgoing calls found.".to_string();
        }
        let mut lines = vec![format!("Found {} outgoing call(s):", arr.len())];
        for (i, call) in arr.iter().enumerate() {
            let to = &call["to"];
            let to_name = to["name"].as_str().unwrap_or("unknown");
            let to_kind = to["kind"]
                .as_u64()
                .map(symbol_kind_name)
                .unwrap_or("Unknown");
            let to_uri = to["uri"].as_str().unwrap_or("unknown");
            let to_path = crate::engine::lsp::uri_to_path(to_uri);
            let ranges = call["fromRanges"].as_array().map(|r| r.len()).unwrap_or(0);
            lines.push(format!(
                "  {}. [{}] {} at {} ({} occurrence(s))",
                i + 1,
                to_kind,
                to_name,
                to_path.display(),
                ranges
            ));
        }
        lines.join("\n")
    } else {
        "No outgoing calls found.".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_marked_string() {
        assert_eq!(extract_marked_string(json!("hello")), "hello");
        assert_eq!(
            extract_marked_string(json!({ "language": "rust", "value": "fn main()" })),
            "```rust\nfn main()\n```"
        );
        assert_eq!(
            extract_marked_string(json!({ "kind": "markdown", "value": "**bold**" })),
            "**bold**"
        );
    }

    #[test]
    fn test_format_locations() {
        let locs = json!([
            { "uri": "file:///src/main.rs", "range": { "start": { "line": 9, "character": 3 } } }
        ]);
        let text = format_locations(&locs);
        assert!(text.contains("src/main.rs"));
        assert!(text.contains("10:4"));
    }

    #[test]
    fn test_format_symbols() {
        let syms = json!([
            { "name": "main", "kind": 12, "detail": "entry point" }
        ]);
        let text = format_symbols(&syms);
        assert!(text.contains("Function"));
        assert!(text.contains("main"));
    }
}
