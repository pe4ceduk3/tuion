//! Application state, screen navigation and the input state machine.

use crate::core::alarm::{Alarm, AlarmForm, AlarmState};
use crate::core::calendar::{
    CalendarFocus, CalendarState, Event, TaskForm, format_datetime, parse_datetime,
};
use crate::core::config::{AppConfig, DEFAULT_CONFIG};
use crate::core::document::Document;
use crate::core::file_manager::{self, FileManager, OpenAction};
use crate::core::markdown;
use crate::core::storage::{self, Layout};
use crate::core::text::{LineInput, TextBuffer};
use crate::core::timer::{TimerForm, TimersState};
use crate::core::ui::hit::HitId;
use bevy::input::mouse::MouseButton;
use bevy_tui_texture::input::{InputEvent, KeyCode, KeyEvent, MouseEventKind};
use chrono::{Local, NaiveDate, NaiveDateTime};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    FileManager,
    Editor,
    Calendar,
    Alarms,
    Timers,
    Config,
}

impl Screen {
    pub fn title(self) -> &'static str {
        match self {
            Self::FileManager => "Files",
            Self::Editor => "Note",
            Self::Calendar => "Calendar",
            Self::Alarms => "Alarm-clock",
            Self::Timers => "Timer",
            Self::Config => "Config",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilePromptKind {
    NewFile,
    NewFolder,
    Rename,
    Path,
    /// Rename or move the note currently open in the editor.
    NotePath,
    /// Rename the heading/list item that starts the zoomed section at the
    /// given index in `Document::zoom`.
    SectionTitle(usize),
}

pub struct FilePrompt {
    pub kind: FilePromptKind,
    pub title: String,
    pub input: LineInput,
    pub error: String,
}

#[derive(Clone, Copy, Debug)]
pub enum ConfirmKind {
    Files,
    Task(u64),
    Alarm(usize),
    Timer(usize),
}

pub struct ConfirmDialog {
    pub kind: ConfirmKind,
    pub text: String,
}

pub enum Modal {
    None,
    FilePrompt(FilePrompt),
    Confirm(ConfirmDialog),
    Task(TaskForm),
    Alarm(AlarmForm),
    Timer(TimerForm),
    Ringing(usize),
    Help,
    Message { title: String, text: String },
}

pub struct Toast {
    pub text: String,
    pub until: Instant,
    pub error: bool,
}

#[derive(bevy::prelude::Resource)]
pub struct AppState {
    pub layout: Layout,
    pub config: AppConfig,
    pub screen: Screen,
    pub modal: Modal,
    pub file_manager: FileManager,
    pub editor: Option<Document>,
    pub calendar: CalendarState,
    pub alarms: AlarmState,
    pub timers: TimersState,
    pub config_editor: TextBuffer,
    pub config_dirty: bool,
    pub config_content_x: u16,
    pub line_clipboard: Vec<String>,
    pub toasts: Vec<Toast>,
    pub now: chrono::DateTime<Local>,
    pub last_tick: Instant,
    pub last_back: Option<Instant>,
    pub last_click: Option<(Instant, HitId)>,
    pub last_minute: Option<String>,
    pub last_autosave: Instant,
    pub quit: bool,
}

impl AppState {
    pub fn load() -> Self {
        let layout = Layout::discover().unwrap_or_else(|err| {
            eprintln!("TUION: cannot create data directory: {err}");
            let fallback = std::env::temp_dir().join("tuion");
            Layout::at(&fallback).expect("cannot create fallback data directory")
        });
        let raw = storage::read_text(&layout.config).unwrap_or_else(|_| DEFAULT_CONFIG.to_string());
        let config = AppConfig::parse(&raw).unwrap_or_else(|err| {
            eprintln!("TUION: config error: {err}");
            let mut cfg = AppConfig::new();
            cfg.raw = raw;
            cfg
        });
        let file_manager = FileManager::new(layout.notes.clone());
        let calendar = CalendarState::load(&layout.calendar_path());
        let alarms = AlarmState::load(&layout.alarms_path());
        let timers = TimersState::load(&layout.timers_path());
        let config_editor = TextBuffer::new(&config.raw);
        Self {
            layout,
            config,
            screen: Screen::FileManager,
            modal: Modal::None,
            file_manager,
            editor: None,
            calendar,
            alarms,
            timers,
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

    pub fn now_text(&self) -> String {
        self.now.format("%Y.%m.%d %H:%M").to_string()
    }

    pub fn nav_active(&self) -> Option<u8> {
        match self.screen {
            Screen::Alarms => Some(0),
            Screen::Timers => Some(1),
            Screen::Calendar => Some(if self.calendar.focus == CalendarFocus::Grid {
                2
            } else {
                3
            }),
            _ => None,
        }
    }

    pub fn toast(&mut self, text: impl Into<String>, error: bool) {
        self.toasts.push(Toast {
            text: text.into(),
            until: Instant::now() + Duration::from_secs(4),
            error,
        });
        if self.toasts.len() > 5 {
            self.toasts.remove(0);
        }
    }

    fn save_calendar(&self) {
        let _ = self.calendar.save(&self.layout.calendar_path());
    }

    fn save_alarms(&self) {
        let _ = self.alarms.save(&self.layout.alarms_path());
    }

    fn save_timers(&self) {
        let _ = self.timers.save(&self.layout.timers_path());
    }

    pub fn save_all(&mut self) {
        if let Some(doc) = &mut self.editor {
            doc.save_if_dirty();
        }
        if self.config_dirty {
            let _ = self.save_config();
        }
        self.save_calendar();
        self.save_alarms();
        self.save_timers();
    }

    pub fn open_config(&mut self) {
        if self.screen == Screen::Config {
            return;
        }
        self.leave_editor();
        self.config_editor = TextBuffer::new(&self.config.raw);
        self.config_dirty = false;
        self.screen = Screen::Config;
        self.modal = Modal::None;
    }

    pub fn save_config(&mut self) -> Result<(), String> {
        let raw = self.config_editor.as_text();
        let cfg = AppConfig::parse(&raw)?;
        storage::write_text(&self.layout.config, &raw).map_err(|e| e.to_string())?;
        self.config = cfg;
        self.config_dirty = false;
        self.toast("Config saved", false);
        Ok(())
    }

    pub fn open_editor(&mut self, path: PathBuf, rel: String) {
        if self.editor.as_ref().is_some_and(|doc| doc.path == path) {
            self.screen = Screen::Editor;
            self.modal = Modal::None;
            return;
        }
        if let Some(doc) = &mut self.editor {
            doc.save_if_dirty();
        }
        match Document::open(path, rel) {
            Ok(doc) => {
                self.editor = Some(doc);
                self.screen = Screen::Editor;
                self.modal = Modal::None;
            }
            Err(err) => self.toast(format!("Cannot open note: {err}"), true),
        }
    }

    fn leave_editor(&mut self) {
        if self.screen == Screen::Editor {
            if let Some(doc) = &mut self.editor {
                doc.save_if_dirty();
            }
            self.editor = None;
        }
    }

    pub fn open_link(&mut self, target: &str) {
        let Some(doc) = &self.editor else {
            return;
        };
        let root = self.file_manager.root().to_path_buf();
        let current_dir = doc
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| root.clone());
        let mut name = target.trim().to_string();
        if name.is_empty() {
            return;
        }
        if Path::new(&name).extension().is_none() {
            name.push_str(".md");
        }
        let candidates: Vec<PathBuf> = if name.starts_with('/') {
            vec![root.join(name.trim_start_matches('/'))]
        } else {
            vec![current_dir.join(&name), root.join(&name)]
        };
        let mut chosen = None;
        for candidate in &candidates {
            if let Ok(rel) = candidate.strip_prefix(&root) {
                if file_manager::normalize_rel(rel).is_ok() {
                    if candidate.exists() {
                        chosen = Some(candidate.clone());
                        break;
                    }
                    if chosen.is_none() {
                        chosen = Some(candidate.clone());
                    }
                }
            }
        }
        let Some(chosen) = chosen else {
            self.toast(format!("Link target not found: {target}"), true);
            return;
        };
        let rel = chosen
            .strip_prefix(&root)
            .map(file_manager::rel_string)
            .unwrap_or_else(|_| target.to_string());
        self.open_editor(chosen, rel);
    }

    pub fn switch_screen(&mut self, screen: Screen) {
        if self.screen == screen {
            return;
        }
        if self.screen == Screen::Config && self.config_dirty && self.save_config().is_err() {
            return;
        }
        self.leave_editor();
        self.screen = screen;
        self.modal = Modal::None;
    }

    pub fn nav_to(&mut self, index: u8) {
        match index {
            0 => self.switch_screen(Screen::Alarms),
            1 => self.switch_screen(Screen::Timers),
            2 => {
                self.switch_screen(Screen::Calendar);
                self.calendar.focus = CalendarFocus::Grid;
            }
            3 => {
                self.switch_screen(Screen::Calendar);
                self.calendar.focus = CalendarFocus::Tasks;
            }
            _ => {}
        }
    }

    pub fn back(&mut self) {
        if !matches!(self.modal, Modal::None) {
            self.close_modal();
            return;
        }
        match self.screen {
            Screen::Editor => {
                self.leave_editor();
                self.screen = Screen::FileManager;
                self.file_manager.refresh_preview();
            }
            Screen::Config => {
                if self.config_dirty && self.save_config().is_err() {
                    return;
                }
                self.screen = Screen::FileManager;
            }
            Screen::FileManager => {
                if self.file_manager.go_up() {
                    return;
                }
                let now = Instant::now();
                if self
                    .last_back
                    .is_some_and(|t| now.duration_since(t) < Duration::from_secs(2))
                {
                    self.quit = true;
                } else {
                    self.last_back = Some(now);
                    self.toast("Press back again to exit", false);
                }
            }
            _ => self.screen = Screen::FileManager,
        }
    }

    pub fn close_modal(&mut self) {
        if matches!(self.modal, Modal::Ringing(_)) {
            self.alarms.dismiss();
        }
        self.modal = Modal::None;
    }

    pub fn open_file_prompt(&mut self, kind: FilePromptKind) {
        let (title, prefill) = match kind {
            FilePromptKind::NewFile => ("New note (path/name.md)", String::new()),
            FilePromptKind::NewFolder => ("New folder", String::new()),
            FilePromptKind::Rename => (
                "Rename / move (relative to this folder)",
                self.file_manager
                    .selected_entry()
                    .map(|e| e.name.clone())
                    .unwrap_or_default(),
            ),
            FilePromptKind::Path => {
                let mut p = self.file_manager.cwd_rel();
                if let Some(entry) = self.file_manager.selected_entry() {
                    if !p.is_empty() {
                        p.push('/');
                    }
                    p.push_str(&entry.name);
                }
                ("Path (edit to move or navigate)", p)
            }
            // These prompts are only opened through the editor breadcrumbs.
            FilePromptKind::NotePath | FilePromptKind::SectionTitle(_) => return,
        };
        self.modal = Modal::FilePrompt(FilePrompt {
            kind,
            title: title.into(),
            input: LineInput::new(prefill),
            error: String::new(),
        });
    }

    /// Clicking an editor breadcrumb edits it: the first crumb is the note
    /// path (rename/move the file), later crumbs are zoomed sections (rename
    /// the heading or list item).
    pub fn open_editor_crumb_prompt(&mut self, index: u16) {
        let Some(doc) = &self.editor else {
            return;
        };
        let (kind, title, prefill) = if index == 0 {
            (
                FilePromptKind::NotePath,
                "Rename / move note (relative to notebook)",
                doc.rel_path.clone(),
            )
        } else {
            let zoom_index = index as usize - 1;
            let Some(zoom) = doc.zoom.get(zoom_index) else {
                return;
            };
            (
                FilePromptKind::SectionTitle(zoom_index),
                "Rename section",
                zoom.label.clone(),
            )
        };
        self.modal = Modal::FilePrompt(FilePrompt {
            kind,
            title: title.into(),
            input: LineInput::new(prefill),
            error: String::new(),
        });
    }

    /// Rename/move the note open in the editor and keep the editor pointed at
    /// the new path. The file manager follows it so going back lands in the
    /// right folder.
    fn rename_open_note(&mut self, input: &str) -> Result<String, String> {
        let Some(doc) = &mut self.editor else {
            return Err("no note is open".into());
        };
        let from = doc.path.clone();
        doc.save_if_dirty();
        let (target, rel) = self.file_manager.move_note(&from, input)?;
        if let Some(doc) = &mut self.editor {
            doc.path = target.clone();
            doc.rel_path = rel.clone();
        }
        let parent = Path::new(&rel)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        if self.file_manager.navigate_to(&parent).is_err() {
            self.file_manager.reload();
        }
        self.file_manager.select_path(&target);
        Ok(format!("Note moved to {rel}"))
    }

    fn rename_section(&mut self, zoom_index: usize, title: &str) -> Result<String, String> {
        let title = title.trim();
        if title.is_empty() {
            return Err("section title cannot be empty".into());
        }
        let syntax = self.config.markdown.clone();
        let Some(doc) = &mut self.editor else {
            return Err("no note is open".into());
        };
        if doc.rename_zoom(zoom_index, title, &syntax) {
            Ok("Section renamed".into())
        } else {
            Err("this section cannot be renamed".into())
        }
    }

    pub fn open_task_form(&mut self, index: Option<usize>) {
        let event: Option<Event> = index.and_then(|i| {
            let occ = self.calendar.selected_occurrences().get(i)?.clone();
            self.calendar.event(occ.event_id).cloned()
        });
        let form = TaskForm::new(event.as_ref(), self.calendar.selected_date);
        self.modal = Modal::Task(form);
    }

    pub fn open_alarm_form(&mut self, index: Option<usize>) {
        let alarm: Option<Alarm> = index.and_then(|i| self.alarms.alarms.get(i).cloned());
        let mut form = AlarmForm::new(alarm.as_ref());
        form.index = index;
        self.modal = Modal::Alarm(form);
    }

    pub fn open_timer_form(&mut self, index: Option<usize>) {
        let timer = index.and_then(|i| self.timers.timers.get(i).cloned());
        self.modal = Modal::Timer(TimerForm::new(timer.as_ref(), index));
    }

    pub fn confirm(&mut self, kind: ConfirmKind, text: impl Into<String>) {
        self.modal = Modal::Confirm(ConfirmDialog {
            kind,
            text: text.into(),
        });
    }

    pub fn tick(&mut self) {
        self.now = Local::now();
        let instant = Instant::now();
        let finished = self.timers.tick(instant);
        for name in finished {
            self.toast(format!("Timer finished: {name}"), false);
        }

        let minute = self.now.format("%Y%m%d%H%M").to_string();
        if self.last_minute.as_deref() != Some(minute.as_str()) {
            self.last_minute = Some(minute);
            let now = self.now.naive_local();
            if let Some(index) = self.alarms.check(now) {
                self.modal = Modal::Ringing(index);
            }
            for message in self.calendar.due_notifications(now) {
                self.toast(message, false);
            }
        }

        if instant.duration_since(self.last_autosave) > Duration::from_secs(5) {
            if let Some(doc) = &mut self.editor {
                if doc.dirty {
                    let _ = doc.save();
                }
            }
            self.last_autosave = instant;
        }
        self.toasts.retain(|toast| toast.until > instant);
    }

    pub fn handle_input(&mut self, input: &InputEvent, hit: Option<HitId>) {
        match input {
            InputEvent::Key(key) if key.kind != bevy_tui_texture::input::KeyEventKind::Release => {
                if matches!(self.modal, Modal::None) {
                    self.handle_screen_key(key);
                } else {
                    self.handle_modal_key(key);
                }
            }
            InputEvent::Paste(text) => match &mut self.modal {
                Modal::FilePrompt(prompt) => prompt.input.insert_str(text),
                Modal::Task(form) => form.active_field().insert_str(text),
                Modal::Alarm(form) => form.active_field().insert_str(text),
                Modal::Timer(form) => form.active_field().insert_str(text),
                _ => {
                    if let Some(doc) = &mut self.editor {
                        doc.insert_str(text);
                    } else if self.screen == Screen::Config {
                        self.config_editor.insert_str(text);
                        self.config_dirty = true;
                    }
                }
            },
            InputEvent::Mouse(mouse) => match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(hit) = hit {
                        self.handle_click(hit, mouse.column, mouse.row);
                    }
                }
                MouseEventKind::ScrollUp => self.handle_scroll(-1),
                MouseEventKind::ScrollDown => self.handle_scroll(1),
                _ => {}
            },
            _ => {}
        }
    }

    fn handle_scroll(&mut self, direction: i32) {
        if !matches!(self.modal, Modal::None) {
            return;
        }
        match self.screen {
            Screen::FileManager => self.file_manager.move_selection(direction as isize),
            Screen::Editor => {
                if let Some(doc) = &mut self.editor {
                    for _ in 0..3 {
                        if direction < 0 {
                            doc.buffer.move_up();
                        } else {
                            doc.buffer.move_down();
                        }
                    }
                }
            }
            Screen::Calendar => {
                if self.calendar.focus == CalendarFocus::Tasks {
                    let len = self.calendar.selected_occurrences().len() as isize;
                    self.calendar.selected_task =
                        (self.calendar.selected_task as isize + direction as isize)
                            .clamp(0, (len - 1).max(0)) as usize;
                } else {
                    self.calendar.shift_day(direction as i64);
                }
            }
            Screen::Alarms => self.alarms.move_selection(direction as isize),
            Screen::Timers => self.timers.move_selection(direction as isize),
            Screen::Config => {
                if direction < 0 {
                    self.config_editor.scroll = self.config_editor.scroll.saturating_sub(3);
                } else {
                    self.config_editor.scroll += 3;
                }
            }
        }
    }

    fn handle_screen_key(&mut self, key: &KeyEvent) {
        if key.modifiers.ctrl && matches!(key.code, KeyCode::Char('q')) {
            self.quit = true;
            return;
        }
        match key.code {
            KeyCode::F(1) => {
                self.nav_to(0);
                return;
            }
            KeyCode::F(2) => {
                self.nav_to(1);
                return;
            }
            KeyCode::F(3) => {
                self.nav_to(2);
                return;
            }
            KeyCode::F(4) => {
                self.nav_to(3);
                return;
            }
            KeyCode::F(5) => {
                self.open_config();
                return;
            }
            KeyCode::F(12) => {
                self.modal = Modal::Help;
                return;
            }
            _ => {}
        }

        match self.screen {
            Screen::FileManager => self.handle_file_manager_key(key),
            Screen::Editor => self.handle_editor_key(key),
            Screen::Calendar => self.handle_calendar_key(key),
            Screen::Alarms => self.handle_alarm_key(key),
            Screen::Timers => self.handle_timer_key(key),
            Screen::Config => self.handle_config_key(key),
        }
    }

    fn handle_file_manager_key(&mut self, key: &KeyEvent) {
        let ctrl = key.modifiers.ctrl;
        match key.code {
            KeyCode::Up => self.file_manager.move_selection(-1),
            KeyCode::Down => self.file_manager.move_selection(1),
            KeyCode::PageUp => self.file_manager.move_selection(-10),
            KeyCode::PageDown => self.file_manager.move_selection(10),
            KeyCode::Home => self.file_manager.select(0),
            KeyCode::End => {
                let last = self.file_manager.entries.len().saturating_sub(1);
                self.file_manager.select(last);
            }
            KeyCode::Enter => match self.file_manager.open_selected() {
                OpenAction::File { abs, rel } => self.open_editor(abs, rel),
                OpenAction::Directory | OpenAction::None => {}
            },
            KeyCode::Backspace | KeyCode::Esc => {
                self.back();
            }
            KeyCode::Char('n') if !ctrl => self.open_file_prompt(FilePromptKind::NewFile),
            KeyCode::Char('N') => self.open_file_prompt(FilePromptKind::NewFolder),
            KeyCode::Char('r') if !ctrl => self.open_file_prompt(FilePromptKind::Rename),
            KeyCode::Char('b') if !ctrl => self.open_file_prompt(FilePromptKind::Path),
            KeyCode::Char('c') if !ctrl => {
                self.file_manager.copy_selected(false);
                self.toast("Copied", false);
            }
            KeyCode::Char('x') if !ctrl => {
                self.file_manager.copy_selected(true);
                self.toast("Cut", false);
            }
            KeyCode::Char('p') if !ctrl => match self.file_manager.paste() {
                Ok(n) => self.toast(format!("Pasted {n} item(s)"), false),
                Err(err) => self.toast(err, true),
            },
            KeyCode::Char('d') if !ctrl => {
                let paths = self.file_manager.selected_paths();
                if paths.is_empty() {
                    self.toast("Nothing selected", true);
                } else {
                    self.confirm(
                        ConfirmKind::Files,
                        format!("Delete {} item(s)?", paths.len()),
                    );
                }
            }
            KeyCode::Char(' ') => self.file_manager.toggle_mark(),
            KeyCode::Char('g') | KeyCode::Char(',') => self.open_config(),
            KeyCode::Char('?') => self.modal = Modal::Help,
            KeyCode::Char('a') if ctrl => {
                for entry in &self.file_manager.entries {
                    self.file_manager.marks.insert(entry.abs.clone());
                }
            }
            _ => {}
        }
    }

    fn handle_editor_key(&mut self, key: &KeyEvent) {
        enum Action {
            None,
            Back,
            OpenLink(String),
            Toast(String, bool),
        }
        let syntax = self.config.markdown.clone();
        let clipboard = self.line_clipboard.clone();
        let action = {
            let Some(doc) = self.editor.as_mut() else {
                return;
            };
            let ctrl = key.modifiers.ctrl;
            let alt = key.modifiers.alt;
            let mut action = Action::None;
            match key.code {
                KeyCode::Char('s') if ctrl => match doc.save() {
                    Ok(()) => action = Action::Toast("Saved".into(), false),
                    Err(err) => action = Action::Toast(format!("Save failed: {err}"), true),
                },
                KeyCode::Char('z') if ctrl && key.modifiers.shift => doc.redo(),
                KeyCode::Char('z') if ctrl => doc.undo(),
                KeyCode::Char('y') if ctrl => doc.redo(),
                KeyCode::Char('c') if ctrl => {
                    self.line_clipboard = doc.copy_lines();
                    action = Action::Toast("Copied line(s)".into(), false);
                }
                KeyCode::Char('x') if ctrl => {
                    self.line_clipboard = doc.cut_lines();
                    action = Action::Toast("Cut line(s)".into(), false);
                }
                KeyCode::Char('v') if ctrl => doc.paste_lines(&clipboard),
                KeyCode::Char('a') if ctrl => {
                    for i in doc.visible_indices(&syntax) {
                        doc.marks.insert(i);
                    }
                }
                KeyCode::Char(' ') if ctrl => doc.toggle_mark(),
                KeyCode::Char('t') if ctrl => doc.toggle_checkbox(&syntax),
                KeyCode::Enter if ctrl => {
                    if let Some(target) = doc.link_at_cursor(&syntax) {
                        action = Action::OpenLink(target);
                    }
                }
                KeyCode::Up if ctrl => doc.zoom_out(),
                KeyCode::Down if ctrl => doc.zoom_in(&syntax),
                KeyCode::Up if alt => doc.move_block(true),
                KeyCode::Down if alt => doc.move_block(false),
                KeyCode::F(2) => doc.toggle_fold(&syntax),
                KeyCode::Tab => doc.indent(),
                KeyCode::BackTab => doc.outdent(),
                KeyCode::Enter => doc.newline(&syntax),
                KeyCode::Backspace => doc.backspace(&syntax),
                KeyCode::Delete => doc.delete(),
                KeyCode::Left => doc.buffer.move_left(),
                KeyCode::Right => doc.buffer.move_right(),
                KeyCode::Up => doc.buffer.move_up(),
                KeyCode::Down => doc.buffer.move_down(),
                KeyCode::Home => {
                    let first = doc.buffer.first_non_space();
                    if doc.buffer.col == first {
                        doc.buffer.home();
                    } else {
                        doc.buffer.col = first;
                    }
                }
                KeyCode::End => doc.buffer.end(),
                KeyCode::PageUp => {
                    for _ in 0..10 {
                        doc.buffer.move_up();
                    }
                }
                KeyCode::PageDown => {
                    for _ in 0..10 {
                        doc.buffer.move_down();
                    }
                }
                KeyCode::Esc => {
                    if !doc.zoom.is_empty() {
                        doc.zoom_out();
                    } else if !doc.marks.is_empty() {
                        doc.clear_marks();
                    } else {
                        action = Action::Back;
                    }
                }
                KeyCode::Char(c) if !ctrl && !alt => {
                    if c == ' '
                        && doc.buffer.col <= doc.buffer.leading_spaces()
                        && markdown::parse_list(doc.buffer.line(), &syntax).is_some()
                    {
                        doc.indent();
                    } else {
                        doc.insert_char(c);
                    }
                }
                _ => {}
            }
            action
        };
        match action {
            Action::None => {}
            Action::Back => self.back(),
            Action::OpenLink(target) => self.open_link(&target),
            Action::Toast(text, error) => self.toast(text, error),
        }
    }

    fn handle_calendar_key(&mut self, key: &KeyEvent) {
        let ctrl = key.modifiers.ctrl;
        match key.code {
            KeyCode::Tab => {
                self.calendar.focus = match self.calendar.focus {
                    CalendarFocus::Grid => CalendarFocus::Tasks,
                    CalendarFocus::Tasks => CalendarFocus::Grid,
                };
            }
            KeyCode::Left if self.calendar.focus == CalendarFocus::Grid => {
                self.calendar.shift_day(-1)
            }
            KeyCode::Right if self.calendar.focus == CalendarFocus::Grid => {
                self.calendar.shift_day(1)
            }
            KeyCode::Up => {
                if self.calendar.focus == CalendarFocus::Grid {
                    self.calendar.shift_day(-7);
                } else {
                    self.calendar.selected_task = self.calendar.selected_task.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if self.calendar.focus == CalendarFocus::Grid {
                    self.calendar.shift_day(7);
                } else {
                    let len = self.calendar.selected_occurrences().len();
                    if len > 0 {
                        self.calendar.selected_task =
                            (self.calendar.selected_task + 1).min(len - 1);
                    }
                }
            }
            KeyCode::PageUp => self.calendar.set_month(-1),
            KeyCode::PageDown => self.calendar.set_month(1),
            KeyCode::Home => self.calendar.today(),
            KeyCode::Enter | KeyCode::Char('e') if !ctrl => {
                let index = self.calendar.selected_task;
                if self.calendar.selected_occurrences().get(index).is_some() {
                    self.open_task_form(Some(index));
                }
            }
            KeyCode::Char('n') if !ctrl => self.open_task_form(None),
            KeyCode::Char('d') if !ctrl => {
                if let Some(occ) = self
                    .calendar
                    .selected_occurrences()
                    .get(self.calendar.selected_task)
                {
                    let id = occ.event_id;
                    let name = self
                        .calendar
                        .event(id)
                        .map(|e| e.name.clone())
                        .unwrap_or_else(|| "task".into());
                    self.confirm(ConfirmKind::Task(id), format!("Delete task \"{name}\"?"));
                }
            }
            KeyCode::Char(' ') => {
                if let Some(occ) = self
                    .calendar
                    .selected_occurrences()
                    .get(self.calendar.selected_task)
                {
                    let id = occ.event_id;
                    self.calendar.toggle_done(id);
                    self.save_calendar();
                }
            }
            KeyCode::Char('t') if !ctrl => self.calendar.today(),
            KeyCode::Char('[') => self.calendar.set_month(-1),
            KeyCode::Char(']') => self.calendar.set_month(1),
            KeyCode::Char('?') => self.modal = Modal::Help,
            KeyCode::Esc => self.back(),
            _ => {}
        }
    }

    fn handle_alarm_key(&mut self, key: &KeyEvent) {
        let ctrl = key.modifiers.ctrl;
        match key.code {
            KeyCode::Up => self.alarms.move_selection(-1),
            KeyCode::Down => self.alarms.move_selection(1),
            KeyCode::PageUp => self.alarms.move_selection(-10),
            KeyCode::PageDown => self.alarms.move_selection(10),
            KeyCode::Enter | KeyCode::Char('e') if !ctrl => {
                let index = self.alarms.selected;
                if self.alarms.selected_alarm().is_some() {
                    self.open_alarm_form(Some(index));
                }
            }
            KeyCode::Char('n') if !ctrl => self.open_alarm_form(None),
            KeyCode::Char(' ') => {
                self.alarms.toggle_selected();
                self.save_alarms();
            }
            KeyCode::Char('d') if !ctrl => {
                if let Some(alarm) = self.alarms.selected_alarm() {
                    let name = alarm.name.clone();
                    self.confirm(
                        ConfirmKind::Alarm(self.alarms.selected),
                        format!("Delete alarm \"{name}\"?"),
                    );
                }
            }
            KeyCode::Char('?') => self.modal = Modal::Help,
            KeyCode::Esc => self.back(),
            _ => {}
        }
    }

    fn handle_timer_key(&mut self, key: &KeyEvent) {
        let ctrl = key.modifiers.ctrl;
        match key.code {
            KeyCode::Up => self.timers.move_selection(-1),
            KeyCode::Down => self.timers.move_selection(1),
            KeyCode::PageUp => self.timers.move_selection(-10),
            KeyCode::PageDown => self.timers.move_selection(10),
            KeyCode::Enter | KeyCode::Char('e') if !ctrl => {
                let index = self.timers.selected;
                if self.timers.selected_timer().is_some() {
                    self.open_timer_form(Some(index));
                }
            }
            KeyCode::Char('n') if !ctrl => self.open_timer_form(None),
            KeyCode::Char(' ') => {
                let now = Instant::now();
                if let Some(timer) = self.timers.selected_timer_mut() {
                    timer.toggle(now);
                }
            }
            KeyCode::Char('s') if !ctrl => {
                if let Some(timer) = self.timers.selected_timer_mut() {
                    timer.stop();
                }
            }
            KeyCode::Char('d') if !ctrl => {
                if let Some(timer) = self.timers.selected_timer() {
                    let name = timer.name.clone();
                    self.confirm(
                        ConfirmKind::Timer(self.timers.selected),
                        format!("Delete timer \"{name}\"?"),
                    );
                }
            }
            KeyCode::Char('?') => self.modal = Modal::Help,
            KeyCode::Esc => self.back(),
            _ => {}
        }
    }

    fn handle_config_key(&mut self, key: &KeyEvent) {
        let mut save = false;
        let mut reset = false;
        let mut back = false;
        {
            let buf = &mut self.config_editor;
            match key.code {
                KeyCode::Char('s') if key.modifiers.ctrl => save = true,
                KeyCode::Char('r') if key.modifiers.ctrl => reset = true,
                KeyCode::Esc => back = true,
                KeyCode::Char(c) if !key.modifiers.ctrl && !key.modifiers.alt => {
                    buf.insert_char(c);
                    self.config_dirty = true;
                }
                KeyCode::Enter => {
                    buf.newline();
                    self.config_dirty = true;
                }
                KeyCode::Backspace => {
                    buf.backspace();
                    self.config_dirty = true;
                }
                KeyCode::Delete => {
                    buf.delete();
                    self.config_dirty = true;
                }
                KeyCode::Left => buf.move_left(),
                KeyCode::Right => buf.move_right(),
                KeyCode::Up => buf.move_up(),
                KeyCode::Down => buf.move_down(),
                KeyCode::Home => buf.home(),
                KeyCode::End => buf.end(),
                KeyCode::PageUp => buf.scroll = buf.scroll.saturating_sub(10),
                KeyCode::PageDown => buf.scroll += 10,
                KeyCode::Tab => {
                    for _ in 0..4 {
                        buf.insert_char(' ');
                    }
                    self.config_dirty = true;
                }
                _ => {}
            }
        }
        if save {
            let _ = self.save_config();
        }
        if reset {
            self.config_editor = TextBuffer::new(DEFAULT_CONFIG);
            self.config_dirty = true;
            self.toast("Config reset to defaults (not saved yet)", false);
        }
        if back {
            self.back();
        }
    }

    fn handle_modal_key(&mut self, key: &KeyEvent) {
        let mut submit = false;
        let mut close = false;
        match &mut self.modal {
            Modal::None => {}
            Modal::Help => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?')) {
                    close = true;
                }
            }
            Modal::Message { .. } => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
                    close = true;
                }
            }
            Modal::Ringing(_) => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ')) {
                    close = true;
                }
            }
            Modal::Confirm(_) => {
                if matches!(
                    key.code,
                    KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y')
                ) {
                    submit = true;
                } else if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N')
                ) {
                    close = true;
                }
            }
            Modal::FilePrompt(prompt) => {
                if key.code == KeyCode::Enter {
                    submit = true;
                } else if key.code == KeyCode::Esc {
                    close = true;
                } else {
                    apply_line_key(&mut prompt.input, key);
                }
            }
            Modal::Task(form) => match key.code {
                KeyCode::Enter => submit = true,
                KeyCode::Esc => close = true,
                KeyCode::Char('s') if key.modifiers.ctrl => submit = true,
                KeyCode::Tab | KeyCode::Down => form.next_field(),
                KeyCode::BackTab | KeyCode::Up => form.prev_field(),
                _ => {
                    apply_line_key(form.active_field(), key);
                }
            },
            Modal::Alarm(form) => match key.code {
                KeyCode::Enter => submit = true,
                KeyCode::Esc => close = true,
                KeyCode::Char('s') if key.modifiers.ctrl => submit = true,
                KeyCode::Tab | KeyCode::Down => form.next_field(),
                KeyCode::BackTab | KeyCode::Up => form.prev_field(),
                _ => {
                    apply_line_key(form.active_field(), key);
                }
            },
            Modal::Timer(form) => match key.code {
                KeyCode::Enter => submit = true,
                KeyCode::Esc => close = true,
                KeyCode::Char('s') if key.modifiers.ctrl => submit = true,
                KeyCode::Tab | KeyCode::Down => form.next_field(),
                KeyCode::BackTab | KeyCode::Up => form.prev_field(),
                _ => {
                    apply_line_key(form.active_field(), key);
                }
            },
        }
        if submit {
            self.submit_modal();
        } else if close {
            self.close_modal();
        }
    }

    fn submit_modal(&mut self) {
        let modal = std::mem::replace(&mut self.modal, Modal::None);
        match modal {
            Modal::FilePrompt(mut prompt) => {
                let input = prompt.input.value.clone();
                let result = match prompt.kind {
                    FilePromptKind::NewFile => self
                        .file_manager
                        .create_file(&input)
                        .map(|p| format!("Created {}", self.file_manager.abs_to_rel(&p))),
                    FilePromptKind::NewFolder => self
                        .file_manager
                        .create_dir(&input)
                        .map(|p| format!("Created {}", self.file_manager.abs_to_rel(&p))),
                    FilePromptKind::Rename => self
                        .file_manager
                        .rename_selected(&input)
                        .map(|p| format!("Renamed to {}", self.file_manager.abs_to_rel(&p))),
                    FilePromptKind::Path => self
                        .file_manager
                        .apply_path(&input)
                        .map(|_| format!("Now in {}", self.file_manager.cwd_rel())),
                    FilePromptKind::NotePath => self.rename_open_note(&input),
                    FilePromptKind::SectionTitle(index) => self.rename_section(index, &input),
                };
                match result {
                    Ok(message) => self.toast(message, false),
                    Err(err) => {
                        prompt.error = err;
                        self.modal = Modal::FilePrompt(prompt);
                    }
                }
            }
            Modal::Confirm(dialog) => match dialog.kind {
                ConfirmKind::Files => match self.file_manager.delete_selected() {
                    Ok(n) => self.toast(format!("Deleted {n} item(s)"), false),
                    Err(err) => self.toast(err, true),
                },
                ConfirmKind::Task(id) => {
                    self.calendar.delete_event(id);
                    self.save_calendar();
                    self.toast("Task deleted", false);
                }
                ConfirmKind::Alarm(index) => {
                    if index < self.alarms.alarms.len() {
                        self.alarms.alarms.remove(index);
                        self.alarms.selected = self
                            .alarms
                            .selected
                            .min(self.alarms.alarms.len().saturating_sub(1));
                        self.save_alarms();
                        self.toast("Alarm deleted", false);
                    }
                }
                ConfirmKind::Timer(index) => {
                    if index < self.timers.timers.len() {
                        self.timers.timers.remove(index);
                        self.timers.selected = self
                            .timers
                            .selected
                            .min(self.timers.timers.len().saturating_sub(1));
                        self.save_timers();
                        self.toast("Timer deleted", false);
                    }
                }
            },
            Modal::Task(mut form) => {
                let id = form.event_id.unwrap_or_else(|| self.calendar.next_id());
                match form.to_event(id) {
                    Ok(event) => {
                        if form.event_id.is_some() {
                            self.calendar.update_event(event);
                        } else {
                            self.calendar.add_event(event);
                        }
                        self.calendar.clamp_task_selection();
                        self.save_calendar();
                        self.toast("Task saved", false);
                    }
                    Err(err) => {
                        form.error = err;
                        self.modal = Modal::Task(form);
                    }
                }
            }
            Modal::Alarm(mut form) => match form.to_alarm() {
                Ok(alarm) => {
                    self.alarms.update(form.index, alarm);
                    self.save_alarms();
                    self.toast("Alarm saved", false);
                }
                Err(err) => {
                    form.error = err;
                    self.modal = Modal::Alarm(form);
                }
            },
            Modal::Timer(mut form) => match form.to_timer() {
                Ok(timer) => {
                    self.timers.update(form.index, timer);
                    self.save_timers();
                    self.toast("Timer saved", false);
                }
                Err(err) => {
                    form.error = err;
                    self.modal = Modal::Timer(form);
                }
            },
            Modal::Ringing(_) | Modal::Help | Modal::Message { .. } | Modal::None => {
                self.close_modal();
            }
        }
    }

    fn is_double_click(&mut self, hit: HitId) -> bool {
        let now = Instant::now();
        let double = self
            .last_click
            .is_some_and(|(t, h)| h == hit && now.duration_since(t) < Duration::from_millis(450));
        self.last_click = Some((now, hit));
        double
    }

    fn handle_click(&mut self, hit: HitId, column: u16, _row: u16) {
        if !matches!(self.modal, Modal::None) {
            match hit {
                HitId::FormField(i) => match &mut self.modal {
                    Modal::Task(form) => form.active = (i as usize).min(form.fields.len() - 1),
                    Modal::Alarm(form) => form.active = (i as usize).min(form.fields.len() - 1),
                    Modal::Timer(form) => form.active = (i as usize).min(form.fields.len() - 1),
                    _ => {}
                },
                HitId::FormButton(0) => self.submit_modal(),
                HitId::FormButton(1) => self.close_modal(),
                HitId::HelpClose | HitId::ModalOk => self.close_modal(),
                _ => {}
            }
            return;
        }

        match hit {
            HitId::Nav(index) => self.nav_to(index),
            HitId::HeaderConfig => self.open_config(),
            HitId::FileCrumb => self.open_file_prompt(FilePromptKind::Path),
            HitId::FileRow(index) => {
                if index == u16::MAX {
                    self.file_manager.go_up();
                    return;
                }
                let double = self.is_double_click(hit);
                self.file_manager.select(index as usize);
                if double {
                    match self.file_manager.open_selected() {
                        OpenAction::File { abs, rel } => self.open_editor(abs, rel),
                        OpenAction::Directory | OpenAction::None => {}
                    }
                }
            }
            HitId::EditorLine(index) => {
                if let Some(doc) = &mut self.editor {
                    let col =
                        (column as i32 - doc.content_x as i32 + doc.hscroll as i32).max(0) as usize;
                    doc.buffer.set_cursor(index as usize, col);
                }
            }
            HitId::EditorFold(index) => {
                if let Some(doc) = &mut self.editor {
                    doc.buffer.row = (index as usize).min(doc.buffer.lines.len().saturating_sub(1));
                    let syntax = self.config.markdown.clone();
                    doc.toggle_fold(&syntax);
                }
            }
            HitId::EditorCrumb(index) => self.open_editor_crumb_prompt(index),
            HitId::ConfigLine(index) => {
                let col = (column as i32 - self.config_content_x as i32).max(0) as usize;
                self.config_editor.set_cursor(index as usize, col);
            }
            HitId::CalDay(index) => {
                let grid = self.calendar.month_grid();
                if let Some(date) = grid.get(index as usize).copied() {
                    self.calendar.set_selected_date(date);
                    self.calendar.focus = CalendarFocus::Grid;
                }
            }
            HitId::CalTask(index) => {
                let double = self.is_double_click(hit);
                self.calendar.focus = CalendarFocus::Tasks;
                self.calendar.selected_task = index as usize;
                if double {
                    self.open_task_form(Some(index as usize));
                }
            }
            HitId::AlarmRow(index) => {
                let double = self.is_double_click(hit);
                self.alarms.selected =
                    (index as usize).min(self.alarms.alarms.len().saturating_sub(1));
                if double {
                    self.open_alarm_form(Some(index as usize));
                }
            }
            HitId::TimerRow(index) => {
                let double = self.is_double_click(hit);
                self.timers.selected =
                    (index as usize).min(self.timers.timers.len().saturating_sub(1));
                if double {
                    self.open_timer_form(Some(index as usize));
                }
            }
            HitId::TimerAction(index, action) => {
                self.timers.selected =
                    (index as usize).min(self.timers.timers.len().saturating_sub(1));
                let now = Instant::now();
                if let Some(timer) = self.timers.timers.get_mut(index as usize) {
                    if action == 0 {
                        timer.toggle(now);
                    } else {
                        timer.stop();
                    }
                }
            }
            HitId::FormField(_) | HitId::FormButton(_) | HitId::HelpClose | HitId::ModalOk => {}
        }
    }
}

fn apply_line_key(input: &mut LineInput, key: &KeyEvent) -> bool {
    match key.code {
        KeyCode::Char(c) if !key.modifiers.ctrl && !key.modifiers.alt => {
            input.insert_char(c);
            true
        }
        KeyCode::Backspace => {
            input.backspace();
            true
        }
        KeyCode::Delete => {
            input.delete();
            true
        }
        KeyCode::Left => {
            input.left();
            true
        }
        KeyCode::Right => {
            input.right();
            true
        }
        KeyCode::Home => {
            input.home();
            true
        }
        KeyCode::End => {
            input.end();
            true
        }
        _ => false,
    }
}

/// Kept for tests / future callers that want the same date helpers as the
/// calendar module without importing chrono directly.
pub fn parse_event_datetime(input: &str) -> Option<NaiveDateTime> {
    parse_datetime(input)
}

pub fn format_event_datetime(dt: NaiveDateTime) -> String {
    format_datetime(dt)
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}
