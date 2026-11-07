//! Shared chrome: header, breadcrumbs, status line, bottom navigation and
//! transient toasts.

use crate::core::app::{AppState, Screen};
use crate::core::ui::hit::HitId;
use bevy_tui_texture::prelude::HitRegions;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

pub fn draw_header(frame: &mut Frame, area: Rect, state: &AppState, hits: &mut HitRegions) {
    let theme = &state.config.theme;
    let chunks = Layout::horizontal([Constraint::Min(20), Constraint::Length(28)]).split(area);
    let mut left = vec![
        Span::styled(
            "TUION",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}", state.screen.title()),
            Style::default().fg(theme.dim),
        ),
    ];
    if state.editor.as_ref().is_some_and(|doc| doc.dirty) {
        left.push(Span::styled(" ●", Style::default().fg(theme.warning)));
    }
    frame.render_widget(Paragraph::new(Line::from(left)), chunks[0]);

    let right = format!("{} {}", state.now_text(), state.config.icons.config);
    frame.render_widget(
        Paragraph::new(right)
            .alignment(Alignment::Right)
            .style(Style::default().fg(theme.dim)),
        chunks[1],
    );
    let icon_x = chunks[1].right().saturating_sub(1);
    hits.add(
        HitId::HeaderConfig,
        Rect::new(icon_x, chunks[1].y, 1, chunks[1].height),
    );
}

pub fn draw_crumbs(frame: &mut Frame, area: Rect, state: &AppState, hits: &mut HitRegions) {
    let theme = &state.config.theme;
    let icons = &state.config.icons;
    match state.screen {
        Screen::FileManager => {
            let mut text = format!("{} Home", icons.folder);
            let cwd = state.file_manager.cwd_rel();
            if !cwd.is_empty() {
                text.push_str(" > ");
                text.push_str(&cwd.replace('/', " > "));
            }
            if let Some(entry) = state.file_manager.selected_entry() {
                text.push_str(" > ");
                text.push_str(&entry.name);
            }
            if !state.file_manager.marks.is_empty() {
                text.push_str(&format!("  ({} marked)", state.file_manager.marks.len()));
            }
            frame.render_widget(
                Paragraph::new(text).style(Style::default().fg(theme.dim)),
                area,
            );
            hits.add(HitId::FileCrumb, area);
        }
        Screen::Editor => {
            let Some(doc) = &state.editor else {
                return;
            };
            let crumbs = doc.breadcrumbs();
            let mut spans = Vec::new();
            let mut x = area.x;
            for (i, crumb) in crumbs.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(" > ", Style::default().fg(theme.dim)));
                    x = x.saturating_add(3);
                }
                let label = format!("[[{crumb}]]");
                let width = label.chars().count() as u16;
                spans.push(Span::styled(
                    label,
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::UNDERLINED),
                ));
                if x < area.right() {
                    let w = width.min(area.right().saturating_sub(x));
                    hits.add(HitId::EditorCrumb(i as u16), Rect::new(x, area.y, w, 1));
                }
                x = x.saturating_add(width);
            }
            frame.render_widget(Paragraph::new(Line::from(spans)), area);
        }
        Screen::Calendar => {
            let text = format!(
                "{} Calendar > {} > {}",
                icons.calendar,
                state.calendar.month_title(),
                if state.calendar.focus == crate::core::calendar::CalendarFocus::Grid {
                    "grid"
                } else {
                    "tasks"
                }
            );
            frame.render_widget(
                Paragraph::new(text).style(Style::default().fg(theme.dim)),
                area,
            );
        }
        Screen::Alarms => {
            frame.render_widget(
                Paragraph::new(format!("{} Alarm-clock", icons.alarm))
                    .style(Style::default().fg(theme.dim)),
                area,
            );
        }
        Screen::Timers => {
            frame.render_widget(
                Paragraph::new(format!("{} Timer", icons.timer))
                    .style(Style::default().fg(theme.dim)),
                area,
            );
        }
        Screen::Config => {
            frame.render_widget(
                Paragraph::new(format!(
                    "{} Config  {}",
                    icons.config,
                    state.layout.config.display()
                ))
                .style(Style::default().fg(theme.dim)),
                area,
            );
        }
    }
}

pub fn draw_status(frame: &mut Frame, area: Rect, state: &AppState) {
    let theme = &state.config.theme;
    let hint = match state.screen {
        Screen::FileManager => {
            "↑↓ select  Enter open  n new  N folder  r rename  c/x/p copy/cut/paste  d delete  b path  g config  ? help"
        }
        Screen::Editor => {
            "Ctrl+S save  Ctrl+Z undo  Tab indent  F2 fold  Ctrl+↑/↓ zoom  Ctrl+Space mark  Ctrl+Enter link  click crumb edit  Esc back"
        }
        Screen::Calendar => {
            "←→ day  ↑↓ week  PgUp/PgDn month  n new  e edit  Space done  Tab focus  Esc back"
        }
        Screen::Alarms => "n new  Enter edit  Space on/off  d delete  Esc back  ? help",
        Screen::Timers => "n new  Space start/pause  s stop  e edit  d delete  Esc back",
        Screen::Config => "Ctrl+S save  Ctrl+R reset  Esc back",
    };
    let chunks = Layout::horizontal([Constraint::Min(20), Constraint::Length(28)]).split(area);
    frame.render_widget(
        Paragraph::new(hint).style(Style::default().fg(theme.dim)),
        chunks[0],
    );
    let right = if state.screen == Screen::Editor {
        if let Some(doc) = &state.editor {
            format!(
                "Ln {}, Col {}{}{}",
                doc.buffer.row + 1,
                doc.buffer.col + 1,
                if doc.dirty { "  modified" } else { "" },
                if doc.marks.is_empty() {
                    String::new()
                } else {
                    format!("  {} marked", doc.marks.len())
                }
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    frame.render_widget(
        Paragraph::new(right)
            .alignment(Alignment::Right)
            .style(Style::default().fg(theme.dim)),
        chunks[1],
    );
}

pub fn draw_nav(frame: &mut Frame, area: Rect, state: &AppState, hits: &mut HitRegions) {
    let theme = &state.config.theme;
    let icons = &state.config.icons;
    let chunks = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ])
    .split(area);
    let entries = [
        (icons.alarm.as_str(), "Alarm"),
        (icons.timer.as_str(), "Timer"),
        (icons.calendar.as_str(), "Calendar"),
        (icons.tasks.as_str(), "Tasks"),
    ];
    let active = state.nav_active();
    for (i, (icon, label)) in entries.iter().enumerate() {
        let selected = active == Some(i as u8);
        let style = if selected {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dim)
        };
        let line = Line::from(vec![
            Span::styled(format!("{icon} "), style),
            Span::styled(*label, style),
        ]);
        frame.render_widget(
            Paragraph::new(line)
                .alignment(Alignment::Center)
                .style(style),
            chunks[i],
        );
        hits.add(HitId::Nav(i as u8), chunks[i]);
    }
}

pub fn draw_toasts(frame: &mut Frame, area: Rect, state: &AppState) {
    let theme = &state.config.theme;
    let mut y = area.bottom().saturating_sub(3);
    for toast in state.toasts.iter().rev().take(3) {
        let width = (toast.text.chars().count() as u16 + 4)
            .min(area.width.saturating_sub(4))
            .max(10);
        let rect = Rect::new(area.x + 2, y, width, 1);
        frame.render_widget(Clear, rect);
        let style = if toast.error {
            Style::default().bg(theme.code_bg).fg(theme.error)
        } else {
            Style::default().bg(theme.code_bg).fg(theme.success)
        };
        frame.render_widget(
            Paragraph::new(format!(" {} ", toast.text)).style(style),
            rect,
        );
        y = y.saturating_sub(2);
        if y <= area.y {
            break;
        }
    }
}

pub fn centered_rect(area: Rect, max_width: u16, max_height: u16) -> Rect {
    let width = area.width.min(max_width).max(20);
    let height = area.height.min(max_height).max(5);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

pub fn panel<'a>(title: impl Into<String>, theme: &crate::core::config::Theme) -> Block<'a> {
    Block::bordered()
        .title(title.into())
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().fg(theme.fg))
}
