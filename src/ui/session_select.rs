use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 60, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(100, 180, 100)))
        .title(Span::styled(
            " 💾 Sessions ",
            Style::default()
                .fg(Color::Rgb(150, 230, 150))
                .add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Span::styled(
            " [↑↓] Navigate  [Enter] Load  [d] Delete  [Esc] Cancel ",
            Style::default().fg(Color::DarkGray),
        ));

    let inner = block.inner(area);

    let items: Vec<ListItem> = app
        .session_list
        .iter()
        .enumerate()
        .map(|(i, meta)| {
            let is_current = meta.id == app.session.id;
            let is_selected = i == app.session_list_idx;

            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(100, 180, 100))
                    .add_modifier(Modifier::BOLD)
            } else if is_current {
                Style::default().fg(Color::Rgb(100, 180, 100))
            } else {
                Style::default().fg(Color::White)
            };

            let prefix = if is_current { "● " } else { "  " };
            let time = meta.updated_at.format("%Y-%m-%d %H:%M").to_string();
            let msgs = format!("{} msgs", meta.message_count);

            ListItem::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::Rgb(100, 180, 100))),
                Span::styled(
                    format!("{:<30}", truncate(&meta.name, 28)),
                    style,
                ),
                Span::styled(
                    format!("  {:16}  {:>8}  {}", time, msgs, truncate(&meta.model, 25)),
                    if is_selected {
                        style
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.session_list_idx));

    f.render_widget(Clear, area);
    f.render_widget(block, area);
    f.render_stateful_widget(List::new(items), inner, &mut list_state);
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
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
