// @anchor infra:cli:cmd
// @tags infra

//! Command enum — clap CLI parsing + command handlers for the testudo trading harness.

use clap::{Parser, Subcommand};
use crate::api::client::ApiClient;
use crate::api::types::ApiError;
use crate::config::Config;
use crate::llm::client::create_client;
use crate::llm::types::LlmMessage;
use crate::model::agent::{AgentMode, AgentPhase, AgentState};
use crate::tools::all_tools;
use crate::tools::types::ToolDef;
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

/// `testudo agent start` — run the autonomous agent loop.
#[allow(unused_assignments)]
pub fn run_agent(
    config: &Config,
    _strategy_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if config.llm.api_key.is_empty() {
        return Err("No LLM API key configured. Set llm.api_key in \
                    ~/.config/testudo/config.toml"
            .into());
    }
    if config.api.agent_key.is_empty() {
        return Err("No agent key configured. Run 'testudo init' first, \
                    or set api.agent_key in ~/.config/testudo/config.toml"
            .into());
    }

    let agent_mode = if config.agent.shadow_only {
        AgentMode::Shadow
    } else {
        AgentMode::Live
    };

    let system_prompt = include_str!("../../AGENT_TRADING.md");

    let llm = create_client(&config.llm);
    let tool_defs: Vec<ToolDef> = all_tools();
    let tools_json: Vec<serde_json::Value> = tool_defs
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters,
            })
        })
        .collect();

    let mut state = AgentState::new(agent_mode.clone());
    state.messages.push(LlmMessage {
        role: "system".into(),
        content: Some(system_prompt.into()),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    });

    eprintln!(
        "Agent starting in {:?} mode | interval: {}s",
        agent_mode, config.agent.loop_interval_secs
    );

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let api = std::sync::Arc::new(ApiClient::new(&config.api));
        let loop_interval = std::time::Duration::from_secs(config.agent.loop_interval_secs);
        let max_iterations: u64 = 100; // Safety cap

        for _iteration in 0..max_iterations {
            // ── Phase: Observing ──
            state.phase = AgentPhase::Observing;
            eprintln!("--- Iteration {} — Observing ---", _iteration + 1);

            // Read journal for context
            let journal_md = match api.get_summary_text("7d", "llm").await {
                Ok(md) => md,
                Err(e) => {
                    tracing::warn!("Failed to read journal: {}", e);
                    "No journal data available.".into()
                }
            };

            let context_msg = format!(
                "Here is your current trading context:\n\n## Journal (7d)\n{}\n\n\
                 Analyze the market and decide on your next action. \
                 If you see a trading opportunity, use submit_signal. \
                 Always use SHADOW mode unless told otherwise.",
                journal_md
            );

            state.messages.push(LlmMessage {
                role: "user".into(),
                content: Some(context_msg),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });

            // ── Phase: Thinking ──
            state.phase = AgentPhase::Thinking;
            eprintln!("Thinking...");

            let response = match llm.send_message(&state.messages, &tools_json).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("LLM error: {}", e);
                    state.phase = AgentPhase::Idle;
                    tokio::time::sleep(loop_interval).await;
                    continue;
                }
            };

            // ── Phase: Acting ──
            state.phase = AgentPhase::Acting;

            if !response.tool_calls.is_empty() {
                eprintln!(
                    "Executing {} tool call(s)...",
                    response.tool_calls.len()
                );

                // Push assistant message with tool calls
                state.messages.push(LlmMessage {
                    role: "assistant".into(),
                    content: response.content,
                    tool_calls: Some(response.tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                });

                for tc in &response.tool_calls {
                    let result = execute_tool_locally(&tc.name, &tc.arguments, &agent_mode);
                    let display = if result.len() > 100 {
                        format!("{}...", &result[..97])
                    } else {
                        result.clone()
                    };
                    eprintln!("  {}: {}", tc.name, display);

                    state.messages.push(LlmMessage {
                        role: "tool".into(),
                        content: Some(result),
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                        name: Some(tc.name.clone()),
                    });
                }
            } else if let Some(ref text) = response.content {
                eprintln!("LLM: {}", if text.len() > 200 {
                    format!("{}...", &text[..197])
                } else {
                    text.clone()
                });
            }

            // ── Phase: Idle / Sleep ──
            state.phase = AgentPhase::Idle;
            eprintln!(
                "Sleeping {}s...\n",
                loop_interval.as_secs()
            );
            tokio::time::sleep(loop_interval).await;
        }

        eprintln!("Agent loop complete (max iterations reached).");
        Ok(())
    })
}

/// Execute a tool call locally (no real API calls — simulation mode).
/// Real execution against the API comes in CP-4 with idempotency + journal.
fn execute_tool_locally(
    tool_name: &str,
    args: &serde_json::Value,
    mode: &AgentMode,
) -> String {
    match tool_name {
        "submit_signal" => {
            let exec_mode = args["execution_mode"]
                .as_str()
                .unwrap_or("SHADOW");
            if mode.is_shadow_only() && exec_mode.eq_ignore_ascii_case("LIVE") {
                return "BLOCKED: Agent is in SHADOW mode. LIVE signals are disabled. \
                        Set shadow_only = false in config to enable live trading."
                    .into();
            }
            let symbol = args["symbol"].as_str().unwrap_or("unknown");
            format!(
                "Signal submitted: {} {} @ {} ({} mode, confidence: {})",
                args["side"].as_str().unwrap_or("LONG"),
                symbol,
                args["entry_price"],
                exec_mode,
                args["confidence"].as_f64().unwrap_or(0.0),
            )
        }
        "fetch_klines" => format!(
            "Fetched klines for {} ({}) — simulated",
            args["symbol"].as_str().unwrap_or("unknown"),
            args["interval"].as_str().unwrap_or("1h"),
        ),
        "read_journal" => "Journal read — see context above.".into(),
        "write_journal" => format!(
            "Journal entry logged: {}",
            args["content"].as_str().unwrap_or("(empty)")
        ),
        "list_positions" => "No positions (use TUI for live position data).".into(),
        "check_risk" => "Risk config: shadow mode active, no live risk limits apply.".into(),
        "check_onboarding" => "Onboarding: check TUI or /onboarding/status endpoint.".into(),
        unknown => format!("Unknown tool: {}", unknown),
    }
}
