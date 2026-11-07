//! Ratatui composition for the whole app.

pub mod alarm;
pub mod calendar;
pub mod common;
pub mod config;
pub mod editor;
pub mod file_manager;
pub mod hit;
pub mod modal;
pub mod timer;

use crate::core::app::{AppState, Screen};
use bevy_tui_texture::prelude::HitRegions;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::widgets::Block;

pub fn draw(frame: &mut Frame, state: &mut AppState, hits: &mut HitRegions) {
    let theme = state.config.theme.clone();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.bg).fg(theme.fg)),
        frame.area(),
    );

    let area = frame.area();
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(1),
        Constraint::Length(3),
    ])
    .split(area);

    common::draw_header(frame, chunks[0], state, hits);
    common::draw_crumbs(frame, chunks[1], state, hits);

    match state.screen {
        Screen::FileManager => file_manager::draw(frame, chunks[2], state, hits),
        Screen::Editor => editor::draw(frame, chunks[2], state, hits),
        Screen::Calendar => calendar::draw(frame, chunks[2], state, hits),
        Screen::Alarms => alarm::draw(frame, chunks[2], state, hits),
        Screen::Timers => timer::draw(frame, chunks[2], state, hits),
        Screen::Config => config::draw(frame, chunks[2], state, hits),
    }

    common::draw_status(frame, chunks[3], state);
    common::draw_nav(frame, chunks[4], state, hits);
    common::draw_toasts(frame, chunks[2], state);
    modal::draw_modal(frame, area, state, hits);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::alarm::{AlarmForm, AlarmState};
    use crate::core::app::{ConfirmDialog, ConfirmKind, FilePrompt, FilePromptKind, Modal};
    use crate::core::calendar::{CalendarState, TaskForm};
    use crate::core::config::{AppConfig, DEFAULT_CONFIG};
    use crate::core::document::Document;
    use crate::core::file_manager::FileManager;
    use crate::core::storage::Layout;
    use crate::core::text::{LineInput, TextBuffer};
    use crate::core::timer::{TimerForm, TimersState};
    use chrono::Local;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::Instant;

    fn test_app() -> AppState {
        let root = std::env::temp_dir().join(format!(
            "tuion-ui-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let layout = Layout::at(&root).expect("layout");
        let config = AppConfig::parse(DEFAULT_CONFIG).expect("config");
        let config_editor = TextBuffer::new(&config.raw);
        AppState {
            file_manager: FileManager::new(layout.notes.clone()),
            calendar: CalendarState::load(&layout.calendar_path()),
            alarms: AlarmState::load(&layout.alarms_path()),
            timers: TimersState::load(&layout.timers_path()),
            layout,
            config,
            screen: Screen::FileManager,
            modal: Modal::None,
            editor: None,
            config_editor,
            config_dirty: false,
            config_content_x: 0,
            line_clipboard: Vec::new(),
            toasts: Vec::new(),
            now: Local::now(),
            last_tick: Instant::now(),
            last_back: None,
            last_click: None,
            last_minute: None,
            last_autosave: Instant::now(),
            quit: false,
        }
    }

    fn render(state: &mut AppState) -> String {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut hits = HitRegions::default();
        terminal
            .draw(|frame| draw(frame, state, &mut hits))
            .expect("draw");
        let buffer = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn every_screen_and_modal_renders() {
        let mut app = test_app();
        let file_screen = render(&mut app);
        assert!(file_screen.contains("TUION"));
        assert!(file_screen.contains("welcome.md"));

        let path = app.layout.notes.join("welcome.md");
        app.editor = Some(Document::open(path, "welcome.md").unwrap());
        app.screen = Screen::Editor;
        let editor = render(&mut app);
        assert!(editor.contains("Welcome to TUION"));
        assert!(editor.contains("[[ideas]]"));

        app.screen = Screen::Calendar;
        let calendar = render(&mut app);
        assert!(calendar.contains("Calendar"));
        assert!(calendar.contains("Mo"));

        app.screen = Screen::Alarms;
        let alarms = render(&mut app);
        assert!(alarms.contains("Good morning"));

        app.screen = Screen::Timers;
        let timers = render(&mut app);
        assert!(timers.contains("Focus"));

        app.screen = Screen::Config;
        let config = render(&mut app);
        assert!(config.contains("TUION configuration"));

        app.modal = Modal::Help;
        assert!(render(&mut app).contains("Help"));
        app.modal = Modal::Task(TaskForm::new(None, Local::now().date_naive()));
        assert!(render(&mut app).contains("New task"));
        app.modal = Modal::Alarm(AlarmForm::new(None));
        assert!(render(&mut app).contains("New alarm"));
        app.modal = Modal::Timer(TimerForm::new(None, None));
        assert!(render(&mut app).contains("New timer"));
        app.modal = Modal::FilePrompt(FilePrompt {
            kind: FilePromptKind::NewFile,
            title: "New note".into(),
            input: LineInput::new("test.md"),
            error: String::new(),
        });
        assert!(render(&mut app).contains("New note"));
        app.modal = Modal::Confirm(ConfirmDialog {
            kind: ConfirmKind::Files,
            text: "Delete one item?".into(),
        });
        assert!(render(&mut app).contains("Confirm"));
        app.modal = Modal::Ringing(0);
        assert!(render(&mut app).contains("Dismiss"));
        app.modal = Modal::Message {
            title: "Note".into(),
            text: "hello".into(),
        };
        assert!(render(&mut app).contains("hello"));

        if std::env::var("TUION_DUMP_UI").is_ok() {
            let _ = std::fs::write("/tmp/tuion-ui-files.txt", &file_screen);
            let _ = std::fs::write("/tmp/tuion-ui-editor.txt", &editor);
            let _ = std::fs::write("/tmp/tuion-ui-calendar.txt", &calendar);
            let _ = std::fs::write("/tmp/tuion-ui-alarms.txt", &alarms);
            let _ = std::fs::write("/tmp/tuion-ui-timers.txt", &timers);
            let _ = std::fs::write("/tmp/tuion-ui-config.txt", &config);
        }
    }
}
