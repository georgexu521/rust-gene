//! Multi-line prompt editor state.
//!
//! The editor manages raw terminal input for the fixed footer prompt. It
//! supports multi-line text, cursor movement, and basic word navigation so
//! that pasted or long prompts render correctly in the footer area.

#[derive(Debug, Clone, Default)]
pub struct PromptEditor {
    /// Lines of text currently in the prompt. There is always at least one line.
    lines: Vec<String>,
    /// Cursor row within `lines`.
    row: usize,
    /// Cursor column within the current line (byte index).
    col: usize,
}

impl PromptEditor {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            row: 0,
            col: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    /// Insert one or more characters at the cursor, handling newlines.
    pub fn insert(&mut self, text: &str) {
        for ch in text.chars() {
            if ch == '\n' {
                self.insert_newline();
            } else {
                self.insert_char(ch);
            }
        }
    }

    fn insert_char(&mut self, ch: char) {
        let line = &mut self.lines[self.row];
        line.insert(self.col, ch);
        self.col += ch.len_utf8();
    }

    fn insert_newline(&mut self) {
        let tail = self.lines[self.row].split_off(self.col);
        self.row += 1;
        self.lines.insert(self.row, tail);
        self.col = 0;
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        if self.col > 0 {
            let line = &mut self.lines[self.row];
            let prev = line[..self.col]
                .char_indices()
                .next_back()
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            line.replace_range(prev..self.col, "");
            self.col = prev;
        } else if self.row > 0 {
            let line = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.lines[self.row].len();
            self.lines[self.row].push_str(&line);
        }
    }

    /// Delete the character under the cursor.
    pub fn delete(&mut self) {
        let line = &mut self.lines[self.row];
        if self.col < line.len() {
            let next = line[self.col..]
                .char_indices()
                .nth(1)
                .map(|(idx, _)| self.col + idx)
                .unwrap_or(line.len());
            line.replace_range(self.col..next, "");
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next);
        }
    }

    pub fn move_left(&mut self) {
        if self.col > 0 {
            let line = &self.lines[self.row];
            let prev = line[..self.col]
                .char_indices()
                .next_back()
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            self.col = prev;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.lines[self.row].len();
        }
    }

    pub fn move_right(&mut self) {
        if self.col < self.lines[self.row].len() {
            let line = &self.lines[self.row];
            let next = line[self.col..]
                .char_indices()
                .nth(1)
                .map(|(idx, _)| self.col + idx)
                .unwrap_or(line.len());
            self.col = next;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
            self.col = self.col.min(self.lines[self.row].len());
        }
    }

    pub fn move_down(&mut self) {
        if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = self.col.min(self.lines[self.row].len());
        }
    }

    pub fn move_home(&mut self) {
        self.col = 0;
    }

    pub fn move_end(&mut self) {
        self.col = self.lines[self.row].len();
    }

    pub fn move_word_left(&mut self) {
        let line = &self.lines[self.row];
        let mut idx = self.col;
        while idx > 0 && line[..idx].ends_with(|ch: char| ch.is_whitespace()) {
            idx -= 1;
        }
        while idx > 0 {
            let prev = line[..idx]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            if line[prev..idx].starts_with(|ch: char| ch.is_whitespace()) {
                break;
            }
            idx = prev;
        }
        self.col = idx;
    }

    pub fn move_word_right(&mut self) {
        let line = &self.lines[self.row];
        let mut idx = self.col;
        while idx < line.len() && line[idx..].starts_with(|ch: char| ch.is_whitespace()) {
            idx += 1;
        }
        while idx < line.len() {
            let next = line[idx..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| idx + i)
                .unwrap_or(line.len());
            if line[idx..next].starts_with(|ch: char| ch.is_whitespace()) {
                break;
            }
            idx = next;
        }
        self.col = idx;
    }

    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.row = 0;
        self.col = 0;
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Set the cursor to a valid (row, byte_col) position, clamping to the
    /// available lines and line lengths.
    pub fn set_cursor(&mut self, row: usize, col: usize) {
        self.row = row.min(self.lines.len().saturating_sub(1));
        self.col = col.min(self.lines[self.row].len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_delete() {
        let mut editor = PromptEditor::new();
        editor.insert("hello");
        assert_eq!(editor.text(), "hello");
        editor.backspace();
        assert_eq!(editor.text(), "hell");
        editor.move_left();
        editor.delete();
        assert_eq!(editor.text(), "hel");
    }

    #[test]
    fn multi_line_navigation() {
        let mut editor = PromptEditor::new();
        editor.insert("ab\ncd");
        assert_eq!(editor.line_count(), 2);
        editor.move_up();
        assert_eq!(editor.cursor(), (0, 2));
        editor.move_end();
        assert_eq!(editor.cursor(), (0, 2));
        editor.move_down();
        assert_eq!(editor.cursor(), (1, 2));
    }

    #[test]
    fn word_movement() {
        let mut editor = PromptEditor::new();
        editor.insert("foo bar baz");
        editor.move_end();
        editor.move_word_left();
        assert_eq!(editor.cursor(), (0, 8));
        editor.move_word_left();
        assert_eq!(editor.cursor(), (0, 4));
    }
}
