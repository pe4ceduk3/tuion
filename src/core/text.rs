//! Small text-editing primitives shared by the note editor, config editor
//! and modal forms. Everything is indexed by `char`, not byte, so Cyrillic,
//! emoji and Nerd Font glyphs behave predictably.

#[derive(Clone, Debug, Default)]
pub struct TextBuffer {
    pub lines: Vec<String>,
    pub row: usize,
    pub col: usize,
    pub scroll: usize,
    pub hscroll: usize,
}

impl TextBuffer {
    pub fn new(text: &str) -> Self {
        let mut lines: Vec<String> = text
            .split('\n')
            .map(|line| line.trim_end_matches('\r').to_string())
            .collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        Self {
            lines,
            row: 0,
            col: 0,
            scroll: 0,
            hscroll: 0,
        }
    }

    pub fn as_text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn line(&self) -> &str {
        self.lines.get(self.row).map(String::as_str).unwrap_or("")
    }

    pub fn line_at(&self, row: usize) -> &str {
        self.lines.get(row).map(String::as_str).unwrap_or("")
    }

    pub fn line_len(&self, row: usize) -> usize {
        self.line_at(row).chars().count()
    }

    pub fn clamp(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.row = self.row.min(self.lines.len() - 1);
        self.col = self.col.min(self.line_len(self.row));
    }

    pub fn set_cursor(&mut self, row: usize, col: usize) {
        self.row = row.min(self.lines.len().saturating_sub(1));
        self.col = col.min(self.line_len(self.row));
    }

    pub fn insert_char(&mut self, ch: char) {
        let mut chars: Vec<char> = self.line().chars().collect();
        let col = self.col.min(chars.len());
        chars.insert(col, ch);
        self.lines[self.row] = chars.into_iter().collect();
        self.col = col + 1;
    }

    pub fn insert_str(&mut self, text: &str) {
        for ch in text.chars() {
            match ch {
                '\n' => self.newline(),
                '\r' => {}
                _ => self.insert_char(ch),
            }
        }
    }

    pub fn newline(&mut self) {
        let chars: Vec<char> = self.line().chars().collect();
        let col = self.col.min(chars.len());
        let left: String = chars[..col].iter().collect();
        let right: String = chars[col..].iter().collect();
        self.lines[self.row] = left;
        self.lines.insert(self.row + 1, right);
        self.row += 1;
        self.col = 0;
    }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            let mut chars: Vec<char> = self.line().chars().collect();
            let col = self.col.min(chars.len());
            chars.remove(col - 1);
            self.lines[self.row] = chars.into_iter().collect();
            self.col = col - 1;
        } else if self.row > 0 {
            let current = self.lines.remove(self.row);
            self.row -= 1;
            let prev_len = self.line_len(self.row);
            self.lines[self.row].push_str(&current);
            self.col = prev_len;
        }
    }

    pub fn delete(&mut self) {
        let len = self.line_len(self.row);
        if self.col < len {
            let mut chars: Vec<char> = self.line().chars().collect();
            chars.remove(self.col);
            self.lines[self.row] = chars.into_iter().collect();
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next);
        }
    }

    pub fn move_left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.line_len(self.row);
        }
    }

    pub fn move_right(&mut self) {
        let len = self.line_len(self.row);
        if self.col < len {
            self.col += 1;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
            self.col = self.col.min(self.line_len(self.row));
        }
    }

    pub fn move_down(&mut self) {
        if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = self.col.min(self.line_len(self.row));
        }
    }

    pub fn home(&mut self) {
        self.col = 0;
    }

    pub fn end(&mut self) {
        self.col = self.line_len(self.row);
    }

    pub fn first_non_space(&self) -> usize {
        self.line()
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .count()
    }

    pub fn leading_spaces(&self) -> usize {
        self.first_non_space()
    }

    pub fn is_blank(&self) -> bool {
        self.line().trim().is_empty()
    }

    pub fn insert_line(&mut self, index: usize, text: impl Into<String>) {
        let index = index.min(self.lines.len());
        self.lines.insert(index, text.into());
    }

    pub fn remove_line(&mut self, index: usize) -> Option<String> {
        if index < self.lines.len() {
            Some(self.lines.remove(index))
        } else {
            None
        }
    }

    pub fn replace_line(&mut self, index: usize, text: impl Into<String>) {
        if index < self.lines.len() {
            self.lines[index] = text.into();
            if self.row == index {
                self.col = self.col.min(self.line_len(index));
            }
        }
    }

    pub fn set_indent(&mut self, row: usize, indent: usize) {
        if row >= self.lines.len() {
            return;
        }
        let content = self.lines[row].trim_start().to_string();
        self.lines[row] = format!("{}{}", " ".repeat(indent), content);
        if self.row == row {
            self.col = self.col.min(self.line_len(row));
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct LineInput {
    pub value: String,
    pub cursor: usize,
}

impl LineInput {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        Self { value, cursor }
    }

    pub fn set(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor = self.value.chars().count();
    }

    pub fn chars(&self) -> Vec<char> {
        self.value.chars().collect()
    }

    pub fn insert_char(&mut self, ch: char) {
        let mut chars = self.chars();
        let cursor = self.cursor.min(chars.len());
        chars.insert(cursor, ch);
        self.value = chars.into_iter().collect();
        self.cursor = cursor + 1;
    }

    pub fn insert_str(&mut self, text: &str) {
        for ch in text.chars() {
            if ch == '\n' || ch == '\r' {
                continue;
            }
            self.insert_char(ch);
        }
    }

    pub fn backspace(&mut self) {
        let mut chars = self.chars();
        if self.cursor > 0 && self.cursor <= chars.len() {
            chars.remove(self.cursor - 1);
            self.value = chars.into_iter().collect();
            self.cursor -= 1;
        }
    }

    pub fn delete(&mut self) {
        let mut chars = self.chars();
        if self.cursor < chars.len() {
            chars.remove(self.cursor);
            self.value = chars.into_iter().collect();
        }
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.value.chars().count());
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.value.chars().count();
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn is_empty(&self) -> bool {
        self.value.trim().is_empty()
    }
}

/// Split a string into chars for display/editing helpers.
pub fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

/// Byte-free prefix/suffix slicing by char index.
pub fn char_slice(s: &str, start: usize, end: usize) -> String {
    let v: Vec<char> = s.chars().collect();
    let start = start.min(v.len());
    let end = end.min(v.len()).max(start);
    v[start..end].iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_is_char_based() {
        let mut b = TextBuffer::new("привет\nмир");
        b.end();
        b.insert_char('!');
        assert_eq!(b.line(), "привет!");
        b.newline();
        assert_eq!(b.lines, vec!["привет!", "", "мир"]);
        b.backspace();
        assert_eq!(b.lines, vec!["привет!", "мир"]);
        assert_eq!(b.row, 0);
        assert_eq!(b.col, 7);
    }

    #[test]
    fn backspace_joins_lines() {
        let mut b = TextBuffer::new("one\ntwo");
        b.set_cursor(1, 0);
        b.backspace();
        assert_eq!(b.lines, vec!["onetwo"]);
        assert_eq!(b.col, 3);
    }

    #[test]
    fn line_input_edits() {
        let mut i = LineInput::new("abc");
        i.left();
        i.insert_char('X');
        assert_eq!(i.value, "abXc");
        i.backspace();
        assert_eq!(i.value, "abc");
    }
}
