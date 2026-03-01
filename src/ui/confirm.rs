use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let Some((tool, summary, _)) = &app.pending_permission else {
        return;
    };

    let area = centered_rect(62, 10, f.area());

    let (icon, tool_color) = match tool.as_str() {
        "run_shell" | "shell" | "bash" => ("🐚", Color::Rgb(255, 200, 80)),
        "write_file" => ("✏️ ", Color::Rgb(100, 180, 255)),
        _ => ("⚙️ ", Color::Rgb(200, 200, 200)),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(255, 160, 40)))
        .title(Span::styled(
            " ⚠  Permission Required ",
            Style::default()
                .fg(Color::Rgb(255, 200, 80))
                .add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Span::styled(
            " [y] Allow  [n / Esc] Deny ",
            Style::default().fg(Color::DarkGray),
        ));

    let inner = block.inner(area);

    let lines = vec![
        Line::default(),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{} {} ", icon, tool),
                Style::default().fg(tool_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "wants to run:",
                Style::default().fg(Color::Rgb(180, 180, 180)),
            ),
        ]),
        Line::default(),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                summary.clone(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::default(),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "[y] ",
                Style::default()
                    .fg(Color::Rgb(100, 220, 100))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Allow    ", Style::default().fg(Color::Rgb(150, 220, 150))),
            Span::styled(
                "[n] ",
                Style::default()
                    .fg(Color::Rgb(220, 100, 100))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Deny", Style::default().fg(Color::Rgb(220, 150, 150))),
        ]),
    ];

    f.render_widget(Clear, area);
    f.render_widget(block, area);
    f.render_widget(Paragraph::new(lines), inner);
}

fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + r.width.saturating_sub(width) / 2;
    let y = r.y + r.height.saturating_sub(height) / 2;
    Rect::new(
        x.min(r.x + r.width),
        y.min(r.y + r.height),
        width.min(r.width),
        height.min(r.height),
    )
}
