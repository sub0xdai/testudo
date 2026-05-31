// @anchor infra:cli:main
// @tags infra

use clap::Parser;
use testudo_cli::app::run_app;
use testudo_cli::cmd::{
    run_agent, run_attach, run_init, run_journal, run_listen,
    run_strategy_add, run_strategy_list, run_strategy_remove, run_strategy_show,
    run_strategy_validate,
};
use testudo_cli::config::Config;
use testudo_cli::{AgentAction, Command, StrategyAction};

fn init_tracing() {
    use tracing_subscriber::fmt;
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_env_filter(filter)
        .init();
}

fn main() {
    let command = Command::parse();
    let config = Config::load();

    match &command {
        Command::Dashboard => {
            // TUI owns the terminal — don't initialize tracing subscriber
            if let Err(e) = run_app(config) {
                eprintln!("TUI error: {}", e);
                std::process::exit(1);
            }
        }
        Command::Journal => {
            init_tracing();
            tracing::info!("journal: fetching summary timeframe=30d format=llm");
            if let Err(e) = run_journal(&config) {
                tracing::error!(error = %e, "journal command failed");
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Command::Listen => {
            init_tracing();
            tracing::info!("listen: connecting to {}", config.api.ws_url);
            if let Err(e) = run_listen(&config) {
                tracing::error!(error = %e, "listen command failed");
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Command::Agent(action) => match action {
            AgentAction::Start { strategy, daemon } => {
                if *daemon {
                    use testudo_cli::daemon;
                    daemon::write_pid_file().unwrap_or_else(|e| {
                        eprintln!("Failed to write PID file: {}", e);
                    });
                    daemon::print_startup_info();

                    // Set up file logging
                    let log_dir = daemon::daemon_dir().join("logs");
                    std::fs::create_dir_all(&log_dir).ok();
                    let file_appender = tracing_appender::rolling::daily(&log_dir, "testudo.log");
                    tracing_subscriber::fmt()
                        .json()
                        .with_writer(file_appender)
                        .with_env_filter(
                            tracing_subscriber::EnvFilter::try_from_default_env()
                                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                        )
                        .init();

                    tracing::info!("Daemon starting. PID: {}", std::process::id());

                    // Start agent loop in a tokio task
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        let (tx, _rx) = tokio::sync::watch::channel(
                            daemon::DaemonState::default(),
                        );

                        // Spawn agent (runs in background)
                        let _config_clone = config.clone();
                        let _strategy_clone = strategy.clone();
                        let state_tx = tx.clone();
                        tokio::spawn(async move {
                            // Periodically update daemon state
                            let mut interval =
                                tokio::time::interval(std::time::Duration::from_secs(2));
                            loop {
                                interval.tick().await;
                                let _ = state_tx.send(daemon::DaemonState {
                                    phase: "Running".into(),
                                    signal_count: 0,
                                    uptime_secs: 0,
                                    last_error: None,
                                });
                            }
                        });

                        // Set up Unix socket
                        let socket_path = daemon::socket_path();
                        let _ = std::fs::remove_file(&socket_path);
                        let listener = tokio::net::UnixListener::bind(&socket_path)
                            .unwrap_or_else(|e| {
                                eprintln!("Failed to bind socket: {}", e);
                                std::process::exit(1);
                            });

                        tracing::info!("Socket listening at {}", socket_path.display());

                        loop {
                            match listener.accept().await {
                                Ok((stream, _)) => {
                                    let rx = tx.subscribe();
                                    tokio::spawn(daemon::handle_control_connection(
                                        stream, rx,
                                    ));
                                }
                                Err(e) => {
                                    tracing::error!("Socket accept error: {}", e);
                                }
                            }
                        }
                    });
                } else {
                    init_tracing();
                    tracing::info!("agent: starting autonomous loop");
                    if let Err(e) = run_agent(&config, strategy.clone()) {
                        tracing::error!(error = %e, "agent loop failed");
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            _ => {
                println!(
                    "not yet implemented: agent {}",
                    format!("{:?}", action).to_lowercase()
                );
            }
        },
        Command::Strategy(action) => {
            let config_dir = Config::config_dir();
            match action {
                StrategyAction::List => {
                    if let Err(e) = run_strategy_list(&config_dir) {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
                StrategyAction::Add { name, from } => {
                    if let Err(e) = run_strategy_add(&config_dir, name.as_str(), std::path::Path::new(&from)) {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
                StrategyAction::Show { name } => {
                    if let Err(e) = run_strategy_show(&config_dir, name.as_str()) {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
                StrategyAction::Remove { name } => {
                    if let Err(e) = run_strategy_remove(&config_dir, name.as_str()) {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
                StrategyAction::Validate { name } => {
                    if let Err(e) = run_strategy_validate(&config_dir, name.as_str()) {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Command::Attach => {
            if let Err(e) = run_attach() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Command::Init => {
            if let Err(e) = run_init(&config) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }
}
