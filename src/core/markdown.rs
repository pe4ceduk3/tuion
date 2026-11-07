//! Markdown/outliner parsing and preview rendering.
//!
//! The dialect is configurable (see `config::MarkdownSyntax`). Preview
//! rendering intentionally changes colour and modifiers rather than font
//! size, because this is a terminal UI.

use crate::core::config::{Icons, MarkdownSyntax, Theme};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

#[derive(Clone, Debug)]
pub struct ListInfo {
    pub indent: usize,
    pub marker: String,
    /// Character index where the item content starts (after marker,
    /// checkbox and the following space).
    pub content_start: usize,
    /// The marker including the trailing space, e.g. `"- "`.
    pub prefix_len: usize,
    pub checked: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct WikiLink {
    pub start: usize,
    pub end: usize,
    pub target: String,
    pub label: String,
}

pub fn heading_level(line: &str, syntax: &MarkdownSyntax) -> Option<(usize, usize)> {
    let chars: Vec<char> = line.chars().collect();
    for (level, marker) in syntax.headings_longest_first() {
        let m: Vec<char> = marker.chars().collect();
        if m.is_empty() || !starts_with_at(&chars, 0, &m) {
            continue;
        }
        let after = m.len();
        if after == chars.len() || chars.get(after) == Some(&' ') || chars.get(after) == Some(&'\t')
        {
            let mut content = after;
            while content < chars.len() && chars[content].is_whitespace() {
                content += 1;
            }
            return Some((level, content));
        }
    }
    None
}

pub fn parse_list(line: &str, syntax: &MarkdownSyntax) -> Option<ListInfo> {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
        i += 1;
    }
    let indent = i;
    if i >= chars.len() {
        return None;
    }

    let marker_start = i;
    let marker = if matches!(chars[i], '-' | '*' | '+') {
        if chars.get(i + 1) != Some(&' ') {
            return None;
        }
        let m = chars[i].to_string();
        i += 2;
        m
    } else if chars[i].is_ascii_digit() {
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        if i >= chars.len() || (chars[i] != '.' && chars[i] != ')') {
            return None;
        }
        let m: String = chars[marker_start..=i].iter().collect();
        i += 1;
        if chars.get(i) != Some(&' ') {
            return None;
        }
        i += 1;
        m
    } else {
        return None;
    };

    let mut checked = None;
    let task_open: Vec<char> = syntax.task_open.chars().collect();
    let task_done: Vec<char> = syntax.task_done.chars().collect();
    if starts_with_at(&chars, i, &task_open) {
        let end = i + task_open.len();
        if chars.get(end) == Some(&' ') {
            checked = Some(false);
            i = end + 1;
        }
    } else if starts_with_at(&chars, i, &task_done) {
        let end = i + task_done.len();
        if chars.get(end) == Some(&' ') {
            checked = Some(true);
            i = end + 1;
        }
    }

    Some(ListInfo {
        indent,
        marker,
        content_start: i,
        prefix_len: i,
        checked,
    })
}

pub fn wiki_links(line: &str, syntax: &MarkdownSyntax) -> Vec<WikiLink> {
    let chars: Vec<char> = line.chars().collect();
    let open: Vec<char> = syntax.link_open.chars().collect();
    let close: Vec<char> = syntax.link_close.chars().collect();
    if open.is_empty() || close.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if starts_with_at(&chars, i, &open) {
            if let Some(rel) = find_sub(&chars, i + open.len(), &close) {
                let end = rel + close.len();
                let inside: String = chars[i + open.len()..rel].iter().collect();
                let (target, label) = match inside.split_once('|') {
                    Some((t, l)) => (t.trim().to_string(), l.trim().to_string()),
                    None => (inside.trim().to_string(), inside.trim().to_string()),
                };
                if !target.is_empty() {
                    out.push(WikiLink {
                        start: i,
                        end,
                        target,
                        label,
                    });
                }
                i = end;
                continue;
            }
        }
        i += 1;
    }
    out
}

pub fn link_at(line: &str, col: usize, syntax: &MarkdownSyntax) -> Option<String> {
    wiki_links(line, syntax)
        .into_iter()
        .find(|l| col >= l.start && col < l.end)
        .map(|l| l.target)
}

pub fn toggle_checkbox(line: &str, syntax: &MarkdownSyntax) -> Option<String> {
    let mut chars: Vec<char> = line.chars().collect();
    let open: Vec<char> = syntax.task_open.chars().collect();
    let done: Vec<char> = syntax.task_done.chars().collect();
    if let Some(i) = find_sub(&chars, 0, &open) {
        if i + open.len() <= chars.len() {
            chars.splice(i..i + open.len(), done);
            return Some(chars.into_iter().collect());
        }
    }
    if let Some(i) = find_sub(&chars, 0, &done) {
        if i + done.len() <= chars.len() {
            chars.splice(i..i + done.len(), open);
            return Some(chars.into_iter().collect());
        }
    }
    None
}

/// Last line belonging to the foldable item at `start` (exclusive end).
/// Headings fold until the next heading of the same or higher level; list
/// items fold until the next non-empty line with the same or smaller indent.
pub fn fold_end(lines: &[String], start: usize, syntax: &MarkdownSyntax) -> Option<usize> {
    if start >= lines.len() {
        return None;
    }
    if let Some((level, _)) = heading_level(&lines[start], syntax) {
        let mut end = lines.len();
        for (j, line) in lines.iter().enumerate().skip(start + 1) {
            if let Some((next, _)) = heading_level(line, syntax) {
                if next <= level {
                    end = j;
                    break;
                }
            }
        }
        while end > start + 1 && lines[end - 1].trim().is_empty() {
            end -= 1;
        }
        return (end > start + 1).then_some(end);
    }
    let info = parse_list(&lines[start], syntax)?;
    let mut end = lines.len();
    for (j, line) in lines.iter().enumerate().skip(start + 1) {
        if line.trim().is_empty() {
            continue;
        }
        if heading_level(line, syntax).is_some() {
            end = j;
            break;
        }
        if let Some(next) = parse_list(line, syntax) {
            if next.indent <= info.indent {
                end = j;
                break;
            }
        } else {
            let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
            if indent <= info.indent {
                end = j;
                break;
            }
        }
    }
    while end > start + 1 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    (end > start + 1).then_some(end)
}

/// The range shown when zooming into the item at `line`.
pub fn zoom_range(
    lines: &[String],
    line: usize,
    syntax: &MarkdownSyntax,
) -> Option<(usize, usize, String)> {
    if line >= lines.len() {
        return None;
    }
    if let Some((level, content)) = heading_level(&lines[line], syntax) {
        let mut end = lines.len();
        for (j, l) in lines.iter().enumerate().skip(line + 1) {
            if let Some((next, _)) = heading_level(l, syntax) {
                if next <= level {
                    end = j;
                    break;
                }
            }
        }
        while end > line + 1 && lines[end - 1].trim().is_empty() {
            end -= 1;
        }
        let label: String = lines[line].chars().skip(content).collect();
        return (end > line + 1).then_some((line, end, label.trim().to_string()));
    }
    if parse_list(&lines[line], syntax).is_some() {
        let end = fold_end(lines, line, syntax)?;
        let label = plain_text(&lines[line], syntax);
        return Some((line, end, label));
    }
    None
}

/// A short, marker-free description of a line (for breadcrumbs and zoom
/// labels).
pub fn plain_text(line: &str, syntax: &MarkdownSyntax) -> String {
    let mut text = line.to_string();
    if let Some((_, content)) = heading_level(line, syntax) {
        text = line.chars().skip(content).collect();
    } else if let Some(info) = parse_list(line, syntax) {
        text = line.chars().skip(info.content_start).collect();
    } else {
        text = text.trim_start_matches('>').trim_start().to_string();
    }
    text = text
        .replace(&syntax.bold, "")
        .replace(&syntax.italic, "")
        .replace(&syntax.strike, "")
        .replace(&syntax.highlight, "")
        .replace(&syntax.code, "")
        .replace(&syntax.link_open, "")
        .replace(&syntax.link_close, "");
    text.trim().to_string()
}

/// Render one non-focused line to styled spans.
pub fn render_line(
    line: &str,
    syntax: &MarkdownSyntax,
    theme: &Theme,
    icons: &Icons,
    in_code: &mut bool,
) -> Vec<Span<'static>> {
    let trimmed = line.trim_start();

    if trimmed.starts_with("```") {
        *in_code = !*in_code;
        return vec![Span::styled(
            line.to_string(),
            Style::default().fg(theme.dim).bg(theme.code_bg),
        )];
    }
    if *in_code {
        return vec![Span::styled(
            line.to_string(),
            Style::default().fg(theme.code_fg).bg(theme.code_bg),
        )];
    }

    if let Some((level, content)) = heading_level(line, syntax) {
        let text: String = line.chars().skip(content).collect();
        let mut spans = vec![Span::styled(
            "▌ ".to_string(),
            Style::default().fg(theme.heading(level)),
        )];
        spans.extend(inline_spans(
            &text,
            syntax,
            theme,
            Style::default()
                .fg(theme.heading(level))
                .add_modifier(Modifier::BOLD),
        ));
        return spans;
    }

    if is_rule(line) {
        return vec![Span::styled(
            "────────────────────────────────────────".to_string(),
            Style::default().fg(theme.border),
        )];
    }

    if trimmed.starts_with('>') {
        let depth = line
            .chars()
            .take_while(|c| *c == '>' || *c == ' ')
            .filter(|c| *c == '>')
            .count();
        let rest: String = line
            .chars()
            .skip_while(|c| *c == ' ' || *c == '>')
            .collect();
        let mut spans = vec![Span::styled(
            format!("{} ", "│".repeat(depth.max(1))),
            Style::default().fg(theme.border),
        )];
        spans.extend(inline_spans(
            &rest,
            syntax,
            theme,
            Style::default()
                .fg(theme.dim)
                .add_modifier(Modifier::ITALIC),
        ));
        return spans;
    }

    if let Some(info) = parse_list(line, syntax) {
        let mut spans = vec![Span::raw(" ".repeat(info.indent))];
        let (marker, marker_style) = match info.checked {
            Some(true) => (icons.check_done.clone(), Style::default().fg(theme.success)),
            Some(false) => (icons.check_todo.clone(), Style::default().fg(theme.accent)),
            None => {
                if info
                    .marker
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '.' || c == ')')
                {
                    (info.marker.clone(), Style::default().fg(theme.dim))
                } else {
                    ("•".to_string(), Style::default().fg(theme.accent))
                }
            }
        };
        spans.push(Span::styled(format!("{marker} "), marker_style));
        let content: String = line.chars().skip(info.content_start).collect();
        let base = if info.checked == Some(true) {
            Style::default()
                .fg(theme.dim)
                .add_modifier(Modifier::CROSSED_OUT)
        } else {
            Style::default().fg(theme.fg)
        };
        spans.extend(inline_spans(&content, syntax, theme, base));
        return spans;
    }

    if trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.matches('|').count() >= 2 {
        return vec![Span::styled(
            line.to_string(),
            Style::default().fg(theme.dim),
        )];
    }

    inline_spans(line, syntax, theme, Style::default().fg(theme.fg))
}

fn is_rule(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 3
        && (t.chars().all(|c| c == '-')
            || t.chars().all(|c| c == '*')
            || t.chars().all(|c| c == '_'))
}

#[derive(Default, Clone, Copy)]
struct InlineState {
    bold: bool,
    italic: bool,
    strike: bool,
    highlight: bool,
}

impl InlineState {
    fn style(&self, base: Style, theme: &Theme) -> Style {
        let mut style = base;
        if self.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        if self.italic {
            style = style.add_modifier(Modifier::ITALIC).fg(theme.h4);
        }
        if self.strike {
            style = style.add_modifier(Modifier::CROSSED_OUT).fg(theme.dim);
        }
        if self.highlight {
            style = style.bg(theme.selection_bg);
        }
        style
    }
}

pub fn inline_spans(
    text: &str,
    syntax: &MarkdownSyntax,
    theme: &Theme,
    base: Style,
) -> Vec<Span<'static>> {
    let chars: Vec<char> = text.chars().collect();
    let bold: Vec<char> = syntax.bold.chars().collect();
    let italic: Vec<char> = syntax.italic.chars().collect();
    let strike: Vec<char> = syntax.strike.chars().collect();
    let highlight: Vec<char> = syntax.highlight.chars().collect();
    let code: Vec<char> = syntax.code.chars().collect();
    let open: Vec<char> = syntax.link_open.chars().collect();
    let close: Vec<char> = syntax.link_close.chars().collect();

    let mut state = InlineState::default();
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut i = 0;

    macro_rules! flush {
        () => {
            if !buf.is_empty() {
                out.push(Span::styled(
                    std::mem::take(&mut buf),
                    state.style(base, theme),
                ));
            }
        };
    }

    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            buf.push(chars[i + 1]);
            i += 2;
            continue;
        }

        if !open.is_empty() && !close.is_empty() && starts_with_at(&chars, i, &open) {
            if let Some(rel) = find_sub(&chars, i + open.len(), &close) {
                let end = rel + close.len();
                let inside: String = chars[i + open.len()..rel].iter().collect();
                let label = inside.split_once('|').map(|(_, l)| l).unwrap_or(&inside);
                flush!();
                out.push(Span::styled(
                    label.trim().to_string(),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::UNDERLINED),
                ));
                i = end;
                continue;
            }
        }

        if !code.is_empty() && starts_with_at(&chars, i, &code) {
            if let Some(rel) = find_sub(&chars, i + code.len(), &code) {
                let end = rel + code.len();
                let inside: String = chars[i + code.len()..rel].iter().collect();
                flush!();
                out.push(Span::styled(
                    inside,
                    Style::default().fg(theme.code_fg).bg(theme.code_bg),
                ));
                i = end;
                continue;
            }
        }

        if !bold.is_empty() && starts_with_at(&chars, i, &bold) {
            flush!();
            state.bold = !state.bold;
            i += bold.len();
            continue;
        }
        if !italic.is_empty() && starts_with_at(&chars, i, &italic) {
            flush!();
            state.italic = !state.italic;
            i += italic.len();
            continue;
        }
        if !strike.is_empty() && starts_with_at(&chars, i, &strike) {
            flush!();
            state.strike = !state.strike;
            i += strike.len();
            continue;
        }
        if !highlight.is_empty() && starts_with_at(&chars, i, &highlight) {
            flush!();
            state.highlight = !state.highlight;
            i += highlight.len();
            continue;
        }

        // [text](url)
        if chars[i] == '[' {
            if let Some(close_bracket) = chars
                .iter()
                .enumerate()
                .skip(i + 1)
                .find(|(_, c)| **c == ']')
                .map(|(j, _)| j)
            {
                if chars.get(close_bracket + 1) == Some(&'(') {
                    if let Some(close_paren) = chars
                        .iter()
                        .enumerate()
                        .skip(close_bracket + 2)
                        .find(|(_, c)| **c == ')')
                        .map(|(j, _)| j)
                    {
                        let label: String = chars[i + 1..close_bracket].iter().collect();
                        flush!();
                        out.push(Span::styled(
                            label,
                            Style::default()
                                .fg(theme.accent)
                                .add_modifier(Modifier::UNDERLINED),
                        ));
                        i = close_paren + 1;
                        continue;
                    }
                }
            }
        }

        // #tag
        if chars[i] == '#' {
            let boundary = i == 0 || chars[i - 1].is_whitespace();
            let mut j = i + 1;
            while j < chars.len()
                && (chars[j].is_alphanumeric() || chars[j] == '_' || chars[j] == '-')
            {
                j += 1;
            }
            if boundary && j > i + 1 {
                flush!();
                let tag: String = chars[i..j].iter().collect();
                out.push(Span::styled(tag, Style::default().fg(theme.warning)));
                i = j;
                continue;
            }
        }

        buf.push(chars[i]);
        i += 1;
    }
    flush!();
    if out.is_empty() {
        out.push(Span::styled(String::new(), base));
    }
    out
}

fn starts_with_at(chars: &[char], at: usize, pat: &[char]) -> bool {
    if pat.is_empty() || at + pat.len() > chars.len() {
        return false;
    }
    chars[at..at + pat.len()] == *pat
}

fn find_sub(chars: &[char], from: usize, pat: &[char]) -> Option<usize> {
    if pat.is_empty() || from > chars.len() {
        return None;
    }
    (from..=chars.len().saturating_sub(pat.len())).find(|&i| starts_with_at(chars, i, pat))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::AppConfig;

    fn cfg() -> (MarkdownSyntax, Theme, Icons) {
        let c = AppConfig::parse(crate::core::config::DEFAULT_CONFIG).unwrap();
        (c.markdown, c.theme, c.icons)
    }

    #[test]
    fn headings_use_longest_marker() {
        let (md, _, _) = cfg();
        assert_eq!(heading_level("## Two", &md).unwrap().0, 2);
        assert_eq!(heading_level("# One", &md).unwrap().0, 1);
        assert_eq!(heading_level("#tag", &md), None);
    }

    #[test]
    fn custom_heading_syntax() {
        let mut md = MarkdownSyntax::default();
        md.h[0] = "#1".into();
        assert_eq!(heading_level("#1 One", &md).unwrap().0, 1);
    }

    #[test]
    fn lists_and_checkboxes_parse() {
        let (md, _, _) = cfg();
        let li = parse_list("  - [x] done", &md).unwrap();
        assert_eq!(li.indent, 2);
        assert_eq!(li.checked, Some(true));
        assert_eq!(li.content_start, 8);
        assert!(parse_list("plain", &md).is_none());
    }

    #[test]
    fn wiki_links_parse_label() {
        let (md, _, _) = cfg();
        let links = wiki_links("see [[note|the note]] now", &md);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "note");
        assert_eq!(links[0].label, "the note");
    }

    #[test]
    fn checkbox_toggles() {
        let (md, _, _) = cfg();
        assert_eq!(toggle_checkbox("- [ ] x", &md).unwrap(), "- [x] x");
        assert_eq!(toggle_checkbox("- [x] x", &md).unwrap(), "- [ ] x");
    }

    #[test]
    fn fold_heading_and_list() {
        let (md, _, _) = cfg();
        let lines: Vec<String> = vec![
            "# A".into(),
            "one".into(),
            "## B".into(),
            "two".into(),
            "# C".into(),
        ];
        assert_eq!(fold_end(&lines, 0, &md), Some(4));
        let lines = vec![
            "- a".into(),
            "  - b".into(),
            "    text".into(),
            "- c".into(),
        ];
        assert_eq!(fold_end(&lines, 0, &md), Some(3));
    }

    #[test]
    fn preview_strips_markers() {
        let (md, theme, icons) = cfg();
        let mut in_code = false;
        let spans = render_line("## Title **bold**", &md, &theme, &icons, &mut in_code);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("Title"));
        assert!(text.contains("bold"));
        assert!(!text.contains("##"));
        assert!(!text.contains("**"));
    }
}
