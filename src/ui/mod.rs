mod chat;
mod confirm;
mod help;
mod model_select;
mod session_select;
mod status_bar;

use crate::app::{App, AppMode};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

/// Max lines to show in input box (plus 2 for border)
const MAX_INPUT_DISPLAY_LINES: u16 = 8;

pub fn draw(f: &mut Frame, app: &App) {
    // Dynamic input height: clamp between 3 (1 line + borders) and MAX
    let input_lines = app.input.line_count() as u16;
    let input_height = (input_lines + 2).clamp(3, MAX_INPUT_DISPLAY_LINES);

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(input_height),
            Constraint::Length(1),
        ])
        .split(f.area());

    chat::draw(f, app, main_layout[0]);
    chat::draw_input(f, app, main_layout[1]);
    status_bar::draw(f, app, main_layout[2]);

    // Overlay popups (order matters — confirm is on top of everything)
    match app.mode {
        AppMode::ModelSelect => model_select::draw(f, app),
        AppMode::SessionSelect => session_select::draw(f, app),
        AppMode::Help => help::draw(f),
        AppMode::Confirm => confirm::draw(f, app),
        _ => {}
    }
}

