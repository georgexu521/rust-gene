//! 文件浏览器组件
//!
//! 树形文件浏览，支持导航和选择

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};
use std::path::{Path, PathBuf};

/// 文件树节点
#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub children: Vec<FileNode>,
    pub depth: usize,
}

impl FileNode {
    pub fn new(path: PathBuf, depth: usize) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let is_dir = path.is_dir();

        Self {
            path,
            name,
            is_dir,
            is_expanded: false,
            children: Vec::new(),
            depth,
        }
    }

    /// 加载子目录
    pub fn load_children(&mut self) {
        if !self.is_dir {
            return;
        }

        self.children.clear();
        if let Ok(entries) = std::fs::read_dir(&self.path) {
            let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            entries.sort_by(|a, b| {
                let a_is_dir = a.path().is_dir();
                let b_is_dir = b.path().is_dir();
                match (a_is_dir, b_is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.file_name().cmp(&b.file_name()),
                }
            });

            for entry in entries {
                let path = entry.path();
                let mut child = FileNode::new(path, self.depth + 1);
                // 预加载一层子目录
                if child.is_dir && self.depth < 1 {
                    child.load_children();
                }
                self.children.push(child);
            }
        }
        self.is_expanded = true;
    }

    /// 切换展开状态
    pub fn toggle(&mut self) {
        if self.is_dir {
            if self.is_expanded {
                self.is_expanded = false;
            } else {
                self.load_children();
            }
        }
    }
}

/// 文件浏览器状态
#[derive(Debug)]
pub struct FileBrowserState {
    root: FileNode,
    flattened: Vec<FileNode>,
    list_state: ListState,
    selected_path: Option<PathBuf>,
}

impl FileBrowserState {
    pub fn new(root_path: impl AsRef<Path>) -> Self {
        let mut root = FileNode::new(root_path.as_ref().to_path_buf(), 0);
        root.load_children();

        let mut state = Self {
            flattened: Vec::new(),
            list_state: ListState::default(),
            root,
            selected_path: None,
        };
        state.flatten();
        if !state.flattened.is_empty() {
            state.list_state.select(Some(0));
        }
        state
    }

    /// 平铺树结构为列表
    fn flatten(&mut self) {
        self.flattened.clear();
        self.flatten_node(&self.root.clone());
    }

    fn flatten_node(&mut self, node: &FileNode) {
        self.flattened.push(node.clone());
        if node.is_expanded {
            for child in &node.children {
                self.flatten_node(child);
            }
        }
    }

    /// 获取当前选中路径
    pub fn selected_path(&self) -> Option<&PathBuf> {
        self.list_state
            .selected()
            .and_then(|i| self.flattened.get(i))
            .map(|n| &n.path)
    }

    /// 向下移动
    pub fn next(&mut self) {
        let len = self.flattened.len();
        if len == 0 {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= len - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    /// 向上移动
    pub fn prev(&mut self) {
        let len = self.flattened.len();
        if len == 0 {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    len - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    /// 展开/折叠当前项
    pub fn toggle_current(&mut self) {
        if let Some(i) = self.list_state.selected() {
            if let Some(node) = self.flattened.get(i).cloned() {
                if node.is_dir {
                    // 在树中找到并切换
                    self.toggle_node(&node.path);
                    self.flatten();
                    // 保持选择位置
                    let new_index = self
                        .flattened
                        .iter()
                        .position(|n| n.path == node.path)
                        .unwrap_or(i);
                    self.list_state.select(Some(new_index));
                }
            }
        }
    }

    fn toggle_node(&mut self, path: &Path) {
        Self::toggle_in_node(&mut self.root, path);
    }

    fn toggle_in_node(node: &mut FileNode, path: &Path) -> bool {
        if node.path == path {
            node.toggle();
            return true;
        }
        if node.is_expanded {
            for child in &mut node.children {
                if Self::toggle_in_node(child, path) {
                    return true;
                }
            }
        }
        false
    }

    /// 选择当前项
    pub fn select_current(&mut self) -> Option<PathBuf> {
        self.selected_path = self.selected_path().cloned();
        self.selected_path.clone()
    }

    /// 渲染文件浏览器
    pub fn render(&self, _area: Rect) -> (List<'_>, ListState) {
        let items: Vec<ListItem> = self
            .flattened
            .iter()
            .map(|node| {
                let indent = "  ".repeat(node.depth);
                let icon = if node.is_dir {
                    if node.is_expanded {
                        "📂 "
                    } else {
                        "📁 "
                    }
                } else {
                    "📄 "
                };

                let style = if Some(&node.path) == self.selected_path.as_ref() {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else if node.is_dir {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                };

                let line = Line::from(vec![
                    Span::raw(indent),
                    Span::raw(icon),
                    Span::styled(&node.name, style),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Files"))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("❯ ");

        (list, self.list_state.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_node_new() {
        let node = FileNode::new(PathBuf::from("/tmp"), 0);
        assert!(node.is_dir);
        assert_eq!(node.depth, 0);
    }

    #[test]
    fn test_file_browser_navigation() {
        // 使用当前目录测试
        let mut browser = FileBrowserState::new(".");
        assert!(!browser.flattened.is_empty());

        browser.next();
        browser.prev();
        assert!(browser.selected_path().is_some());
    }
}
