//! Alarm-clock screen, rendered in the concept's human-readable alarm
//! syntax: name, `- HH.MM-days`, optional `> description`, `- false`.

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
    let icons = state.config.icons.clone();
    let title = format!(
        " {} Alarm-clock · {} ",
        icons.alarm,
        state.alarms.alarms.len()
    );
    let block = panel(title, &theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let list_height = inner.height.saturating_sub(1) as usize;
    let card_h = 3usize;
    let gap = 1usize;
    let per_page = (list_height / (card_h + gap)).max(1);
    let selected = state.alarms.selected;
    let start = if selected >= per_page {
        selected + 1 - per_page
    } else {
        0
    }
    .min(state.alarms.alarms.len().saturating_sub(per_page));

    if state.alarms.alarms.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "No alarms yet.",
                    Style::default().fg(theme.dim),
                )),
                Line::from(Span::styled(
                    "Press n to add one.",
                    Style::default().fg(theme.dim),
                )),
            ]),
            inner,
        );
        return;
    }

    let mut y = inner.y;
    for index in start..(start + per_page).min(state.alarms.alarms.len()) {
        let alarm = &state.alarms.alarms[index];
        if y + 2 >= inner.y + list_height as u16 {
            break;
        }
        let selected_row = index == selected;
        let base = if selected_row {
            Style::default().bg(theme.selection_bg).fg(theme.fg)
        } else {
            Style::default().fg(theme.fg)
        };
        let name_color = if !alarm.enabled {
            theme.dim
        } else if alarm.is_event() {
            theme.warning
        } else {
            theme.fg
        };
        let mut name_style = base.fg(name_color);
        if !alarm.enabled {
            name_style = name_style.add_modifier(Modifier::CROSSED_OUT);
        }
        let name_line = Line::from(vec![
            Span::styled(
                format!(
                    " {} ",
                    if alarm.enabled {
                        &icons.alarm
                    } else {
                        &icons.bell
                    }
                ),
                base.fg(if alarm.enabled {
                    theme.accent
                } else {
                    theme.dim
                }),
            ),
            Span::styled(alarm.name.clone(), name_style),
            Span::styled(
                if alarm.enabled { "" } else { "   disabled" },
                base.fg(theme.dim).add_modifier(Modifier::ITALIC),
            ),
        ]);
        let schedule = format!("   - {}-{}", alarm.time_label(), alarm.days_label());
        let schedule_line = Line::from(vec![
            Span::styled(schedule, base.fg(theme.dim)),
            if alarm.date.is_some() {
                Span::styled(
                    format!("  {} event", icons.calendar),
                    base.fg(theme.warning),
                )
            } else {
                Span::raw("")
            },
        ]);
        let desc = alarm.description.clone().unwrap_or_default();
        let desc_line = Line::from(Span::styled(
            if desc.is_empty() {
                String::new()
            } else {
                format!("   > {desc}")
            },
            base.fg(theme.dim),
        ));

        let card = Rect::new(inner.x, y, inner.width, card_h as u16);
        for (row, line) in [name_line, schedule_line, desc_line]
            .into_iter()
            .enumerate()
        {
            let rect = Rect::new(inner.x, y + row as u16, inner.width, 1);
            frame.render_widget(Paragraph::new(line).style(base), rect);
        }
        hits.add(HitId::AlarmRow(index as u16), card);
        y += (card_h + gap) as u16;
    }

    let syntax = "  Name   - HH.MM-mon,tue,…   > description   - false (disabled)";
    frame.render_widget(
        Paragraph::new(syntax).style(Style::default().fg(theme.dim)),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}
