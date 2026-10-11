mod core;

use bevy::prelude::*;
use bevy_tui_texture::prelude::*;

use core::render_terminal::render_terminal;
use core::setup::setup;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TerminalPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, render_terminal.in_set(TerminalSystemSet::Render))
        .run();
}
