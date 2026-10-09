use anyhow::Result;
use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};

use super::state::App;
use super::worker::{AppCommand, AppUpdate};

const REFRESH_INTERVAL_LOCAL_MS: u64 = 1_000;
const REFRESH_INTERVAL_REMOTE_MS: u64 = 10_000;

pub async fn run(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    cmd_tx: mpsc::Sender<AppCommand>,
    mut update_rx: mpsc::Receiver<AppUpdate>,
) -> Result<()> {
    let (key_tx, mut key_rx) = tokio::sync::mpsc::channel::<crossterm::event::KeyEvent>(64);

    tokio::spawn(async move {
        let mut stream = EventStream::new();
        while let Some(Ok(event)) = stream.next().await {
            if let Event::Key(key) = event
                && key_tx.send(key).await.is_err()
            {
                break;
            }
        }
    });

    let refresh_ms = if app.is_local {
        REFRESH_INTERVAL_LOCAL_MS
    } else {
        REFRESH_INTERVAL_REMOTE_MS
    };
    let mut render_tick = interval(Duration::from_millis(100));
    let mut data_tick = interval(Duration::from_millis(refresh_ms));
    // Skip the first immediate tick so we don't double-refresh at startup.
    data_tick.tick().await;

    loop {
        terminal.draw(|f| crate::ui::render(f, app))?;

        tokio::select! {
            Some(key) = key_rx.recv() => {
                app.handle_key(key, &cmd_tx);
            }
            Some(update) = update_rx.recv() => {
                app.apply_update(update);
            }
            _ = render_tick.tick() => {}
            _ = data_tick.tick() => {
                let _ = cmd_tx.send(AppCommand::Refresh {
                    selected_workflow: app.selected_workflow.clone(),
                    selected_run: app.selected_run.clone(),
                }).await;
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
