use crate::app::{App, AppMode, EntryKind};
use crate::ai::Role;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthStr;

const USER_COLOR: Color = Color::Cyan;
const ASSISTANT_COLOR: Color = Color::Green;
const DIM: Color = Color::DarkGray;
const CODE_BG: Color = Color::Rgb(30, 30, 40);

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 80)))
        .title(Span::styled(
            format!(" 🤖 aicode — {} ", app.backend.model()),
            Style::default().fg(Color::Rgb(150, 100, 255)).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    for entry in &app.chat_log {
        match &entry.kind {
            EntryKind::Tool { success } => {
                render_tool_card(&entry.content, *success, &mut lines);
            }
            EntryKind::User => {
                let time_str = entry.timestamp.format("%H:%M:%S").to_string();
                lines.push(Line::from(vec![
                    Span::styled("╭─ You ", Style::default().fg(USER_COLOR).add_modifier(Modifier::BOLD)),
                    Span::styled(time_str, Style::default().fg(DIM)),
                ]));
                render_markdown_lines(&entry.content, USER_COLOR, &mut lines);
                lines.push(Line::from(Span::styled("╰──────────────────────────────────────", Style::default().fg(Color::Rgb(40, 40, 60)))));
                lines.push(Line::default());
            }
            EntryKind::Assistant => {
                let time_str = entry.timestamp.format("%H:%M:%S").to_string();
                lines.push(Line::from(vec![
                    Span::styled("╭─ Assistant ", Style::default().fg(ASSISTANT_COLOR).add_modifier(Modifier::BOLD)),
                    Span::styled(time_str, Style::default().fg(DIM)),
                ]));
                render_markdown_lines(&entry.content, ASSISTANT_COLOR, &mut lines);
                lines.push(Line::from(Span::styled("╰──────────────────────────────────────", Style::default().fg(Color::Rgb(40, 40, 60)))));
                lines.push(Line::default());
            }
        }
    }

    // Live streaming indicator
    if app.streaming {
        if !app.streaming_buffer.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("╭─ Assistant ", Style::default().fg(ASSISTANT_COLOR).add_modifier(Modifier::BOLD)),
                Span::styled("⟳ streaming…", Style::default().fg(DIM)),
            ]));
            render_markdown_lines(&app.streaming_buffer, ASSISTANT_COLOR, &mut lines);
        } else {
            let label = if app.streaming_label.is_empty() {
                "⟳ Working…".to_string()
            } else {
                format!("⟳ {}", app.streaming_label)
            };
            lines.push(Line::from(Span::styled(
                format!("  {}", label),
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            )));
        }
    }

    let content_height = lines.len() as u16;
    let scroll = if app.scroll == u16::MAX {
        content_height.saturating_sub(inner.height)
    } else {
        app.scroll.min(content_height.saturating_sub(inner.height))
    };

    let paragraph = Paragraph::new(Text::from(lines))
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, inner);
}

pub fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    let is_insert = matches!(app.mode, AppMode::Insert);
    let border_color = if is_insert {
        Color::Rgb(100, 150, 255)
    } else {
        Color::Rgb(60, 60, 80)
    };

    let mode_label = match app.mode {
        AppMode::Insert => " INSERT ",
        AppMode::Normal => " NORMAL ",
        _ => "        ",
    };

    let hint = if is_insert {
        " Ctrl+Enter: Send  Enter: Newline  Alt+↑↓: History  Esc: Normal "
    } else {
        " [i] Insert  [m] Model  [?] Help  [q] Quit  Ctrl+S: Save session "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            mode_label,
            Style::default().fg(border_color).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Span::styled(hint, Style::default().fg(DIM)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.input.is_empty() && !is_insert {
        let placeholder = Paragraph::new(Span::styled(
            "Press [i] to start typing…",
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        ));
        f.render_widget(placeholder, inner);
        return;
    }

    // Render each line of the multi-line buffer
    let lines: Vec<Line> = app
        .input
        .lines
        .iter()
        .enumerate()
        .map(|(row, line_text)| {
            // Highlight the active row slightly differently
            let style = if row == app.input.row && is_insert {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Rgb(200, 200, 200))
            };
            Line::from(Span::styled(line_text.clone(), style))
        })
        .collect();

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner);

    // Place terminal cursor at the correct position
    if is_insert {
        let cursor_col = app.input.lines[app.input.row][..app.input.col].width() as u16;
        let cursor_x = (inner.x + cursor_col).min(inner.x + inner.width.saturating_sub(1));
        let cursor_y = (inner.y + app.input.row as u16).min(inner.y + inner.height.saturating_sub(1));
        f.set_cursor_position((cursor_x, cursor_y));
    }
}

fn render_tool_card<'a>(content: &str, success: bool, lines: &mut Vec<Line<'a>>) {
    let (border_color, bg_accent) = if success {
        (Color::Rgb(60, 140, 60), Color::Rgb(20, 40, 20))
    } else {
        (Color::Rgb(160, 60, 60), Color::Rgb(40, 15, 15))
    };

    // First line is the label e.g. "✓ Tool: run_shell"
    let mut content_lines = content.lines();
    let label = content_lines.next().unwrap_or("");
    let output: Vec<&str> = content_lines.collect();

    lines.push(Line::from(vec![
        Span::styled("  ┌─ ", Style::default().fg(border_color)),
        Span::styled(label.to_string(), Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
    ]));

    for line in &output {
        lines.push(Line::from(vec![
            Span::styled("  │ ", Style::default().fg(border_color)),
            Span::styled(line.to_string(), Style::default().fg(Color::Rgb(200, 210, 200)).bg(bg_accent)),
        ]));
    }

    lines.push(Line::from(Span::styled(
        "  └──────────────────────────────────────",
        Style::default().fg(border_color),
    )));
    lines.push(Line::default());
}

fn render_markdown_lines<'a>(content: &str, accent: Color, lines: &mut Vec<Line<'a>>) {
    let mut in_code_block = false;
    let mut code_lang = String::new();

    for raw_line in content.lines() {
        if raw_line.starts_with("```") {
            if in_code_block {
                // End of code block
                lines.push(Line::from(Span::styled(
                    "  └─────────────────────────────────",
                    Style::default().fg(Color::Rgb(60, 80, 60)),
                )));
                in_code_block = false;
                code_lang.clear();
            } else {
                // Start of code block
                code_lang = raw_line.trim_start_matches('`').to_string();
                let lang_label = if code_lang.is_empty() {
                    "code".to_string()
                } else {
                    code_lang.clone()
                };
                lines.push(Line::from(Span::styled(
                    format!("  ┌── {} ", lang_label),
                    Style::default().fg(Color::Rgb(60, 120, 60)),
                )));
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            lines.push(Line::from(vec![
                Span::styled("  │ ", Style::default().fg(Color::Rgb(60, 120, 60))),
                Span::styled(raw_line.to_string(), Style::default().fg(Color::Rgb(200, 220, 200)).bg(CODE_BG)),
            ]));
            continue;
        }

        // Heading
        if raw_line.starts_with("# ") {
            lines.push(Line::from(Span::styled(
                format!("  {}", &raw_line[2..]),
                Style::default().fg(accent).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )));
            continue;
        }
        if raw_line.starts_with("## ") {
            lines.push(Line::from(Span::styled(
                format!("  {}", &raw_line[3..]),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            )));
            continue;
        }

        // Bold **text**
        let line_text = render_inline_markdown(raw_line, accent);
        lines.push(Line::from(vec![
            Span::raw("  "),
            line_text,
        ]));
    }
}

fn render_inline_markdown(text: &str, _accent: Color) -> Span<'static> {
    // Simple fallback: strip ** markers for now
    // Full inline parsing would need to return Vec<Span>
    let cleaned = text
        .replace("**", "")
        .replace("__", "")
        .replace('`', "'");
    Span::styled(cleaned, Style::default().fg(Color::White))
}
