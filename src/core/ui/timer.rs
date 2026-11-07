//! Timer screen: large countdown readout, progress bar and per-timer
//! start/pause/stop controls.

use crate::core::app::AppState;
use crate::core::timer::TimerStatus;
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
    let title = format!(" {} Timer · {} ", icons.timer, state.timers.timers.len());
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
    let selected = state.timers.selected;
    let start = if selected >= per_page {
        selected + 1 - per_page
    } else {
        0
    }
    .min(state.timers.timers.len().saturating_sub(per_page));

    if state.timers.timers.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "No timers yet.",
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
    for index in start..(start + per_page).min(state.timers.timers.len()) {
        let timer = &state.timers.timers[index];
        if y + 2 >= inner.y + list_height as u16 {
            break;
        }
        let is_selected = index == selected;
        let base = if is_selected {
            Style::default().bg(theme.selection_bg).fg(theme.fg)
        } else {
            Style::default().fg(theme.fg)
        };
        let (action_icon, action_label, status_color) = match timer.status {
            TimerStatus::Idle => (&icons.play, "start", theme.dim),
            TimerStatus::Running => (&icons.pause, "pause", theme.success),
            TimerStatus::Paused => (&icons.play, "resume", theme.warning),
            TimerStatus::Finished => (&icons.play, "restart", theme.error),
        };

        let name_line = Line::from(vec![
            Span::styled(
                format!(" {action_icon} "),
                base.fg(status_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                timer.name.clone(),
                base.fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("   {} · {action_label}", timer.status.label()),
                base.fg(status_color),
            ),
        ]);
        let display_line = Line::from(vec![
            Span::styled("   ", base),
            Span::styled(
                timer.display(),
                base.fg(if timer.status == TimerStatus::Finished {
                    theme.error
                } else {
                    theme.accent
                })
                .add_modifier(Modifier::BOLD),
            ),
        ]);

        let bar_width = inner.width.saturating_sub(7) as usize;
        let filled = ((timer.progress() * bar_width as f64).round() as usize).min(bar_width);
        let bar = format!(
            "   {}{}",
            "█".repeat(filled),
            "░".repeat(bar_width.saturating_sub(filled))
        );
        let bar_line = Line::from(Span::styled(bar, base.fg(status_color)));

        let card = Rect::new(inner.x, y, inner.width, card_h as u16);
        for (row, line) in [name_line, display_line, bar_line].into_iter().enumerate() {
            let rect = Rect::new(inner.x, y + row as u16, inner.width, 1);
            frame.render_widget(Paragraph::new(line).style(base), rect);
        }
        hits.add(HitId::TimerRow(index as u16), card);
        hits.add(
            HitId::TimerAction(index as u16, 0),
            Rect::new(inner.x, y, 3.min(inner.width), 1),
        );
        hits.add(
            HitId::TimerAction(index as u16, 1),
            Rect::new(inner.right().saturating_sub(3), y, 3.min(inner.width), 1),
        );
        frame.render_widget(
            Paragraph::new(icons.stop.clone())
                .style(base.fg(theme.dim))
                .alignment(ratatui::layout::Alignment::Right),
            Rect::new(inner.x, y, inner.width, 1),
        );
        y += (card_h + gap) as u16;
    }

    let hint = "  Space start/pause · s stop · e edit · n new";
    frame.render_widget(
        Paragraph::new(hint).style(Style::default().fg(theme.dim)),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}
