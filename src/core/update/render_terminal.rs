use bevy::prelude::*;
use bevy_tui_texture::prelude::*;
use ratatui::{prelude::*, style::Color as RatatuiColor, widgets::*};

use crate::core::HelloTerminal;

pub fn render_terminal(mut screens: Query<&mut Tui, With<HelloTerminal>>) {
    let Ok(mut term) = screens.single_mut() else {
        return;
    };
    term.draw(|frame| {
        let text = Paragraph::new("Hello, World!")
            .style(Style::default().fg(RatatuiColor::Green).bold())
            .alignment(Alignment::Center)
            .block(Block::bordered().title(" Minimal Example "));
        frame.render_widget(text, frame.area());
    });
}
