//! 消息搜索组件
//!
//! 在对话历史中搜索特定内容
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};

/// 搜索结果项
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub message_index: usize,
    pub line_number: usize,
    pub preview: String,
    pub matched_text: String,
}

/// 消息搜索状态
#[derive(Debug, Default)]
pub struct MessageSearchState {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub list_state: ListState,
    pub is_active: bool,
    pub case_sensitive: bool,
}

impl MessageSearchState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            list_state: ListState::default(),
            is_active: false,
            case_sensitive: false,
        }
    }

    /// 激活搜索
    pub fn activate(&mut self) {
        self.is_active = true;
        self.query.clear();
        self.results.clear();
    }

    /// 取消搜索
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// 是否处于搜索模式
    pub fn is_searching(&self) -> bool {
        self.is_active
    }

    /// 添加字符到搜索查询
    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
    }

    /// 删除最后一个字符
    pub fn pop_char(&mut self) {
        self.query.pop();
    }

    /// 清空查询
    pub fn clear(&mut self) {
        self.query.clear();
        self.results.clear();
    }

    /// 切换大小写敏感
    pub fn toggle_case_sensitive(&mut self) {
        self.case_sensitive = !self.case_sensitive;
    }

    /// 执行搜索
    pub fn search(&mut self, messages: &[String]) {
        self.results.clear();

        if self.query.is_empty() {
            return;
        }

        let query = if self.case_sensitive {
            self.query.clone()
        } else {
            self.query.to_lowercase()
        };

        for (msg_idx, message) in messages.iter().enumerate() {
            let search_text = if self.case_sensitive {
                message.clone()
            } else {
                message.to_lowercase()
            };

            // 查找所有匹配位置
            let mut start = 0;
            while let Some(pos) = search_text[start..].find(&query) {
                let match_start = start + pos;
                let match_end = match_start + query.len();

                // 提取上下文预览
                let context_start = match_start.saturating_sub(30);
                let context_end = (match_end + 30).min(message.len());
                let preview = format!(
                    "{}{}{}",
                    if context_start > 0 { "..." } else { "" },
                    &message[context_start..context_end],
                    if context_end < message.len() {
                        "..."
                    } else {
                        ""
                    }
                );

                // 计算行号
                let line_number = message[..match_start].lines().count();

                self.results.push(SearchResult {
                    message_index: msg_idx,
                    line_number,
                    preview,
                    matched_text: self.query.clone(),
                });

                start = match_end;
                if start >= message.len() {
                    break;
                }
            }
        }

        // 选中第一个结果
        if !self.results.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// 获取当前选中的结果索引
    pub fn selected_result(&self) -> Option<&SearchResult> {
        self.list_state.selected().and_then(|i| self.results.get(i))
    }

    /// 选择下一个结果
    pub fn next_result(&mut self) {
        if self.results.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.results.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    /// 选择上一个结果
    pub fn prev_result(&mut self) {
        if self.results.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.results.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    /// 获取搜索状态显示文本
    pub fn status_text(&self) -> String {
        if self.query.is_empty() {
            "Search: (type to search)".to_string()
        } else if self.results.is_empty() {
            format!("Search: '{}' (no results)", self.query)
        } else {
            let current = self.list_state.selected().map(|i| i + 1).unwrap_or(0);
            format!(
                "Search: '{}' ({}/{}) {}",
                self.query,
                current,
                self.results.len(),
                if self.case_sensitive { "[Aa]" } else { "" }
            )
        }
    }

    /// 渲染搜索结果列表
    pub fn render_results(&self) -> List<'_> {
        let items: Vec<ListItem> = self
            .results
            .iter()
            .enumerate()
            .map(|(i, result)| {
                let style = if self.list_state.selected() == Some(i) {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };

                let line = Line::from(vec![
                    Span::raw(format!("[Msg {}] ", result.message_index + 1)),
                    Span::styled(&result.preview, style),
                ]);

                ListItem::new(line)
            })
            .collect();

        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Search Results ({})", self.results.len())),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_state() {
        let mut search = MessageSearchState::new();
        assert!(!search.is_searching());

        search.activate();
        assert!(search.is_searching());

        search.push_char('t');
        search.push_char('e');
        search.push_char('s');
        assert_eq!(search.query, "tes");

        search.pop_char();
        assert_eq!(search.query, "te");
    }

    #[test]
    fn test_search_execution() {
        let mut search = MessageSearchState::new();
        search.query = "hello".to_string();

        let messages = vec![
            "Hello world".to_string(),
            "This is a test".to_string(),
            "Say hello to everyone".to_string(),
        ];

        search.search(&messages);

        // 默认不区分大小写，应该找到 2 个匹配
        assert_eq!(search.results.len(), 2);
        assert_eq!(search.results[0].message_index, 0);
        assert_eq!(search.results[1].message_index, 2);
    }

    #[test]
    fn test_case_sensitive_search() {
        let mut search = MessageSearchState::new();
        search.query = "Hello".to_string();
        search.case_sensitive = true;

        let messages = vec!["Hello world".to_string(), "hello there".to_string()];

        search.search(&messages);

        // 区分大小写，只找到 1 个匹配
        assert_eq!(search.results.len(), 1);
        assert_eq!(search.results[0].message_index, 0);
    }

    #[test]
    fn test_navigation() {
        let mut search = MessageSearchState::new();
        search.query = "test".to_string();

        let messages = vec![
            "test one".to_string(),
            "test two".to_string(),
            "test three".to_string(),
        ];

        search.search(&messages);
        assert_eq!(search.results.len(), 3);

        // 默认选中第一个
        assert_eq!(search.list_state.selected(), Some(0));

        search.next_result();
        assert_eq!(search.list_state.selected(), Some(1));

        search.next_result();
        assert_eq!(search.list_state.selected(), Some(2));

        // 循环回到开头
        search.next_result();
        assert_eq!(search.list_state.selected(), Some(0));

        search.prev_result();
        assert_eq!(search.list_state.selected(), Some(2));
    }
}
