//! Text-file configuration for TUION.
//!
//! The configuration is deliberately a plain text file with comments and
//! examples, exactly like the concept asks for. The in-app config editor
//! edits this text directly; `AppConfig::parse` only understands a tiny
//! `[section]` / `key = value` subset of TOML, which keeps the file pleasant
//! to hand-edit while avoiding a full TOML dependency.

use ratatui::style::Color;

/// Default config written on first launch. It is also the text shown in the
/// in-app config editor, so the comments double as documentation.
pub const DEFAULT_CONFIG: &str = r########"# TUION configuration
# Everything here is optional. Comment a line out to use its default.
# The file is edited directly from the app: press `,` in the file manager.

[icons]
# Any Nerd Font glyph (or ordinary text) works.
folder = "󰉋"
file = "󰈙"
note = "󰈙"
link = "󰌷"
alarm = "󰀠"
timer = "󰔚"
calendar = "󰃭"
tasks = "󰄱"
check_done = "󰄲"
check_todo = "󰄱"
fold_closed = "󰅂"
fold_open = "󰅀"
play = "󰐊"
pause = "󰏤"
stop = "󰓛"
config = "󰒓"
back = "󰁍"
bright = "󰃠"
color = "󰏘"
repeat = "󰑖"
bell = "󰂚"
description = "󰦨"
name = "󰓹"
clock = "󰔟"

[markdown]
# Change these to customise the markdown dialect. The parser uses them for
# both reading and preview rendering.
# Examples:
#   h1 = "#1"          -> "#1 Heading"
#   italic = "/"       -> "/italic/"
#   link_open = "(("   -> "((wiki link))"
h1 = "#"
h2 = "##"
h3 = "###"
h4 = "####"
h5 = "#####"
h6 = "######"
bold = "**"
italic = "*"
strike = "~~"
highlight = "=="
code = "`"
link_open = "[["
link_close = "]]"
task_open = "[ ]"
task_done = "[x]"

[theme]
# Colours accept #rrggbb or a ratatui colour name (black, red, green,
# yellow, blue, magenta, cyan, gray, darkgray, white).
bg = "#11131a"
fg = "#d7dae0"
dim = "#6b7280"
accent = "#7f9cf5"
border = "#3b4261"
selection_bg = "#2e3a59"
marked_bg = "#3d3359"
h1 = "#f7768e"
h2 = "#7aa2f7"
h3 = "#7dcfff"
h4 = "#9ece6a"
h5 = "#e0af68"
h6 = "#bb9af7"
code_bg = "#1f2335"
code_fg = "#a9b1d6"
success = "#9ece6a"
warning = "#e0af68"
error = "#f7768e"
"########;

#[derive(Clone, Debug)]
pub struct Icons {
    pub folder: String,
    pub file: String,
    pub note: String,
    pub link: String,
    pub alarm: String,
    pub timer: String,
    pub calendar: String,
    pub tasks: String,
    pub check_done: String,
    pub check_todo: String,
    pub fold_closed: String,
    pub fold_open: String,
    pub play: String,
    pub pause: String,
    pub stop: String,
    pub config: String,
    pub back: String,
    pub bright: String,
    pub color: String,
    pub repeat: String,
    pub bell: String,
    pub description: String,
    pub name: String,
    pub clock: String,
}

impl Default for Icons {
    fn default() -> Self {
        Self {
            folder: "󰉋".into(),
            file: "󰈙".into(),
            note: "󰈙".into(),
            link: "󰌷".into(),
            alarm: "󰀠".into(),
            timer: "󰔚".into(),
            calendar: "󰃭".into(),
            tasks: "󰄱".into(),
            check_done: "󰄲".into(),
            check_todo: "󰄱".into(),
            fold_closed: "󰅂".into(),
            fold_open: "󰅀".into(),
            play: "󰐊".into(),
            pause: "󰏤".into(),
            stop: "󰓛".into(),
            config: "󰒓".into(),
            back: "󰁍".into(),
            bright: "󰃠".into(),
            color: "󰏘".into(),
            repeat: "󰑖".into(),
            bell: "󰂚".into(),
            description: "󰦨".into(),
            name: "󰓹".into(),
            clock: "󰔟".into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MarkdownSyntax {
    pub h: [String; 6],
    pub bold: String,
    pub italic: String,
    pub strike: String,
    pub highlight: String,
    pub code: String,
    pub link_open: String,
    pub link_close: String,
    pub task_open: String,
    pub task_done: String,
}

impl Default for MarkdownSyntax {
    fn default() -> Self {
        Self {
            h: [
                "#".into(),
                "##".into(),
                "###".into(),
                "####".into(),
                "#####".into(),
                "######".into(),
            ],
            bold: "**".into(),
            italic: "*".into(),
            strike: "~~".into(),
            highlight: "==".into(),
            code: "`".into(),
            link_open: "[[".into(),
            link_close: "]]".into(),
            task_open: "[ ]".into(),
            task_done: "[x]".into(),
        }
    }
}

impl MarkdownSyntax {
    /// The six heading markers, longest first, for line classification.
    pub fn headings_longest_first(&self) -> Vec<(usize, &str)> {
        let mut v: Vec<(usize, &str)> = self
            .h
            .iter()
            .enumerate()
            .map(|(i, s)| (i + 1, s.as_str()))
            .collect();
        v.sort_by_key(|(_, s)| std::cmp::Reverse(s.chars().count()));
        v
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub dim: Color,
    pub accent: Color,
    pub border: Color,
    pub selection_bg: Color,
    pub marked_bg: Color,
    pub h1: Color,
    pub h2: Color,
    pub h3: Color,
    pub h4: Color,
    pub h5: Color,
    pub h6: Color,
    pub code_bg: Color,
    pub code_fg: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Rgb(0x11, 0x13, 0x1a),
            fg: Color::Rgb(0xd7, 0xda, 0xe0),
            dim: Color::Rgb(0x6b, 0x72, 0x80),
            accent: Color::Rgb(0x7f, 0x9c, 0xf5),
            border: Color::Rgb(0x3b, 0x42, 0x61),
            selection_bg: Color::Rgb(0x2e, 0x3a, 0x59),
            marked_bg: Color::Rgb(0x3d, 0x33, 0x59),
            h1: Color::Rgb(0xf7, 0x76, 0x8e),
            h2: Color::Rgb(0x7a, 0xa2, 0xf7),
            h3: Color::Rgb(0x7d, 0xcf, 0xff),
            h4: Color::Rgb(0x9e, 0xce, 0x6a),
            h5: Color::Rgb(0xe0, 0xaf, 0x68),
            h6: Color::Rgb(0xbb, 0x9a, 0xf7),
            code_bg: Color::Rgb(0x1f, 0x23, 0x35),
            code_fg: Color::Rgb(0xa9, 0xb1, 0xd6),
            success: Color::Rgb(0x9e, 0xce, 0x6a),
            warning: Color::Rgb(0xe0, 0xaf, 0x68),
            error: Color::Rgb(0xf7, 0x76, 0x8e),
        }
    }
}

impl Theme {
    pub fn heading(&self, level: usize) -> Color {
        match level {
            1 => self.h1,
            2 => self.h2,
            3 => self.h3,
            4 => self.h4,
            5 => self.h5,
            _ => self.h6,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AppConfig {
    pub icons: Icons,
    pub markdown: MarkdownSyntax,
    pub theme: Theme,
    /// The exact text loaded from disk, shown in the config editor.
    pub raw: String,
}

impl AppConfig {
    pub fn new() -> Self {
        Self {
            icons: Icons::default(),
            markdown: MarkdownSyntax::default(),
            theme: Theme::default(),
            raw: DEFAULT_CONFIG.to_string(),
        }
    }

    /// Parse the text form. Unknown keys are ignored; malformed values are
    /// reported so the editor can show a useful error instead of silently
    /// dropping a user's change.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut cfg = Self::new();
        cfg.raw = raw.to_string();
        let mut section = String::new();
        for (lineno, original) in raw.lines().enumerate() {
            let line = original.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim().to_ascii_lowercase();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!("line {}: expected `key = value`", lineno + 1));
            };
            let key = key.trim().to_ascii_lowercase();
            let value = unquote(value.trim());
            let ctx = || format!("line {}: bad value for `{key}`", lineno + 1);
            match section.as_str() {
                "icons" => set_icon(&mut cfg.icons, &key, value),
                "markdown" => set_markdown(&mut cfg.markdown, &key, value),
                "theme" => {
                    let color = parse_color(&value).ok_or_else(ctx)?;
                    match key.as_str() {
                        "bg" => cfg.theme.bg = color,
                        "fg" => cfg.theme.fg = color,
                        "dim" => cfg.theme.dim = color,
                        "accent" => cfg.theme.accent = color,
                        "border" => cfg.theme.border = color,
                        "selection_bg" => cfg.theme.selection_bg = color,
                        "marked_bg" => cfg.theme.marked_bg = color,
                        "h1" => cfg.theme.h1 = color,
                        "h2" => cfg.theme.h2 = color,
                        "h3" => cfg.theme.h3 = color,
                        "h4" => cfg.theme.h4 = color,
                        "h5" => cfg.theme.h5 = color,
                        "h6" => cfg.theme.h6 = color,
                        "code_bg" => cfg.theme.code_bg = color,
                        "code_fg" => cfg.theme.code_fg = color,
                        "success" => cfg.theme.success = color,
                        "warning" => cfg.theme.warning = color,
                        "error" => cfg.theme.error = color,
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        Ok(cfg)
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }
}

fn set_icon(icons: &mut Icons, key: &str, value: String) {
    match key {
        "folder" => icons.folder = value,
        "file" => icons.file = value,
        "note" => icons.note = value,
        "link" => icons.link = value,
        "alarm" => icons.alarm = value,
        "timer" => icons.timer = value,
        "calendar" => icons.calendar = value,
        "tasks" => icons.tasks = value,
        "check_done" => icons.check_done = value,
        "check_todo" => icons.check_todo = value,
        "fold_closed" => icons.fold_closed = value,
        "fold_open" => icons.fold_open = value,
        "play" => icons.play = value,
        "pause" => icons.pause = value,
        "stop" => icons.stop = value,
        "config" => icons.config = value,
        "back" => icons.back = value,
        "bright" => icons.bright = value,
        "color" => icons.color = value,
        "repeat" => icons.repeat = value,
        "bell" => icons.bell = value,
        "description" => icons.description = value,
        "name" => icons.name = value,
        "clock" => icons.clock = value,
        _ => {}
    }
}

fn set_markdown(md: &mut MarkdownSyntax, key: &str, value: String) {
    match key {
        "h1" => md.h[0] = value,
        "h2" => md.h[1] = value,
        "h3" => md.h[2] = value,
        "h4" => md.h[3] = value,
        "h5" => md.h[4] = value,
        "h6" => md.h[5] = value,
        "bold" => md.bold = value,
        "italic" => md.italic = value,
        "strike" => md.strike = value,
        "highlight" => md.highlight = value,
        "code" => md.code = value,
        "link_open" => md.link_open = value,
        "link_close" => md.link_close = value,
        "task_open" => md.task_open = value,
        "task_done" => md.task_done = value,
        _ => {}
    }
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 {
        let bytes = s.as_bytes();
        if (bytes[0] == b'"' && bytes[s.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[s.len() - 1] == b'\'')
        {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim().trim_matches('"').trim_matches('\'');
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Color::Rgb(r, g, b));
        }
    }
    match s.to_ascii_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" => Some(Color::DarkGray),
        "lightred" => Some(Color::LightRed),
        "lightgreen" => Some(Color::LightGreen),
        "lightyellow" => Some(Color::LightYellow),
        "lightblue" => Some(Color::LightBlue),
        "lightmagenta" => Some(Color::LightMagenta),
        "lightcyan" => Some(Color::LightCyan),
        "white" => Some(Color::White),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let cfg = AppConfig::parse(DEFAULT_CONFIG).expect("default config must parse");
        assert_eq!(cfg.icons.folder, "󰉋");
        assert_eq!(cfg.markdown.h[1], "##");
        assert_eq!(cfg.theme.accent, Color::Rgb(0x7f, 0x9c, 0xf5));
    }

    #[test]
    fn custom_markdown_and_colours_parse() {
        let cfg = AppConfig::parse(
            r##"
[markdown]
h1 = "#1"
italic = "/"
link_open = "(("
link_close = "))"
[theme]
accent = "#010203"
"##,
        )
        .unwrap();
        assert_eq!(cfg.markdown.h[0], "#1");
        assert_eq!(cfg.markdown.italic, "/");
        assert_eq!(cfg.markdown.link_open, "((");
        assert_eq!(cfg.theme.accent, Color::Rgb(1, 2, 3));
    }

    #[test]
    fn bad_colour_is_an_error() {
        assert!(AppConfig::parse("[theme]\naccent = nope\n").is_err());
    }
}
