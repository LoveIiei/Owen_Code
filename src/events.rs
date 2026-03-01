use crate::agent::AgentEvent;
use anyhow::Result;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Agent(AgentEvent),
    Tick,
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
    pub tx: mpsc::UnboundedSender<AppEvent>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            loop {
                if event::poll(Duration::from_millis(16)).unwrap_or(false) {
                    match event::read() {
                        Ok(CrosstermEvent::Key(key)) => {
                            let _ = tx_clone.send(AppEvent::Key(key));
                        }
                        Ok(CrosstermEvent::Mouse(mouse)) => {
                            let _ = tx_clone.send(AppEvent::Mouse(mouse));
                        }
                        Ok(CrosstermEvent::Resize(w, h)) => {
                            let _ = tx_clone.send(AppEvent::Resize(w, h));
                        }
                        _ => {}
                    }
                }
                let _ = tx_clone.send(AppEvent::Tick);
                tokio::time::sleep(Duration::from_millis(16)).await;
            }
        });

        Self { rx, tx }
    }

    pub async fn next(&mut self) -> Result<AppEvent> {
        self.rx.recv().await.ok_or_else(|| anyhow::anyhow!("Event channel closed"))
    }
}
