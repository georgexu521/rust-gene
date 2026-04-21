//! Notebook 工具 - 读取和编辑 Jupyter Notebook (.ipynb)
//!
//! 支持读取、编辑、插入、删除单元格。

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;

/// Jupyter Notebook 单元格
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookCell {
    /// 单元格类型: "code" 或 "markdown"
    pub cell_type: String,
    /// 单元格内容 (每行一个字符串)
    pub source: Vec<String>,
    /// 输出 (仅代码单元格)
    #[serde(default)]
    pub outputs: Vec<serde_json::Value>,
    /// 执行计数 (仅代码单元格)
    pub execution_count: Option<u32>,
    /// 元数据
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Jupyter Notebook 结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notebook {
    /// Notebook 版本
    pub nbformat: u32,
    pub nbformat_minor: u32,
    /// 内核信息
    pub metadata: serde_json::Value,
    /// 单元格列表
    pub cells: Vec<NotebookCell>,
}

/// Notebook 工具
pub struct NotebookTool;

#[async_trait]
impl Tool for NotebookTool {
    fn name(&self) -> &str {
        "notebook"
    }

    fn description(&self) -> &str {
        "Read and edit Jupyter Notebook (.ipynb) files. \
         Actions: 'read' (read all cells), 'read_cell' (read specific cell), \
         'edit_cell' (edit cell content), 'insert_cell' (insert new cell), \
         'delete_cell' (delete a cell)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["read", "read_cell", "edit_cell", "insert_cell", "delete_cell"],
                    "description": "The notebook action to perform"
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to the .ipynb file (required for all actions)"
                },
                "cell_index": {
                    "type": "integer",
                    "description": "0-based cell index (required for read_cell, edit_cell, delete_cell)",
                    "minimum": 0
                },
                "content": {
                    "type": "string",
                    "description": "New cell content (required for edit_cell)"
                },
                "cell_type": {
                    "type": "string",
                    "enum": ["code", "markdown"],
                    "description": "Type of new cell (for insert_cell, default: code)"
                },
                "position": {
                    "type": "integer",
                    "description": "Position to insert new cell (0-based, for insert_cell)",
                    "minimum": 0
                }
            },
            "required": ["action", "file_path"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");
        let file_path = params["file_path"].as_str().unwrap_or("");

        if file_path.is_empty() {
            return ToolResult::error("file_path is required".to_string());
        }

        let path = match crate::tools::file_tool::resolve_path(file_path, &context.working_dir) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(e.to_string()),
        };

        match action {
            "read" => self.read_notebook(&path).await,
            "read_cell" => {
                let cell_index = params["cell_index"].as_u64().unwrap_or(0) as usize;
                self.read_cell(&path, cell_index).await
            }
            "edit_cell" => {
                let cell_index = params["cell_index"].as_u64().unwrap_or(0) as usize;
                let content = params["content"].as_str().unwrap_or("");
                if content.is_empty() {
                    return ToolResult::error("content is required for edit_cell".to_string());
                }
                self.edit_cell(&path, cell_index, content).await
            }
            "insert_cell" => {
                let cell_type = params["cell_type"].as_str().unwrap_or("code");
                let position = params["position"].as_u64().map(|p| p as usize);
                let content = params["content"].as_str().unwrap_or("");
                self.insert_cell(&path, cell_type, position, content).await
            }
            "delete_cell" => {
                let cell_index = params["cell_index"].as_u64().unwrap_or(0) as usize;
                self.delete_cell(&path, cell_index).await
            }
            _ => ToolResult::error(format!("Unknown notebook action: {}", action)),
        }
    }
}

impl NotebookTool {
    /// 读取整个 Notebook
    async fn read_notebook(&self, path: &Path) -> ToolResult {
        match tokio::fs::read_to_string(path).await {
            Ok(content) => match serde_json::from_str::<Notebook>(&content) {
                Ok(notebook) => {
                    let mut lines = vec![format!("Notebook: {} cells", notebook.cells.len())];
                    for (i, cell) in notebook.cells.iter().enumerate() {
                        let cell_type = &cell.cell_type;
                        let preview: String = cell.source.join("");
                        let preview = if preview.len() > 80 {
                            format!("{}...", &preview[..77])
                        } else {
                            preview
                        };
                        lines.push(format!("  [{}] {}: {}", i, cell_type, preview));
                    }
                    ToolResult::success(lines.join("\n"))
                }
                Err(e) => ToolResult::error(format!("Failed to parse notebook: {}", e)),
            },
            Err(e) => ToolResult::error(format!("Failed to read file: {}", e)),
        }
    }

    /// 读取特定单元格
    async fn read_cell(&self, path: &Path, cell_index: usize) -> ToolResult {
        match self.load_notebook(path).await {
            Ok(notebook) => {
                if cell_index >= notebook.cells.len() {
                    return ToolResult::error(format!(
                        "Cell index {} out of range (0-{})",
                        cell_index,
                        notebook.cells.len() - 1
                    ));
                }
                let cell = &notebook.cells[cell_index];
                let content = cell.source.join("");
                ToolResult::success_with_data(
                    content.clone(),
                    json!({
                        "cell_index": cell_index,
                        "cell_type": cell.cell_type,
                        "content": content
                    }),
                )
            }
            Err(e) => e,
        }
    }

    /// 编辑单元格内容
    async fn edit_cell(&self, path: &Path, cell_index: usize, content: &str) -> ToolResult {
        match self.load_notebook(path).await {
            Ok(mut notebook) => {
                if cell_index >= notebook.cells.len() {
                    return ToolResult::error(format!(
                        "Cell index {} out of range (0-{})",
                        cell_index,
                        notebook.cells.len() - 1
                    ));
                }

                // 将内容按行分割为 Vec<String>
                let new_source: Vec<String> = content.lines().map(|l| format!("{}\n", l)).collect();

                notebook.cells[cell_index].source = new_source;

                match self.save_notebook(path, &notebook).await {
                    Ok(_) => {
                        ToolResult::success(format!("Edited cell {} successfully", cell_index))
                    }
                    Err(e) => e,
                }
            }
            Err(e) => e,
        }
    }

    /// 插入新单元格
    async fn insert_cell(
        &self,
        path: &Path,
        cell_type: &str,
        position: Option<usize>,
        content: &str,
    ) -> ToolResult {
        match self.load_notebook(path).await {
            Ok(mut notebook) => {
                let insert_pos = position.unwrap_or(notebook.cells.len());
                if insert_pos > notebook.cells.len() {
                    return ToolResult::error(format!(
                        "Position {} out of range (0-{})",
                        insert_pos,
                        notebook.cells.len()
                    ));
                }

                let new_source: Vec<String> = if content.is_empty() {
                    vec![]
                } else {
                    content.lines().map(|l| format!("{}\n", l)).collect()
                };

                let new_cell = NotebookCell {
                    cell_type: cell_type.to_string(),
                    source: new_source,
                    outputs: vec![],
                    execution_count: None,
                    metadata: json!({}),
                };

                notebook.cells.insert(insert_pos, new_cell);

                match self.save_notebook(path, &notebook).await {
                    Ok(_) => ToolResult::success(format!(
                        "Inserted {} cell at position {}",
                        cell_type, insert_pos
                    )),
                    Err(e) => e,
                }
            }
            Err(e) => e,
        }
    }

    /// 删除单元格
    async fn delete_cell(&self, path: &Path, cell_index: usize) -> ToolResult {
        match self.load_notebook(path).await {
            Ok(mut notebook) => {
                if cell_index >= notebook.cells.len() {
                    return ToolResult::error(format!(
                        "Cell index {} out of range (0-{})",
                        cell_index,
                        notebook.cells.len() - 1
                    ));
                }

                let removed = notebook.cells.remove(cell_index);
                match self.save_notebook(path, &notebook).await {
                    Ok(_) => ToolResult::success(format!(
                        "Deleted cell {} ({})",
                        cell_index, removed.cell_type
                    )),
                    Err(e) => e,
                }
            }
            Err(e) => e,
        }
    }

    /// 加载 Notebook
    async fn load_notebook(&self, path: &Path) -> Result<Notebook, ToolResult> {
        match tokio::fs::read_to_string(path).await {
            Ok(content) => match serde_json::from_str::<Notebook>(&content) {
                Ok(notebook) => Ok(notebook),
                Err(e) => Err(ToolResult::error(format!(
                    "Failed to parse notebook: {}",
                    e
                ))),
            },
            Err(e) => Err(ToolResult::error(format!("Failed to read file: {}", e))),
        }
    }

    /// 保存 Notebook
    async fn save_notebook(&self, path: &Path, notebook: &Notebook) -> Result<(), ToolResult> {
        match serde_json::to_string_pretty(notebook) {
            Ok(content) => match tokio::fs::write(path, content).await {
                Ok(_) => Ok(()),
                Err(e) => Err(ToolResult::error(format!("Failed to write file: {}", e))),
            },
            Err(e) => Err(ToolResult::error(format!(
                "Failed to serialize notebook: {}",
                e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_notebook_cell_deserialize() {
        let cell_json = json!({
            "cell_type": "code",
            "source": ["print('hello')\n"],
            "outputs": [],
            "execution_count": 1,
            "metadata": {}
        });

        let cell: NotebookCell = serde_json::from_value(cell_json).unwrap();
        assert_eq!(cell.cell_type, "code");
        assert_eq!(cell.source, vec!["print('hello')\n"]);
        assert_eq!(cell.execution_count, Some(1));
    }

    #[test]
    fn test_notebook_deserialize() {
        let notebook_json = json!({
            "nbformat": 4,
            "nbformat_minor": 5,
            "metadata": {
                "kernelspec": {
                    "display_name": "Python 3",
                    "language": "python",
                    "name": "python3"
                }
            },
            "cells": [
                {
                    "cell_type": "code",
                    "source": ["print('hello')\n"],
                    "outputs": [],
                    "execution_count": 1,
                    "metadata": {}
                },
                {
                    "cell_type": "markdown",
                    "source": ["# Title\n"],
                    "metadata": {}
                }
            ]
        });

        let notebook: Notebook = serde_json::from_value(notebook_json).unwrap();
        assert_eq!(notebook.nbformat, 4);
        assert_eq!(notebook.cells.len(), 2);
        assert_eq!(notebook.cells[0].cell_type, "code");
        assert_eq!(notebook.cells[1].cell_type, "markdown");
    }
}
