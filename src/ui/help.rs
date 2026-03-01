use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

const HELP_TEXT: &[(&str, &str)] = &[
    ("NAVIGATION", ""),
    ("[i] / [a]", "Enter insert mode"),
    ("[Esc]", "Return to normal mode"),
    ("[↑↓] / [j][k]", "Scroll chat history"),
    ("[g] / [G]", "Scroll to top / bottom"),
    ("", ""),
    ("COMPOSING (Insert mode)", ""),
    ("[Enter]", "Insert a new line"),
    ("[Ctrl+Enter]", "Send message"),
    ("[Alt+↑↓]", "Navigate input history"),
    ("[↑↓ ← →]", "Move cursor within input"),
    ("[Home] / [End]", "Jump to line start / end"),
    ("", ""),
    ("ACTIONS (Normal mode)", ""),
    ("[m]", "Open model selector"),
    ("[Ctrl+S]", "Quick-save session"),
    ("[?]", "Show this help"),
    ("[q] / Ctrl+C", "Quit (auto-saves)"),
    ("", ""),
    ("SLASH COMMANDS", ""),
    ("/run <cmd>", "Execute shell command"),
    ("/read <file>", "Read file into context"),
    ("/ls [path]", "List directory"),
    ("/cd <path>", "Change working directory"),
    ("/save [name]", "Save session with optional name"),
    ("/sessions", "Browse & load saved sessions"),
    ("/new", "Start a new session"),
    ("/load <id>", "Load session by ID"),
    ("/model", "Switch AI model"),
    ("/clear", "Clear chat history"),
    ("/quit", "Exit OwenCode"),
];

pub fn draw(f: &mut Frame) {
    let area = centered_rect(60, 80, f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(100, 150, 255)))
        .title(Span::styled(
            " ❓ Help ",
            Style::default()
                .fg(Color::Rgb(150, 200, 255))
                .add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Span::styled(
            " [Any key] Close ",
            Style::default().fg(Color::DarkGray),
        ));

    let inner = block.inner(area);

    let lines: Vec<Line> = HELP_TEXT
        .iter()
        .map(|(key, desc)| {
            if desc.is_empty() && !key.is_empty() {
                // Section header
                Line::from(Span::styled(
                    format!(" ── {} ", key),
                    Style::default()
                        .fg(Color::Rgb(150, 100, 255))
                        .add_modifier(Modifier::BOLD),
                ))
            } else if key.is_empty() {
                Line::default()
            } else {
                Line::from(vec![
                    Span::styled(
                        format!("  {:20}", key),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(*desc, Style::default().fg(Color::White)),
                ])
            }
        })
        .collect();

    let paragraph = Paragraph::new(lines);

    f.render_widget(Clear, area);
    f.render_widget(block, area);
    f.render_widget(paragraph, inner);
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
