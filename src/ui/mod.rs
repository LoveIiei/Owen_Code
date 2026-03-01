mod chat;
mod file_tree;
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
    let show_tree = app.config.ui.show_file_tree && !app.file_tree.is_empty();

    let outer = if show_tree {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(24), Constraint::Min(0)])
            .split(f.area())
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0)])
            .split(f.area())
    };

    let main_area = if show_tree {
        file_tree::draw(f, app, outer[0]);
        outer[1]
    } else {
        outer[0]
    };

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
        .split(main_area);

    chat::draw(f, app, main_layout[0]);
    chat::draw_input(f, app, main_layout[1]);
    status_bar::draw(f, app, main_layout[2]);

    // Overlay popups
    match app.mode {
        AppMode::ModelSelect => model_select::draw(f, app),
        AppMode::SessionSelect => session_select::draw(f, app),
        AppMode::Help => help::draw(f),
        _ => {}
    }
}
