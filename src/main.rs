//! The application entry point and Bevy schedule.

mod core;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_tui_texture::prelude::*;
use core::setup::setup;
use core::systems::{exit_if_requested, handle_terminal_events, render_terminal, tick_app};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "TUION — local outliner".to_string(),
                resolution: WindowResolution::new(1600, 1000),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(TerminalPlugin::default())
        .insert_resource(ClearColor(Color::srgb(0.045, 0.05, 0.075)))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (tick_app, handle_terminal_events, exit_if_requested)
                .chain()
                .in_set(TerminalSystemSet::UserUpdate),
        )
        .add_systems(Update, render_terminal.in_set(TerminalSystemSet::Render))
        .run();
}
