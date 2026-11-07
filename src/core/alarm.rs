//! Alarms and the human-readable `state/alarms.txt` format from the concept.

use crate::core::storage;
use crate::core::text::LineInput;
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Weekday};
use std::io;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Alarm {
    pub name: String,
    pub time: NaiveTime,
    pub days: Vec<Weekday>,
    pub date: Option<NaiveDate>,
    pub enabled: bool,
    pub description: Option<String>,
    pub last_triggered: Option<NaiveDate>,
}

impl Alarm {
    pub fn is_event(&self) -> bool {
        self.description.is_some() || self.date.is_some()
    }

    pub fn days_label(&self) -> String {
        if let Some(date) = self.date {
            return date.format("%Y.%m.%d").to_string();
        }
        if self.days.is_empty() {
            return "every day".into();
        }
        self.days
            .iter()
            .map(|d| weekday_short(*d))
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn time_label(&self) -> String {
        self.time.format("%H.%M").to_string()
    }

    pub fn matches(&self, now: NaiveDateTime) -> bool {
        if !self.enabled {
            return false;
        }
        if now.hour() != self.time.hour() || now.minute() != self.time.minute() {
            return false;
        }
        if let Some(date) = self.date {
            return now.date() == date;
        }
        if self.days.is_empty() {
            return true;
        }
        self.days.contains(&now.weekday())
    }

    pub fn to_storage(&self) -> String {
        let mut out = format!("{}\n", self.name);
        let mut days = self.days_label();
        if days == "every day" {
            days = String::new();
        }
        out.push_str(&format!("- {}-{}\n", self.time_label(), days));
        if let Some(desc) = &self.description {
            out.push_str(&format!("> {desc}\n"));
        }
        if !self.enabled {
            out.push_str("- false\n");
        }
        out
    }
}

pub struct AlarmState {
    pub alarms: Vec<Alarm>,
    pub selected: usize,
    pub ringing: Option<usize>,
}

impl AlarmState {
    pub fn load(path: &Path) -> Self {
        let text = storage::read_text(path).unwrap_or_default();
        Self {
            alarms: parse_alarms(&text),
            selected: 0,
            ringing: None,
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let mut text = String::from(
            "# TUION alarms\n# Name\n# - 07.00-mon,tue,wed,thu,fri\n# > optional description\n# - false (disabled)\n\n",
        );
        for alarm in &self.alarms {
            text.push_str(&alarm.to_storage());
            text.push('\n');
        }
        storage::write_text(path, &text)
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.alarms.is_empty() {
            self.selected = 0;
            return;
        }
        self.selected =
            (self.selected as isize + delta).clamp(0, self.alarms.len() as isize - 1) as usize;
    }

    pub fn selected_alarm(&self) -> Option<&Alarm> {
        self.alarms.get(self.selected)
    }

    pub fn toggle_selected(&mut self) {
        if let Some(alarm) = self.alarms.get_mut(self.selected) {
            alarm.enabled = !alarm.enabled;
            if !alarm.enabled {
                alarm.last_triggered = None;
            }
        }
    }

    pub fn delete_selected(&mut self) {
        if self.selected < self.alarms.len() {
            self.alarms.remove(self.selected);
            self.selected = self.selected.min(self.alarms.len().saturating_sub(1));
        }
    }

    /// Returns the index of an alarm that should ring now, if any.
    pub fn check(&mut self, now: NaiveDateTime) -> Option<usize> {
        if self.ringing.is_some() {
            return None;
        }
        for (i, alarm) in self.alarms.iter_mut().enumerate() {
            if alarm.matches(now) && alarm.last_triggered != Some(now.date()) {
                alarm.last_triggered = Some(now.date());
                self.ringing = Some(i);
                return Some(i);
            }
        }
        None
    }

    pub fn dismiss(&mut self) {
        self.ringing = None;
    }

    pub fn update(&mut self, index: Option<usize>, alarm: Alarm) {
        match index {
            Some(i) if i < self.alarms.len() => self.alarms[i] = alarm,
            _ => {
                self.alarms.push(alarm);
                self.selected = self.alarms.len() - 1;
            }
        }
    }
}

pub fn parse_alarms(text: &str) -> Vec<Alarm> {
    let mut alarms: Vec<Alarm> = Vec::new();
    let mut current: Option<Alarm> = None;
    for raw in text.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('-') {
            let Some(alarm) = current.as_mut() else {
                continue;
            };
            let rest = rest.trim();
            if matches!(
                rest.to_ascii_lowercase().as_str(),
                "false" | "off" | "disabled"
            ) {
                alarm.enabled = false;
                continue;
            }
            if let Some((time_s, days_s)) = rest.split_once('-') {
                if let Ok(time) = NaiveTime::parse_from_str(time_s.trim(), "%H.%M") {
                    alarm.time = time;
                }
                let (days, date) = parse_days(days_s);
                alarm.days = days;
                alarm.date = date;
            } else if let Ok(time) = NaiveTime::parse_from_str(rest, "%H.%M") {
                alarm.time = time;
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('>') {
            if let Some(alarm) = current.as_mut() {
                alarm.description = Some(rest.trim().to_string());
            }
            continue;
        }
        if let Some(alarm) = current.take() {
            alarms.push(alarm);
        }
        current = Some(Alarm {
            name: trimmed.to_string(),
            time: NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
            days: Vec::new(),
            date: None,
            enabled: true,
            description: None,
            last_triggered: None,
        });
    }
    if let Some(alarm) = current {
        alarms.push(alarm);
    }
    alarms
}

fn parse_days(input: &str) -> (Vec<Weekday>, Option<NaiveDate>) {
    let mut days = Vec::new();
    let mut date = None;
    for token in input.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if let Some(wd) = parse_weekday(token) {
            days.push(wd);
            continue;
        }
        if token.eq_ignore_ascii_case("today") {
            date = Some(Local::now().date_naive());
            continue;
        }
        if token.eq_ignore_ascii_case("tomorrow") {
            date = Some(Local::now().date_naive() + chrono::Duration::days(1));
            continue;
        }
        if let Ok(d) = NaiveDate::parse_from_str(token, "%Y.%m.%d") {
            date = Some(d);
        }
    }
    (days, date)
}

fn parse_weekday(token: &str) -> Option<Weekday> {
    match token.to_ascii_lowercase().as_str() {
        "mon" | "monday" | "1" => Some(Weekday::Mon),
        "tue" | "tuesday" | "2" => Some(Weekday::Tue),
        "wed" | "wednesday" | "3" => Some(Weekday::Wed),
        "thu" | "thursday" | "4" => Some(Weekday::Thu),
        "fri" | "friday" | "5" => Some(Weekday::Fri),
        "sat" | "saturday" | "6" => Some(Weekday::Sat),
        "sun" | "sunday" | "7" => Some(Weekday::Sun),
        _ => None,
    }
}

fn weekday_short(day: Weekday) -> &'static str {
    match day {
        Weekday::Mon => "mon",
        Weekday::Tue => "tue",
        Weekday::Wed => "wed",
        Weekday::Thu => "thu",
        Weekday::Fri => "fri",
        Weekday::Sat => "sat",
        Weekday::Sun => "sun",
    }
}

pub struct AlarmField {
    pub label: &'static str,
    pub hint: &'static str,
    pub value: LineInput,
}

pub struct AlarmForm {
    pub index: Option<usize>,
    pub fields: Vec<AlarmField>,
    pub active: usize,
    pub error: String,
}

pub const ALARM_FIELDS: [(&str, &str); 6] = [
    ("name", "Good morning"),
    ("time", "HH.MM"),
    ("days", "mon,tue,wed,thu,fri"),
    ("date", "yyyy.mm.dd, today or tomorrow"),
    ("description", "event text (optional)"),
    ("enabled", "true / false"),
];

impl AlarmForm {
    pub fn new(alarm: Option<&Alarm>) -> Self {
        let values = if let Some(alarm) = alarm {
            vec![
                alarm.name.clone(),
                alarm.time_label(),
                if alarm.days.is_empty() {
                    String::new()
                } else {
                    alarm
                        .days
                        .iter()
                        .map(|d| weekday_short(*d))
                        .collect::<Vec<_>>()
                        .join(",")
                },
                alarm
                    .date
                    .map(|d| d.format("%Y.%m.%d").to_string())
                    .unwrap_or_default(),
                alarm.description.clone().unwrap_or_default(),
                if alarm.enabled { "true" } else { "false" }.to_string(),
            ]
        } else {
            vec![
                String::new(),
                "07.00".into(),
                "mon,tue,wed,thu,fri".into(),
                String::new(),
                String::new(),
                "true".into(),
            ]
        };
        let fields = ALARM_FIELDS
            .iter()
            .zip(values)
            .map(|((label, hint), value)| AlarmField {
                label,
                hint,
                value: LineInput::new(value),
            })
            .collect();
        Self {
            index: None,
            fields,
            active: 0,
            error: String::new(),
        }
    }

    pub fn with_index(mut self, index: Option<usize>) -> Self {
        self.index = index;
        self
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

    pub fn to_alarm(&self) -> Result<Alarm, String> {
        let name = self.value(0).trim();
        if name.is_empty() {
            return Err("name is required".into());
        }
        let time_s = self.value(1).trim();
        let time = NaiveTime::parse_from_str(time_s, "%H.%M")
            .map_err(|_| format!("bad time: {time_s}"))?;
        let (days, mut date) = parse_days(self.value(2));
        let date_s = self.value(3).trim();
        if !date_s.is_empty() {
            date = Some(
                NaiveDate::parse_from_str(date_s, "%Y.%m.%d")
                    .map_err(|_| format!("bad date: {date_s}"))?,
            );
        }
        let description = {
            let d = self.value(4).trim();
            if d.is_empty() {
                None
            } else {
                Some(d.to_string())
            }
        };
        let enabled = !matches!(
            self.value(5).trim().to_ascii_lowercase().as_str(),
            "false" | "off" | "no" | "0"
        );
        Ok(Alarm {
            name: name.to_string(),
            time,
            days,
            date,
            enabled,
            description,
            last_triggered: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_alarm_block() {
        let text = r#"
Good morning
- 07.00-mon,tue,wed,thu,fri

Event
- 18.00-2026.10.14
> don't forget the present!
"#;
        let alarms = parse_alarms(text);
        assert_eq!(alarms.len(), 2);
        assert_eq!(alarms[0].name, "Good morning");
        assert_eq!(alarms[0].time.format("%H.%M").to_string(), "07.00");
        assert_eq!(alarms[0].days.len(), 5);
        assert_eq!(
            alarms[1].date.unwrap().format("%Y.%m.%d").to_string(),
            "2026.10.14"
        );
        assert!(alarms[1].is_event());
    }

    #[test]
    fn disabled_roundtrip() {
        let mut alarm = Alarm {
            name: "x".into(),
            time: NaiveTime::from_hms_opt(8, 30, 0).unwrap(),
            days: vec![Weekday::Sat, Weekday::Sun],
            date: None,
            enabled: false,
            description: None,
            last_triggered: None,
        };
        let text = alarm.to_storage();
        let parsed = parse_alarms(&text);
        assert_eq!(parsed.len(), 1);
        assert!(!parsed[0].enabled);
        assert_eq!(parsed[0].days, vec![Weekday::Sat, Weekday::Sun]);
        alarm.enabled = true;
        assert!(parse_alarms(&alarm.to_storage())[0].enabled);
    }

    #[test]
    fn matches_days_and_date() {
        let alarm = Alarm {
            name: "x".into(),
            time: NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
            days: vec![Weekday::Mon],
            date: None,
            enabled: true,
            description: None,
            last_triggered: None,
        };
        let monday = NaiveDate::from_ymd_opt(2026, 10, 26).unwrap();
        assert!(alarm.matches(monday.and_hms_opt(7, 0, 0).unwrap()));
        assert!(!alarm.matches(monday.and_hms_opt(7, 1, 0).unwrap()));
        assert!(
            !alarm.matches(
                (monday + chrono::Duration::days(1))
                    .and_hms_opt(7, 0, 0)
                    .unwrap()
            )
        );
    }
}
