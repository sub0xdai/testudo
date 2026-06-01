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
use tokio::io::AsyncWriteExt;

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
    Start {
        /// Strategy name to use (optional)
        #[arg(long)]
        strategy: Option<String>,
        /// Run in daemon mode (background, socket control)
        #[arg(long)]
        daemon: bool,
    },
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
    Add {
        /// Name for the strategy
        name: String,
        /// Path to the TOML file
        #[arg(long)]
        from: String,
    },
    /// Show a strategy's details
    Show {
        /// Strategy name
        name: String,
    },
    /// Remove a strategy
    Remove {
        /// Strategy name
        name: String,
    },
    /// Validate a strategy against proof artifacts
    Validate {
        /// Strategy name
        name: String,
    },
    /// Create a new strategy interactively (wizard)
    Create,
}

impl Command {
    /// Returns a human-readable description of the command variant.
    /// Used by stub handlers to print "not yet implemented" messages.
    pub fn description(&self) -> String {
        match self {
            Command::Init => "init".into(),
            Command::Agent(action) => match action {
                AgentAction::Start { .. } => "agent start".into(),
                other => format!("agent {:?}", other).to_lowercase(),
            },
            Command::Dashboard => "dashboard".into(),
            Command::Listen => "listen".into(),
            Command::Journal => "journal".into(),
            Command::Strategy(action) => match action {
                StrategyAction::List => "strategy list".into(),
                StrategyAction::Add { .. } => "strategy add".into(),
                StrategyAction::Show { .. } => "strategy show".into(),
                StrategyAction::Remove { .. } => "strategy remove".into(),
                StrategyAction::Validate { .. } => "strategy validate".into(),
                StrategyAction::Create => "strategy create".into(),
            },
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
    strategy_name: Option<String>,
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

    // Load strategy (or use defaults)
    let config_dir = Config::config_dir();
    let registry = StrategyRegistry::new(&config_dir);

    let (system_prompt, loop_interval_secs, filtered_tools, strategy_constraints) =
        if let Some(ref name) = strategy_name {
            match registry.get(name) {
                Some(strat) => {
                    eprintln!("Loaded strategy: {} v{}", strat.meta.name, strat.meta.version);

                    let prompt = strat.prompt.system.clone();
                    let interval = strat
                        .loop_config
                        .as_ref()
                        .and_then(|l| l.interval_secs)
                        .unwrap_or(config.agent.loop_interval_secs);

                    let all_defs = all_tools();
                    let filtered: Vec<ToolDef> = if let Some(ref allowed) = strat.allowed_tools {
                        let allowed_names: std::collections::HashSet<&str> =
                            allowed.tools.iter().map(|s| s.as_str()).collect();
                        all_defs
                            .into_iter()
                            .filter(|t| allowed_names.contains(t.name.as_str()))
                            .collect()
                    } else {
                        all_defs
                    };

                    let constraints = strat.constraints.clone();
                    (prompt, interval, filtered, constraints)
                }
                None => {
                    eprintln!("Strategy '{}' not found. Available strategies:", name);
                    for meta in registry.list() {
                        eprintln!("  - {}", meta.name);
                    }
                    return Err(format!("Strategy '{}' not found", name).into());
                }
            }
        } else {
            let prompt = include_str!("../../AGENT_TRADING.md").to_string();
            let interval = config.agent.loop_interval_secs;
            let defs = all_tools();
            (prompt, interval, defs, None)
        };

    let agent_mode = if config.agent.shadow_only
        || strategy_constraints
            .as_ref()
            .and_then(|c| c.shadow_only)
            .unwrap_or(false)
    {
        AgentMode::Shadow
    } else {
        AgentMode::Live
    };

    let llm = create_client(&config.llm);
    let tool_defs: Vec<ToolDef> = filtered_tools;
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
        content: Some(system_prompt),
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
        let loop_interval = std::time::Duration::from_secs(loop_interval_secs);
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
                    let result = if tc.name == "submit_signal" {
                        // Rate limiting
                        if !state.rate_limiter.try_signal() {
                            format!(
                                "BLOCKED: Signal rate limit reached ({} per window). \
                                 Wait for the next window.",
                                state.rate_limiter.remaining()
                            )
                        } else {
                            // Idempotency
                            let key = state.idempotency.next_key();
                            let exec_result = execute_tool_locally(
                                &tc.name, &tc.arguments, &agent_mode
                            );
                            format!(
                                "{} | idempotency_key: {} | attempt: {}/{}",
                                exec_result,
                                key,
                                state.idempotency.attempt_count() + 1,
                                state.idempotency.max_retries(),
                            )
                        }
                    } else {
                        execute_tool_locally(&tc.name, &tc.arguments, &agent_mode)
                    };

                    let display = if result.len() > 100 {
                        format!("{}...", &result[..97])
                    } else {
                        result.clone()
                    };
                    eprintln!("  {}: {}", tc.name, display);

                    // Journal: log signal submissions
                    if tc.name == "submit_signal" && !result.contains("BLOCKED") {
                        let reasoning = tc.arguments["reasoning"]
                            .as_str()
                            .unwrap_or("(no reasoning)");
                        tracing::info!(
                            tool = "submit_signal",
                            idempotency_key = %state.idempotency.current_key(),
                            reasoning = reasoning,
                            "Pre-trade journal: signal submitted"
                        );
                    }

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

// ── Strategy command handlers ────────────────────────────────────────

use crate::strategies::registry::StrategyRegistry;
use crate::strategies::template::StrategyTemplate;
use std::path::Path;

/// `testudo strategy list` — print all available strategies.
pub fn run_strategy_list(config_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let registry = StrategyRegistry::new(config_dir);
    let strategies = registry.list();

    if strategies.is_empty() {
        println!("No strategies registered.");
        return Ok(());
    }

    println!("{:<25} {:<10} {:<50} {:<30}", "NAME", "VERSION", "DESCRIPTION", "SOURCE");
    println!("{:-<115}", "");

    for meta in &strategies {
        let source = if registry.get(&meta.name).is_some() {
            "builtin"
        } else {
            "user"
        };
        println!(
            "{:<25} {:<10} {:<50} {:<30}",
            meta.name,
            meta.version,
            if meta.description.len() > 48 {
                format!("{}...", &meta.description[..45])
            } else {
                meta.description.clone()
            },
            source
        );
    }

    Ok(())
}

/// `testudo strategy add <name> --from <path>` — register a user strategy.
pub fn run_strategy_add(
    config_dir: &Path,
    name: &str,
    from: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(from)?;
    let registry = StrategyRegistry::new(config_dir);
    registry.add(name, &content)?;
    println!("Strategy '{}' registered successfully.", name);
    Ok(())
}

/// `testudo strategy show <name>` — print strategy details.
pub fn run_strategy_show(
    config_dir: &Path,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let registry = StrategyRegistry::new(config_dir);
    let tmpl = registry
        .get(name)
        .ok_or_else(|| format!("Strategy '{}' not found.", name))?;

    println!("Name:        {}", tmpl.meta.name);
    println!("Version:     {}", tmpl.meta.version);
    println!("Description: {}", tmpl.meta.description);
    println!();
    println!("── System Prompt ──────────────────────────────");
    println!("{}", tmpl.prompt.system);

    if let Some(ref constraints) = tmpl.constraints {
        println!("── Constraints ─────────────────────────────────");
        if let Some(lev) = constraints.max_leverage {
            println!("  Max leverage: {}×", lev);
        }
        if let Some(ref symbols) = constraints.allowed_symbols {
            println!("  Allowed symbols: {}", symbols.join(", "));
        }
    }

    if let Some(ref tools) = tmpl.allowed_tools {
        println!("── Allowed Tools ───────────────────────────────");
        for tool in &tools.tools {
            println!("  - {}", tool);
        }
    }

    Ok(())
}

/// `testudo init` — guided onboarding wizard.
///
/// Walks the user through 5 steps to configure the harness from scratch.
/// Uses stdin/stdout terminal prompts (not TUI) for CLI-04.
/// TUI upgrade deferred to CLI-05.
pub fn run_init(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════╗");
    println!("║        Testudo Harness — First Setup         ║");
    println!("╚══════════════════════════════════════════════╝");
    println!();

    let mut base_url = config.api.base_url.clone();
    let mut agent_key = config.api.agent_key.clone();
    let mut llm_provider = config.llm.provider.clone();
    let mut llm_api_key = config.llm.api_key.clone();
    let mut llm_model = config.llm.model.clone();
    let mut llm_base_url: Option<String> = config.llm.base_url.clone();
    let mut leverage: u8 = 5;
    let mut account_risk_pct: f64 = 2.0;
    let mut drawdown_pct: f64 = 20.0;

    // Step 1: Base URL
    println!("── Step 1/6: Backend URL ──────────────────────");
    println!("Just press Enter — the default is correct unless you're");
    println!("running your own Testudo server.");
    println!();
    print!("Backend URL [{}] ", base_url);
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();
    if !trimmed.is_empty() {
        if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
            eprintln!("Warning: URL should start with http:// or https://");
        }
        base_url = trimmed.to_string();
    }
    println!();

    // Step 2: Agent Key
    println!("── Step 2/6: Agent Key ────────────────────────");
    println!("An agent key is a scoped API key that lets the harness submit");
    println!("signals and read your journal on your behalf.");
    println!("Create one in the Testudo web desk → Settings → Agent Keys.");
    println!("It should look like: testudo_sk_...");
    println!();
    println!("Paste your agent key:");
    if !agent_key.is_empty() {
        let masked = format!("{}...{}", &agent_key[..12], &agent_key[agent_key.len()-4..]);
        print!("[{}] ", masked);
    }
    input.clear();
    std::io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();
    if !trimmed.is_empty() {
        if !trimmed.starts_with("testudo_sk_") {
            eprintln!("Warning: agent keys should start with 'testudo_sk_'");
        }
        agent_key = trimmed.to_string();
    }
    println!();

    // Step 3: Exchange
    println!("── Step 3/6: Exchange ─────────────────────────");
    println!("Before trading, you need to connect an exchange account.");
    println!("This is done through the Testudo web desk (not the CLI).");
    println!("Visit your desk → Exchanges → Connect to add one.");
    println!("Supported exchanges: Binance, Bybit, Hyperliquid.");
    println!("(Press Enter to continue)");
    input.clear();
    std::io::stdin().read_line(&mut input)?;
    println!();

    // Step 4: LLM Configuration
    println!("── Step 4/6: LLM Configuration ────────────────");
    println!("The agent loop needs an LLM to think. Pick your provider:");
    println!();
    println!("   1. Anthropic (Claude)          api.anthropic.com");
    println!("   2. OpenAI (GPT)                api.openai.com");
    println!("   3. DeepSeek                    api.deepseek.com");
    println!("   4. Groq                        api.groq.com");
    println!("   5. Together AI                 api.together.xyz");
    println!("   6. xAI (Grok)                  api.x.ai");
    println!("   7. Mistral                     api.mistral.ai");
    println!("   8. OpenRouter                  openrouter.ai");
    println!("   9. Qwen (Alibaba)              dashscope.aliyuncs.com");
    println!("  10. Google (Gemini)             generativelanguage.googleapis.com");
    println!("  11. Ollama (local)              localhost:11434");
    println!("  12. Custom (enter base URL)     any OpenAI-compatible endpoint");
    println!();

    let provider_idx = if llm_provider == "anthropic" { 1 }
        else if llm_provider == "openai" { 2 }
        else if llm_provider == "deepseek" { 3 }
        else if llm_provider == "groq" { 4 }
        else if llm_provider == "together" { 5 }
        else if llm_provider == "xai" { 6 }
        else if llm_provider == "mistral" { 7 }
        else if llm_provider == "openrouter" { 8 }
        else if llm_provider == "qwen" { 9 }
        else if llm_provider == "gemini" { 10 }
        else if llm_provider == "ollama" { 11 }
        else { 1 };

    print!("Provider [{}]: ", provider_idx);
    input.clear();
    std::io::stdin().read_line(&mut input)?;
    let choice = input.trim().to_string();
    let choice_str = choice.as_str();

    let chosen = if choice_str.is_empty() {
        // Keep existing or default to anthropic
        let (provider, model, url) = match llm_provider.as_str() {
            "openai" => ("openai", "gpt-4o", "https://api.openai.com/v1"),
            "deepseek" => ("deepseek", "deepseek-chat", "https://api.deepseek.com/v1"),
            "groq" => ("groq", "llama-3.3-70b-versatile", "https://api.groq.com/openai/v1"),
            "together" => ("together", "meta-llama/Llama-3.3-70B-Instruct-Turbo", "https://api.together.xyz/v1"),
            "xai" => ("xai", "grok-2", "https://api.x.ai/v1"),
            "mistral" => ("mistral", "mistral-large-latest", "https://api.mistral.ai/v1"),
            "openrouter" => ("openrouter", "anthropic/claude-sonnet-4", "https://openrouter.ai/api/v1"),
            "qwen" => ("qwen", "qwen-max", "https://dashscope.aliyuncs.com/compatible-mode/v1"),
            "gemini" => ("gemini", "gemini-2.5-flash", "https://generativelanguage.googleapis.com/v1beta/models"),
            "ollama" => ("ollama", "llama3", "http://localhost:11434/v1"),
            _ => ("anthropic", "claude-sonnet-4-20250514", "https://api.anthropic.com"),
        };
        (provider.to_string(), model.to_string(), url.to_string())
    } else if choice_str == "12" {
        // Custom
        print!("Base URL (must end with /v1 for OpenAI-compatible): ");
        input.clear();
        std::io::stdin().read_line(&mut input)?;
        let custom_url = input.trim().to_string();
        print!("Model: ");
        input.clear();
        std::io::stdin().read_line(&mut input)?;
        let custom_model = input.trim().to_string();
        llm_base_url = Some(custom_url.clone());
        llm_model = custom_model.clone();
        ("openai".to_string(), custom_model, custom_url)
    } else {
        match choice_str {
            "1" => ("anthropic".to_string(), "claude-sonnet-4-20250514".to_string(), "https://api.anthropic.com".to_string()),
            "2" => ("openai".to_string(), "gpt-4o".to_string(), "https://api.openai.com/v1".to_string()),
            "3" => ("deepseek".to_string(), "deepseek-chat".to_string(), "https://api.deepseek.com/v1".to_string()),
            "4" => ("groq".to_string(), "llama-3.3-70b-versatile".to_string(), "https://api.groq.com/openai/v1".to_string()),
            "5" => ("together".to_string(), "meta-llama/Llama-3.3-70B-Instruct-Turbo".to_string(), "https://api.together.xyz/v1".to_string()),
            "6" => ("xai".to_string(), "grok-2".to_string(), "https://api.x.ai/v1".to_string()),
            "7" => ("mistral".to_string(), "mistral-large-latest".to_string(), "https://api.mistral.ai/v1".to_string()),
            "8" => ("openrouter".to_string(), "anthropic/claude-sonnet-4".to_string(), "https://openrouter.ai/api/v1".to_string()),
            "9" => ("qwen".to_string(), "qwen-max".to_string(), "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string()),
            "10" => ("gemini".to_string(), "gemini-2.5-flash".to_string(), "https://generativelanguage.googleapis.com/v1beta/models".to_string()),
            "11" => ("ollama".to_string(), "llama3".to_string(), "http://localhost:11434/v1".to_string()),
            _ => {
                eprintln!("Invalid choice. Defaulting to Anthropic.");
                ("anthropic".to_string(), "claude-sonnet-4-20250514".to_string(), "https://api.anthropic.com".to_string())
            }
        }
    };

    llm_provider = chosen.0;
    if choice_str != "12" && !choice_str.is_empty() {
        llm_model = chosen.1;
    }
    println!();

    // API Key
    println!("API key for {}:", llm_provider);
    print!("API key [{}] ", if llm_api_key.is_empty() { "(required)" } else { "****" });
    input.clear();
    std::io::stdin().read_line(&mut input)?;
    let trimmed_key = input.trim();
    if !trimmed_key.is_empty() {
        llm_api_key = trimmed_key.to_string();
    }
    if llm_api_key.is_empty() {
        eprintln!("Warning: No API key set. The agent loop won't work until you add one.");
    }

    // Model
    print!("Model [{}]: ", llm_model);
    input.clear();
    std::io::stdin().read_line(&mut input)?;
    let trimmed_model = input.trim();
    if !trimmed_model.is_empty() {
        llm_model = trimmed_model.to_string();
    }
    println!();

    // Step 5: Risk Config
    println!("── Step 5/6: Risk Configuration ───────────────");
    println!("Set your personal risk limits. These act as a safety net:");
    println!("the harness can only tighten these — never loosen them.");
    println!();

    print!("Max leverage [{}]: ", leverage);
    input.clear();
    std::io::stdin().read_line(&mut input)?;
    if let Ok(val) = input.trim().parse::<u8>() {
        if val > 0 && val <= 125 {
            leverage = val;
        } else {
            eprintln!("Leverage must be 1-125. Keeping default.");
        }
    }

    print!("Account risk per trade % [{:.1}]: ", account_risk_pct);
    input.clear();
    std::io::stdin().read_line(&mut input)?;
    if let Ok(val) = input.trim().parse::<f64>() {
        if val > 0.0 && val <= 100.0 {
            account_risk_pct = val;
        } else {
            eprintln!("Risk must be 0.1-100%. Keeping default.");
        }
    }

    print!("Max drawdown % [{:.1}]: ", drawdown_pct);
    input.clear();
    std::io::stdin().read_line(&mut input)?;
    if let Ok(val) = input.trim().parse::<f64>() {
        if val > 0.0 && val <= 100.0 {
            drawdown_pct = val;
        } else {
            eprintln!("Drawdown must be 0.1-100%. Keeping default.");
        }
    }
    println!();

    // Step 6: Save
    println!("── Step 6/6: Save Configuration ───────────────");
    println!();
    println!("  Backend URL:     {}", base_url);
    println!("  Agent Key:       {}", if agent_key.is_empty() { "(not set)" } else { "****" });
    println!("  LLM Provider:    {}", llm_provider);
    println!("  LLM Model:       {}", llm_model);
    if let Some(ref url) = llm_base_url {
        println!("  LLM Base URL:    {}", url);
    }
    println!("  Max Leverage:    {}×", leverage);
    println!("  Risk/Trade:      {:.1}%", account_risk_pct);
    println!("  Max Drawdown:    {:.1}%", drawdown_pct);
    println!();

    print!("Save to ~/.config/testudo/config.toml? [Y/n] ");
    input.clear();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().eq_ignore_ascii_case("n") {
        println!("Aborted. Config not saved.");
        return Ok(());
    }

    // Build and save config
    let new_config = Config {
        api: crate::config::ApiConfig {
            base_url,
            agent_key,
            ws_url: config.api.ws_url.clone(),
        },
        llm: crate::config::LlmConfig {
            provider: llm_provider,
            api_key: llm_api_key,
            model: llm_model,
            base_url: llm_base_url,
        },
        ..Config::default()
    };

    let config_dir = Config::config_dir();
    let config_path = config_dir.join("config.toml");
    let tmp_path = config_dir.join("config.toml.tmp");

    std::fs::create_dir_all(&config_dir)?;
    let toml_str = toml::to_string_pretty(&new_config)?;
    std::fs::write(&tmp_path, &toml_str)?;

    // Atomic rename
    std::fs::rename(&tmp_path, &config_path)?;

    println!();
    println!("✅ Configuration saved to {}", config_path.display());
    println!();
    println!("Next steps:");
    println!("  testudo agent start          Start autonomous trading");
    println!("  testudo journal              View trading summary");
    println!("  testudo strategy list        Browse strategies");
    println!("  testudo dashboard            Open live TUI");

    Ok(())
}

/// `testudo attach` — connect to a running daemon and show live status.
pub fn run_attach() -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = crate::daemon::socket_path();

    if !socket_path.exists() {
        return Err(
            "No daemon socket found. Start the daemon with:\n  \
             testudo agent start --daemon"
                .into(),
        );
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let stream = tokio::net::UnixStream::connect(&socket_path).await?;
        let (reader, mut writer) = stream.into_split();

        // Send status request
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"status"}"#;
        writer.write_all(request.as_bytes()).await?;
        writer.write_all(b"\n").await?;

        // Read response
        use tokio::io::{AsyncBufReadExt, BufReader};
        let mut lines = BufReader::new(reader).lines();
        if let Ok(Some(line)) = lines.next_line().await {
            let resp: serde_json::Value =
                serde_json::from_str(&line).unwrap_or_default();

            if let Some(result) = resp.get("result") {
                println!("╔══════════════════════════════════════╗");
                println!("║        Daemon Status                  ║");
                println!("╚══════════════════════════════════════╝");
                println!();
                println!(
                    "  Phase:        {}",
                    result["phase"].as_str().unwrap_or("unknown")
                );
                println!(
                    "  Signals:      {}",
                    result["signal_count"].as_u64().unwrap_or(0)
                );
                println!(
                    "  Uptime:       {}s",
                    result["uptime_secs"].as_u64().unwrap_or(0)
                );
                if let Some(err) = result["last_error"].as_str() {
                    println!("  Last error:   {}", err);
                }
                println!();
                println!("Run 'testudo attach' for live TUI (coming in CP-3).");
            } else if let Some(err) = resp.get("error") {
                eprintln!("Daemon error: {}", err);
            }
        }

        Ok(())
    })
}

/// `testudo strategy validate <name>` — validate strategy against proof artifacts.
pub fn run_strategy_validate(
    config_dir: &Path,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let registry = StrategyRegistry::new(config_dir);
    let tmpl = registry
        .get(name)
        .ok_or_else(|| format!("Strategy '{}' not found.", name))?;

    // Load proof artifacts
    let proofs_dir = crate::strategies::loader::StrategyLoader::proofs_dir();
    let loader = crate::strategies::loader::StrategyLoader::new(proofs_dir);
    let artifacts = loader.load_all().unwrap_or_default();

    // Validate
    let result =
        crate::strategies::validator::StrategyValidator::validate(&tmpl, &artifacts);

    println!("Strategy: {} v{}", tmpl.meta.name, tmpl.meta.version);
    println!("Description: {}", tmpl.meta.description);
    println!();

    if tmpl.required_proofs.is_empty() {
        println!("No proof artifacts required.");
    } else {
        println!("Required proofs:");
        for proof in &tmpl.required_proofs {
            let status = if artifacts.contains_key(proof) {
                "✓ loaded"
            } else {
                "✗ missing"
            };
            println!("  {} {}", status, proof);
        }
    }

    // Show constraints from loaded artifacts
    if !artifacts.is_empty() {
        let mut cs = crate::strategies::constraints::ConstraintSet::defaults();
        for (name, artifact) in &artifacts {
            for (key, value) in &artifact.constraints {
                cs.apply_toml_constraint(name, key, value);
            }
        }

        println!();
        println!("Proof-backed constraints:");
        println!("  Max leverage:       {}×", cs.max_leverage as u64);
        println!("  Account risk/trade: {:.1}%", cs.max_account_risk_pct);
        println!("  Max drawdown:       {:.1}%", cs.max_drawdown_pct);
        println!("  Stop loss required: {}", cs.stop_loss_required);
    }

    println!();
    if result.valid {
        println!("✅ Strategy is valid.");
    } else {
        println!("❌ Strategy has {} error(s):", result.errors.len());
        for error in &result.errors {
            println!("  - {}", error);
        }
    }

    for warning in &result.warnings {
        println!("⚠  {}", warning);
    }

    Ok(())
}

/// `testudo strategy remove <name>` — remove a user strategy.
pub fn run_strategy_remove(
    config_dir: &Path,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let registry = StrategyRegistry::new(config_dir);
    registry.remove(name)?;
    println!("Strategy '{}' removed.", name);
    Ok(())
}

/// `testudo strategy create` — interactive wizard for building a strategy TOML.
///
/// For tests, the wizard can be driven programmatically via direct parameters.
/// When `name` is non-empty, it runs in test mode (no stdin prompts).
pub fn run_strategy_create(
    config_dir: &Path,
    name: &str,
    description: &str,
    leverage: u8,
    symbols: &[&str],
    system_prompt: &str,
    tools: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate name
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        || name.starts_with('-')
        || name.ends_with('-')
    {
        return Err("Strategy name must be kebab-case (lowercase, hyphens, digits).".into());
    }

    // Validate prompt
    let prompt = system_prompt.trim();
    if prompt.is_empty() {
        return Err("System prompt must not be empty.".into());
    }

    // Validate symbols
    if symbols.is_empty() {
        return Err("At least one trading symbol is required.".into());
    }

    // Build the TOML
    let symbols_toml = symbols
        .iter()
        .map(|s| format!("\"{}\"", s))
        .collect::<Vec<_>>()
        .join(", ");

    let tools_toml = tools
        .iter()
        .map(|s| format!("\"{}\"", s))
        .collect::<Vec<_>>()
        .join(", ");

    let toml_content = format!(
        r#"# {} — {}
#
# Auto-generated by `testudo strategy create`.

[meta]
name = "{}"
version = "0.1.0"
description = "{}"

[loop]
interval_secs = 120
shadow_only = true
max_signals_per_hour = 3

[prompt]
system = """
{}"""

[constraints]
max_leverage = {}
allowed_symbols = [{}]

[allowed_tools]
tools = [{}]
"#,
        name,
        description,
        name,
        description,
        prompt,
        leverage,
        symbols_toml,
        tools_toml,
    );

    // Validate parses correctly
    if toml::from_str::<StrategyTemplate>(&toml_content).is_err() {
        return Err("Generated TOML failed to parse — this is a bug.".into());
    }

    // Save via registry (which handles collision check)
    let registry = StrategyRegistry::new(config_dir);
    registry.add(name, &toml_content)?;

    println!("✅ Strategy '{}' created and saved.", name);
    println!("   → testudo strategy validate {}", name);
    println!("   → testudo agent start --strategy {}", name);

    Ok(())
}
