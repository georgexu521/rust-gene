//! 输入框组件
//!
//! 支持多行输入、光标移动

/// 输入状态管理
#[derive(Debug, Clone)]
pub struct InputState {
    /// 输入内容
    value: String,
    /// 光标位置（字符索引）
    cursor_position: usize,
    /// 选择锚点（字符索引）。`None` 表示没有选择。
    selection_anchor: Option<usize>,
    /// 撤销栈
    undo_stack: Vec<String>,
    /// 重做栈
    redo_stack: Vec<String>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            value: String::new(),
            cursor_position: 0,
            selection_anchor: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// 获取当前值
    pub fn value(&self) -> &str {
        &self.value
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn selection_anchor(&self) -> Option<usize> {
        self.selection_anchor
    }

    pub fn set_selection_anchor(&mut self, anchor: Option<usize>) {
        self.selection_anchor = anchor;
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    pub fn has_selection(&self) -> bool {
        self.selection_range().is_some()
    }

    /// 返回已排序的选择范围 `(start, end)`（字符索引）。
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        let cursor = self.cursor_position;
        if anchor == cursor {
            None
        } else {
            Some((anchor.min(cursor), anchor.max(cursor)))
        }
    }

    /// 返回选中的文本切片。
    pub fn selected_text(&self) -> Option<&str> {
        let (start, end) = self.selection_range()?;
        let byte_start = self
            .value
            .char_indices()
            .nth(start)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let byte_end = self
            .value
            .char_indices()
            .nth(end)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len());
        Some(&self.value[byte_start..byte_end])
    }

    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor_position = self.value.chars().count();
    }

    /// 在变异前保存当前状态到撤销栈。
    fn save_undo(&mut self) {
        if self.undo_stack.last() != Some(&self.value) {
            self.undo_stack.push(self.value.clone());
            self.redo_stack.clear();
        }
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.value.clone());
            self.value = prev;
            self.cursor_position = self.value.chars().count();
            self.selection_anchor = None;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.value.clone());
            self.value = next;
            self.cursor_position = self.value.chars().count();
            self.selection_anchor = None;
            true
        } else {
            false
        }
    }

    /// 删除当前选区并返回被删除的文本。
    pub fn delete_selection(&mut self) -> Option<String> {
        let (start, end) = self.selection_range()?;
        self.save_undo();
        let removed = self.replace_range(start, end, "");
        self.selection_anchor = None;
        self.cursor_position = start;
        Some(removed)
    }

    /// 替换选区；若无选区则在光标处插入。
    pub fn replace_selection(&mut self, text: &str) {
        self.save_undo();
        if let Some((start, _end)) = self.selection_range() {
            self.replace_range(
                start,
                self.selection_range().map(|(_, e)| e).unwrap_or(start),
                text,
            );
            self.cursor_position = start + text.chars().count();
            self.selection_anchor = None;
        } else {
            self.insert_str(text);
        }
    }

    fn replace_range(&mut self, start: usize, end: usize, text: &str) -> String {
        let byte_start = self
            .value
            .char_indices()
            .nth(start)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let byte_end = self
            .value
            .char_indices()
            .nth(end)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len());
        let removed = self.value[byte_start..byte_end].to_string();
        self.value.replace_range(byte_start..byte_end, text);
        removed
    }

    /// 将选区复制到剪贴板并返回文本。
    pub fn copy_selection(&self) -> Option<String> {
        let text = self.selected_text()?.to_string();
        if let Ok(mut ctx) = arboard::Clipboard::new() {
            let _ = ctx.set_text(text.clone());
        }
        Some(text)
    }

    /// 剪切选区。
    pub fn cut_selection(&mut self) -> Option<String> {
        let text = self.copy_selection()?;
        self.delete_selection();
        Some(text)
    }

    /// 从剪贴板粘贴到当前光标或选区位置。
    pub fn paste_from_clipboard(&mut self) -> Option<()> {
        let text = arboard::Clipboard::new().ok()?.get_text().ok()?;
        if text.is_empty() {
            return None;
        }
        self.replace_selection(&text);
        Some(())
    }

    /// 插入字符
    pub fn insert(&mut self, c: char) {
        self.save_undo();
        if self.has_selection() {
            self.replace_selection(&c.to_string());
            return;
        }
        let byte_pos = self
            .value
            .char_indices()
            .nth(self.cursor_position)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len());
        self.value.insert(byte_pos, c);
        self.cursor_position += 1;
    }

    /// 插入字符串，保持光标位置按 Unicode 字符计数
    pub fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.save_undo();
        if self.has_selection() {
            self.replace_selection(text);
            return;
        }
        let byte_pos = self
            .value
            .char_indices()
            .nth(self.cursor_position)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len());
        self.value.insert_str(byte_pos, text);
        self.cursor_position += text.chars().count();
    }

    /// 在光标位置删除字符（退格）
    pub fn delete_char_before_cursor(&mut self) {
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        if self.cursor_position > 0 {
            self.save_undo();
            let char_idx = self.cursor_position - 1;
            let byte_pos = self
                .value
                .char_indices()
                .nth(char_idx)
                .map(|(i, _)| i)
                .unwrap_or(0);
            let next_byte_pos = self
                .value
                .char_indices()
                .nth(self.cursor_position)
                .map(|(i, _)| i)
                .unwrap_or(self.value.len());
            self.value.drain(byte_pos..next_byte_pos);
            self.cursor_position -= 1;
        }
    }

    /// 在光标位置删除字符（Delete 键）
    pub fn delete_char_at_cursor(&mut self) {
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        if self.cursor_position < self.value.chars().count() {
            self.save_undo();
            let byte_pos = self
                .value
                .char_indices()
                .nth(self.cursor_position)
                .map(|(i, _)| i)
                .unwrap_or(0);
            let next_byte_pos = self
                .value
                .char_indices()
                .nth(self.cursor_position + 1)
                .map(|(i, _)| i)
                .unwrap_or(self.value.len());
            self.value.drain(byte_pos..next_byte_pos);
        }
    }

    /// 光标左移
    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
        self.clear_selection();
    }

    /// 光标右移
    pub fn move_cursor_right(&mut self) {
        if self.cursor_position < self.value.chars().count() {
            self.cursor_position += 1;
        }
        self.clear_selection();
    }

    /// 光标移到开头
    pub fn move_cursor_to_start(&mut self) {
        self.cursor_position = 0;
        self.clear_selection();
    }

    /// 光标移到结尾
    pub fn move_cursor_to_end(&mut self) {
        self.cursor_position = self.value.chars().count();
        self.clear_selection();
    }

    /// 插入换行符
    pub fn insert_newline(&mut self) {
        self.insert('\n');
    }

    /// 光标上移
    pub fn move_cursor_up(&mut self) {
        self.clear_selection();
        let chars: Vec<char> = self.value.chars().collect();
        if self.cursor_position == 0 || chars.is_empty() {
            return;
        }

        // 找到当前行的起始位置
        let current_pos = self.cursor_position.min(chars.len());
        let line_start = chars[..current_pos]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, c)| **c == '\n')
            .map(|(i, _)| i + 1)
            .unwrap_or(0);
        let col = current_pos - line_start;

        if line_start == 0 {
            // 已经在第一行
            self.cursor_position = 0;
            return;
        }

        // 找到上一行的起始位置
        let prev_line_start = chars[..line_start - 1]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, c)| **c == '\n')
            .map(|(i, _)| i + 1)
            .unwrap_or(0);

        // 移动到上一行的相同列（或行尾）
        let prev_line_end = line_start - 1;
        self.cursor_position = prev_line_start + col.min(prev_line_end - prev_line_start);
    }

    /// 光标下移
    pub fn move_cursor_down(&mut self) {
        self.clear_selection();
        let chars: Vec<char> = self.value.chars().collect();
        if chars.is_empty() || self.cursor_position >= chars.len() {
            return;
        }

        let current_pos = self.cursor_position.min(chars.len());
        let line_start = chars[..current_pos]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, c)| **c == '\n')
            .map(|(i, _)| i + 1)
            .unwrap_or(0);
        let col = current_pos - line_start;

        // 找到当前行结束位置
        let line_end = chars[current_pos..]
            .iter()
            .enumerate()
            .find(|(_, c)| **c == '\n')
            .map(|(i, _)| current_pos + i)
            .unwrap_or(chars.len());

        if line_end >= chars.len() {
            // 已经在最后一行
            return;
        }

        // 下一行起始位置
        let next_line_start = line_end + 1;
        let next_line_end = chars[next_line_start..]
            .iter()
            .enumerate()
            .find(|(_, c)| **c == '\n')
            .map(|(i, _)| next_line_start + i)
            .unwrap_or(chars.len());

        self.cursor_position = next_line_start + col.min(next_line_end - next_line_start);
    }

    /// 获取光标位置
    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    /// 获取光标所在的行和列（列使用显示宽度，支持 CJK/Emoji）
    pub fn cursor_line_column(&self) -> (usize, usize) {
        let chars: Vec<char> = self.value.chars().collect();
        let pos = self.cursor_position.min(chars.len());
        let line = chars[..pos].iter().filter(|&&c| c == '\n').count();
        let line_start_idx = chars[..pos]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, &c)| c == '\n')
            .map(|(i, _)| i + 1)
            .unwrap_or(0);
        let col = chars[line_start_idx..pos]
            .iter()
            .map(|&c| unicode_width::UnicodeWidthChar::width(c).unwrap_or(0))
            .sum();
        (line, col)
    }

    /// 获取输入内容的行数
    pub fn line_count(&self) -> usize {
        self.value.lines().count().max(1)
    }

    /// 光标是否在第一行
    pub fn is_cursor_on_first_line(&self) -> bool {
        let (line, _) = self.cursor_line_column();
        line == 0
    }

    /// 光标是否在最后一行
    pub fn is_cursor_on_last_line(&self) -> bool {
        let (line, _) = self.cursor_line_column();
        line + 1 >= self.line_count()
    }

    /// 获取字节位置（用于显示）
    pub fn cursor_byte_position(&self) -> usize {
        self.value
            .chars()
            .take(self.cursor_position)
            .map(|c| c.len_utf8())
            .sum()
    }

    /// 清空输入
    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor_position = 0;
        self.selection_anchor = None;
    }

    /// 设置值
    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor_position = self.value.chars().count();
        self.selection_anchor = None;
    }

    // 选择移动 --------------------------------------------------

    pub fn select_left(&mut self) {
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    pub fn select_right(&mut self) {
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
        if self.cursor_position < self.value.chars().count() {
            self.cursor_position += 1;
        }
    }

    pub fn select_up(&mut self) {
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
        self.move_cursor_up_preserving_anchor();
    }

    pub fn select_down(&mut self) {
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
        self.move_cursor_down_preserving_anchor();
    }

    fn move_cursor_up_preserving_anchor(&mut self) {
        let chars: Vec<char> = self.value.chars().collect();
        if self.cursor_position == 0 || chars.is_empty() {
            return;
        }
        let current_pos = self.cursor_position.min(chars.len());
        let line_start = chars[..current_pos]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, c)| **c == '\n')
            .map(|(i, _)| i + 1)
            .unwrap_or(0);
        let col = current_pos - line_start;
        if line_start == 0 {
            self.cursor_position = 0;
            return;
        }
        let prev_line_start = chars[..line_start - 1]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, c)| **c == '\n')
            .map(|(i, _)| i + 1)
            .unwrap_or(0);
        let prev_line_end = line_start - 1;
        self.cursor_position = prev_line_start + col.min(prev_line_end - prev_line_start);
    }

    fn move_cursor_down_preserving_anchor(&mut self) {
        let chars: Vec<char> = self.value.chars().collect();
        if chars.is_empty() || self.cursor_position >= chars.len() {
            return;
        }
        let current_pos = self.cursor_position.min(chars.len());
        let line_start = chars[..current_pos]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, c)| **c == '\n')
            .map(|(i, _)| i + 1)
            .unwrap_or(0);
        let col = current_pos - line_start;
        let line_end = chars[current_pos..]
            .iter()
            .enumerate()
            .find(|(_, c)| **c == '\n')
            .map(|(i, _)| current_pos + i)
            .unwrap_or(chars.len());
        if line_end >= chars.len() {
            return;
        }
        let next_line_start = line_end + 1;
        let next_line_end = chars[next_line_start..]
            .iter()
            .enumerate()
            .find(|(_, c)| **c == '\n')
            .map(|(i, _)| next_line_start + i)
            .unwrap_or(chars.len());
        self.cursor_position = next_line_start + col.min(next_line_end - next_line_start);
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_insert() {
        let mut input = InputState::new();
        input.insert('H');
        input.insert('i');
        assert_eq!(input.value(), "Hi");
        assert_eq!(input.cursor_position(), 2);
    }

    #[test]
    fn test_input_backspace() {
        let mut input = InputState::new();
        input.insert('H');
        input.insert('i');
        input.delete_char_before_cursor();
        assert_eq!(input.value(), "H");
        assert_eq!(input.cursor_position(), 1);
    }

    #[test]
    fn test_input_cursor_movement() {
        let mut input = InputState::new();
        input.insert('a');
        input.insert('b');
        input.insert('c');

        input.move_cursor_to_start();
        assert_eq!(input.cursor_position(), 0);

        input.move_cursor_right();
        assert_eq!(input.cursor_position(), 1);

        input.move_cursor_to_end();
        assert_eq!(input.cursor_position(), 3);
    }

    #[test]
    fn test_input_unicode() {
        let mut input = InputState::new();
        input.insert('你');
        input.insert('好');
        assert_eq!(input.value(), "你好");
        assert_eq!(input.cursor_position(), 2);
    }

    #[test]
    fn test_insert_str_unicode_at_cursor() {
        let mut input = InputState::new();
        input.insert_str("你好世界");
        input.move_cursor_left();
        input.move_cursor_left();
        input.insert_str(" Rust ");

        assert_eq!(input.value(), "你好 Rust 世界");
        assert_eq!(input.cursor_position(), 8);
    }

    #[test]
    fn test_selection_and_replace() {
        let mut input = InputState::new();
        input.insert_str("hello world");
        input.move_cursor_to_start();
        input.select_right();
        input.select_right();
        input.select_right();
        input.select_right();
        input.select_right();
        assert_eq!(input.selected_text(), Some("hello"));
        input.replace_selection("hi");
        assert_eq!(input.value(), "hi world");
        assert_eq!(input.cursor_position(), 2);
    }

    #[test]
    fn test_delete_selection() {
        let mut input = InputState::new();
        input.insert_str("abc def");
        input.move_cursor_to_start();
        input.select_right();
        input.select_right();
        input.select_right();
        assert_eq!(input.delete_selection(), Some("abc".to_string()));
        assert_eq!(input.value(), " def");
    }

    #[test]
    fn test_undo_redo() {
        let mut input = InputState::new();
        input.insert_str("one");
        input.insert_str(" two");
        assert!(input.undo());
        assert_eq!(input.value(), "one");
        assert!(input.redo());
        assert_eq!(input.value(), "one two");
    }

    #[test]
    fn test_insert_replaces_selection() {
        let mut input = InputState::new();
        input.insert_str("hello");
        input.move_cursor_to_start();
        input.select_right();
        input.select_right();
        input.select_right();
        input.insert('X');
        assert_eq!(input.value(), "Xlo");
    }

    #[test]
    fn test_copy_selection_returns_text() {
        let mut input = InputState::new();
        input.insert_str("copy me");
        input.move_cursor_to_start();
        input.select_right();
        input.select_right();
        input.select_right();
        input.select_right();
        input.select_right();
        input.select_right();
        input.select_right();
        assert_eq!(input.copy_selection(), Some("copy me".to_string()));
    }

    #[test]
    fn test_select_all() {
        let mut input = InputState::new();
        input.insert_str("all of it");
        input.select_all();
        assert_eq!(input.selected_text(), Some("all of it"));
    }
}
