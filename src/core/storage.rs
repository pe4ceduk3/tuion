//! On-disk layout and small file helpers.
//!
//! Everything is local, plain text, and easy to inspect:
//!
//! ```text
//! ~/.local/share/tuion/
//!   config.toml
//!   notes/            <- the file manager starts here
//!   state/calendar.md
//!   state/alarms.txt
//!   state/timers.txt
//! ```
//!
//! `TUION_HOME` overrides the location (useful for tests and for running
//! several independent notebooks).

use crate::core::config::DEFAULT_CONFIG;
use chrono::{Local, NaiveDate, NaiveTime, Timelike};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Layout {
    pub root: PathBuf,
    pub notes: PathBuf,
    pub state: PathBuf,
    pub config: PathBuf,
}

impl Layout {
    pub fn discover() -> io::Result<Self> {
        let root = if let Ok(path) = std::env::var("TUION_HOME") {
            PathBuf::from(path)
        } else if let Some(data) = dirs::data_dir() {
            data.join("tuion")
        } else {
            PathBuf::from(".tuion")
        };
        Self::at(root)
    }

    pub fn at(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        let notes = root.join("notes");
        let state = root.join("state");
        fs::create_dir_all(&notes)?;
        fs::create_dir_all(&state)?;
        let layout = Self {
            config: root.join("config.toml"),
            root,
            notes,
            state,
        };
        layout.seed()?;
        Ok(layout)
    }

    pub fn calendar_path(&self) -> PathBuf {
        self.state.join("calendar.md")
    }

    pub fn alarms_path(&self) -> PathBuf {
        self.state.join("alarms.txt")
    }

    pub fn timers_path(&self) -> PathBuf {
        self.state.join("timers.txt")
    }

    fn seed(&self) -> io::Result<()> {
        write_if_missing(&self.config, DEFAULT_CONFIG)?;
        write_if_missing(&self.notes.join("welcome.md"), WELCOME)?;
        write_if_missing(&self.notes.join("ideas.md"), IDEAS)?;

        let today = Local::now().date_naive();
        write_if_missing(&self.calendar_path(), &seed_calendar(today))?;
        write_if_missing(&self.alarms_path(), SEED_ALARMS)?;
        write_if_missing(&self.timers_path(), SEED_TIMERS)?;
        Ok(())
    }
}

fn write_if_missing(path: &Path, text: &str) -> io::Result<()> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_text(path, text)?;
    }
    Ok(())
}

pub fn read_text(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}

/// Write via a sibling temporary file, then rename. This keeps a crash from
/// leaving a half-written note behind.
pub fn write_text(path: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}tmp",
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| format!("{e}."))
            .unwrap_or_default()
    ));
    fs::write(&tmp, text)?;
    if path.exists() {
        // `rename` replaces on Unix; on Android too. Keep the fallback for
        // exotic filesystems.
        if let Err(first) = fs::rename(&tmp, path) {
            fs::remove_file(path).ok();
            fs::rename(&tmp, path).map_err(|_| first)?;
        }
    } else {
        fs::rename(&tmp, path)?;
    }
    Ok(())
}

/// A path that does not exist yet, with a ` copy`/` copy 2` suffix if needed.
pub fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    let ext = path.extension().and_then(|s| s.to_str());
    for n in 1..10_000 {
        let suffix = if n == 1 {
            " copy".to_string()
        } else {
            format!(" copy {n}")
        };
        let name = match ext {
            Some(ext) => format!("{stem}{suffix}.{ext}"),
            None => format!("{stem}{suffix}"),
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
}

fn seed_calendar(today: NaiveDate) -> String {
    let d = today.format("%Y.%m.%d");
    let tomorrow = today.succ_opt().unwrap_or(today).format("%Y.%m.%d");
    format!(
        r#"# TUION calendar
# Blocks are human-readable:
#   - [ ] name
#   :start (yyyy.mm.dd-hh.mm)
#   -deadline
#   !10            (10 minutes before start) or !2026.10.14-13.50
#   #b.blue        (b = bright, d = normal)
#   &d1            (repeat: dN every N days, wN weekday 1-7, mN month 1-12)
#   > description

- [x] Morning planning
:{d}-09.00
-{d}-10.00
#d.green
> Review the day and pick three priorities.

- [ ] Созвон с командой
:{d}-14.00
-{d}-15.00
!10
#b.blue
> Обсудить спринт и блокеры.

- [ ] Привычка
:{d}-18.00
-{d}-18.30
#d.magenta
&d1
> Каждый день, без исключений.

- [ ] Забрать посылку
:{tomorrow}-12.00
-{tomorrow}-13.00
!30
#b.yellow
"#
    )
}

const WELCOME: &str = r#"# Welcome to TUION

TUION is a local-first notebook: plain `.md` files, an outliner, a
calendar, alarms and timers, all in one terminal texture.

## Start here

- Press `Enter` on a note to open it.
- In the editor, `Esc` goes back to the file manager.
- `F1` alarm clock, `F2` timer, `F3` calendar, `F4` tasks.
- `,` opens the commented config file.
- `?` shows the keys for the current screen.

## Outliner basics

- `Tab` / `Shift+Tab` change list nesting.
- `Enter` keeps the current list marker and indentation.
- `F2` folds a heading or a list item.
- `Ctrl+↑` zooms into the current item, `Ctrl+↓` zooms out.
- `Ctrl+Space` marks a line for multi-line copy, cut and move.
- `Ctrl+Enter` opens the wiki link under the cursor.
- `[[ideas]]` is a link to `ideas.md`.

> Everything is a text file. You can edit it with any editor.
"#;

const IDEAS: &str = r#"# Ideas

- [ ] Try the calendar
  - recurring habits use `&d1`
  - tasks are stored in `state/calendar.md`
- [ ] Try the timer
- [ ] Change the icons in `config.toml`

## Markdown

**bold** *italic* ~~strike~~ ==highlight== `inline code`

- a list
  - nested
    - deeper

[[welcome]] links back home.
"#;

const SEED_ALARMS: &str = r#"# TUION alarms
# Name
# - 07.00-mon,tue,wed,thu,fri
# - 09.00-sat,sun
# > optional event description
# - false            (disabled)

Good morning
- 07.00-mon,tue,wed,thu,fri

Good weekend's morning
- 09.00-sat,sun

Event
- 18.00-2026.10.14
> don't forget to buy a present for your daughter!
"#;

const SEED_TIMERS: &str = r#"# TUION timers
# name | hh.mm.ss
Focus | 00.25.00
Tea | 00.05.00
"#;

/// Helper used by the seed data and tests: format a time as `HH.MM`.
pub fn format_clock(time: NaiveTime) -> String {
    format!("{:02}.{:02}", time.hour(), time.minute())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_path_adds_copy_suffix() {
        let dir = std::env::temp_dir().join(format!("tuion-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.md");
        fs::write(&p, "x").unwrap();
        let q = unique_path(&p);
        assert_eq!(q.file_name().unwrap(), "a copy.md");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn atomic_write_replaces() {
        let dir = std::env::temp_dir().join(format!("tuion-test-w-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.txt");
        write_text(&p, "one").unwrap();
        write_text(&p, "two").unwrap();
        assert_eq!(read_text(&p).unwrap(), "two");
        fs::remove_dir_all(&dir).unwrap();
    }
}
