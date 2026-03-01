use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let dir_short = app
        .working_dir
        .split('/')
        .last()
        .unwrap_or(&app.working_dir);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 80)))
        .title(Span::styled(
            format!(" 📁 {} ", dir_short),
            Style::default()
                .fg(Color::Rgb(200, 180, 100))
                .add_modifier(Modifier::BOLD),
        ));

    let items: Vec<ListItem> = app
        .file_tree
        .iter()
        .map(|entry| {
            let (icon, color) = if entry.ends_with('/') {
                ("󰉋 ", Color::Rgb(200, 180, 100))
            } else {
                file_icon(entry)
            };

            ListItem::new(Line::from(vec![
                Span::styled(icon, Style::default().fg(color)),
                Span::styled(entry.clone(), Style::default().fg(color)),
            ]))
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn file_icon(name: &str) -> (&'static str, Color) {
    let ext = name.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => ("󱘗 ", Color::Rgb(222, 165, 132)),
        "py" => ("󰌠 ", Color::Rgb(255, 215, 0)),
        "js" | "mjs" | "cjs" => ("󰌞 ", Color::Rgb(240, 219, 79)),
        "ts" => ("󰛦 ", Color::Rgb(0, 122, 204)),
        "go" => ("󰟓 ", Color::Rgb(0, 173, 216)),
        "c" | "h" => (" ", Color::Rgb(85, 107, 211)),
        "cpp" | "cxx" | "cc" => (" ", Color::Rgb(85, 107, 211)),
        "md" => ("󰍔 ", Color::Rgb(255, 255, 255)),
        "toml" | "yaml" | "yml" | "json" => ("󰒓 ", Color::Rgb(150, 200, 150)),
        "html" | "htm" => ("󰌝 ", Color::Rgb(228, 79, 38)),
        "css" => ("󰌜 ", Color::Rgb(38, 139, 210)),
        "sh" | "bash" | "zsh" => ("󰆍 ", Color::Rgb(100, 200, 100)),
        "lock" => ("󰌾 ", Color::DarkGray),
        _ => (" ", Color::Gray),
    }
}
