//! Note editor: live markdown preview, raw syntax on the focused line,
//! folding gutter, zoom breadcrumbs and outliner cursor.

use crate::core::app::AppState;
use crate::core::markdown;
use crate::core::ui::common::panel;
use crate::core::ui::hit::HitId;
use bevy_tui_texture::prelude::HitRegions;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const GUTTER: u16 = 2;

pub fn draw(frame: &mut Frame, area: Rect, state: &mut AppState, hits: &mut HitRegions) {
    let Some(doc) = state.editor.as_mut() else {
        return;
    };
    let theme = state.config.theme.clone();
    let icons = state.config.icons.clone();
    let syntax = state.config.markdown.clone();

    let title = format!(" {} {} ", icons.note, doc.rel_path);
    let block = panel(title, &theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let visible = doc.visible_indices(&syntax);
    let cursor_visual = doc.visual_index(&syntax);
    let viewport = inner.height as usize;

    // Keep the cursor on screen.
    if cursor_visual < doc.scroll {
        doc.scroll = cursor_visual;
    }
    if viewport > 0 && cursor_visual >= doc.scroll + viewport {
        doc.scroll = cursor_visual + 1 - viewport;
    }
    doc.scroll = doc
        .scroll
        .min(visible.len().saturating_sub(viewport.min(visible.len())));

    let text_width = inner.width.saturating_sub(GUTTER) as usize;
    if text_width > 0 {
        if doc.buffer.col < doc.hscroll {
            doc.hscroll = doc.buffer.col;
        }
        if doc.buffer.col >= doc.hscroll + text_width {
            doc.hscroll = doc.buffer.col + 1 - text_width;
        }
    } else {
        doc.hscroll = 0;
    }

    doc.content_x = inner.x + GUTTER;
    doc.content_width = inner.width.saturating_sub(GUTTER);

    // Reconstruct fenced-code state for the first visible row.
    let start = doc.scroll.min(visible.len());
    let end = (start + viewport).min(visible.len());
    let mut in_code = false;
    for &index in &visible[..start] {
        if doc.buffer.line_at(index).trim_start().starts_with("```") {
            in_code = !in_code;
        }
    }

    if visible.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "(empty note)",
                Style::default()
                    .fg(theme.dim)
                    .add_modifier(Modifier::ITALIC),
            )),
            inner,
        );
        return;
    }

    for (offset, &index) in visible[start..end].iter().enumerate() {
        let y = inner.y + offset as u16;
        if y >= inner.bottom() {
            break;
        }
        let row_rect = Rect::new(inner.x, y, inner.width, 1);
        let is_active = index == doc.buffer.row;
        let marked = doc.marks.contains(&index);
        let folded = doc.folded.contains(&index);
        let foldable = markdown::fold_end(&doc.buffer.lines, index, &syntax).is_some();

        let mut row_style = Style::default().fg(theme.fg);
        if marked {
            row_style = row_style.bg(theme.marked_bg);
        }
        if is_active {
            row_style = row_style.bg(theme.selection_bg);
        }

        let fold_marker = if foldable {
            if folded {
                icons.fold_closed.clone()
            } else {
                icons.fold_open.clone()
            }
        } else {
            " ".to_string()
        };
        let mut spans = vec![
            Span::styled(
                fold_marker,
                Style::default().fg(if foldable { theme.accent } else { theme.dim }),
            ),
            Span::raw(" "),
        ];

        let line = doc.buffer.line_at(index);
        if is_active {
            let chars: Vec<char> = line.chars().collect();
            let col = doc.buffer.col.min(chars.len());
            let rel = col.saturating_sub(doc.hscroll);
            let take = text_width.saturating_add(1);
            let view: Vec<char> = chars.iter().skip(doc.hscroll).take(take).cloned().collect();

            let before: String = view.iter().take(rel.min(view.len())).collect();
            if !before.is_empty() {
                spans.push(Span::styled(before, Style::default().fg(theme.fg)));
            }
            let cursor_ch = view.get(rel).copied().unwrap_or(' ');
            spans.push(Span::styled(
                cursor_ch.to_string(),
                Style::default()
                    .fg(theme.bg)
                    .bg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ));
            if rel + 1 < view.len() {
                let after: String = view.iter().skip(rel + 1).collect();
                spans.push(Span::styled(after, Style::default().fg(theme.fg)));
            }
        } else {
            let preview = markdown::render_line(line, &syntax, &theme, &icons, &mut in_code);
            spans.extend(slice_spans(preview, doc.hscroll, text_width));
        }

        frame.render_widget(Paragraph::new(Line::from(spans)).style(row_style), row_rect);
        hits.add(HitId::EditorLine(index as u16), row_rect);
        if foldable {
            hits.add(
                HitId::EditorFold(index as u16),
                Rect::new(inner.x, y, GUTTER, 1),
            );
        }
    }

    // A subtle scrollbar on the right edge when the note is longer than the
    // viewport.
    if visible.len() > viewport && viewport > 1 {
        let track_h = inner.height as usize;
        let thumb_h = (viewport * track_h / visible.len()).max(1);
        let max_scroll = visible.len().saturating_sub(viewport);
        let pos = if max_scroll == 0 {
            0
        } else {
            start * (track_h.saturating_sub(thumb_h)) / max_scroll
        };
        let x = inner.right().saturating_sub(1);
        for i in 0..track_h {
            let y = inner.y + i as u16;
            let style = if i >= pos && i < pos + thumb_h {
                Style::default().fg(theme.accent)
            } else {
                Style::default().fg(theme.border)
            };
            frame.render_widget(
                Paragraph::new(Span::styled("│", style)),
                Rect::new(x, y, 1, 1),
            );
        }
    }
}

/// Drop `skip` cells from the front of styled spans, keeping at most `width`
/// cells. Character-indexed, like the rest of the editor.
fn slice_spans(spans: Vec<Span<'static>>, skip: usize, width: usize) -> Vec<Span<'static>> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut to_skip = skip;
    let mut left = width;
    for span in spans {
        if left == 0 {
            break;
        }
        let text = span.content.into_owned();
        let chars: Vec<char> = text.chars().collect();
        if to_skip >= chars.len() {
            to_skip -= chars.len();
            continue;
        }
        let start = to_skip.min(chars.len());
        to_skip = 0;
        let take = (chars.len() - start).min(left);
        let piece: String = chars[start..start + take].iter().collect();
        if !piece.is_empty() {
            out.push(Span::styled(piece, span.style));
            left -= take;
        }
    }
    out
}
