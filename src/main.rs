mod agent;
mod ai;
mod app;
mod config;
mod events;
mod planner;
mod session;
mod tools;
mod ui;

use anyhow::Result;
use app::App;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize panic handler so terminal is restored on crash
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        crossterm::terminal::disable_raw_mode().ok();
        crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen
        )
        .ok();
        original_hook(panic_info);
    }));

    let mut app = App::new().await?;
    app.run().await?;

    Ok(())
}
