// @anchor infra:cli:cmd
// @tags infra

//! Command enum — clap CLI parsing for the testudo trading harness.

use clap::{Parser, Subcommand};

/// Testudo trading agent harness — terminal-first trading client.
#[derive(Parser, Debug)]
#[command(name = "testudo", version = "0.1.0", about = "Testudo trading agent harness")]
pub enum Command {
    /// Initialize everything: onboarding wizard, API key config, exchange credentials
    Init,

    /// Control the autonomous agent loop
    #[command(subcommand)]
    Agent(AgentAction),

    /// Open the live TUI dashboard
    Dashboard,

    /// Stream WebSocket alerts to stdout (JSON Lines)
    Listen,

    /// Print journal summary
    Journal,

    /// Manage trading strategies
    #[command(subcommand)]
    Strategy(StrategyAction),

    /// Attach to a running daemon process
    Attach,
}

#[derive(Subcommand, Debug)]
pub enum AgentAction {
    /// Start the autonomous agent loop
    Start,
    /// Stop the agent gracefully
    Stop,
    /// Pause the agent (preserves state)
    Pause,
    /// Resume a paused agent
    Resume,
}

#[derive(Subcommand, Debug)]
pub enum StrategyAction {
    /// List all installed strategies
    List,
    /// Add a new strategy from a TOML file
    Add,
    /// Show a strategy's details
    Show,
    /// Remove a strategy
    Remove,
}

impl Command {
    /// Returns a human-readable description of the command variant.
    /// Used by stub handlers to print "not yet implemented" messages.
    pub fn description(&self) -> String {
        match self {
            Command::Init => "init".into(),
            Command::Agent(action) => format!("agent {:?}", action).to_lowercase(),
            Command::Dashboard => "dashboard".into(),
            Command::Listen => "listen".into(),
            Command::Journal => "journal".into(),
            Command::Strategy(action) => format!("strategy {:?}", action).to_lowercase(),
            Command::Attach => "attach".into(),
        }
    }
}
