use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 60, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(150, 100, 255)))
        .title(Span::styled(
            " 🔧 Select Model ",
            Style::default()
                .fg(Color::Rgb(200, 150, 255))
                .add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Span::styled(
            " [↑↓] Navigate  [Enter] Select  [Esc] Cancel ",
            Style::default().fg(Color::DarkGray),
        ));

    let inner = block.inner(area);

    // Layout: list + footer
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(inner);

    let items: Vec<ListItem> = app
        .available_models
        .iter()
        .enumerate()
        .map(|(i, model)| {
            let is_current = model == app.backend.model();
            let is_selected = i == app.selected_model_idx;

            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(150, 100, 255))
                    .add_modifier(Modifier::BOLD)
            } else if is_current {
                Style::default().fg(Color::Rgb(150, 100, 255))
            } else {
                Style::default().fg(Color::White)
            };

            let prefix = if is_current { "✓ " } else { "  " };

            ListItem::new(Line::from(Span::styled(
                format!("{}{}", prefix, model),
                style,
            )))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.selected_model_idx));

    f.render_widget(Clear, area);
    f.render_widget(block, area);
    f.render_stateful_widget(List::new(items), layout[0], &mut list_state);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
