//! Bevy setup: one terminal entity filling the window.

use crate::core::app::AppState;
use bevy::prelude::*;
use bevy_tui_texture::Font as TerminalFont;
use bevy_tui_texture::prelude::*;
use std::sync::Arc;

const FONT_DATA: &[u8] = include_bytes!("../../assets/fonts/jetbrains-nerdfont.ttf");

/// Marker for the application's single terminal.
#[derive(Component)]
pub struct MainTerminal;

/// Font metrics shared with the resize handler.
#[derive(Resource, Clone)]
pub struct TerminalFonts(pub Arc<Fonts>);

pub fn setup(mut commands: Commands) {
    let fonts = Arc::new(Fonts::new(
        TerminalFont::new(FONT_DATA).expect("failed to parse bundled Nerd Font"),
        24,
    ));

    let terminal = commands
        .spawn((
            TuiRequest::ui(110, 40, fonts.clone()).with_config(TerminalConfig {
                keyboard: true,
                mouse: true,
                initial_fill: [0x11, 0x13, 0x1a, 0xff],
                ..default()
            }),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            MainTerminal,
        ))
        .id();

    commands.insert_resource(TerminalFocus {
        focused: Some(terminal),
    });
    commands.insert_resource(TerminalFonts(fonts));
    commands.insert_resource(AppState::load());
    commands.spawn(Camera2d);
}
