//! 重构工具
//!
//! 基于 AST 符号索引提供语义级重构能力：
//! - rename: 重命名符号（利用 symbol_index 确认存在后执行精确替换）
//! - extract_function: 提取行范围为新函数
//! - add_impl_method: 给结构体 impl 块添加方法

use crate::engine::symbol_index::SymbolIndex;
use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

pub struct RefactorTool;

#[async_trait]
impl Tool for RefactorTool {
    fn name(&self) -> &str {
        "refactor"
    }

    fn description(&self) -> &str {
        "Semantic refactoring tool based on AST symbol index. \
         Actions: 'rename' (rename symbol with existence check), \
         'extract_function' (extract line range to function), \
         'add_impl_method' (add method to struct impl block)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["rename", "extract_function", "add_impl_method"],
                    "description": "Refactoring action"
                },
                "file_path": {
                    "type": "string",
                    "description": "Target file path"
                },
                "symbol_name": {
                    "type": "string",
                    "description": "For 'rename': current symbol name"
                },
                "new_name": {
                    "type": "string",
                    "description": "For 'rename': new symbol name"
                },
                "line_start": {
                    "type": "number",
                    "description": "For 'extract_function': 1-indexed start line"
                },
                "line_end": {
                    "type": "number",
                    "description": "For 'extract_function': 1-indexed end line"
                },
                "function_name": {
                    "type": "string",
                    "description": "For 'extract_function'/'add_impl_method': name of new function"
                },
                "struct_name": {
                    "type": "string",
                    "description": "For 'add_impl_method': target struct name"
                },
                "method_body": {
                    "type": "string",
                    "description": "For 'add_impl_method': method body (without fn signature)"
                },
                "scope": {
                    "type": "string",
                    "enum": ["file", "project"],
                    "description": "For 'rename': replacement scope (default: file)"
                }
            },
            "required": ["action", "file_path"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");
        let file_path = params["file_path"].as_str().unwrap_or("");

        if action.is_empty() || file_path.is_empty() {
            return ToolResult::error("action and file_path are required");
        }

        let path = crate::tools::file_tool::resolve_path(file_path, &context.working_dir)
            .unwrap_or_else(|_| context.working_dir.join(file_path));

        match action {
            "rename" => do_rename(&params, &path, &context),
            "extract_function" => do_extract_function(&params, &path),
            "add_impl_method" => do_add_impl_method(&params, &path),
            _ => ToolResult::error(format!("Unknown action: {}", action)),
        }
    }
}

// ────────────────────────────────────────────────
// rename
// ────────────────────────────────────────────────

fn do_rename(
    params: &serde_json::Value,
    path: &std::path::Path,
    context: &ToolContext,
) -> ToolResult {
    let symbol_name = params["symbol_name"].as_str().unwrap_or("");
    let new_name = params["new_name"].as_str().unwrap_or("");
    let scope = params["scope"].as_str().unwrap_or("file");

    if symbol_name.is_empty() || new_name.is_empty() {
        return ToolResult::error("symbol_name and new_name are required for rename");
    }

    // 1. 用 symbol_index 确认符号存在
    let mut index = SymbolIndex::new();
    index.index_project(&context.working_dir);

    let symbols = index.find_exact(symbol_name);
    if symbols.is_empty() {
        return ToolResult::error_with_content(
            format!("Symbol '{}' not found in project index.", symbol_name),
            "Use symbol_query to search for the correct name before renaming.".to_string(),
        );
    }

    // 2. 确定替换范围
    let target_files: Vec<std::path::PathBuf> = if scope == "project" {
        // 全局：所有 .rs 文件
        let mut files = Vec::new();
        collect_rs_files(&context.working_dir, &mut files);
        files
    } else {
        // 文件级：只替换指定文件
        vec![path.to_path_buf()]
    };

    let mut total_replacements = 0;
    let mut modified_files = Vec::new();

    for file in &target_files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // 精确词级别替换，避免误替换子串
        let new_content = replace_word(&content, symbol_name, new_name);

        if new_content != content {
            if let Err(e) = std::fs::write(file, &new_content) {
                return ToolResult::error(format!(
                    "Failed to write {}: {}",
                    file.display(),
                    e
                ));
            }
            let count = count_replacements(&content, symbol_name, new_name);
            total_replacements += count;
            modified_files.push(format!("{} ({} replacements)", file.display(), count));
        }
    }

    if total_replacements == 0 {
        return ToolResult::success_with_data(
            format!(
                "Symbol '{}' was found in index but no occurrences were replaced in {}. \
                 It may only appear in other files.",
                symbol_name,
                if scope == "project" {
                    "the project"
                } else {
                    "the target file"
                }
            ),
            json!({
                "symbol": symbol_name,
                "new_name": new_name,
                "scope": scope,
                "replacements": 0
            }),
        );
    }

    ToolResult::success_with_data(
        format!(
            "Renamed '{}' -> '{}' in {} file(s), {} total replacement(s):\n{}",
            symbol_name,
            new_name,
            modified_files.len(),
            total_replacements,
            modified_files.join("\n")
        ),
        json!({
            "symbol": symbol_name,
            "new_name": new_name,
            "scope": scope,
            "replacements": total_replacements,
            "files": modified_files
        }),
    )
}

/// 词级别精确替换：只替换完整的词，不替换子串
fn replace_word(content: &str, old: &str, new: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut i = 0;
    let bytes = content.as_bytes();

    while i < content.len() {
        // 检查是否匹配 old（作为完整词）
        if is_word_at(bytes, i, old) {
            result.push_str(new);
            i += old.len();
        } else {
            result.push(content[i..].chars().next().unwrap());
            i += content[i..].chars().next().unwrap().len_utf8();
        }
    }

    result
}

fn is_word_at(bytes: &[u8], pos: usize, word: &str) -> bool {
    let word_bytes = word.as_bytes();
    if pos + word_bytes.len() > bytes.len() {
        return false;
    }
    if &bytes[pos..pos + word_bytes.len()] != word_bytes {
        return false;
    }
    // 检查前后不是词字符
    let is_word_char = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let before_ok = pos == 0 || !is_word_char(bytes[pos - 1]);
    let after_ok = pos + word_bytes.len() == bytes.len()
        || !is_word_char(bytes[pos + word_bytes.len()]);
    before_ok && after_ok
}

fn count_replacements(content: &str, old: &str, new: &str) -> usize {
    let original = content.len();
    let replaced = replace_word(content, old, new).len();
    // 粗略估计：每次替换长度变化为 new.len() - old.len()
    let delta = new.len() as isize - old.len() as isize;
    if delta == 0 {
        // 无法通过长度差计算，直接扫描
        let mut count = 0;
        let bytes = content.as_bytes();
        let mut i = 0;
        while i < content.len() {
            if is_word_at(bytes, i, old) {
                count += 1;
                i += old.len();
            } else {
                i += content[i..].chars().next().unwrap().len_utf8();
            }
        }
        count
    } else {
        ((replaced as isize - original as isize) / delta).max(0) as usize
    }
}

fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !["target", "node_modules", ".git"].contains(&name) {
                collect_rs_files(&path, out);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

// ────────────────────────────────────────────────
// extract_function
// ────────────────────────────────────────────────

fn do_extract_function(params: &serde_json::Value, path: &std::path::Path) -> ToolResult {
    let line_start = params["line_start"].as_u64().unwrap_or(0) as usize;
    let line_end = params["line_end"].as_u64().unwrap_or(0) as usize;
    let function_name = params["function_name"].as_str().unwrap_or("");

    if line_start == 0 || line_end == 0 || line_start > line_end {
        return ToolResult::error("Invalid line_start/line_end for extract_function");
    }
    if function_name.is_empty() {
        return ToolResult::error("function_name is required for extract_function");
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return ToolResult::error(format!("Failed to read file: {}", e)),
    };

    // 提取指定行范围
    let lines: Vec<&str> = content.lines().collect();
    if line_end > lines.len() {
        return ToolResult::error(format!(
            "line_end ({}) exceeds file length ({})",
            line_end,
            lines.len()
        ));
    }

    let extracted_lines = &lines[line_start - 1..line_end];
    let body = extracted_lines.join("\n");

    // 分析使用了哪些外部变量（简单启发式）
    let declared = extract_declared_vars(extracted_lines);
    let used = extract_used_vars(extracted_lines);
    let params_needed: Vec<String> = used
        .difference(&declared)
        .cloned()
        .collect();

    // 构建新函数
    let param_list = if params_needed.is_empty() {
        String::new()
    } else {
        format!(
            "({})",
            params_needed
                .iter()
                .map(|v| format!("{}: /* TODO: add type */", v))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    let new_function = format!(
        "fn {}{} {{\n{}\n}}\n",
        function_name,
        param_list,
        body
    );

    // 在原位置替换为函数调用
    let call_expr = if params_needed.is_empty() {
        format!("{}();", function_name)
    } else {
        format!(
            "{}({});",
            function_name,
            params_needed.join(", ")
        )
    };

    let mut new_lines = lines[..line_start - 1].to_vec();
    new_lines.push(&call_expr);
    new_lines.extend_from_slice(&lines[line_end..]);

    let mut new_content = new_lines.join("\n");
    if content.ends_with('\n') && !new_content.ends_with('\n') {
        new_content.push('\n');
    }

    // 找到合适位置插入新函数（文件末尾或最后一个函数之后）
    new_content.push('\n');
    new_content.push_str(&new_function);

    if let Err(e) = std::fs::write(path, &new_content) {
        return ToolResult::error(format!("Failed to write file: {}", e));
    }

    ToolResult::success_with_data(
        format!(
            "Extracted lines {}-{} into function '{}'. \
             Parameters detected: {:?}. \
             Please review the generated function signature and add proper types.",
            line_start, line_end, function_name, params_needed
        ),
        json!({
            "function_name": function_name,
            "line_start": line_start,
            "line_end": line_end,
            "parameters": params_needed,
            "body": body
        }),
    )
}

/// 简单启发式：从 let/const 声明中提取变量名
fn extract_declared_vars(lines: &[&str]) -> std::collections::HashSet<String> {
    let mut vars = std::collections::HashSet::new();
    for line in lines {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("let ") {
            // let x = ... 或 let mut x = ...
            let name_part = rest.trim_start_matches("mut ").trim();
            if let Some(pos) = name_part.find([':', '=', ' ']) {
                let name = name_part[..pos].trim();
                if !name.is_empty() {
                    vars.insert(name.to_string());
                }
            }
        }
    }
    vars
}

/// 简单启发式：提取使用的标识符（排除关键字和已声明变量）
fn extract_used_vars(lines: &[&str]) -> std::collections::HashSet<String> {
    let mut vars = std::collections::HashSet::new();
    let keywords: std::collections::HashSet<&str> = [
        "fn", "let", "mut", "if", "else", "match", "for", "while", "loop",
        "return", "break", "continue", "use", "mod", "pub", "struct", "enum",
        "trait", "impl", "type", "const", "static", "async", "await", "move",
        "where", "ref", "self", "Self", "true", "false", "None", "Some", "Ok",
        "Err", "println", "format", "vec", "String", "str", "i32", "u32", "i64",
        "u64", "f32", "f64", "bool", "char", "usize", "isize",
    ]
    .iter()
    .copied()
    .collect();

    for line in lines {
        // 简单的标识符提取：连续的字母数字下划线
        let mut current = String::new();
        for ch in line.chars() {
            if ch.is_alphanumeric() || ch == '_' {
                current.push(ch);
            } else {
                if !current.is_empty()
                    && !current.starts_with(|c: char| c.is_ascii_digit())
                    && !keywords.contains(current.as_str())
                    && !current.is_empty()
                {
                    vars.insert(current.clone());
                }
                current.clear();
            }
        }
        if !current.is_empty()
            && !current.starts_with(|c: char| c.is_ascii_digit())
            && !keywords.contains(current.as_str())
            && current.len() > 1
        {
            vars.insert(current);
        }
    }
    vars
}

// ────────────────────────────────────────────────
// add_impl_method
// ────────────────────────────────────────────────

fn do_add_impl_method(params: &serde_json::Value, path: &std::path::Path) -> ToolResult {
    let struct_name = params["struct_name"].as_str().unwrap_or("");
    let function_name = params["function_name"].as_str().unwrap_or("");
    let method_body = params["method_body"].as_str().unwrap_or("");

    if struct_name.is_empty() || function_name.is_empty() {
        return ToolResult::error("struct_name and function_name are required for add_impl_method");
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return ToolResult::error(format!("Failed to read file: {}", e)),
    };

    // 查找 struct 的 impl 块
    let impl_pattern = format!("impl {}", struct_name);
    let mut impl_end = None;

    if let Some(pos) = content.find(&impl_pattern) {
        // 找到 impl 块的开头大括号
        let after_impl = &content[pos..];
        if let Some(open_brace) = after_impl.find('{') {
            let brace_start = pos + open_brace;
            // 找到匹配的闭合大括号
            let mut depth = 1;
            for (idx, ch) in content[brace_start + 1..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
                if depth == 0 {
                    impl_end = Some(brace_start + 1 + idx);
                    break;
                }
            }
        }
    }

    let method_code = format!(
        "\n    pub fn {}() {{\n        {}\n    }}\n",
        function_name,
        method_body
    );

    let new_content = if let Some(end) = impl_end {
        // 在 impl 块结束前插入新方法
        let before = &content[..end];
        let after = &content[end..];
        format!("{}{}{}", before, method_code, after)
    } else {
        // 没有找到 impl 块，在文件末尾创建新 impl 块
        let mut new = content;
        if !new.ends_with('\n') {
            new.push('\n');
        }
        new.push_str(&format!("impl {} {{{}}}\n", struct_name, method_code));
        new
    };

    if let Err(e) = std::fs::write(path, &new_content) {
        return ToolResult::error(format!("Failed to write file: {}", e));
    }

    ToolResult::success_with_data(
        format!(
            "Added method '{}' to impl block of '{}'.",
            function_name, struct_name
        ),
        json!({
            "struct_name": struct_name,
            "function_name": function_name,
            "created_new_impl": impl_end.is_none()
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_word() {
        let content = "let foo = bar.foo(); let foobar = 1;";
        let result = replace_word(content, "foo", "baz");
        assert_eq!(result, "let baz = bar.baz(); let foobar = 1;");
    }

    #[test]
    fn test_replace_word_no_substring() {
        let content = "foobar foo_bar";
        let result = replace_word(content, "foo", "x");
        // foobar 中 foo 后面是 b（词字符），不替换
        // foo_bar 中 foo 后面是 _（词字符），不替换
        assert_eq!(result, "foobar foo_bar");
    }

    #[test]
    fn test_count_replacements() {
        let content = "foo bar foo baz foo";
        assert_eq!(count_replacements(content, "foo", "x"), 3);
    }

    #[test]
    fn test_extract_declared_vars() {
        let lines = ["let x = 1;", "let mut y = 2;", "z = 3;"];
        let vars = extract_declared_vars(&lines);
        assert!(vars.contains("x"));
        assert!(vars.contains("y"));
        assert!(!vars.contains("z"));
    }

    #[test]
    fn test_extract_used_vars() {
        let lines = ["let x = a + b;", "println!(\"{}\", c);"];
        let vars = extract_used_vars(&lines);
        assert!(vars.contains("a"));
        assert!(vars.contains("b"));
        assert!(vars.contains("c"));
        assert!(!vars.contains("let"));
        assert!(!vars.contains("println"));
    }
}
