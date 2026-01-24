mod app;
mod config;
mod events;
mod ui;
mod worker;

use std::io;
use std::time::Duration;

use clap::Parser;
use color_eyre::Result;
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use tokio::sync::mpsc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::sync::Arc;

use app::App;
use config::{check_ytdlp, Config};
use events::AppEvent;
use worker::WorkerPool;

#[derive(Parser)]
#[command(name = "oxidlp")]
#[command(about = "A beautiful TUI YouTube downloader", long_about = None)]
struct Cli {
    #[arg(value_name = "URL")]
    urls: Vec<String>,
    #[arg(short, long)]
    output: Option<String>,
    #[arg(short = 'j', long, default_value = "3")]
    concurrent: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let file_appender = tracing_appender::rolling::daily(
        directories::ProjectDirs::from("com", "oxidlp", "oxidlp")
            .map(|d| d.data_dir().to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from(".")),
        "oxidlp.log",
    );
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(non_blocking))
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let ytdlp_version = check_ytdlp().await?;
    tracing::info!("Found yt-dlp version: {}", ytdlp_version);

    let cli = Cli::parse();
    
    let mut config = Config::load().await?;
    if let Some(output) = cli.output {
        config.output_dir = output.into();
    }
    config.max_concurrent_downloads = cli.concurrent;
    let config = Arc::new(config);
    let (worker_tx, worker_rx) = mpsc::channel(32);
    let (event_tx, mut event_rx) = mpsc::channel(32);
    let mut app = App::new((*config).clone(), worker_tx);
    
    for url in cli.urls {
        app.handle_event(AppEvent::AddUrl(url));
    }

    let worker = WorkerPool::new(config, worker_rx, event_tx);
    tokio::spawn(worker.run());

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, &mut app, &mut event_rx).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    event_rx: &mut mpsc::Receiver<AppEvent>,
) -> Result<()> {
    // Initial CPU refresh - need two calls with delay to establish baseline
    app.sysinfo.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    std::thread::sleep(Duration::from_millis(500));
    app.sysinfo.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    
    let mut last_sysinfo_refresh = std::time::Instant::now();
    const SYSINFO_REFRESH_INTERVAL: Duration = Duration::from_millis(1000);
    
    loop {
        while event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Some(app_event) = ui::input::handle_key(key, app) {
                        app.handle_event(app_event);
                    }
                }
            }
        }
        
        while let Ok(worker_event) = event_rx.try_recv() {
            app.handle_event(worker_event);
        }
        
        if app.show_sysinfo && last_sysinfo_refresh.elapsed() >= SYSINFO_REFRESH_INTERVAL {
            app.sysinfo.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            last_sysinfo_refresh = std::time::Instant::now();
        }
        
        if app.loading_playlists > 0 {
            app.spinner_frame = app.spinner_frame.wrapping_add(1);
        }
        
        terminal.draw(|f| ui::render(f, app))?;

        if app.should_quit {
            break;
        }
        
        tokio::time::sleep(Duration::from_millis(16)).await;
    }

    Ok(())
}
