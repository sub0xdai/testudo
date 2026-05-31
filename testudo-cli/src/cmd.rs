// @anchor infra:cli:cmd
// @tags infra

//! Command enum — clap CLI parsing + command handlers for the testudo trading harness.

use clap::{Parser, Subcommand};
use crate::api::client::ApiClient;
use crate::api::types::ApiError;
use crate::config::Config;
use crate::ws::client::WsClient;

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

// ── Command handlers ──────────────────────────────────────────────────

/// `testudo journal` — fetch and print the agent journal summary.
pub fn run_journal(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    if config.api.agent_key.is_empty() {
        tracing::warn!("journal: no agent key configured");
        return Err(
            "No agent key configured. Run 'testudo init' first, \
             or set api.agent_key in ~/.config/testudo/config.toml"
                .into(),
        );
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let client = ApiClient::new(&config.api);
        match client.get_summary_text("30d", "llm").await {
            Ok(markdown) => {
                println!("{}", markdown);
                Ok(())
            }
            Err(ApiError::Unauthorized) => {
                Err("Unauthorized — check your agent key in \
                     ~/.config/testudo/config.toml"
                    .into())
            }
            Err(ApiError::NotFound(_)) => {
                println!("No trades found for this period.");
                Ok(())
            }
            Err(e) => Err(format!("Failed to fetch journal: {}", e).into()),
        }
    })
}

/// `testudo listen` — stream WebSocket events to stdout as JSON Lines.
pub fn run_listen(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    if config.api.agent_key.is_empty() {
        tracing::warn!("listen: no agent key configured");
        return Err(
            "No agent key configured. Run 'testudo init' first, \
             or set api.agent_key in ~/.config/testudo/config.toml"
                .into(),
        );
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let client = WsClient::new(&config.api.ws_url, &config.api.agent_key);

        // Subscribe to all agent channels (user_id omitted — server handles routing via auth)
        let channels = vec![
            "agent.alert".to_string(),
            "agent.execution".to_string(),
        ];

        eprintln!("Connecting to {}...", config.api.ws_url);
        let mut stream = client.connect(&channels).await.map_err(|e| {
            format!("Failed to connect: {}", e)
        })?;

        eprintln!("Listening for agent events (Ctrl-C to stop)...\n");

        while let Some(event) = stream.recv().await {
            match serde_json::to_string(&event) {
                Ok(line) => println!("{}", line),
                Err(e) => eprintln!("json serialize error: {}", e),
            }
        }

        eprintln!("WebSocket stream ended.");
        Ok(())
    })
}
