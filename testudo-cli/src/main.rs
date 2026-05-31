// @anchor infra:cli:main
// @tags infra

use clap::Parser;
use testudo_cli::app::run_app;
use testudo_cli::cmd::{
    run_agent, run_journal, run_listen,
    run_strategy_add, run_strategy_list, run_strategy_remove, run_strategy_show,
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
            AgentAction::Start { strategy } => {
                init_tracing();
                tracing::info!("agent: starting autonomous loop");
                if let Err(e) = run_agent(&config, strategy.clone()) {
                    tracing::error!(error = %e, "agent loop failed");
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
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
            }
        }
        other => {
            println!(
                "not yet implemented: {} | Config loaded: {}",
                other.description(),
                config.api.base_url
            );
        }
    }
}
