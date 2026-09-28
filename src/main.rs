use bevy::prelude::*;
use bevy_tui_texture::prelude::*;

fn main() {
  App::new()
    .add_plugins(DefaultPlugins)
    .add_plugins(TerminalPlugins::default())
    .run();
}
