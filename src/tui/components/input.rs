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
}

impl InputState {
    pub fn new() -> Self {
        Self {
            value: String::new(),
            cursor_position: 0,
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

    /// 插入字符
    pub fn insert(&mut self, c: char) {
        let byte_pos = self
            .value
            .char_indices()
            .nth(self.cursor_position)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len());
        self.value.insert(byte_pos, c);
        self.cursor_position += 1;
    }

    /// 在光标位置删除字符（退格）
    pub fn delete_char_before_cursor(&mut self) {
        if self.cursor_position > 0 {
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
        if self.cursor_position < self.value.chars().count() {
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
    }

    /// 光标右移
    pub fn move_cursor_right(&mut self) {
        if self.cursor_position < self.value.chars().count() {
            self.cursor_position += 1;
        }
    }

    /// 光标移到开头
    pub fn move_cursor_to_start(&mut self) {
        self.cursor_position = 0;
    }

    /// 光标移到结尾
    pub fn move_cursor_to_end(&mut self) {
        self.cursor_position = self.value.chars().count();
    }

    /// 插入换行符
    pub fn insert_newline(&mut self) {
        self.insert('\n');
    }

    /// 光标上移
    pub fn move_cursor_up(&mut self) {
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
    }

    /// 设置值
    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor_position = self.value.chars().count();
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
}
