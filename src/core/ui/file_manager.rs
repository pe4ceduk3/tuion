//! File manager screen: list, marks, preview.

use crate::core::app::AppState;
use crate::core::markdown;
use crate::core::ui::common::panel;
use crate::core::ui::hit::HitId;
use bevy_tui_texture::prelude::HitRegions;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

pub fn draw(frame: &mut Frame, area: Rect, state: &mut AppState, hits: &mut HitRegions) {
    let theme = &state.config.theme;
    let icons = &state.config.icons;
    let wide = area.width >= 86 && area.height >= 14;
    let chunks = if wide {
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).split(area)
    } else {
        Layout::horizontal([Constraint::Percentage(100)]).split(area)
    };

    let list_rect = chunks[0];
    let cwd = state.file_manager.cwd_rel();
    let title = if cwd.is_empty() {
        format!(" {} Notes ", icons.folder)
    } else {
        format!(" {} {} ", icons.folder, cwd)
    };
    let block = panel(title, theme);
    let inner = block.inner(list_rect);
    frame.render_widget(block, list_rect);

    let fm = &mut state.file_manager;
    let parent_row = if fm.cwd.as_os_str().is_empty() { 0 } else { 1 };
    let total = fm.entries.len() + parent_row;
    let avail = inner.height as usize;
    let selected_row = fm.selected + parent_row;
    if selected_row < fm.scroll {
        fm.scroll = selected_row;
    }
    if avail > 0 && selected_row >= fm.scroll + avail {
        fm.scroll = selected_row + 1 - avail;
    }
    fm.scroll = fm.scroll.min(total.saturating_sub(avail));

    for offset in 0..avail {
        let index = fm.scroll + offset;
        let y = inner.y + offset as u16;
        if y >= inner.bottom() {
            break;
        }
        let row_rect = Rect::new(inner.x, y, inner.width, 1);
        if parent_row == 1 && index == 0 {
            let line = Line::from(vec![
                Span::styled(format!("{} ", icons.back), Style::default().fg(theme.dim)),
                Span::styled("..", Style::default().fg(theme.dim)),
            ]);
            frame.render_widget(Paragraph::new(line), row_rect);
            hits.add(HitId::FileRow(u16::MAX), row_rect);
            continue;
        }
        let Some(entry) = fm.entries.get(index - parent_row) else {
            continue;
        };
        let selected = index - parent_row == fm.selected;
        let marked = fm.marks.contains(&entry.abs);
        let mut style = Style::default().fg(if entry.is_dir { theme.accent } else { theme.fg });
        if marked {
            style = style.fg(theme.warning);
        }
        if selected {
            style = style.bg(theme.selection_bg).fg(theme.fg);
        }
        let icon = if entry.is_dir {
            &icons.folder
        } else {
            &icons.file
        };
        let mut spans = vec![
            Span::styled(
                if marked { "● " } else { "  " },
                Style::default().fg(theme.warning),
            ),
            Span::styled(format!("{icon} "), style),
            Span::styled(entry.name.clone(), style),
        ];
        if entry.is_dir {
            spans.push(Span::styled("/", style));
        }
        if entry.is_md {
            spans.push(Span::styled("  md", Style::default().fg(theme.dim)));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)).style(style), row_rect);
        hits.add(HitId::FileRow((index - parent_row) as u16), row_rect);
    }

    if !wide {
        return;
    }

    let preview_rect = chunks[1];
    let (title, preview_is_dir, is_markdown, lines) = match &fm.preview {
        Some(preview) => (
            preview.title.clone(),
            preview.is_dir,
            preview.is_markdown,
            preview.lines.clone(),
        ),
        None => ("Preview".to_string(), false, false, Vec::new()),
    };
    let title = if preview_is_dir {
        format!(" {} {}/ ", icons.folder, title)
    } else {
        format!(" {} {} ", icons.file, title)
    };
    let block = panel(title, theme);
    let inner = block.inner(preview_rect);
    frame.render_widget(block, preview_rect);
    if preview_is_dir {
        frame.render_widget(
            Paragraph::new("Directory\n\nEnter to open, r to rename, d to delete.")
                .style(Style::default().fg(theme.dim)),
            inner,
        );
        return;
    }
    let mut out: Vec<Line> = Vec::new();
    let mut in_code = false;
    for line in lines.iter().take(inner.height as usize) {
        let spans = if is_markdown {
            markdown::render_line(line, &state.config.markdown, theme, icons, &mut in_code)
        } else {
            vec![Span::styled(line.clone(), Style::default().fg(theme.fg))]
        };
        out.push(Line::from(spans));
    }
    if out.is_empty() {
        out.push(Line::from(Span::styled(
            "(empty file)",
            Style::default()
                .fg(theme.dim)
                .add_modifier(Modifier::ITALIC),
        )));
    }
    frame.render_widget(Paragraph::new(Text::from(out)), inner);
}
