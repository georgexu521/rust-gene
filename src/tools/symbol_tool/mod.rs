//! 符号查询工具
//!
//! 利用项目级 AST 符号索引，查询代码库中的函数、结构体、枚举等定义。

use crate::engine::symbol_index::{SymbolIndex, SymbolKind};
use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct SymbolQueryTool;

#[async_trait]
impl Tool for SymbolQueryTool {
    fn name(&self) -> &str {
        "symbol_query"
    }

    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        ToolOperationKind::Search
    }

    fn description(&self) -> &str {
        "Query the codebase symbol index using tree-sitter — find where functions, \
         structs, enums, traits, and modules are defined. Returns 1-based line/column \
         positions with parent nesting. \
         \
         Actions: 'search' (fuzzy name lookup — 'where is X defined'), \
         'list_file' (all symbols in a single file), 'list_kind' (all symbols \
         of a given type across the project, e.g. 'all traits'). \
         Grammar-aware — ignores names inside comments and strings. \
         For cross-file reference tracing use grep; for file name search use glob."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["search", "list_file", "list_kind"],
                    "description": "Query action"
                },
                "query": {
                    "type": "string",
                    "description": "Required for 'search' action. Fuzzy symbol name to search."
                },
                "file_path": {
                    "type": "string",
                    "description": "Required for 'list_file' action. Path to the source file."
                },
                "kind": {
                    "type": "string",
                    "enum": ["function", "struct", "enum", "trait", "impl", "module"],
                    "description": "Required for 'list_kind' action. Symbol type to list."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");
        if action.is_empty() {
            return ToolResult::error("action is required");
        }

        let mut index = SymbolIndex::new();
        index.index_project(&context.working_dir);

        if index.is_empty() {
            return ToolResult::success_with_data(
                "No symbols indexed. This may be a non-Rust project or the source files are empty."
                    .to_string(),
                json!({ "count": 0 }),
            );
        }

        match action {
            "search" => {
                let query = params["query"].as_str().unwrap_or("");
                if query.is_empty() {
                    return ToolResult::error("query is required for search action");
                }
                let symbols = index.find_by_name(query);
                if symbols.is_empty() {
                    return ToolResult::success_with_data(
                        format!("No symbols matching '{}' found.", query),
                        json!({ "query": query, "count": 0, "symbols": [] }),
                    );
                }
                let lines: Vec<String> = symbols
                    .iter()
                    .map(|s| {
                        let sig = s.signature.as_deref().unwrap_or("");
                        format!(
                            "- {} [{}] {}:{} | {}",
                            s.name,
                            kind_to_str(&s.kind),
                            s.file.display(),
                            s.line + 1,
                            if sig.len() > 80 {
                                format!("{}...", &sig[..80])
                            } else {
                                sig.to_string()
                            }
                        )
                    })
                    .collect();
                ToolResult::success_with_data(
                    format!(
                        "Found {} symbol(s) matching '{}':\n{}",
                        symbols.len(),
                        query,
                        lines.join("\n")
                    ),
                    json!({
                        "query": query,
                        "count": symbols.len(),
                        "symbols": symbols.iter().map(|s| json!({
                            "name": s.name,
                            "kind": kind_to_str(&s.kind),
                            "file": s.file.to_string_lossy().to_string(),
                            "line": s.line + 1,
                            "signature": s.signature
                        })).collect::<Vec<_>>()
                    }),
                )
            }
            "list_file" => {
                let file_path = params["file_path"].as_str().unwrap_or("");
                if file_path.is_empty() {
                    return ToolResult::error("file_path is required for list_file action");
                }
                let path = crate::tools::file_tool::resolve_path(file_path, &context.working_dir)
                    .unwrap_or_else(|_| context.working_dir.join(file_path));
                let symbols = index.symbols_in_file(&path);
                if symbols.is_empty() {
                    return ToolResult::success_with_data(
                        format!("No symbols found in {}.", file_path),
                        json!({ "file": file_path, "count": 0, "symbols": [] }),
                    );
                }
                let lines: Vec<String> = symbols
                    .iter()
                    .map(|s| {
                        format!(
                            "- {} [{}] line {}",
                            s.name,
                            kind_to_str(&s.kind),
                            s.line + 1
                        )
                    })
                    .collect();
                ToolResult::success_with_data(
                    format!(
                        "Symbols in {} ({} total):\n{}",
                        file_path,
                        symbols.len(),
                        lines.join("\n")
                    ),
                    json!({
                        "file": file_path,
                        "count": symbols.len(),
                        "symbols": symbols.iter().map(|s| json!({
                            "name": s.name,
                            "kind": kind_to_str(&s.kind),
                            "line": s.line + 1
                        })).collect::<Vec<_>>()
                    }),
                )
            }
            "list_kind" => {
                let kind_str = params["kind"].as_str().unwrap_or("");
                let kind = match kind_str {
                    "function" => SymbolKind::Function,
                    "struct" => SymbolKind::Struct,
                    "enum" => SymbolKind::Enum,
                    "trait" => SymbolKind::Trait,
                    "impl" => SymbolKind::Impl,
                    "module" => SymbolKind::Module,
                    "class" => SymbolKind::Struct, // TypeScript/Python class
                    "interface" => SymbolKind::Trait, // TypeScript interface
                    "type" => SymbolKind::TypeAlias,
                    _ => {
                        return ToolResult::error(format!(
                            "Unknown kind: {}. Valid kinds: function, struct, enum, trait, impl, module, class, interface, type",
                            kind_str
                        ));
                    }
                };
                let symbols = index.find_by_kind(kind);
                if symbols.is_empty() {
                    return ToolResult::success_with_data(
                        format!("No {} symbols found in the project.", kind_str),
                        json!({ "kind": kind_str, "count": 0, "symbols": [] }),
                    );
                }
                let lines: Vec<String> = symbols
                    .iter()
                    .map(|s| format!("- {} in {}:{}", s.name, s.file.display(), s.line + 1))
                    .collect();
                ToolResult::success_with_data(
                    format!(
                        "Found {} {} symbol(s):\n{}",
                        symbols.len(),
                        kind_str,
                        lines.join("\n")
                    ),
                    json!({
                        "kind": kind_str,
                        "count": symbols.len(),
                        "symbols": symbols.iter().map(|s| json!({
                            "name": s.name,
                            "file": s.file.to_string_lossy().to_string(),
                            "line": s.line + 1
                        })).collect::<Vec<_>>()
                    }),
                )
            }
            _ => ToolResult::error(format!("Unknown action: {}", action)),
        }
    }
}

fn kind_to_str(kind: &SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "fn",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Trait => "trait",
        SymbolKind::Impl => "impl",
        SymbolKind::Module => "mod",
        SymbolKind::Variable => "var",
        SymbolKind::TypeAlias => "type",
        SymbolKind::Macro => "macro",
        SymbolKind::Unknown => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_symbol_query_search() {
        let tmp = std::env::temp_dir().join(format!(
            "priority-agent-symbol-tool-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("main.rs"), "fn hello() {}\nfn world() {}\n").unwrap();

        let tool = SymbolQueryTool;
        let ctx = ToolContext::new(&tmp, "s1");
        let res = tool
            .execute(json!({"action":"search","query":"hello"}), ctx)
            .await;
        assert!(res.success, "{:?}", res.error);
        assert!(res.content.contains("hello"));

        let _ = std::fs::remove_dir_all(tmp);
    }
}
