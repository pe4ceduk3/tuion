//! Bevy systems: terminal input/resize, clock ticking, rendering and exit.

use crate::core::app::AppState;
use crate::core::setup::{MainTerminal, TerminalFonts};
use crate::core::ui;
use crate::core::ui::hit::HitId;
use bevy::prelude::*;
use bevy_tui_texture::input::{InputEvent, TerminalEvent};
use bevy_tui_texture::prelude::Tui;

/// Translate terminal events into app actions, and window resizes into a new
/// terminal grid.
pub fn handle_terminal_events(
    mut events: MessageReader<TerminalEvent>,
    fonts: Res<TerminalFonts>,
    mut terminals: Query<&mut Tui, With<MainTerminal>>,
    mut app: ResMut<AppState>,
) {
    let Ok(mut terminal) = terminals.single_mut() else {
        return;
    };
    for event in events.read() {
        if let InputEvent::Resize { pixels } = &event.input {
            let char_w = fonts.0.min_width_px().max(1);
            let char_h = fonts.0.height_px().max(1);
            let cols = (pixels.x / char_w).max(1).min(u16::MAX as u32) as u16;
            let rows = (pixels.y / char_h).max(1).min(u16::MAX as u32) as u16;
            terminal.request_resize(cols, rows);
            continue;
        }

        let hit = match &event.input {
            InputEvent::Mouse(mouse) => terminal
                .hit_regions()
                .hit_at::<HitId>((mouse.column, mouse.row)),
            _ => None,
        };
        app.handle_input(&event.input, hit);
    }
}

pub fn tick_app(mut app: ResMut<AppState>) {
    app.tick();
}

pub fn exit_if_requested(mut app: ResMut<AppState>, mut exit: MessageWriter<AppExit>) {
    if app.quit {
        app.quit = false;
        app.save_all();
        exit.write(AppExit::Success);
    }
}

pub fn render_terminal(
    mut terminals: Query<&mut Tui, With<MainTerminal>>,
    mut app: ResMut<AppState>,
) {
    let Ok(mut terminal) = terminals.single_mut() else {
        return;
    };
    let state = app.as_mut();
    terminal.draw_with_hits(|frame, hits| {
        ui::draw(frame, state, hits);
    });
}
