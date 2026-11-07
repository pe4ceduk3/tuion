//! Calendar events, recurrence, notifications and the human-readable
//! `state/calendar.md` format.

use crate::core::storage;
use crate::core::text::LineInput;
use chrono::{
    Datelike, Duration as ChronoDuration, Local, NaiveDate, NaiveDateTime, NaiveTime, Weekday,
};
use ratatui::style::Color;
use std::collections::HashSet;
use std::io;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventColor {
    White,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
}

impl EventColor {
    pub const ALL: [EventColor; 8] = [
        EventColor::White,
        EventColor::Black,
        EventColor::Red,
        EventColor::Green,
        EventColor::Yellow,
        EventColor::Blue,
        EventColor::Magenta,
        EventColor::Cyan,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::White => "white",
            Self::Black => "black",
            Self::Red => "red",
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Blue => "blue",
            Self::Magenta => "magenta",
            Self::Cyan => "cyan",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.trim().trim_start_matches('#').to_ascii_lowercase();
        let name = name
            .strip_prefix("b.")
            .or_else(|| name.strip_prefix("d."))
            .unwrap_or(&name);
        Self::ALL.iter().copied().find(|c| c.name() == name)
    }

    pub fn ratatui(self, bright: bool) -> Color {
        match (self, bright) {
            (Self::White, false) => Color::White,
            (Self::White, true) => Color::Gray,
            (Self::Black, false) => Color::Black,
            (Self::Black, true) => Color::DarkGray,
            (Self::Red, false) => Color::Red,
            (Self::Red, true) => Color::LightRed,
            (Self::Green, false) => Color::Green,
            (Self::Green, true) => Color::LightGreen,
            (Self::Yellow, false) => Color::Yellow,
            (Self::Yellow, true) => Color::LightYellow,
            (Self::Blue, false) => Color::Blue,
            (Self::Blue, true) => Color::LightBlue,
            (Self::Magenta, false) => Color::Magenta,
            (Self::Magenta, true) => Color::LightMagenta,
            (Self::Cyan, false) => Color::Cyan,
            (Self::Cyan, true) => Color::LightCyan,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    /// Every N days.
    Days(u32),
    /// Every week on weekday N (1 = Monday, 7 = Sunday).
    Weekday(u8),
    /// Every year in month N (1 = January, 12 = December).
    Month(u8),
}

impl Repeat {
    pub fn storage(self) -> String {
        match self {
            Self::Days(n) => format!("d{n}"),
            Self::Weekday(n) => format!("w{n}"),
            Self::Month(n) => format!("m{n}"),
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Days(n) => format!("every {n} day(s)"),
            Self::Weekday(n) => format!("every {}", weekday_name(n as u32)),
            Self::Month(n) => format!("every year in {}", month_name(n as u32)),
        }
    }

    pub fn parse(input: &str) -> Result<Option<Self>, String> {
        let raw = input.trim().trim_start_matches('&').to_ascii_lowercase();
        if raw.is_empty() {
            return Ok(None);
        }
        let (kind, number) = if let Some(rest) = raw.strip_prefix("day") {
            ('d', rest.trim())
        } else if let Some(rest) = raw.strip_prefix("week") {
            ('w', rest.trim())
        } else if let Some(rest) = raw.strip_prefix("month") {
            ('m', rest.trim())
        } else {
            let mut chars = raw.chars();
            let kind = chars.next().unwrap_or('d');
            (kind, chars.as_str())
        };
        let n: u32 = if number.is_empty() {
            1
        } else {
            number
                .parse()
                .map_err(|_| format!("bad repeat number: {number}"))?
        };
        match kind {
            'd' => {
                if n == 0 {
                    Err("repeat day must be >= 1".into())
                } else {
                    Ok(Some(Self::Days(n)))
                }
            }
            'w' => {
                if (1..=7).contains(&n) {
                    Ok(Some(Self::Weekday(n as u8)))
                } else {
                    Err("repeat week must be 1..7 (1 = Monday)".into())
                }
            }
            'm' => {
                if (1..=12).contains(&n) {
                    Ok(Some(Self::Month(n as u8)))
                } else {
                    Err("repeat month must be 1..12".into())
                }
            }
            _ => Err("repeat must start with d, w or m".into()),
        }
    }

    pub fn occurs_on(self, base: NaiveDate, day: NaiveDate) -> bool {
        if day < base {
            return false;
        }
        match self {
            Self::Days(n) => {
                let n = n.max(1) as i64;
                (day - base).num_days() % n == 0
            }
            Self::Weekday(n) => day.weekday().number_from_monday() as u8 == n,
            Self::Month(n) => day.month() as u8 == n && day.day() == base.day(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notify {
    At(NaiveDateTime),
    Before(u32),
    After(u32),
}

impl Notify {
    pub fn storage(self) -> String {
        match self {
            Self::At(dt) => format_datetime(dt),
            Self::Before(n) => n.to_string(),
            Self::After(n) => format!("+{n}"),
        }
    }

    pub fn parse(input: &str) -> Result<Option<Self>, String> {
        let raw = input.trim().trim_start_matches('!');
        if raw.is_empty() {
            return Ok(None);
        }
        if let Some(rest) = raw.strip_prefix('+') {
            let n = rest
                .parse::<u32>()
                .map_err(|_| format!("bad notification offset: {rest}"))?;
            return Ok(Some(Self::After(n)));
        }
        if raw.chars().all(|c| c.is_ascii_digit()) {
            let n = raw
                .parse::<u32>()
                .map_err(|_| format!("bad notification offset: {raw}"))?;
            return Ok(Some(Self::Before(n)));
        }
        let dt = parse_datetime(raw).ok_or_else(|| format!("bad notification time: {raw}"))?;
        Ok(Some(Self::At(dt)))
    }

    pub fn resolve(self, occurrence_start: NaiveDateTime) -> NaiveDateTime {
        match self {
            Self::At(dt) => dt,
            Self::Before(n) => occurrence_start - ChronoDuration::minutes(n as i64),
            Self::After(n) => occurrence_start + ChronoDuration::minutes(n as i64),
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::At(dt) => format_datetime(dt),
            Self::Before(n) => format!("{n} min before"),
            Self::After(n) => format!("{n} min after"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Event {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub start: Option<NaiveDateTime>,
    pub deadline: Option<NaiveDateTime>,
    pub repeat: Option<Repeat>,
    pub notify: Option<Notify>,
    pub color: EventColor,
    pub bright: bool,
    pub done: bool,
}

impl Event {
    pub fn duration(&self) -> Option<ChronoDuration> {
        match (self.start, self.deadline) {
            (Some(a), Some(b)) if b >= a => Some(b - a),
            _ => None,
        }
    }

    pub fn base_date(&self) -> Option<NaiveDate> {
        self.start
            .map(|d| d.date())
            .or_else(|| self.deadline.map(|d| d.date()))
    }

    pub fn occurrence_on(&self, day: NaiveDate) -> Option<Occurrence> {
        let duration = self.duration();
        if let Some(repeat) = self.repeat {
            let base = self.base_date()?;
            if !repeat.occurs_on(base, day) {
                return None;
            }
            let time = self.start.map(|d| d.time()).unwrap_or(NaiveTime::MIN);
            let start = day.and_time(time);
            let deadline = duration.map(|d| start + d);
            return Some(Occurrence {
                event_id: self.id,
                start: Some(start),
                deadline,
                all_day: self.start.is_none(),
            });
        }

        let start_date = self.start.map(|d| d.date());
        let deadline_date = self.deadline.map(|d| d.date());
        let in_range = match (start_date, deadline_date) {
            (Some(a), Some(b)) => day >= a && day <= b,
            (Some(a), None) => day == a,
            (None, Some(b)) => day == b,
            (None, None) => false,
        };
        if !in_range {
            return None;
        }
        Some(Occurrence {
            event_id: self.id,
            start: self.start,
            deadline: self.deadline,
            all_day: self.start.is_none(),
        })
    }

    pub fn time_label_on(&self, day: NaiveDate) -> String {
        let occ = self.occurrence_on(day);
        let start = occ.as_ref().and_then(|o| o.start);
        let deadline = occ.as_ref().and_then(|o| o.deadline);
        match start {
            Some(start) => match deadline {
                Some(deadline) => {
                    format!("{} - {}", start.format("%H.%M"), deadline.format("%H.%M"))
                }
                None => start.format("%H.%M").to_string(),
            },
            None => "all day".into(),
        }
    }

    pub fn to_storage(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "- [{}] {}\n",
            if self.done { "x" } else { " " },
            self.name.replace('\n', " ")
        ));
        if let Some(start) = self.start {
            out.push_str(&format!(":{}\n", format_datetime(start)));
        }
        if let Some(deadline) = self.deadline {
            out.push_str(&format!("-{}\n", format_datetime(deadline)));
        }
        if let Some(notify) = self.notify {
            out.push_str(&format!("!{}\n", notify.storage()));
        }
        out.push_str(&format!(
            "#{}.{}\n",
            if self.bright { "b" } else { "d" },
            self.color.name()
        ));
        if let Some(repeat) = self.repeat {
            out.push_str(&format!("&{}\n", repeat.storage()));
        }
        for line in self.description.lines() {
            out.push_str(&format!("> {line}\n"));
        }
        out
    }
}

#[derive(Clone, Debug)]
pub struct Occurrence {
    pub event_id: u64,
    pub start: Option<NaiveDateTime>,
    pub deadline: Option<NaiveDateTime>,
    pub all_day: bool,
}

impl Occurrence {
    pub fn time_label(&self) -> String {
        match (self.start, self.deadline) {
            (Some(a), Some(b)) => format!("{} - {}", a.format("%H.%M"), b.format("%H.%M")),
            (Some(a), None) => a.format("%H.%M").to_string(),
            (None, Some(b)) => format!("until {}", b.format("%H.%M")),
            (None, None) => "all day".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarFocus {
    Grid,
    Tasks,
}

pub struct CalendarState {
    pub events: Vec<Event>,
    pub selected_date: NaiveDate,
    pub visible_month: NaiveDate,
    pub focus: CalendarFocus,
    pub selected_task: usize,
    pub scroll: usize,
    pub notified: HashSet<(u64, NaiveDate)>,
    next_id: u64,
}

impl CalendarState {
    pub fn load(path: &Path) -> Self {
        let text = storage::read_text(path).unwrap_or_default();
        let mut events = parse_calendar(&text);
        let mut next_id = 1;
        for event in &mut events {
            event.id = next_id;
            next_id += 1;
        }
        let today = Local::now().date_naive();
        Self {
            events,
            selected_date: today,
            visible_month: first_of_month(today),
            focus: CalendarFocus::Grid,
            selected_task: 0,
            scroll: 0,
            notified: HashSet::new(),
            next_id,
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let mut text = String::from(
            "# TUION calendar\n# name:start-deadline!notify#b.color\n# repeat: &d1 (days), &w1..w7 (weekday), &m1..m12 (month)\n\n",
        );
        for event in &self.events {
            text.push_str(&event.to_storage());
            text.push('\n');
        }
        storage::write_text(path, &text)
    }

    pub fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_event(&mut self, event: Event) {
        self.events.push(event);
    }

    pub fn update_event(&mut self, event: Event) {
        if let Some(slot) = self.events.iter_mut().find(|e| e.id == event.id) {
            *slot = event;
        }
    }

    pub fn delete_event(&mut self, id: u64) {
        self.events.retain(|e| e.id != id);
        self.selected_task = self
            .selected_task
            .min(self.selected_occurrences().len().saturating_sub(1));
    }

    pub fn event(&self, id: u64) -> Option<&Event> {
        self.events.iter().find(|e| e.id == id)
    }

    pub fn toggle_done(&mut self, id: u64) {
        if let Some(event) = self.events.iter_mut().find(|e| e.id == id) {
            event.done = !event.done;
        }
    }

    pub fn occurrences_on(&self, date: NaiveDate) -> Vec<Occurrence> {
        let mut out: Vec<Occurrence> = self
            .events
            .iter()
            .filter_map(|event| event.occurrence_on(date))
            .collect();
        out.sort_by_key(|o| o.start.map(|s| s.time()).unwrap_or(NaiveTime::MIN));
        out
    }

    pub fn selected_occurrences(&self) -> Vec<Occurrence> {
        self.occurrences_on(self.selected_date)
    }

    pub fn has_events(&self, date: NaiveDate) -> bool {
        self.events
            .iter()
            .any(|event| event.occurrence_on(date).is_some())
    }

    pub fn month_grid(&self) -> Vec<NaiveDate> {
        let first = self.visible_month;
        let offset = first.weekday().num_days_from_monday() as i64;
        let start = first - ChronoDuration::days(offset);
        (0..42).map(|i| start + ChronoDuration::days(i)).collect()
    }

    pub fn month_title(&self) -> String {
        self.visible_month.format("%B %Y").to_string()
    }

    pub fn set_month(&mut self, delta: i32) {
        let year = self.visible_month.year();
        let month = self.visible_month.month() as i32 + delta;
        let (year, month) = normalize_month(year, month);
        self.visible_month = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(self.visible_month);
        let day = self.selected_date.day();
        self.selected_date = NaiveDate::from_ymd_opt(year, month, day)
            .or_else(|| {
                // Moving from e.g. the 31st to a shorter month: clamp to the
                // last valid day instead of silently losing the selection.
                let next = normalize_month(year, month as i32 + 1);
                NaiveDate::from_ymd_opt(next.0, next.1, 1).and_then(|d| d.pred_opt())
            })
            .unwrap_or(self.visible_month);
        self.selected_task = 0;
        self.scroll = 0;
    }

    pub fn shift_day(&mut self, days: i64) {
        if let Some(date) = self
            .selected_date
            .checked_add_signed(ChronoDuration::days(days))
        {
            self.selected_date = date;
            self.visible_month = first_of_month(date);
            self.selected_task = 0;
            self.scroll = 0;
        }
    }

    pub fn set_selected_date(&mut self, date: NaiveDate) {
        self.selected_date = date;
        self.visible_month = first_of_month(date);
        self.selected_task = 0;
        self.scroll = 0;
    }

    pub fn today(&mut self) {
        self.set_selected_date(Local::now().date_naive());
    }

    pub fn clamp_task_selection(&mut self) {
        let len = self.selected_occurrences().len();
        self.selected_task = self.selected_task.min(len.saturating_sub(1));
    }

    /// Check notifications once per minute. Returns human-readable messages
    /// for notifications that became due in this minute.
    pub fn due_notifications(&mut self, now: NaiveDateTime) -> Vec<String> {
        let mut out = Vec::new();
        let days = [now.date(), now.date() + ChronoDuration::days(1)];
        for day in days {
            for event in &self.events {
                let Some(occ) = event.occurrence_on(day) else {
                    continue;
                };
                let Some(base) = occ.start.or(occ.deadline) else {
                    continue;
                };
                // Dated tasks notify ten minutes before their start by
                // default; an explicit `!` line overrides that.
                let notify = event.notify.unwrap_or(Notify::Before(10));
                let at = notify.resolve(base);
                if now >= at && now < at + ChronoDuration::minutes(1) {
                    if self.notified.insert((event.id, day)) {
                        out.push(format!(
                            "{} {} {}",
                            event.name,
                            at.format("%H.%M"),
                            if event.done { "(done)" } else { "" }
                        ));
                    }
                }
            }
        }
        out
    }
}

fn normalize_month(year: i32, month: i32) -> (i32, u32) {
    let zero = year * 12 + (month - 1);
    let y = zero.div_euclid(12);
    let m = zero.rem_euclid(12) + 1;
    (y, m as u32)
}

pub fn first_of_month(date: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(date)
}

pub fn month_name(month: u32) -> &'static str {
    const NAMES: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    NAMES
        .get(month.saturating_sub(1) as usize)
        .copied()
        .unwrap_or("?")
}

pub fn weekday_name(n: u32) -> &'static str {
    const NAMES: [&str; 7] = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    NAMES
        .get(n.saturating_sub(1) as usize)
        .copied()
        .unwrap_or("?")
}

pub fn parse_datetime(input: &str) -> Option<NaiveDateTime> {
    let input = input.trim();
    NaiveDateTime::parse_from_str(input, "%Y.%m.%d-%H.%M")
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(input, "%Y.%m.%d")
                .ok()
                .map(|d| d.and_time(NaiveTime::MIN))
        })
}

pub fn format_datetime(dt: NaiveDateTime) -> String {
    dt.format("%Y.%m.%d-%H.%M").to_string()
}

pub fn parse_calendar(text: &str) -> Vec<Event> {
    let mut events = Vec::new();
    let mut current: Option<Event> = None;
    let mut id = 1;

    let flush = |current: &mut Option<Event>, events: &mut Vec<Event>, id: &mut u64| {
        if let Some(mut event) = current.take() {
            event.id = *id;
            *id += 1;
            event.description = event.description.trim().to_string();
            events.push(event);
        }
    };

    for raw in text.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if let Some(event) = current.as_mut() {
                if !event.description.is_empty() && !event.description.ends_with('\n') {
                    event.description.push('\n');
                }
            }
            continue;
        }
        if trimmed.starts_with('#') && !trimmed.starts_with("#b.") && !trimmed.starts_with("#d.") {
            continue;
        }
        if let Some((done, name)) = task_heading(trimmed) {
            flush(&mut current, &mut events, &mut id);
            current = Some(Event {
                id: 0,
                name: name.to_string(),
                description: String::new(),
                start: None,
                deadline: None,
                repeat: None,
                notify: None,
                color: EventColor::White,
                bright: false,
                done,
            });
            continue;
        }
        if current.is_none() {
            if let Some(event) = parse_compact(trimmed, id) {
                id += 1;
                events.push(event);
            }
            continue;
        }
        let event = current.as_mut().unwrap();
        if let Some(rest) = trimmed.strip_prefix(':') {
            if let Some(dt) = parse_datetime(rest) {
                event.start = Some(dt);
            }
        } else if let Some(rest) = trimmed.strip_prefix('-') {
            if let Some(dt) = parse_datetime(rest.trim()) {
                event.deadline = Some(dt);
            }
        } else if let Some(rest) = trimmed.strip_prefix('!') {
            if let Ok(Some(notify)) = Notify::parse(rest) {
                event.notify = Some(notify);
            }
        } else if trimmed.starts_with("#b.") || trimmed.starts_with("#d.") {
            event.bright = trimmed.starts_with("#b.");
            if let Some(color) = EventColor::from_name(trimmed) {
                event.color = color;
            }
        } else if let Some(rest) = trimmed.strip_prefix('&') {
            if let Ok(repeat) = Repeat::parse(rest) {
                event.repeat = repeat;
            }
        } else if let Some(rest) = trimmed.strip_prefix('>') {
            if !event.description.is_empty() {
                event.description.push('\n');
            }
            event.description.push_str(rest.trim_start());
        } else {
            if !event.description.is_empty() {
                event.description.push('\n');
            }
            event.description.push_str(trimmed);
        }
    }
    flush(&mut current, &mut events, &mut id);
    events
}

fn task_heading(line: &str) -> Option<(bool, &str)> {
    let rest = line.strip_prefix("- [")?;
    let mut chars = rest.chars();
    let mark = chars.next()?;
    if chars.next()? != ']' {
        return None;
    }
    let done = match mark {
        ' ' => false,
        'x' | 'X' => true,
        _ => return None,
    };
    let rest = chars.as_str().strip_prefix(' ').unwrap_or(chars.as_str());
    Some((done, rest.trim()))
}

fn parse_compact(line: &str, id: u64) -> Option<Event> {
    // name:start-deadline!notify#b.color
    let colon = line
        .char_indices()
        .find(|(i, c)| {
            *c == ':'
                && line[*i + 1..]
                    .chars()
                    .next()
                    .is_some_and(|n| n.is_ascii_digit())
        })
        .map(|(i, _)| i)?;
    let name = line[..colon].trim();
    if name.is_empty() {
        return None;
    }
    let rest = &line[colon + 1..];
    let (rest, color) = match rest.find('#') {
        Some(i) => (&rest[..i], Some(&rest[i..])),
        None => (rest, None),
    };
    let (rest, notify) = match rest.find('!') {
        Some(i) => (&rest[..i], Some(&rest[i..])),
        None => (rest, None),
    };
    let (start_s, deadline_s) = split_two_datetimes(rest)?;
    let start = parse_datetime(start_s)?;
    let deadline = deadline_s.and_then(parse_datetime);
    let bright = color.map(|c| c.starts_with("#b.")).unwrap_or(false);
    let event_color = color
        .and_then(EventColor::from_name)
        .unwrap_or(EventColor::White);
    let notify = notify.and_then(|n| Notify::parse(n).ok().flatten());
    Some(Event {
        id,
        name: name.to_string(),
        description: String::new(),
        start: Some(start),
        deadline,
        repeat: None,
        notify,
        color: event_color,
        bright,
        done: false,
    })
}

fn split_two_datetimes(s: &str) -> Option<(&str, Option<&str>)> {
    let s = s.trim();
    let first_len = datetime_prefix_len(s)?;
    let first = &s[..first_len];
    let rest = &s[first_len..];
    if let Some(rest) = rest.strip_prefix('-') {
        let second_len = datetime_prefix_len(rest)?;
        let second = &rest[..second_len];
        Some((first, Some(second)))
    } else {
        Some((first, None))
    }
}

fn datetime_prefix_len(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.len() < 10 {
        return None;
    }
    let date_ok = bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'.'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'.'
        && bytes[8..10].iter().all(u8::is_ascii_digit);
    if !date_ok {
        return None;
    }
    if bytes.len() >= 16
        && bytes[10] == b'-'
        && bytes[11..13].iter().all(u8::is_ascii_digit)
        && bytes[13] == b'.'
        && bytes[14..16].iter().all(u8::is_ascii_digit)
    {
        Some(16)
    } else {
        Some(10)
    }
}

pub struct TaskField {
    pub label: &'static str,
    pub hint: &'static str,
    pub value: LineInput,
}

pub struct TaskForm {
    pub event_id: Option<u64>,
    pub done: bool,
    pub fields: Vec<TaskField>,
    pub active: usize,
    pub error: String,
}

pub const TASK_FIELDS: [(&str, &str); 8] = [
    ("name", "task name"),
    ("description", "markdown text"),
    ("start", "yyyy.mm.dd-hh.mm"),
    ("deadline", "yyyy.mm.dd-hh.mm"),
    ("repeat", "d1 / w1..w7 / m1..m12"),
    ("notification", "10, +30 or yyyy.mm.dd-hh.mm"),
    ("bright", "bright / dark"),
    ("color", "white black red green yellow blue magenta cyan"),
];

impl TaskForm {
    pub fn new(event: Option<&Event>, date: NaiveDate) -> Self {
        let fields = if let Some(event) = event {
            vec![
                TaskField {
                    label: TASK_FIELDS[0].0,
                    hint: TASK_FIELDS[0].1,
                    value: LineInput::new(event.name.clone()),
                },
                TaskField {
                    label: TASK_FIELDS[1].0,
                    hint: TASK_FIELDS[1].1,
                    value: LineInput::new(event.description.replace('\n', " ")),
                },
                TaskField {
                    label: TASK_FIELDS[2].0,
                    hint: TASK_FIELDS[2].1,
                    value: LineInput::new(event.start.map(format_datetime).unwrap_or_default()),
                },
                TaskField {
                    label: TASK_FIELDS[3].0,
                    hint: TASK_FIELDS[3].1,
                    value: LineInput::new(event.deadline.map(format_datetime).unwrap_or_default()),
                },
                TaskField {
                    label: TASK_FIELDS[4].0,
                    hint: TASK_FIELDS[4].1,
                    value: LineInput::new(event.repeat.map(|r| r.storage()).unwrap_or_default()),
                },
                TaskField {
                    label: TASK_FIELDS[5].0,
                    hint: TASK_FIELDS[5].1,
                    value: LineInput::new(event.notify.map(|n| n.storage()).unwrap_or_default()),
                },
                TaskField {
                    label: TASK_FIELDS[6].0,
                    hint: TASK_FIELDS[6].1,
                    value: LineInput::new(if event.bright { "bright" } else { "dark" }),
                },
                TaskField {
                    label: TASK_FIELDS[7].0,
                    hint: TASK_FIELDS[7].1,
                    value: LineInput::new(event.color.name()),
                },
            ]
        } else {
            let default_start = date.and_time(NaiveTime::from_hms_opt(9, 0, 0).unwrap());
            let default_deadline = date.and_time(NaiveTime::from_hms_opt(10, 0, 0).unwrap());
            vec![
                TaskField {
                    label: TASK_FIELDS[0].0,
                    hint: TASK_FIELDS[0].1,
                    value: LineInput::default(),
                },
                TaskField {
                    label: TASK_FIELDS[1].0,
                    hint: TASK_FIELDS[1].1,
                    value: LineInput::default(),
                },
                TaskField {
                    label: TASK_FIELDS[2].0,
                    hint: TASK_FIELDS[2].1,
                    value: LineInput::new(format_datetime(default_start)),
                },
                TaskField {
                    label: TASK_FIELDS[3].0,
                    hint: TASK_FIELDS[3].1,
                    value: LineInput::new(format_datetime(default_deadline)),
                },
                TaskField {
                    label: TASK_FIELDS[4].0,
                    hint: TASK_FIELDS[4].1,
                    value: LineInput::default(),
                },
                TaskField {
                    label: TASK_FIELDS[5].0,
                    hint: TASK_FIELDS[5].1,
                    value: LineInput::new("10"),
                },
                TaskField {
                    label: TASK_FIELDS[6].0,
                    hint: TASK_FIELDS[6].1,
                    value: LineInput::new("dark"),
                },
                TaskField {
                    label: TASK_FIELDS[7].0,
                    hint: TASK_FIELDS[7].1,
                    value: LineInput::new("white"),
                },
            ]
        };
        Self {
            event_id: event.map(|e| e.id),
            done: event.map(|e| e.done).unwrap_or(false),
            fields,
            active: 0,
            error: String::new(),
        }
    }

    pub fn active_field(&mut self) -> &mut LineInput {
        &mut self.fields[self.active].value
    }

    pub fn next_field(&mut self) {
        self.active = (self.active + 1) % self.fields.len();
    }

    pub fn prev_field(&mut self) {
        self.active = (self.active + self.fields.len() - 1) % self.fields.len();
    }

    pub fn value(&self, index: usize) -> &str {
        self.fields
            .get(index)
            .map(|f| f.value.as_str())
            .unwrap_or("")
    }

    pub fn to_event(&self, id: u64) -> Result<Event, String> {
        let name = self.value(0).trim();
        if name.is_empty() {
            return Err("name is required".into());
        }
        let description = self.value(1).trim().to_string();
        let start_raw = self.value(2).trim();
        let deadline_raw = self.value(3).trim();
        let start = if start_raw.is_empty() {
            None
        } else {
            Some(parse_datetime(start_raw).ok_or_else(|| format!("bad start: {start_raw}"))?)
        };
        let deadline = if deadline_raw.is_empty() {
            None
        } else {
            Some(
                parse_datetime(deadline_raw)
                    .ok_or_else(|| format!("bad deadline: {deadline_raw}"))?,
            )
        };
        if let (Some(a), Some(b)) = (start, deadline) {
            if b < a {
                return Err("deadline is before start".into());
            }
        }
        let repeat = Repeat::parse(self.value(4))?;
        let notify = Notify::parse(self.value(5))?;
        let bright_field = self.value(6).trim().to_ascii_lowercase();
        let mut bright = matches!(bright_field.as_str(), "bright" | "true" | "b" | "1" | "yes");
        let color_raw = self.value(7).trim();
        if color_raw.starts_with("#b.") {
            bright = true;
        }
        let color = EventColor::from_name(color_raw)
            .ok_or_else(|| format!("unknown colour: {color_raw}"))?;
        Ok(Event {
            id,
            name: name.to_string(),
            description,
            start,
            deadline,
            repeat,
            notify,
            color,
            bright,
            done: self.done,
        })
    }
}

pub fn weekday_number(weekday: Weekday) -> u8 {
    weekday.number_from_monday() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_serialize_roundtrip() {
        let text = r#"
- [ ] Созвон с командой
:2026.10.29-14.00
-2026.10.29-15.00
!10
#b.blue
> Обсудить спринт.
> И блокеры.
"#;
        let events = parse_calendar(text);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "Созвон с командой");
        assert_eq!(
            events[0].start.unwrap().format("%H.%M").to_string(),
            "14.00"
        );
        assert_eq!(events[0].notify, Some(Notify::Before(10)));
        assert!(events[0].bright);
        assert_eq!(events[0].color, EventColor::Blue);
        assert!(events[0].description.contains("блокеры"));
        let round = parse_calendar(&events[0].to_storage());
        assert_eq!(round.len(), 1);
        assert_eq!(round[0].name, events[0].name);
    }

    #[test]
    fn compact_line_parses() {
        let event =
            parse_calendar("name:2026.10.29-14.00-2026.10.29-15.00!2026.10.29-13.50#b.blue");
        assert_eq!(event.len(), 1);
        assert_eq!(event[0].name, "name");
        assert_eq!(
            event[0].notify,
            Some(Notify::At(parse_datetime("2026.10.29-13.50").unwrap()))
        );
        assert!(event[0].bright);
    }

    #[test]
    fn recurrence_days_weeks_months() {
        let base = NaiveDate::from_ymd_opt(2026, 10, 26).unwrap(); // Monday
        let days = Repeat::Days(2);
        assert!(days.occurs_on(base, base));
        assert!(days.occurs_on(base, base + ChronoDuration::days(2)));
        assert!(!days.occurs_on(base, base + ChronoDuration::days(1)));
        let week = Repeat::Weekday(1);
        assert!(week.occurs_on(base, base + ChronoDuration::days(7)));
        assert!(!week.occurs_on(base, base + ChronoDuration::days(1)));
        let month = Repeat::Month(10);
        assert!(month.occurs_on(base, NaiveDate::from_ymd_opt(2027, 10, 26).unwrap()));
        assert!(!month.occurs_on(base, NaiveDate::from_ymd_opt(2027, 11, 26).unwrap()));
    }

    #[test]
    fn recurrence_produces_occurrences() {
        let mut event = Event {
            id: 1,
            name: "habit".into(),
            description: String::new(),
            start: Some(parse_datetime("2026.10.26-18.00").unwrap()),
            deadline: Some(parse_datetime("2026.10.26-18.30").unwrap()),
            repeat: Some(Repeat::Days(1)),
            notify: None,
            color: EventColor::White,
            bright: false,
            done: false,
        };
        let occ = event
            .occurrence_on(NaiveDate::from_ymd_opt(2026, 11, 5).unwrap())
            .unwrap();
        assert_eq!(occ.start.unwrap().format("%H.%M").to_string(), "18.00");
        assert_eq!(occ.deadline.unwrap().format("%H.%M").to_string(), "18.30");
        event.repeat = Some(Repeat::Weekday(7));
        assert!(
            event
                .occurrence_on(NaiveDate::from_ymd_opt(2026, 11, 1).unwrap())
                .is_some()
        );
    }

    #[test]
    fn form_roundtrip() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
        let mut form = TaskForm::new(None, date);
        form.fields[0].value.set("Test");
        form.fields[4].value.set("d2");
        form.fields[5].value.set("10");
        form.fields[7].value.set("red");
        let event = form.to_event(7).unwrap();
        assert_eq!(event.id, 7);
        assert_eq!(event.name, "Test");
        assert_eq!(event.repeat, Some(Repeat::Days(2)));
        assert_eq!(event.color, EventColor::Red);
        assert_eq!(event.notify, Some(Notify::Before(10)));
    }
}
