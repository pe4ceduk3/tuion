//! Calendar screen: month grid on the left, the selected day's timeline and
//! task cards on the right.

use crate::core::app::AppState;
use crate::core::calendar::{CalendarFocus, EventColor};
use crate::core::ui::common::panel;
use crate::core::ui::hit::HitId;
use bevy_tui_texture::prelude::HitRegions;
use chrono::Datelike;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub fn draw(frame: &mut Frame, area: Rect, state: &mut AppState, hits: &mut HitRegions) {
    let theme = state.config.theme.clone();
    let icons = state.config.icons.clone();
    let wide = area.width >= 82;
    let chunks = if wide {
        Layout::horizontal([Constraint::Percentage(44), Constraint::Percentage(56)]).split(area)
    } else {
        Layout::vertical([Constraint::Length(10), Constraint::Min(4)]).split(area)
    };

    draw_month(frame, chunks[0], state, hits, &theme, &icons);
    draw_timeline(frame, chunks[1], state, hits, &theme, &icons);
}

fn draw_month(
    frame: &mut Frame,
    area: Rect,
    state: &mut AppState,
    hits: &mut HitRegions,
    theme: &crate::core::config::Theme,
    icons: &crate::core::config::Icons,
) {
    let title = format!(" {} {} ", icons.calendar, state.calendar.month_title());
    let block = panel(title, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let focus_style = if state.calendar.focus == CalendarFocus::Grid {
        Style::default().fg(theme.accent)
    } else {
        Style::default().fg(theme.dim)
    };
    let headers = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
    let cell_w = (inner.width / 7).max(1);
    for (i, label) in headers.iter().enumerate() {
        let x = inner.x + i as u16 * cell_w;
        let w = if i == 6 {
            inner.right().saturating_sub(x)
        } else {
            cell_w.min(inner.right().saturating_sub(x))
        };
        frame.render_widget(
            Paragraph::new(*label)
                .alignment(Alignment::Center)
                .style(focus_style.add_modifier(Modifier::BOLD)),
            Rect::new(x, inner.y, w, 1),
        );
    }

    let grid = state.calendar.month_grid();
    let today = chrono::Local::now().date_naive();
    let selected = state.calendar.selected_date;
    let visible_month = state.calendar.visible_month.month();
    let mut rows = inner.height.saturating_sub(1) as usize;
    rows = rows.min(6);
    for (index, date) in grid.iter().enumerate().take(rows * 7) {
        let row = index / 7;
        let col = index % 7;
        let x = inner.x + col as u16 * cell_w;
        let y = inner.y + 1 + row as u16;
        if y >= inner.bottom() {
            break;
        }
        let w = if col == 6 {
            inner.right().saturating_sub(x)
        } else {
            cell_w.min(inner.right().saturating_sub(x))
        };
        let rect = Rect::new(x, y, w, 1);
        let is_selected = *date == selected;
        let is_today = *date == today;
        let in_month = date.month() == visible_month;
        let has_events = state.calendar.has_events(*date);

        let mut style = if is_selected {
            Style::default()
                .bg(theme.selection_bg)
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else if is_today {
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else if in_month {
            Style::default().fg(theme.fg)
        } else {
            Style::default().fg(theme.dim)
        };
        if is_selected && is_today {
            style = style.fg(theme.warning);
        }
        let mut label = format!("{:02}", date.day());
        if has_events {
            label.push('·');
        }
        frame.render_widget(
            Paragraph::new(label)
                .alignment(Alignment::Center)
                .style(style),
            rect,
        );
        hits.add(HitId::CalDay(index as u16), rect);
    }
}

fn draw_timeline(
    frame: &mut Frame,
    area: Rect,
    state: &mut AppState,
    hits: &mut HitRegions,
    theme: &crate::core::config::Theme,
    icons: &crate::core::config::Icons,
) {
    let date = state.calendar.selected_date;
    let title = format!(
        " {} {} {} ",
        icons.tasks,
        date.format("%a %d.%m.%Y"),
        if state.calendar.focus == CalendarFocus::Tasks {
            "· focused"
        } else {
            ""
        }
    );
    let block = panel(title, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let occurrences = state.calendar.selected_occurrences();
    if occurrences.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "No tasks for this day.",
                    Style::default().fg(theme.dim),
                )),
                Line::from(Span::styled(
                    "Press n to add one, t to jump to today.",
                    Style::default().fg(theme.dim),
                )),
            ]),
            inner,
        );
        return;
    }

    // Each card is three rows: name, time, description.
    let card_h = 3usize;
    let per_page = (inner.height as usize / card_h).max(1);
    if state.calendar.selected_task < state.calendar.scroll {
        state.calendar.scroll = state.calendar.selected_task;
    }
    if state.calendar.selected_task >= state.calendar.scroll + per_page {
        state.calendar.scroll = state.calendar.selected_task + 1 - per_page;
    }
    state.calendar.scroll = state
        .calendar
        .scroll
        .min(occurrences.len().saturating_sub(per_page));

    let start = state.calendar.scroll.min(occurrences.len());
    let end = (start + per_page).min(occurrences.len());
    for (slot, index) in (start..end).enumerate() {
        let occ = &occurrences[index];
        let Some(event) = state.calendar.event(occ.event_id) else {
            continue;
        };
        let y = inner.y + slot as u16 * card_h as u16;
        if y + 2 >= inner.bottom() {
            break;
        }
        let selected = index == state.calendar.selected_task;
        let focused = selected && state.calendar.focus == CalendarFocus::Tasks;
        let base_style = if focused {
            Style::default().bg(theme.selection_bg).fg(theme.fg)
        } else if selected {
            Style::default().bg(theme.selection_bg).fg(theme.fg)
        } else {
            Style::default().fg(theme.fg)
        };
        let color = event.color.ratatui(event.bright);
        let done = event.done;

        let check = if done {
            icons.check_done.clone()
        } else {
            icons.check_todo.clone()
        };
        let mut name_style = base_style.fg(color);
        if done {
            name_style = name_style.fg(theme.dim).add_modifier(Modifier::CROSSED_OUT);
        }
        let name_line = Line::from(vec![
            Span::styled(
                format!(" {check} "),
                base_style.fg(if done { theme.success } else { color }),
            ),
            Span::styled(event.name.clone(), name_style),
            if let Some(repeat) = event.repeat {
                Span::styled(
                    format!("  {} {}", icons.repeat, repeat.storage()),
                    base_style.fg(theme.dim),
                )
            } else {
                Span::raw("")
            },
        ]);

        let mut meta = vec![
            Span::styled("   ", base_style),
            Span::styled(
                format!("{}  {}", icons.clock, occ.time_label()),
                base_style.fg(theme.dim),
            ),
        ];
        if event.notify.is_some() {
            meta.push(Span::styled(
                format!(
                    "  {} {}",
                    icons.bell,
                    event.notify.map(|n| n.label()).unwrap_or_default()
                ),
                base_style.fg(theme.warning),
            ));
        }
        let time_line = Line::from(meta);

        let desc = event.description.lines().next().unwrap_or("").trim();
        let desc_text = if desc.is_empty() {
            String::new()
        } else {
            format!("   {} {desc}", icons.description)
        };
        let desc_line = Line::from(Span::styled(desc_text, base_style.fg(theme.dim)));

        for (row, line) in [name_line, time_line, desc_line].into_iter().enumerate() {
            let rect = Rect::new(inner.x, y + row as u16, inner.width, 1);
            frame.render_widget(Paragraph::new(line).style(base_style), rect);
            hits.add(HitId::CalTask(index as u16), rect);
        }
    }

    if occurrences.len() > per_page {
        let suffix = format!(
            " {}/{} ",
            state.calendar.selected_task + 1,
            occurrences.len()
        );
        frame.render_widget(
            Paragraph::new(suffix)
                .alignment(Alignment::Right)
                .style(Style::default().fg(theme.dim)),
            Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
        );
    }
}

#[allow(dead_code)]
fn _color_name(color: EventColor) -> &'static str {
    color.name()
}
