use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(30),
        ])
        .split(area);

    // Left: status message
    let status_color = if app.status.starts_with("Error") {
        Color::Red
    } else if app.streaming {
        Color::Yellow
    } else {
        Color::Rgb(100, 180, 100)
    };

    let streaming_indicator = if app.streaming { " ⟳" } else { "" };

    let left = Paragraph::new(Line::from(vec![
        Span::styled(" ● ", Style::default().fg(status_color)),
        Span::styled(
            format!("{}{}", app.status, streaming_indicator),
            Style::default().fg(Color::White),
        ),
    ]))
    .style(Style::default().bg(Color::Rgb(20, 20, 30)));

    // Right: backend + model info
    let right = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {} ", app.backend.name()),
            Style::default()
                .fg(Color::Rgb(150, 100, 255))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("│", Style::default().fg(Color::Rgb(60, 60, 80))),
        Span::styled(
            format!(" ✦ {} ", app.backend.model()),
            Style::default().fg(Color::Rgb(180, 180, 180)),
        ),
        Span::styled("│", Style::default().fg(Color::Rgb(60, 60, 80))),
        Span::styled(
            format!(" ⚡ {} ", app.config.planner.model),
            Style::default().fg(Color::Rgb(120, 180, 120)),
        ),
        Span::styled("│", Style::default().fg(Color::Rgb(60, 60, 80))),
        Span::styled(
            format!(" {} ", shorten_path(&app.working_dir)),
            Style::default().fg(Color::Rgb(200, 180, 100)),
        ),
    ]))
    .style(Style::default().bg(Color::Rgb(20, 20, 30)));

    f.render_widget(left, layout[0]);
    f.render_widget(right, layout[1]);
}

fn shorten_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 2 {
        return path.to_string();
    }
    format!("…/{}/{}", parts[parts.len() - 2], parts[parts.len() - 1])
}
