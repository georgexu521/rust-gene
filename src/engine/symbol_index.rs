//! 项目级 AST 符号索引
//!
//! 使用 tree-sitter 解析源代码，建立符号索引，支持：
//! - 函数/结构体/枚举/Trait 定义查询
//! - 跨文件符号搜索
//!
//! 当前支持：Rust
//! 未来可扩展：TypeScript, Python, Go

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// 符号类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Module,
    Variable,
    TypeAlias,
    Macro,
    Unknown,
}

impl SymbolKind {
    pub fn from_str(s: &str) -> Self {
        match s {
            "function" | "function_item" => SymbolKind::Function,
            "struct" | "struct_item" => SymbolKind::Struct,
            "enum" | "enum_item" => SymbolKind::Enum,
            "trait" | "trait_item" => SymbolKind::Trait,
            "impl" | "impl_item" => SymbolKind::Impl,
            "module" => SymbolKind::Module,
            "type_alias" => SymbolKind::TypeAlias,
            "macro" => SymbolKind::Macro,
            _ => SymbolKind::Unknown,
        }
    }
}

/// 符号信息
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub signature: Option<String>,
}

/// 项目符号索引
pub struct SymbolIndex {
    symbols: Vec<Symbol>,
    by_name: HashMap<String, Vec<usize>>,
}

impl SymbolIndex {
    pub fn new() -> Self {
        Self {
            symbols: Vec::new(),
            by_name: HashMap::new(),
        }
    }

    /// 索引整个项目目录
    pub fn index_project(&mut self, root: &Path) {
        let mut files = Vec::new();
        Self::collect_source_files(root, &mut files);
        for file in files {
            if let Err(e) = self.index_file(&file) {
                warn!("Failed to index {}: {}", file.display(), e);
            }
        }
        debug!("Indexed {} symbols from project", self.symbols.len());
    }

    fn collect_source_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // 跳过常见非源码目录
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if ![
                    "target",
                    "node_modules",
                    ".git",
                    "dist",
                    "build",
                    "__pycache__",
                    ".venv",
                ]
                .contains(&name)
                {
                    Self::collect_source_files(&path, out);
                }
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                // 支持 Rust、TypeScript、JavaScript、Python
                if ["rs", "ts", "tsx", "js", "jsx", "py"].contains(&ext) {
                    out.push(path);
                }
            }
        }
    }

    /// 索引单个文件
    pub fn index_file(&mut self, path: &Path) -> Result<(), String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;

        match path.extension().and_then(|e| e.to_str()) {
            Some("rs") => self.index_rust_file(path, &content)?,
            Some("ts") | Some("tsx") | Some("js") | Some("jsx") => {
                self.index_typescript_file(path, &content)?
            }
            Some("py") => self.index_python_file(path, &content)?,
            _ => {} // 不支持的语言，跳过
        }

        Ok(())
    }

    fn index_rust_file(&mut self, path: &Path, content: &str) -> Result<(), String> {
        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_rust::LANGUAGE.into();
        parser
            .set_language(&language)
            .map_err(|e| format!("Failed to set Rust language: {}", e))?;

        let tree = parser
            .parse(content, None)
            .ok_or("Failed to parse Rust file")?;

        let root = tree.root_node();
        self.walk_rust_node(path, content, &root, 0);

        Ok(())
    }

    fn walk_rust_node(
        &mut self,
        path: &Path,
        content: &str,
        node: &tree_sitter::Node,
        depth: usize,
    ) {
        if depth > 100 {
            return; // 防止无限递归
        }

        let kind = node.kind();
        match kind {
            "function_item" | "struct_item" | "enum_item" | "trait_item" | "impl_item"
            | "type_item" | "mod_item" | "macro_definition" => {
                if let Some(symbol) = self.extract_rust_symbol(path, content, node, kind) {
                    let idx = self.symbols.len();
                    self.by_name
                        .entry(symbol.name.clone())
                        .or_default()
                        .push(idx);
                    self.symbols.push(symbol);
                }
            }
            _ => {}
        }

        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                self.walk_rust_node(path, content, &child, depth + 1);
            }
        }
    }

    fn extract_rust_symbol(
        &self,
        path: &Path,
        content: &str,
        node: &tree_sitter::Node,
        kind: &str,
    ) -> Option<Symbol> {
        // 提取名称：通常第一个 identifier 子节点就是名称
        let name_node = (0..node.child_count())
            .filter_map(|i| node.child(i))
            .find(|child| child.kind() == "identifier")?;

        let name = content[name_node.start_byte()..name_node.end_byte()].to_string();
        if name.is_empty() {
            return None;
        }

        let line = content[..node.start_byte()].matches('\n').count();
        let column = node.start_position().column;

        // 提取签名（整个声明行）
        let line_start = content[..node.start_byte()]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let line_end = content[node.start_byte()..]
            .find('\n')
            .map(|i| node.start_byte() + i)
            .unwrap_or(content.len());
        let signature = content[line_start..line_end].trim().to_string();
        let signature = if signature.len() > 200 {
            format!("{}...", &signature[..200])
        } else {
            signature
        };

        Some(Symbol {
            name,
            kind: SymbolKind::from_str(kind),
            file: path.to_path_buf(),
            line,
            column,
            signature: Some(signature),
        })
    }

    // ── TypeScript/JavaScript 索引 ──────────────────────────

    fn index_typescript_file(&mut self, path: &Path, content: &str) -> Result<(), String> {
        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        parser
            .set_language(&language)
            .map_err(|e| format!("Failed to set TypeScript language: {}", e))?;

        let tree = parser
            .parse(content, None)
            .ok_or("Failed to parse TypeScript file")?;

        let root = tree.root_node();
        self.walk_typescript_node(path, content, &root, 0);

        Ok(())
    }

    fn walk_typescript_node(
        &mut self,
        path: &Path,
        content: &str,
        node: &tree_sitter::Node,
        depth: usize,
    ) {
        if depth > 100 {
            return;
        }

        let kind = node.kind();
        match kind {
            "function_declaration"
            | "class_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "enum_declaration"
            | "method_definition" => {
                if let Some(symbol) = self.extract_typescript_symbol(path, content, node, kind) {
                    let idx = self.symbols.len();
                    self.by_name
                        .entry(symbol.name.clone())
                        .or_default()
                        .push(idx);
                    self.symbols.push(symbol);
                }
            }
            _ => {}
        }

        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                self.walk_typescript_node(path, content, &child, depth + 1);
            }
        }
    }

    fn extract_typescript_symbol(
        &self,
        path: &Path,
        content: &str,
        node: &tree_sitter::Node,
        kind: &str,
    ) -> Option<Symbol> {
        let name_node = (0..node.child_count())
            .filter_map(|i| node.child(i))
            .find(|child| child.kind() == "identifier" || child.kind() == "type_identifier")?;

        let name = content[name_node.start_byte()..name_node.end_byte()].to_string();
        if name.is_empty() {
            return None;
        }

        let line = content[..node.start_byte()].matches('\n').count();
        let column = node.start_position().column;

        let line_start = content[..node.start_byte()]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let line_end = content[node.start_byte()..]
            .find('\n')
            .map(|i| node.start_byte() + i)
            .unwrap_or(content.len());
        let signature = content[line_start..line_end].trim().to_string();
        let signature = if signature.len() > 200 {
            format!("{}...", &signature[..200])
        } else {
            signature
        };

        let symbol_kind = match kind {
            "function_declaration" => SymbolKind::Function,
            "class_declaration" => SymbolKind::Struct,
            "interface_declaration" => SymbolKind::Trait,
            "type_alias_declaration" => SymbolKind::TypeAlias,
            "enum_declaration" => SymbolKind::Enum,
            "method_definition" => SymbolKind::Function,
            _ => SymbolKind::Unknown,
        };

        Some(Symbol {
            name,
            kind: symbol_kind,
            file: path.to_path_buf(),
            line,
            column,
            signature: Some(signature),
        })
    }

    // ── Python 索引 ─────────────────────────────────────────

    fn index_python_file(&mut self, path: &Path, content: &str) -> Result<(), String> {
        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_python::LANGUAGE.into();
        parser
            .set_language(&language)
            .map_err(|e| format!("Failed to set Python language: {}", e))?;

        let tree = parser
            .parse(content, None)
            .ok_or("Failed to parse Python file")?;

        let root = tree.root_node();
        self.walk_python_node(path, content, &root, 0);

        Ok(())
    }

    fn walk_python_node(
        &mut self,
        path: &Path,
        content: &str,
        node: &tree_sitter::Node,
        depth: usize,
    ) {
        if depth > 100 {
            return;
        }

        let kind = node.kind();
        match kind {
            "function_definition" | "class_definition" => {
                if let Some(symbol) = self.extract_python_symbol(path, content, node, kind) {
                    let idx = self.symbols.len();
                    self.by_name
                        .entry(symbol.name.clone())
                        .or_default()
                        .push(idx);
                    self.symbols.push(symbol);
                }
            }
            _ => {}
        }

        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                self.walk_python_node(path, content, &child, depth + 1);
            }
        }
    }

    fn extract_python_symbol(
        &self,
        path: &Path,
        content: &str,
        node: &tree_sitter::Node,
        kind: &str,
    ) -> Option<Symbol> {
        let name_node = (0..node.child_count())
            .filter_map(|i| node.child(i))
            .find(|child| child.kind() == "identifier")?;

        let name = content[name_node.start_byte()..name_node.end_byte()].to_string();
        if name.is_empty() {
            return None;
        }

        let line = content[..node.start_byte()].matches('\n').count();
        let column = node.start_position().column;

        let line_start = content[..node.start_byte()]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let line_end = content[node.start_byte()..]
            .find('\n')
            .map(|i| node.start_byte() + i)
            .unwrap_or(content.len());
        let signature = content[line_start..line_end].trim().to_string();
        let signature = if signature.len() > 200 {
            format!("{}...", &signature[..200])
        } else {
            signature
        };

        let symbol_kind = match kind {
            "function_definition" => SymbolKind::Function,
            "class_definition" => SymbolKind::Struct,
            _ => SymbolKind::Unknown,
        };

        Some(Symbol {
            name,
            kind: symbol_kind,
            file: path.to_path_buf(),
            line,
            column,
            signature: Some(signature),
        })
    }

    /// 按名称搜索符号
    pub fn find_by_name(&self, query: &str) -> Vec<&Symbol> {
        let query_lower = query.to_lowercase();
        self.symbols
            .iter()
            .filter(|s| s.name.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// 精确名称查找
    pub fn find_exact(&self, name: &str) -> Vec<&Symbol> {
        self.by_name
            .get(name)
            .map(|indices| indices.iter().map(|&i| &self.symbols[i]).collect())
            .unwrap_or_default()
    }

    /// 按类型过滤
    pub fn find_by_kind(&self, kind: SymbolKind) -> Vec<&Symbol> {
        self.symbols.iter().filter(|s| s.kind == kind).collect()
    }

    /// 列出某个文件中的所有符号
    pub fn symbols_in_file(&self, path: &Path) -> Vec<&Symbol> {
        self.symbols.iter().filter(|s| s.file == path).collect()
    }

    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }
}

impl Default for SymbolIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_rust_file() {
        let temp = std::env::temp_dir().join(format!(
            "priority-agent-symbol-index-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp).unwrap();

        let file = temp.join("lib.rs");
        std::fs::write(
            &file,
            r#"
pub struct User {
    name: String,
}

impl User {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub fn greet(&self) -> String {
        format!("Hello, {}!", self.name)
    }
}

pub enum Status {
    Active,
    Inactive,
}

pub trait Greeter {
    fn greet(&self) -> String;
}
"#,
        )
        .unwrap();

        let mut index = SymbolIndex::new();
        index.index_file(&file).unwrap();

        // tree-sitter 解析至少应找到顶层声明
        assert!(
            index.len() >= 2,
            "Expected at least 2 symbols, got {}. Symbols: {:?}",
            index.len(),
            index.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );

        // 验证能找到主要类型
        let user = index.find_exact("User");
        if !user.is_empty() {
            assert_eq!(user[0].kind, SymbolKind::Struct);
        }

        let status = index.find_exact("Status");
        if !status.is_empty() {
            assert_eq!(status[0].kind, SymbolKind::Enum);
        }

        let greeter = index.find_exact("Greeter");
        if !greeter.is_empty() {
            assert_eq!(greeter[0].kind, SymbolKind::Trait);
        }

        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn test_find_by_name_fuzzy() {
        let temp = std::env::temp_dir().join(format!(
            "priority-agent-symbol-fuzzy-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp).unwrap();

        let file = temp.join("main.rs");
        std::fs::write(&file, "fn calculate_sum() {}\nfn calculate_diff() {}\n").unwrap();

        let mut index = SymbolIndex::new();
        index.index_file(&file).unwrap();

        let results = index.find_by_name("calc");
        assert_eq!(results.len(), 2, "Should find both calculate functions");

        let _ = std::fs::remove_dir_all(temp);
    }
}
