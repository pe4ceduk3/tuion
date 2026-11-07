//! In-app config editor. The file is shown verbatim, comments included.

use crate::core::app::AppState;
use crate::core::ui::common::panel;
use crate::core::ui::hit::HitId;
use bevy_tui_texture::prelude::HitRegions;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub fn draw(frame: &mut Frame, area: Rect, state: &mut AppState, hits: &mut HitRegions) {
    let theme = state.config.theme.clone();
    let title = format!(
        " Config · {}{} ",
        state.layout.config.display(),
        if state.config_dirty {
            " · modified"
        } else {
            ""
        }
    );
    let block = panel(title, &theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let total = state.config_editor.lines.len();
    let width = total.to_string().len().max(3) as u16;
    let gutter = width + 1;
    state.config_content_x = inner.x + gutter;

    let viewport = inner.height as usize;
    let cursor = state.config_editor.row;
    if cursor < state.config_editor.scroll {
        state.config_editor.scroll = cursor;
    }
    if viewport > 0 && cursor >= state.config_editor.scroll + viewport {
        state.config_editor.scroll = cursor + 1 - viewport;
    }
    state.config_editor.scroll = state
        .config_editor
        .scroll
        .min(total.saturating_sub(viewport.min(total)));

    let start = state.config_editor.scroll.min(total);
    let end = (start + viewport).min(total);
    for row in start..end {
        let y = inner.y + (row - start) as u16;
        if y >= inner.bottom() {
            break;
        }
        let rect = Rect::new(inner.x, y, inner.width, 1);
        let active = row == state.config_editor.row;
        let row_style = if active {
            Style::default().bg(theme.selection_bg).fg(theme.fg)
        } else {
            Style::default().fg(theme.fg)
        };
        let number = format!("{:>width$} ", row + 1, width = width as usize);
        let mut spans = vec![Span::styled(number, Style::default().fg(theme.dim))];

        let line = state.config_editor.line_at(row);
        if active {
            let chars: Vec<char> = line.chars().collect();
            let col = state.config_editor.col.min(chars.len());
            let before: String = chars.iter().take(col).collect();
            let cursor_ch = chars.get(col).copied().unwrap_or(' ');
            if !before.is_empty() {
                spans.push(Span::styled(before, Style::default().fg(theme.fg)));
            }
            spans.push(Span::styled(
                cursor_ch.to_string(),
                Style::default()
                    .fg(theme.bg)
                    .bg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ));
            if col + 1 < chars.len() {
                let after: String = chars.iter().skip(col + 1).collect();
                spans.push(Span::styled(after, Style::default().fg(theme.fg)));
            }
        } else if !line.is_empty() {
            let style = if line.trim_start().starts_with('#') {
                Style::default().fg(theme.dim)
            } else if line.trim_start().starts_with('[') {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg)
            };
            spans.push(Span::styled(line.to_string(), style));
        }

        frame.render_widget(Paragraph::new(Line::from(spans)).style(row_style), rect);
        hits.add(HitId::ConfigLine(row as u16), rect);
    }

    if total > viewport && viewport > 1 {
        let track_h = inner.height as usize;
        let thumb_h = (viewport * track_h / total).max(1);
        let max_scroll = total.saturating_sub(viewport);
        let pos = if max_scroll == 0 {
            0
        } else {
            start * (track_h.saturating_sub(thumb_h)) / max_scroll
        };
        let x = inner.right().saturating_sub(1);
        for i in 0..track_h {
            let style = if i >= pos && i < pos + thumb_h {
                Style::default().fg(theme.accent)
            } else {
                Style::default().fg(theme.border)
            };
            frame.render_widget(
                Paragraph::new(Span::styled("│", style)),
                Rect::new(x, inner.y + i as u16, 1, 1),
            );
        }
    }
}
