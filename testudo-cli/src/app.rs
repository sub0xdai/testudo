// @anchor infra:cli:app
// @tags infra

//! TEA App wiring: Model + Update + View.

use crate::config::Config;
use crate::model::state::{AppState, StatusBar};
use crate::msg::Message;
use crate::theme::Theme;
use crate::update::update;
use crate::view::dashboard;
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::Terminal;
use std::io;
use tokio::sync::mpsc;

/// Run the TUI application. Blocks until the user quits.
pub fn run_app(config: Config) -> io::Result<()> {
    // Install panic hook to restore terminal before printing panic message
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stderr(), LeaveAlternateScreen);
        default_hook(info);
    }));

    // Init terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Build initial model
    let theme = Theme::from_name(&config.ui.theme);
    let state = AppState {
        screen: crate::model::state::Screen::Dashboard,
        status: StatusBar::new(),
        theme,
        error: None,
        positions: Vec::new(),
        signal_log: Vec::new(),
        equity_curve: Vec::new(),
        risk_snapshot: None,
        journal_summary: None,
    };

    // Run the TEA loop in a tokio runtime
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime");

    rt.block_on(async {
        tea_loop(&mut terminal, state).await
    })?;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}

async fn tea_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    mut state: AppState,
) -> io::Result<()> {
    let (tx, mut rx) = mpsc::channel::<Message>(32);

    // Spawn key reader
    let key_tx = tx.clone();
    tokio::spawn(async move {
        loop {
            if let Ok(event) = event::read() {
                let msg = match event {
                    Event::Key(key) if key.kind != KeyEventKind::Release => {
                        Message::KeyPress(key)
                    }
                    Event::Resize(cols, rows) => Message::Resize(cols, rows),
                    _ => continue,
                };
                if key_tx.send(msg).await.is_err() {
                    break;
                }
            }
        }
    });

    // Spawn tick timer
    let tick_tx = tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            if tick_tx.send(Message::Tick).await.is_err() {
                break;
            }
        }
    });

    // Drop the original tx so the channel closes when both tasks die
    drop(tx);

    // Initial render
    terminal.draw(|f| dashboard::render(f, &state))?;

    // TEA loop
    loop {
        let msg = rx.recv().await;
        let msg = match msg {
            Some(m) => m,
            None => break, // Channel closed — all senders dropped
        };

        let should_continue = update(&mut state, msg);
        if !should_continue {
            break;
        }

        terminal.draw(|f| dashboard::render(f, &state))?;
    }

    Ok(())
}
