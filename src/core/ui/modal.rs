//! Modal overlay: prompts, confirmations, task/alarm/timer forms, help and
//! ringing alarms.

use crate::core::app::{AppState, Modal};
use crate::core::ui::common::centered_rect;
use crate::core::ui::hit::HitId;
use bevy_tui_texture::prelude::HitRegions;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

struct FormRow {
    label: String,
    hint: String,
    value: String,
    cursor: usize,
}

pub fn draw_modal(frame: &mut Frame, area: Rect, state: &mut AppState, hits: &mut HitRegions) {
    let theme = state.config.theme.clone();
    let icons = state.config.icons.clone();
    match &state.modal {
        Modal::None => {}
        Modal::FilePrompt(prompt) => {
            let rows = [FormRow {
                label: prompt.kind_title().to_string(),
                hint: prompt.kind_hint().into(),
                value: prompt.input.value.clone(),
                cursor: prompt.input.cursor,
            }];
            draw_form(
                frame,
                area,
                &prompt.title,
                &rows,
                0,
                &prompt.error,
                &theme,
                &icons,
                hits,
                prompt.confirm_label(),
            );
        }
        Modal::Confirm(dialog) => draw_confirm(frame, area, &dialog.text, &theme, hits),
        Modal::Task(form) => {
            let rows = form
                .fields
                .iter()
                .map(|field| FormRow {
                    label: field.label.to_string(),
                    hint: field.hint.to_string(),
                    value: field.value.value.clone(),
                    cursor: field.value.cursor,
                })
                .collect::<Vec<_>>();
            draw_form(
                frame,
                area,
                if form.event_id.is_some() {
                    "Edit task"
                } else {
                    "New task"
                },
                &rows,
                form.active,
                &form.error,
                &theme,
                &icons,
                hits,
                "Save",
            );
        }
        Modal::Alarm(form) => {
            let rows = form
                .fields
                .iter()
                .map(|field| FormRow {
                    label: field.label.to_string(),
                    hint: field.hint.to_string(),
                    value: field.value.value.clone(),
                    cursor: field.value.cursor,
                })
                .collect::<Vec<_>>();
            draw_form(
                frame,
                area,
                if form.index.is_some() {
                    "Edit alarm"
                } else {
                    "New alarm"
                },
                &rows,
                form.active,
                &form.error,
                &theme,
                &icons,
                hits,
                "Save",
            );
        }
        Modal::Timer(form) => {
            let rows = form
                .fields
                .iter()
                .map(|field| FormRow {
                    label: field.label.to_string(),
                    hint: field.hint.to_string(),
                    value: field.value.value.clone(),
                    cursor: field.value.cursor,
                })
                .collect::<Vec<_>>();
            draw_form(
                frame,
                area,
                if form.index.is_some() {
                    "Edit timer"
                } else {
                    "New timer"
                },
                &rows,
                form.active,
                &form.error,
                &theme,
                &icons,
                hits,
                "Save",
            );
        }
        Modal::Ringing(index) => draw_ringing(frame, area, *index, state, &theme, &icons, hits),
        Modal::Help => draw_help(frame, area, &theme, &icons, hits),
        Modal::Message { title, text } => {
            draw_message(frame, area, title, text, &theme, &icons, hits)
        }
    }
}

fn draw_form(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    rows: &[FormRow],
    active: usize,
    error: &str,
    theme: &crate::core::config::Theme,
    icons: &crate::core::config::Icons,
    hits: &mut HitRegions,
    confirm_label: &str,
) {
    let height = rows.len() as u16 + 6;
    let rect = centered_rect(area, 78, height);
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::default().fg(theme.accent))
        .style(Style::default().bg(theme.bg).fg(theme.fg));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let label_w = 17u16.min(inner.width.saturating_sub(8));
    for (index, row) in rows.iter().enumerate() {
        let y = inner.y + index as u16;
        if y >= inner.bottom().saturating_sub(2) {
            break;
        }
        let rect = Rect::new(inner.x, y, inner.width, 1);
        let selected = index == active;
        let row_style = if selected {
            Style::default().bg(theme.selection_bg).fg(theme.fg)
        } else {
            Style::default().fg(theme.fg)
        };
        let icon = field_icon(&row.label, icons);
        let label = format!(
            " {icon} {:<width$}",
            row.label,
            width = label_w.saturating_sub(3) as usize
        );
        let label_style = if selected {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dim)
        };
        let mut spans = vec![Span::styled(label, label_style)];

        let input_w = inner.width.saturating_sub(label_w + 2) as usize;
        if selected {
            spans.extend(input_spans(row, input_w, theme));
        } else if row.value.is_empty() {
            spans.push(Span::styled(
                row.hint.clone(),
                Style::default()
                    .fg(theme.dim)
                    .add_modifier(Modifier::ITALIC),
            ));
        } else {
            spans.push(Span::styled(
                row.value.clone(),
                Style::default().fg(theme.fg),
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)).style(row_style), rect);
        hits.add(HitId::FormField(index as u16), rect);
    }

    if !error.is_empty() {
        let y = inner.bottom().saturating_sub(2);
        frame.render_widget(
            Paragraph::new(format!(" ! {error}")).style(Style::default().fg(theme.error)),
            Rect::new(inner.x, y, inner.width, 1),
        );
    }

    draw_buttons(frame, inner, theme, hits, confirm_label, "Cancel", true);
}

fn input_spans<'a>(
    row: &'a FormRow,
    width: usize,
    theme: &crate::core::config::Theme,
) -> Vec<Span<'a>> {
    let chars: Vec<char> = row.value.chars().collect();
    let col = row.cursor.min(chars.len());
    let start = if width > 0 && col >= width {
        col + 1 - width
    } else {
        0
    };
    let mut spans = Vec::new();
    if start > 0 {
        spans.push(Span::styled("…", Style::default().fg(theme.dim)));
    }
    let visible_start = start;
    let before: String = chars[visible_start..col].iter().collect();
    if !before.is_empty() {
        spans.push(Span::styled(before, Style::default().fg(theme.fg)));
    }
    let cursor_ch = chars.get(col).copied().unwrap_or(' ');
    spans.push(Span::styled(
        cursor_ch.to_string(),
        Style::default()
            .fg(theme.bg)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD),
    ));
    if col + 1 < chars.len() {
        let after: String = chars[col + 1..]
            .iter()
            .take(width.saturating_sub(1))
            .collect();
        spans.push(Span::styled(after, Style::default().fg(theme.fg)));
    } else if row.value.is_empty() {
        spans.push(Span::styled(
            format!(" {}", row.hint),
            Style::default()
                .fg(theme.dim)
                .add_modifier(Modifier::ITALIC),
        ));
    }
    spans
}

fn draw_buttons(
    frame: &mut Frame,
    inner: Rect,
    theme: &crate::core::config::Theme,
    hits: &mut HitRegions,
    confirm: &str,
    cancel: &str,
    danger: bool,
) {
    if inner.height == 0 {
        return;
    }
    let y = inner.bottom().saturating_sub(1);
    let confirm_w = (confirm.chars().count() as u16 + 6).max(10);
    let cancel_w = (cancel.chars().count() as u16 + 6).max(10);
    let total = confirm_w + cancel_w + 3;
    let x = inner.x + inner.width.saturating_sub(total) / 2;
    let confirm_rect = Rect::new(x, y, confirm_w.min(inner.width), 1);
    let cancel_rect = Rect::new(
        x + confirm_w + 3,
        y,
        cancel_w.min(inner.width.saturating_sub(confirm_w + 3)),
        1,
    );
    let confirm_style = if danger {
        Style::default()
            .bg(theme.accent)
            .fg(theme.bg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .bg(theme.selection_bg)
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    };
    frame.render_widget(
        Paragraph::new(format!("[ {confirm} ]"))
            .alignment(Alignment::Center)
            .style(confirm_style),
        confirm_rect,
    );
    frame.render_widget(
        Paragraph::new(format!("[ {cancel} ]"))
            .alignment(Alignment::Center)
            .style(Style::default().bg(theme.selection_bg).fg(theme.fg)),
        cancel_rect,
    );
    hits.add(HitId::FormButton(0), confirm_rect);
    hits.add(HitId::FormButton(1), cancel_rect);
}

fn draw_confirm(
    frame: &mut Frame,
    area: Rect,
    text: &str,
    theme: &crate::core::config::Theme,
    hits: &mut HitRegions,
) {
    let rect = centered_rect(area, 58, 8);
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .title(" Confirm ")
        .border_style(Style::default().fg(theme.warning))
        .style(Style::default().bg(theme.bg).fg(theme.fg));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(theme.fg)),
        Rect::new(
            inner.x,
            inner.y + 1,
            inner.width,
            inner.height.saturating_sub(3),
        ),
    );
    draw_buttons(frame, inner, theme, hits, "Yes", "No", true);
}

fn draw_ringing(
    frame: &mut Frame,
    area: Rect,
    index: usize,
    state: &AppState,
    theme: &crate::core::config::Theme,
    icons: &crate::core::config::Icons,
    hits: &mut HitRegions,
) {
    let rect = centered_rect(area, 62, 10);
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .title(format!(" {}{} ", icons.alarm, " Alarm "))
        .border_style(Style::default().fg(theme.warning))
        .style(Style::default().bg(theme.bg).fg(theme.fg));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let alarm = state.alarms.alarms.get(index);
    let mut lines = vec![
        Line::from(Span::styled(
            alarm
                .map(|a| a.name.clone())
                .unwrap_or_else(|| "Alarm".into()),
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            alarm
                .map(|a| format!("{}  {}", a.time_label(), a.days_label()))
                .unwrap_or_default(),
            Style::default().fg(theme.accent),
        )),
    ];
    if let Some(desc) = alarm.and_then(|a| a.description.clone()) {
        lines.push(Line::from(Span::styled(
            desc,
            Style::default().fg(theme.dim),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press Enter, Space or Esc to dismiss.",
        Style::default()
            .fg(theme.dim)
            .add_modifier(Modifier::ITALIC),
    )));
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }),
        Rect::new(
            inner.x,
            inner.y + 1,
            inner.width,
            inner.height.saturating_sub(3),
        ),
    );
    let btn = Rect::new(
        inner.x + inner.width.saturating_sub(12) / 2,
        inner.bottom().saturating_sub(1),
        12.min(inner.width),
        1,
    );
    frame.render_widget(
        Paragraph::new("[ Dismiss ]")
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .bg(theme.warning)
                    .fg(theme.bg)
                    .add_modifier(Modifier::BOLD),
            ),
        btn,
    );
    hits.add(HitId::ModalOk, btn);
}

fn draw_help(
    frame: &mut Frame,
    area: Rect,
    theme: &crate::core::config::Theme,
    icons: &crate::core::config::Icons,
    hits: &mut HitRegions,
) {
    let rect = centered_rect(area, 82, 30);
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .title(format!(" {} Help ", icons.config))
        .border_style(Style::default().fg(theme.accent))
        .style(Style::default().bg(theme.bg).fg(theme.fg));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let sections: &[(&str, &[&str])] = &[
        (
            "Global",
            &[
                "F1 alarm · F2 timer · F3 calendar · F4 tasks · F5 config",
                "F12 help · Ctrl+Q quit · bottom buttons are clickable",
            ],
        ),
        (
            "Files",
            &[
                "Enter open · n new note · N new folder · r rename/move",
                "c copy · x cut · p paste · d delete · Space mark · b path",
            ],
        ),
        (
            "Editor",
            &[
                "Ctrl+S save · Ctrl+Z undo · Ctrl+Shift+Z redo · Ctrl+Y redo",
                "Enter new item · Tab indent · Shift+Tab outdent · F2 fold",
                "Ctrl+↑/↓ zoom · Ctrl+Space mark · Ctrl+T checkbox",
                "Ctrl+C/X/V copy/cut/paste lines · Alt+↑/↓ move block",
                "Ctrl+Enter open link · Esc back/zoom out/clear marks",
                "Click a breadcrumb to rename/move the note or its section",
            ],
        ),
        (
            "Calendar",
            &[
                "Arrows move · PgUp/PgDn month · t today · Tab grid/tasks",
                "n new task · e/Enter edit · Space done · d delete",
            ],
        ),
        (
            "Alarms & timers",
            &[
                "Alarms: n new · Enter edit · Space on/off · d delete",
                "Timers: n new · Space start/pause · s stop · e edit · d delete",
            ],
        ),
    ];
    let mut lines = Vec::new();
    for (title, entries) in sections {
        lines.push(Line::from(Span::styled(
            format!(" {title}"),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )));
        for entry in *entries {
            lines.push(Line::from(Span::styled(
                format!("   {entry}"),
                Style::default().fg(theme.fg),
            )));
        }
        lines.push(Line::from(""));
    }
    frame.render_widget(
        Paragraph::new(lines),
        Rect::new(
            inner.x,
            inner.y,
            inner.width,
            inner.height.saturating_sub(2),
        ),
    );
    let btn = Rect::new(
        inner.x + inner.width.saturating_sub(10) / 2,
        inner.bottom().saturating_sub(1),
        10.min(inner.width),
        1,
    );
    frame.render_widget(
        Paragraph::new("[ Close ]")
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .bg(theme.accent)
                    .fg(theme.bg)
                    .add_modifier(Modifier::BOLD),
            ),
        btn,
    );
    hits.add(HitId::HelpClose, btn);
}

fn draw_message(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    text: &str,
    theme: &crate::core::config::Theme,
    icons: &crate::core::config::Icons,
    hits: &mut HitRegions,
) {
    let rect = centered_rect(area, 64, 10);
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .title(format!(" {} {title} ", icons.note))
        .border_style(Style::default().fg(theme.accent))
        .style(Style::default().bg(theme.bg).fg(theme.fg));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(theme.fg)),
        Rect::new(
            inner.x,
            inner.y + 1,
            inner.width,
            inner.height.saturating_sub(3),
        ),
    );
    let btn = Rect::new(
        inner.x + inner.width.saturating_sub(10) / 2,
        inner.bottom().saturating_sub(1),
        10.min(inner.width),
        1,
    );
    frame.render_widget(
        Paragraph::new("[ OK ]").alignment(Alignment::Center).style(
            Style::default()
                .bg(theme.accent)
                .fg(theme.bg)
                .add_modifier(Modifier::BOLD),
        ),
        btn,
    );
    hits.add(HitId::ModalOk, btn);
}

fn field_icon<'a>(label: &str, icons: &'a crate::core::config::Icons) -> &'a str {
    match label {
        "name" => &icons.name,
        "description" => &icons.description,
        "start" | "date" | "days" => &icons.calendar,
        "deadline" | "time" | "duration" => &icons.clock,
        "repeat" => &icons.repeat,
        "notification" => &icons.bell,
        "bright" => &icons.bright,
        "color" => &icons.color,
        "enabled" => &icons.check_done,
        _ => &icons.file,
    }
}

// FilePromptKind's user-facing label. Kept here rather than in app.rs so the
// modal owns its own presentation.
impl crate::core::app::FilePrompt {
    fn kind_title(&self) -> &'static str {
        use crate::core::app::FilePromptKind::*;
        match self.kind {
            NewFile => "note",
            NewFolder => "folder",
            Rename => "new path",
            Path => "path",
            NotePath => "note path",
            SectionTitle(_) => "section",
        }
    }

    fn kind_hint(&self) -> &'static str {
        use crate::core::app::FilePromptKind::*;
        match self.kind {
            NewFile => "relative path, e.g. directory/name.md",
            NewFolder => "relative path, e.g. directory/subdirectory",
            Rename => "relative to this folder",
            Path => "existing folder, or a path to move the selection",
            NotePath => "relative to the notebook, e.g. folder/note.md",
            SectionTitle(_) => "new heading text (markdown marker is kept)",
        }
    }

    fn confirm_label(&self) -> &'static str {
        use crate::core::app::FilePromptKind::*;
        match self.kind {
            NewFile | NewFolder => "Create",
            Rename | SectionTitle(_) => "Rename",
            Path => "Go",
            NotePath => "Move",
        }
    }
}
