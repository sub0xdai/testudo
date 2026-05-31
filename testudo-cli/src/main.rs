// @anchor infra:cli:main
// @tags infra

use clap::Parser;
use testudo_cli::app::run_app;
use testudo_cli::cmd::{run_journal, run_listen};
use testudo_cli::config::Config;
use testudo_cli::Command;

fn main() {
    let command = Command::parse();
    let config = Config::load();

    match &command {
        Command::Dashboard => {
            if let Err(e) = run_app(config) {
                eprintln!("TUI error: {}", e);
                std::process::exit(1);
            }
        }
        Command::Journal => {
            if let Err(e) = run_journal(&config) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Command::Listen => {
            if let Err(e) = run_listen(&config) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
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
