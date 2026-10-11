use crate::core::HelloTerminal;
use bevy::prelude::*;
use bevy_tui_texture::{Font, Fonts, TerminalConfig, TuiRequest};
use std::sync::Arc;

const FONT_DATA: &[u8] =
    include_bytes!("../../../assets/fonts/jetbrains-mono-nerdfont-regular.ttf");

pub fn setup(mut commands: Commands) {
    let fonts = Arc::new(Fonts::new(
        Font::new(FONT_DATA).expect("Failed to parse font"),
        32,
    ));

    commands.spawn((
        TuiRequest::ui(80, 25, fonts).with_config(TerminalConfig {
            keyboard: false,
            mouse: false,
            ..default()
        }),
        Node::default(),
        HelloTerminal,
    ));
    commands.spawn(Camera2d);
}
