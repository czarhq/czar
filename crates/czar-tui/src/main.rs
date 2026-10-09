mod api;
mod app;
mod dag;
mod debug_dump;
mod ui;

#[cfg(test)]
mod tests;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use tokio::sync::mpsc;

use app::worker::{AppCommand, run_worker};

#[derive(Parser, Default)]
#[command(
    name = "czar-tui",
    about = "Terminal UI for the Czar workflow orchestrator"
)]
struct Cli {
    /// Base URL of the Czar API server
    #[arg(long, default_value = "http://127.0.0.1:4000")]
    base_url: String,
    /// API token of the server (`czar serve --auth-token`)
    #[arg(long, env = "CZAR_AUTH_TOKEN", hide_env_values = true)]
    auth_token: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let client = api::ApiClient::new(&cli.base_url).with_token(cli.auth_token);
    let result = run_app(&mut terminal, client).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(ref e) = result {
        eprintln!("Error: {e}");
    }

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    client: api::ApiClient,
) -> Result<()> {
    let is_local = client.is_local();

    let (cmd_tx, cmd_rx) = mpsc::channel::<AppCommand>(64);
    let (update_tx, update_rx) = mpsc::channel(256);

    // Spawn the background worker — all API calls happen here.
    tokio::spawn(run_worker(client, cmd_rx, update_tx));

    let mut app = app::App::new(is_local);

    // Kick off the initial data load.
    let _ = cmd_tx
        .send(AppCommand::Refresh {
            selected_workflow: None,
            selected_run: None,
        })
        .await;

    app::run(terminal, &mut app, cmd_tx, update_rx).await
}
