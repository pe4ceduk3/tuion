//! The outliner note model. A note is just a markdown text buffer plus
//! outliner behaviour: folding, zooming, indentation-aware Enter, line
//! marks and undo.

use crate::core::config::MarkdownSyntax;
use crate::core::markdown;
use crate::core::storage;
use crate::core::text::TextBuffer;
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct Zoom {
    pub start: usize,
    pub end: usize,
    pub label: String,
}

#[derive(Clone, Debug)]
struct Snapshot {
    lines: Vec<String>,
    row: usize,
    col: usize,
    marks: BTreeSet<usize>,
    folded: BTreeSet<usize>,
}

pub struct Document {
    pub path: PathBuf,
    pub rel_path: String,
    pub buffer: TextBuffer,
    pub dirty: bool,
    pub marks: BTreeSet<usize>,
    pub folded: BTreeSet<usize>,
    pub zoom: Vec<Zoom>,
    pub scroll: usize,
    pub hscroll: usize,
    /// Set by the renderer; used to translate mouse columns back to text
    /// columns.
    pub content_x: u16,
    pub content_width: u16,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last_edit: Option<Instant>,
}

impl Document {
    pub fn open(path: impl Into<PathBuf>, rel_path: impl Into<String>) -> io::Result<Self> {
        let path = path.into();
        let text = if path.exists() {
            storage::read_text(&path)?
        } else {
            let title = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("untitled");
            let initial = format!("# {title}\n\n");
            storage::write_text(&path, &initial)?;
            initial
        };
        Ok(Self::from_text(path, rel_path.into(), &text))
    }

    pub fn from_text(path: PathBuf, rel_path: String, text: &str) -> Self {
        Self {
            path,
            rel_path,
            buffer: TextBuffer::new(text),
            dirty: false,
            marks: BTreeSet::new(),
            folded: BTreeSet::new(),
            zoom: Vec::new(),
            scroll: 0,
            hscroll: 0,
            content_x: 0,
            content_width: 80,
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: None,
        }
    }

    pub fn save(&mut self) -> io::Result<()> {
        let mut text = self.buffer.as_text();
        if !text.ends_with('\n') {
            text.push('\n');
        }
        storage::write_text(&self.path, &text)?;
        self.dirty = false;
        Ok(())
    }

    pub fn save_if_dirty(&mut self) {
        if self.dirty {
            let _ = self.save();
        }
    }

    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled.md")
            .to_string()
    }

    pub fn visible_indices(&self, syntax: &MarkdownSyntax) -> Vec<usize> {
        let (start, end) = self
            .zoom
            .last()
            .map(|z| {
                (
                    z.start.min(self.buffer.lines.len()),
                    z.end.min(self.buffer.lines.len()),
                )
            })
            .unwrap_or((0, self.buffer.lines.len()));
        let mut out = Vec::new();
        let mut skip_until = start;
        for i in start..end {
            if i < skip_until {
                continue;
            }
            out.push(i);
            if self.folded.contains(&i) {
                if let Some(fold_end) = markdown::fold_end(&self.buffer.lines, i, syntax) {
                    skip_until = skip_until.max(fold_end);
                }
            }
        }
        out
    }

    pub fn visual_index(&self, syntax: &MarkdownSyntax) -> usize {
        let visible = self.visible_indices(syntax);
        visible
            .iter()
            .position(|i| *i == self.buffer.row)
            .unwrap_or(visible.len().saturating_sub(1))
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            lines: self.buffer.lines.clone(),
            row: self.buffer.row,
            col: self.buffer.col,
            marks: self.marks.clone(),
            folded: self.folded.clone(),
        }
    }

    fn before_edit(&mut self) {
        let now = Instant::now();
        let coalesce = self
            .last_edit
            .map(|t| now.duration_since(t) < Duration::from_millis(650))
            .unwrap_or(false);
        if !coalesce {
            self.undo.push(self.snapshot());
            if self.undo.len() > 300 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_edit = Some(now);
        self.dirty = true;
    }

    pub fn undo(&mut self) {
        if let Some(snap) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.restore(snap);
            self.last_edit = None;
            self.dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(snap) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.restore(snap);
            self.last_edit = None;
            self.dirty = true;
        }
    }

    fn restore(&mut self, snap: Snapshot) {
        self.buffer.lines = snap.lines;
        self.buffer.row = snap.row;
        self.buffer.col = snap.col;
        self.marks = snap.marks;
        self.folded = snap.folded;
        self.buffer.clamp();
    }

    pub fn insert_char(&mut self, ch: char) {
        self.before_edit();
        self.buffer.insert_char(ch);
    }

    pub fn insert_str(&mut self, text: &str) {
        self.before_edit();
        self.buffer.insert_str(text);
    }

    /// Outliner-aware Enter: keep the current list marker and indentation;
    /// an empty item ends the list and outdents.
    pub fn newline(&mut self, syntax: &MarkdownSyntax) {
        self.before_edit();
        let line = self.buffer.line().to_string();
        if let Some(info) = markdown::parse_list(&line, syntax) {
            let chars: Vec<char> = line.chars().collect();
            let content: String = chars[info.content_start.min(chars.len())..]
                .iter()
                .collect();
            if content.trim().is_empty() {
                let indent = info.indent.saturating_sub(2);
                self.buffer.lines[self.buffer.row] = " ".repeat(indent);
                self.buffer.col = indent;
                return;
            }
            let col = self.buffer.col.max(info.content_start).min(chars.len());
            let left: String = chars[..col].iter().collect();
            let right: String = chars[col..].iter().collect();
            self.buffer.lines[self.buffer.row] = left;
            let prefix: String = chars[..info.content_start].iter().collect();
            self.buffer
                .insert_line(self.buffer.row + 1, format!("{prefix}{right}"));
            self.buffer.row += 1;
            self.buffer.col = info.content_start;
            return;
        }

        let indent = self.buffer.leading_spaces();
        self.buffer.newline();
        if indent > 0 {
            self.buffer.lines[self.buffer.row] = " ".repeat(indent);
            self.buffer.col = indent;
        }
    }

    pub fn backspace(&mut self, syntax: &MarkdownSyntax) {
        let line = self.buffer.line().to_string();
        if let Some(info) = markdown::parse_list(&line, syntax) {
            let content: String = line
                .chars()
                .skip(info.content_start.min(line.chars().count()))
                .collect();
            if content.trim().is_empty() && self.buffer.col >= info.indent {
                self.before_edit();
                self.buffer.lines[self.buffer.row] = " ".repeat(info.indent.saturating_sub(2));
                self.buffer.col = info.indent.saturating_sub(2);
                return;
            }
            if self.buffer.col <= info.indent && info.indent > 0 {
                self.outdent();
                return;
            }
        }
        self.before_edit();
        self.buffer.backspace();
    }

    pub fn delete(&mut self) {
        self.before_edit();
        self.buffer.delete();
    }

    pub fn indent(&mut self) {
        self.before_edit();
        let rows = self.selected_rows();
        let current = self.buffer.row;
        for row in rows {
            let indent = self
                .buffer
                .line_at(row)
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .count();
            self.buffer.set_indent(row, indent + 2);
            if row == current {
                self.buffer.col += 2;
            }
        }
    }

    pub fn outdent(&mut self) {
        self.before_edit();
        for row in self.selected_rows() {
            let indent = self
                .buffer
                .line_at(row)
                .chars()
                .take_while(|c| *c == ' ')
                .count();
            let remove = indent.min(2);
            if remove > 0 {
                let rest: String = self.buffer.line_at(row).chars().skip(remove).collect();
                self.buffer.lines[row] = rest;
                if row == self.buffer.row {
                    self.buffer.col = self.buffer.col.saturating_sub(remove);
                }
            }
        }
    }

    fn selected_rows(&self) -> Vec<usize> {
        if self.marks.is_empty() {
            vec![self.buffer.row]
        } else {
            self.marks.iter().copied().collect()
        }
    }

    pub fn toggle_mark(&mut self) {
        if !self.marks.remove(&self.buffer.row) {
            self.marks.insert(self.buffer.row);
        }
    }

    pub fn clear_marks(&mut self) {
        self.marks.clear();
    }

    pub fn copy_lines(&self) -> Vec<String> {
        self.selected_rows()
            .into_iter()
            .filter_map(|i| self.buffer.lines.get(i).cloned())
            .collect()
    }

    pub fn cut_lines(&mut self) -> Vec<String> {
        let rows = self.selected_rows();
        let out = self.copy_lines();
        self.before_edit();
        let mut rows = rows;
        rows.sort_unstable();
        rows.dedup();
        for row in rows.into_iter().rev() {
            if self.buffer.lines.len() > 1 {
                self.buffer.lines.remove(row);
            } else {
                self.buffer.lines[0].clear();
            }
        }
        self.marks.clear();
        self.buffer.row = self.buffer.row.min(self.buffer.lines.len() - 1);
        self.buffer.col = self.buffer.col.min(self.buffer.line_len(self.buffer.row));
        out
    }

    pub fn paste_lines(&mut self, lines: &[String]) {
        if lines.is_empty() {
            return;
        }
        self.before_edit();
        let at = (self.buffer.row + 1).min(self.buffer.lines.len());
        for (offset, line) in lines.iter().enumerate() {
            self.buffer.insert_line(at + offset, line.clone());
        }
        self.buffer.row = at;
        self.buffer.col = 0;
    }

    pub fn move_block(&mut self, up: bool) {
        if self.buffer.lines.len() < 2 {
            return;
        }
        let rows: Vec<usize> = if self.marks.is_empty() {
            vec![self.buffer.row]
        } else {
            self.marks.iter().copied().collect()
        };
        let start = *rows.first().unwrap_or(&0);
        let end = *rows.last().unwrap_or(&start);
        if up && start == 0 {
            return;
        }
        if !up && end + 1 >= self.buffer.lines.len() {
            return;
        }
        self.before_edit();
        if up {
            let moved = self.buffer.lines.remove(start - 1);
            self.buffer.lines.insert(end, moved);
            self.buffer.row = self.buffer.row.saturating_sub(1);
            self.marks = rows.iter().map(|r| r - 1).collect();
        } else {
            let moved = self.buffer.lines.remove(end + 1);
            self.buffer.lines.insert(start, moved);
            self.buffer.row += 1;
            self.marks = rows.iter().map(|r| r + 1).collect();
        }
    }

    pub fn toggle_fold(&mut self, syntax: &MarkdownSyntax) {
        let row = self.buffer.row;
        if !self.folded.remove(&row) {
            if markdown::fold_end(&self.buffer.lines, row, syntax).is_some() {
                self.folded.insert(row);
            }
        }
    }

    pub fn toggle_checkbox(&mut self, syntax: &MarkdownSyntax) {
        if let Some(line) = markdown::toggle_checkbox(self.buffer.line(), syntax) {
            self.before_edit();
            self.buffer.replace_line(self.buffer.row, line);
        }
    }

    pub fn zoom_in(&mut self, syntax: &MarkdownSyntax) {
        let mut target = self.buffer.row;
        let mut range = markdown::zoom_range(&self.buffer.lines, target, syntax);
        while range.is_none() && target > 0 {
            target -= 1;
            range = markdown::zoom_range(&self.buffer.lines, target, syntax);
        }
        if let Some((start, end, label)) = range {
            if end > start + 1 || target != self.buffer.row {
                self.zoom.push(Zoom { start, end, label });
                self.buffer.row = start;
                self.buffer.col = 0;
                self.scroll = 0;
                self.hscroll = 0;
            }
        }
    }

    /// Rename the heading or list item that starts the zoomed section at
    /// `index` (0 = outermost zoom). Markdown/list markers are preserved.
    pub fn rename_zoom(&mut self, index: usize, title: &str, syntax: &MarkdownSyntax) -> bool {
        let Some(zoom) = self.zoom.get(index).cloned() else {
            return false;
        };
        let Some(line) = self.buffer.lines.get(zoom.start) else {
            return false;
        };
        let prefix_end = markdown::heading_level(line, syntax)
            .map(|(_, content)| content)
            .or_else(|| markdown::parse_list(line, syntax).map(|info| info.content_start));
        let Some(prefix_end) = prefix_end else {
            return false;
        };
        let prefix: String = line.chars().take(prefix_end).collect();
        let new_line = format!("{prefix}{title}");
        if new_line != *line {
            self.before_edit();
            self.buffer.lines[zoom.start] = new_line;
        }
        self.zoom[index].label = title.to_string();
        true
    }

    pub fn zoom_out(&mut self) {
        if let Some(z) = self.zoom.pop() {
            self.buffer.row = z.start.min(self.buffer.lines.len().saturating_sub(1));
            self.buffer.col = 0;
            self.scroll = 0;
            self.hscroll = 0;
        }
    }

    pub fn link_at_cursor(&self, syntax: &MarkdownSyntax) -> Option<String> {
        markdown::link_at(self.buffer.line(), self.buffer.col, syntax)
    }

    pub fn line_count(&self) -> usize {
        self.buffer.lines.len()
    }

    pub fn breadcrumbs(&self) -> Vec<String> {
        let mut out = vec![self.file_name()];
        out.extend(self.zoom.iter().map(|z| z.label.clone()));
        out
    }

    pub fn move_cursor_to_end(&mut self) {
        self.buffer.row = self.buffer.lines.len().saturating_sub(1);
        self.buffer.col = self.buffer.line_len(self.buffer.row);
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::MarkdownSyntax;

    fn doc(text: &str) -> Document {
        Document::from_text(PathBuf::from("/tmp/x.md"), "x.md".into(), text)
    }

    #[test]
    fn enter_keeps_list_marker_and_indent() {
        let md = MarkdownSyntax::default();
        let mut d = doc("- one");
        d.buffer.set_cursor(0, 3);
        d.newline(&md);
        assert_eq!(d.buffer.lines, vec!["- o", "- ne"]);
        assert_eq!(d.buffer.row, 1);
        assert_eq!(d.buffer.col, 2);
    }

    #[test]
    fn enter_on_empty_item_ends_list() {
        let md = MarkdownSyntax::default();
        let mut d = doc("  - ");
        d.buffer.set_cursor(0, 4);
        d.newline(&md);
        assert_eq!(d.buffer.lines, vec![""]);
        assert_eq!(d.buffer.col, 0);
    }

    #[test]
    fn checkbox_toggles() {
        let md = MarkdownSyntax::default();
        let mut d = doc("- [ ] task");
        d.toggle_checkbox(&md);
        assert_eq!(d.buffer.line(), "- [x] task");
    }

    #[test]
    fn fold_hides_children() {
        let md = MarkdownSyntax::default();
        let mut d = doc("# A\none\n# B\ntwo");
        d.folded.insert(0);
        assert_eq!(d.visible_indices(&md), vec![0, 2, 3]);
    }

    #[test]
    fn cut_and_paste_lines() {
        let mut d = doc("a\nb\nc");
        d.buffer.set_cursor(1, 0);
        let cut = d.cut_lines();
        assert_eq!(cut, vec!["b"]);
        assert_eq!(d.buffer.lines, vec!["a", "c"]);
        d.paste_lines(&cut);
        assert_eq!(d.buffer.lines, vec!["a", "c", "b"]);
    }

    #[test]
    fn rename_zoom_preserves_marker_and_updates_label() {
        let md = MarkdownSyntax::default();
        let mut d = doc("# Old\nbody\n# Other");
        d.buffer.set_cursor(0, 0);
        d.zoom_in(&md);
        assert_eq!(d.breadcrumbs(), vec!["x.md", "Old"]);
        assert!(d.rename_zoom(0, "New", &md));
        assert_eq!(d.buffer.lines[0], "# New");
        assert_eq!(d.breadcrumbs(), vec!["x.md", "New"]);
        assert!(d.dirty);
    }

    #[test]
    fn indent_block_uses_each_line() {
        let mut d = doc("a\nb");
        d.marks.insert(0);
        d.marks.insert(1);
        d.buffer.set_cursor(0, 0);
        d.indent();
        assert_eq!(d.buffer.lines, vec!["  a", "  b"]);
    }
}
