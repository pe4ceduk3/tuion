//! Countdown timers, stored as plain text in `state/timers.txt`.

use crate::core::storage;
use crate::core::text::LineInput;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerStatus {
    Idle,
    Running,
    Paused,
    Finished,
}

impl TimerStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "ready",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Finished => "finished",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Timer {
    pub name: String,
    pub total: Duration,
    pub remaining: Duration,
    pub status: TimerStatus,
    pub last_tick: Option<Instant>,
}

impl Timer {
    pub fn new(name: impl Into<String>, total: Duration) -> Self {
        Self {
            name: name.into(),
            total,
            remaining: total,
            status: TimerStatus::Idle,
            last_tick: None,
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.total.saturating_sub(self.remaining)
    }

    pub fn start(&mut self, now: Instant) {
        if self.status == TimerStatus::Finished || self.status == TimerStatus::Idle {
            self.remaining = self.total;
        }
        if self.total.is_zero() {
            self.status = TimerStatus::Finished;
            self.last_tick = None;
            return;
        }
        self.status = TimerStatus::Running;
        self.last_tick = Some(now);
    }

    pub fn pause(&mut self, now: Instant) {
        self.advance(now);
        if self.status == TimerStatus::Running {
            self.status = TimerStatus::Paused;
            self.last_tick = None;
        }
    }

    pub fn stop(&mut self) {
        self.status = TimerStatus::Idle;
        self.remaining = self.total;
        self.last_tick = None;
    }

    pub fn toggle(&mut self, now: Instant) {
        match self.status {
            TimerStatus::Running => self.pause(now),
            TimerStatus::Idle | TimerStatus::Paused | TimerStatus::Finished => self.start(now),
        }
    }

    /// Advance a running timer. Returns true when it just finished.
    pub fn advance(&mut self, now: Instant) -> bool {
        if self.status != TimerStatus::Running {
            return false;
        }
        let last = self.last_tick.unwrap_or(now);
        let delta = now.saturating_duration_since(last);
        self.last_tick = Some(now);
        self.remaining = self.remaining.saturating_sub(delta);
        if self.remaining.is_zero() {
            self.status = TimerStatus::Finished;
            self.last_tick = None;
            return true;
        }
        false
    }

    pub fn display(&self) -> String {
        match self.status {
            TimerStatus::Idle => format_duration(self.total),
            TimerStatus::Running | TimerStatus::Paused | TimerStatus::Finished => format!(
                "{} - {}",
                format_duration(self.total),
                format_duration(self.remaining)
            ),
        }
    }

    pub fn progress(&self) -> f64 {
        if self.total.is_zero() {
            return 1.0;
        }
        1.0 - (self.remaining.as_secs_f64() / self.total.as_secs_f64()).clamp(0.0, 1.0)
    }

    pub fn to_storage(&self) -> String {
        format!("{} | {}\n", self.name, format_duration(self.total))
    }
}

pub struct TimersState {
    pub timers: Vec<Timer>,
    pub selected: usize,
}

impl TimersState {
    pub fn load(path: &Path) -> Self {
        let text = storage::read_text(path).unwrap_or_default();
        let mut timers = Vec::new();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((name, duration)) = line.split_once('|') {
                if let Ok(total) = parse_duration(duration.trim()) {
                    timers.push(Timer::new(name.trim(), total));
                }
            }
        }
        Self {
            timers,
            selected: 0,
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let mut text = String::from("# TUION timers\n# name | hh.mm.ss\n\n");
        for timer in &self.timers {
            text.push_str(&timer.to_storage());
        }
        storage::write_text(path, &text)
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.timers.is_empty() {
            self.selected = 0;
            return;
        }
        self.selected =
            (self.selected as isize + delta).clamp(0, self.timers.len() as isize - 1) as usize;
    }

    pub fn selected_timer(&self) -> Option<&Timer> {
        self.timers.get(self.selected)
    }

    pub fn selected_timer_mut(&mut self) -> Option<&mut Timer> {
        self.timers.get_mut(self.selected)
    }

    pub fn delete_selected(&mut self) {
        if self.selected < self.timers.len() {
            self.timers.remove(self.selected);
            self.selected = self.selected.min(self.timers.len().saturating_sub(1));
        }
    }

    pub fn update(&mut self, index: Option<usize>, timer: Timer) {
        match index {
            Some(i) if i < self.timers.len() => self.timers[i] = timer,
            _ => {
                self.timers.push(timer);
                self.selected = self.timers.len() - 1;
            }
        }
    }

    pub fn tick(&mut self, now: Instant) -> Vec<String> {
        let mut finished = Vec::new();
        for timer in &mut self.timers {
            if timer.advance(now) {
                finished.push(timer.name.clone());
            }
        }
        finished
    }
}

pub fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    format!(
        "{:02}.{:02}.{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

pub fn parse_duration(input: &str) -> Result<Duration, String> {
    let raw = input.trim().to_ascii_lowercase();
    if raw.is_empty() {
        return Err("empty duration".into());
    }
    if raw.contains(':') {
        return parse_hms(&raw.replace(':', "."));
    }
    if raw.contains('h') || raw.contains('m') || raw.contains('s') {
        let mut total = 0u64;
        let mut number = String::new();
        for ch in raw.chars() {
            if ch.is_ascii_digit() || ch == '.' {
                number.push(ch);
            } else if matches!(ch, 'h' | 'm' | 's') {
                let value: f64 = number
                    .parse()
                    .map_err(|_| format!("bad duration component: {number}"))?;
                total += match ch {
                    'h' => (value * 3600.0) as u64,
                    'm' => (value * 60.0) as u64,
                    's' => value as u64,
                    _ => 0,
                };
                number.clear();
            } else if ch.is_whitespace() {
                continue;
            } else {
                return Err(format!("bad duration: {input}"));
            }
        }
        if !number.is_empty() {
            return Err(format!("duration component without unit: {number}"));
        }
        return Ok(Duration::from_secs(total));
    }
    if raw.contains('.') {
        return parse_hms(&raw);
    }
    let minutes: u64 = raw.parse().map_err(|_| format!("bad duration: {input}"))?;
    Ok(Duration::from_secs(minutes * 60))
}

fn parse_hms(raw: &str) -> Result<Duration, String> {
    let parts: Vec<&str> = raw.split('.').collect();
    if parts.len() > 3 {
        return Err(format!("bad duration: {raw}"));
    }
    let mut secs = 0u64;
    for (i, part) in parts.iter().enumerate() {
        let value: u64 = part
            .parse()
            .map_err(|_| format!("bad duration component: {part}"))?;
        let weight = match parts.len() {
            3 => [3600, 60, 1][i],
            2 => [60, 1][i],
            _ => 1,
        };
        secs += value * weight;
    }
    Ok(Duration::from_secs(secs))
}

pub struct TimerField {
    pub label: &'static str,
    pub hint: &'static str,
    pub value: LineInput,
}

pub struct TimerForm {
    pub index: Option<usize>,
    pub fields: Vec<TimerField>,
    pub active: usize,
    pub error: String,
}

pub const TIMER_FIELDS: [(&str, &str); 2] =
    [("name", "Focus"), ("duration", "hh.mm.ss, 25m, 1h30m")];

impl TimerForm {
    pub fn new(timer: Option<&Timer>, index: Option<usize>) -> Self {
        let values = if let Some(timer) = timer {
            vec![timer.name.clone(), format_duration(timer.total)]
        } else {
            vec![String::new(), "00.25.00".into()]
        };
        let fields = TIMER_FIELDS
            .iter()
            .zip(values)
            .map(|((label, hint), value)| TimerField {
                label,
                hint,
                value: LineInput::new(value),
            })
            .collect();
        Self {
            index,
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

    pub fn to_timer(&self) -> Result<Timer, String> {
        let name = self.fields[0].value.value.trim();
        if name.is_empty() {
            return Err("name is required".into());
        }
        let total = parse_duration(&self.fields[1].value.value)?;
        if total.is_zero() {
            return Err("duration must be greater than zero".into());
        }
        Ok(Timer::new(name, total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_parse() {
        assert_eq!(
            parse_duration("00.05.00").unwrap(),
            Duration::from_secs(300)
        );
        assert_eq!(parse_duration("05.30").unwrap(), Duration::from_secs(330));
        assert_eq!(parse_duration("25m").unwrap(), Duration::from_secs(1500));
        assert_eq!(parse_duration("1h30m").unwrap(), Duration::from_secs(5400));
        assert_eq!(parse_duration("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse_duration("25").unwrap(), Duration::from_secs(1500));
    }

    #[test]
    fn timer_runs_and_finishes() {
        let mut timer = Timer::new("x", Duration::from_secs(10));
        let start = Instant::now();
        timer.start(start);
        assert!(!timer.advance(start + Duration::from_secs(5)));
        assert_eq!(timer.remaining, Duration::from_secs(5));
        assert!(timer.advance(start + Duration::from_secs(11)));
        assert_eq!(timer.status, TimerStatus::Finished);
    }

    #[test]
    fn pause_and_resume() {
        let mut timer = Timer::new("x", Duration::from_secs(10));
        let start = Instant::now();
        timer.start(start);
        timer.pause(start + Duration::from_secs(4));
        assert_eq!(timer.remaining, Duration::from_secs(6));
        assert_eq!(timer.status, TimerStatus::Paused);
        timer.start(start + Duration::from_secs(20));
        assert_eq!(timer.status, TimerStatus::Running);
        assert!(!timer.advance(start + Duration::from_secs(21)));
    }
}
