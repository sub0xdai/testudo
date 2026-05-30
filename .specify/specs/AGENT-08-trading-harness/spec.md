# Specification: Trading Agent Harness — Rust TEA-based TUI for Autonomous Trading

**Spec ID:** AGENT-08-trading-harness
**Date:** 2026-05-30
**Status:** Draft
**Class:** Feature / Application
**Priority:** P1 — completes the agent UX; users get a single binary that onboards, trades, journals, and reports
**Depends on:** AGENT-06-onboarding-status, AGENT-07-agent-api-keys, AGENT-01-signal-endpoint, AGENT-02-websocket-alerts, AGENT-03-journal-memory
**Series:** AGENT-08 (Trading Harness)

---

## Problem Statement

pi.dev provides an agentic harness for coding: tools (file I/O, shell), an LLM loop, a TUI, context management, skills, and safety guards. It transforms a general-purpose LLM into a competent coding agent. Testudo has the backend infrastructure for trading — signals, WebSocket alerts, journal memory, risk engine — but the agent experience is fragmented. Today:

1. An external LLM (Hermes on n0x, pi, OpenClaw) reads `AGENT_TRADING.md` and makes raw HTTP calls.
2. The LLM manages its own token lifecycle, context window, and tool definitions.
3. Onboarding requires the LLM to interpret multi-step discovery responses.
4. WebSocket alerts require the LLM to maintain a persistent connection and parse streaming events.
5. There is no native TUI — no live P&L dashboard, no position monitor, no strategy registry, no signal log.

Every agent runtime that connects to Testudo reinvents the same scaffolding. And users interacting with Testudo directly (not through an agent) have no terminal-first experience at all — they must use the browser journal or write raw `curl` commands.

The solution: a purpose-built trading harness in Rust — the same relationship pi has to coding, but for trading. A single binary (`tudo`) that onboards, connects exchanges, runs strategies, monitors risk, journals results, and provides a live TUI dashboard.

---

## User Stories

- **As a developer**, I want to run `tudo init` and be trading in under 5 minutes, so that I don't need to read API docs or write agent scaffolding.
- **As a trader**, I want a live TUI showing P&L, open positions, recent signals, and risk alerts, so that I can monitor my agents in real time without a browser.
- **As a strategy developer**, I want to register a strategy as a prompt template and let the harness execute it in a loop, so that I can deploy strategies without writing agent infrastructure.
- **As a risk-conscious user**, I want the harness to enforce client-side risk checks before even hitting the API, so that I don't waste LLM calls or API bandwidth on signals that will be rejected.
- **As an n0x operator**, I want the harness to run headless on my server (daemon mode), so that agents trade 24/7 without a terminal attached.
- **As a multi-agent operator**, I want to run multiple strategies in separate sessions with independent risk budgets, so that I can diversify approaches without cross-contamination.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `tudo init` onboards user: SIWE auth, exchange connection (CEX or HL), risk config setup. Uses AGENT-06 onboarding status to guide the flow conversationally. | High | CLI |
| FR-2 | `tudo agent start` launches the autonomous LLM loop: analyze → signal → journal → repeat. Loop interval configurable (default: 60s). | High | Agent loop |
| FR-3 | `tudo agent start --strategy momentum-breakout` loads a named strategy template as the system prompt, overriding default behavior. | High | Strategy registry |
| FR-4 | `tudo dashboard` opens the full TUI with panes: positions, P&L chart, signal log, risk alerts, LLM reasoning. Pure read-only mode — no signals sent. | High | TUI |
| FR-5 | TUI renders promptly at 60fps for keypresses and 1fps for data refreshes, never blocking on network I/O. | High | TUI |
| FR-6 | `tudo listen` subscribes to WebSocket channels (`agent.alert.*`, `agent.execution.*`) and streams events to stdout in JSON Lines format. Purely a pipe — no LLM, no TUI. | Medium | CLI |
| FR-7 | `tudo journal` fetches agent summary (LLM format), insights, and period comparison. Prints to stdout. | Medium | CLI |
| FR-8 | Harness uses `tudo_sk_...` agent keys (AGENT-07) for all API calls after initial SIWE auth. Key stored in `~/.config/tudo/credentials`. | High | Auth |
| FR-9 | LLM tool definitions are typed and validated: `fetch_klines`, `submit_signal`, `read_journal`, `write_journal_entry`, `check_risk`, `list_positions`, `check_onboarding`. Tools serialize to OpenAI-compatible function calling format. | High | Tools |
| FR-10 | Client-side risk pre-check: before calling `submit_signal`, harness validates against local risk config cache (drawdown, max positions, leverage). Rejects early with a structured error, avoiding wasted LLM + API calls. | Medium | Risk |
| FR-11 | Harness supports multiple LLM providers via config: Anthropic (Claude), OpenAI (GPT), Google (Gemini), local (Ollama). Provider config in `~/.config/tudo/config.toml`. | Medium | Providers |
| FR-12 | `tudo agent start --daemon` runs headless (no TUI), logging to `~/.config/tudo/logs/`. Exposes a Unix socket for `tudo attach` to reconnect a TUI. | Low | Daemon |
| FR-13 | `tudo strategy list` shows registered strategies. `tudo strategy add <name> --from <file>` registers a strategy prompt template. Strategies are TOML files with system prompt, default params, and allowed tool list. | Medium | Strategy registry |
| FR-14 | Signal deduplication: harness generates and tracks `Idempotency-Key` per signal. Retries on network failure with same key. | High | Agent loop |
| FR-15 | Journal write-after-signal: every signal automatically writes a pre-trade journal entry and tags the trade. Post-trade entries written on close from execution WebSocket events. | High | Agent loop |

---

## Technical Implementation

### Architecture — The Elm Architecture (TEA) in Rust

```
┌──────────────────────────────────────────────────────────┐
│                     App (tears)                           │
│                                                           │
│  ┌─────────┐    ┌──────────┐    ┌──────────────────┐    │
│  │  Model  │◄───│  Update  │◄───│    Messages       │    │
│  │         │    │          │    │                   │    │
│  │ State   │    │ (Model,  │    │ KeyPress          │    │
│  │ Tree    │    │  Message)│    │ PriceTick         │    │
│  └────┬────┘    │  → Model │    │ AlertReceived     │    │
│       │         │  → Cmd   │    │ LlmToken          │    │
│       │         └──────────┘    │ SignalResult      │    │
│       │                         │ WebSocketEvent    │    │
│       ▼                         │ TimerTick         │    │
│  ┌─────────┐                    └────────┬─────────┘    │
│  │  View   │                             │               │
│  │         │                    ┌────────┴─────────┐    │
│  │ ratatui │                    │   Event Sources   │    │
│  │ Widgets │                    │                   │    │
│  └─────────┘                    │ crossterm (keys)  │    │
│                                 │ tokio (timers)    │    │
│                                 │ ws_stream (ticks) │    │
│                                 │ llm_stream (AI)   │    │
│                                 └───────────────────┘    │
│                                                           │
│  ┌──────────────────────────────────────────────────┐    │
│  │                  Commands                         │    │
│  │                                                   │    │
│  │  Cmd::FetchKlines       → GET /klines             │    │
│  │  Cmd::SubmitSignal      → POST /signals           │    │
│  │  Cmd::ReadJournal       → GET /journal/agent/...  │    │
│  │  Cmd::WriteJournal      → POST /journal/entries   │    │
│  │  Cmd::CheckOnboarding   → GET /onboarding/status  │    │
│  │  Cmd::CallLlm           → Anthropic/OpenAI/etc.   │    │
│  │  Cmd::SubscribeWs       → agent.alert.* / exec.*  │    │
│  │  Cmd::Sleep(duration)   → tokio::time::sleep      │    │
│  └──────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────┘
```

### Crate Structure

```
tudo/
├── Cargo.toml
├── src/
│   ├── main.rs                  // clap CLI + app startup
│   ├── app.rs                   // TEA App: Model + Update + View wiring
│   │
│   ├── model/
│   │   ├── mod.rs
│   │   ├── state.rs             // AppState: top-level model
│   │   ├── session.rs           // Session: auth, accounts, risk config
│   │   ├── positions.rs         // PositionMap: live position tracking
│   │   ├── journal.rs           // JournalCache: recent summary, insights
│   │   ├── market.rs            // MarketData: klines, tickers, orderbook snapshots
│   │   └── agent.rs             // AgentState: current LLM session, tool calls, loop phase
│   │
│   ├── msg.rs                   // Message enum — all events
│   ├── update.rs                // Update function — pure state transitions
│   ├── view/
│   │   ├── mod.rs
│   │   ├── dashboard.rs         // Full dashboard layout
│   │   ├── positions_pane.rs    // Open positions table
│   │   ├── pnl_chart.rs         // Equity curve (sparkline)
│   │   ├── signal_log.rs        // Recent signals with status
│   │   ├── risk_pane.rs         // Drawdown gauge, risk limits
│   │   ├── agent_pane.rs        // LLM reasoning stream
│   │   ├── journal_pane.rs      // Journal summary
│   │   └── status_bar.rs        // Bottom bar: exchange, mode, uptime
│   │
│   ├── cmd.rs                   // Command enum — async side effects
│   │
│   ├── tools/
│   │   ├── mod.rs
│   │   ├── fetch_klines.rs      // GET /klines → structured OHLCV
│   │   ├── submit_signal.rs     // POST /signals with idempotency
│   │   ├── read_journal.rs      // GET /journal/agent/summary?format=llm
│   │   ├── write_journal.rs     // POST /journal/entries
│   │   ├── list_positions.rs    // GET /positions
│   │   ├── check_risk.rs        // Local risk pre-check
│   │   ├── check_onboarding.rs  // GET /onboarding/status
│   │   └── types.rs             // ToolInput/ToolOutput, OpenAIFunctionDef
│   │
│   ├── llm/
│   │   ├── mod.rs
│   │   ├── client.rs            // LlmClient trait
│   │   ├── anthropic.rs         // Anthropic provider
│   │   ├── openai.rs            // OpenAI provider
│   │   ├── gemini.rs            // Google provider
│   │   ├── ollama.rs            // Local provider
│   │   └── stream.rs            // Token stream → Message::LlmToken
│   │
│   ├── api/
│   │   ├── mod.rs
│   │   ├── client.rs            // Reqwest client with auth header injection
│   │   ├── signals.rs           // POST /signals
│   │   ├── journal.rs           // GET/POST journal endpoints
│   │   ├── klines.rs            // GET /klines
│   │   ├── exchanges.rs         // GET /exchanges, POST /exchanges/accounts
│   │   ├── onboarding.rs        // GET /onboarding/status
│   │   ├── risk.rs              // GET/PUT /risk-config
│   │   └── agent_keys.rs        // POST/GET/DELETE /agent-keys (AGENT-07)
│   │
│   ├── ws/
│   │   ├── mod.rs
│   │   ├── client.rs            // WebSocket connection (tokio-tungstenite)
│   │   └── stream.rs            // Event stream → Message::WebSocketEvent
│   │
│   ├── strategies/
│   │   ├── mod.rs
│   │   ├── registry.rs          // StrategyRegistry: load/store/validate
│   │   ├── template.rs          // StrategyTemplate: system prompt + params + tools
│   │   └── builtins/
│   │       ├── mean_reversion.toml
│   │       ├── momentum_breakout.toml
│   │       └── funding_arb.toml
│   │
│   ├── risk/
│   │   ├── mod.rs
│   │   └── precheck.rs          // Client-side risk validation
│   │
│   ├── config.rs                // Config loading: ~/.config/tudo/config.toml
│   ├── auth.rs                  // Credential storage, SIWE flow, agent key mgmt
│   └── daemon.rs                // Headless mode + Unix socket for attach
│
├── strategies/                  // User-installed strategies
│   └── .gitkeep
│
└── tests/
    ├── integration/
    │   ├── tools.rs
    │   └── loop.rs
    └── fixtures/
```

### Model

```rust
// src/model/state.rs

/// Top-level application state — the Elm Model.
pub struct AppState {
    /// Which screen is active.
    pub screen: Screen,

    /// Auth + exchange + risk state.
    pub session: Session,

    /// Live position tracking.
    pub positions: PositionMap,

    /// Cached journal data (summary, insights).
    pub journal: JournalCache,

    /// Market data (klines, ticker snapshots).
    pub market: MarketData,

    /// Agent state (only populated when agent is running).
    pub agent: Option<AgentState>,

    /// Event log (signals, alerts, errors) — ring buffer.
    pub event_log: RingBuffer<EventLogEntry>,

    /// Global error state. If set, displayed as banner.
    pub error: Option<String>,

    /// Status bar info.
    pub status: StatusBar,
}

pub enum Screen {
    Dashboard,
    Onboarding(OnboardingStep),
    Journal,
    StrategyList,
    Config,
    Help,
}
```

```rust
// src/model/agent.rs

/// State of the currently running agent.
pub struct AgentState {
    /// Current phase in the autonomous loop.
    pub phase: AgentPhase,

    /// The strategy being executed (if any).
    pub strategy: Option<StrategyHandle>,

    /// LLM conversation history.
    pub messages: Vec<LlmMessage>,

    /// Pending tool calls waiting for execution.
    pub pending_tool_calls: Vec<PendingToolCall>,

    /// Recent signal results.
    pub recent_signals: Vec<SignalResult>,

    /// LLM streaming state.
    pub stream: Option<LlmStreamState>,

    /// Loop timer state.
    pub loop_config: LoopConfig,

    /// Agent mode.
    pub mode: AgentMode,
}

pub enum AgentPhase {
    /// Reading journal + market data before deciding.
    Observing,
    /// LLM is streaming a response (may contain tool calls or text).
    Thinking { tokens_received: usize },
    /// Executing tool calls (fetching data, submitting signals).
    Acting,
    /// Waiting for the next loop iteration.
    Idle,
}

pub enum AgentMode {
    Shadow,
    Live,
}

pub struct LoopConfig {
    pub interval_secs: u64,
    pub shadow_only: bool,
    pub max_signals_per_hour: u32,
}
```

### Messages

```rust
// src/msg.rs

/// Every event in the system.
#[derive(Debug)]
pub enum Message {
    // ── User input ──
    KeyPress(KeyEvent),
    Resize(u16, u16),

    // ── Timer ──
    Tick,                         // 1Hz heartbeat for UI refresh + loop stepping

    // ── Market data (WebSocket) ──
    PriceTick(TickerUpdate),
    KlineUpdate(Kline),

    // ── Risk alerts (WebSocket) ──
    AlertReceived(AgentAlert),

    // ── Execution reports (WebSocket) ──
    ExecutionReport(ExecutionReport),

    // ── LLM stream ──
    LlmToken(String),             // Streaming token from LLM
    LlmDone(Result<LlmResponse>), // LLM response complete (may contain tool calls)
    LlmError(String),

    // ── API results ──
    KlinesFetched(Result<Vec<Kline>>),
    SignalSubmitted(Result<SignalResult>),
    JournalFetched(Result<AgentSummary>),
    JournalWritten(Result<()>),
    OnboardingChecked(Result<OnboardingStatus>),
    PositionsFetched(Result<Vec<Position>>),
    RiskConfigFetched(Result<RiskConfigSummary>),

    // ── Agent lifecycle ──
    AgentStart,
    AgentStop,
    AgentPause,
    AgentResume,

    // ── Navigation ──
    SwitchScreen(Screen),
    ShowHelp,
    Quit,
}
```

### Commands

```rust
// src/cmd.rs

/// Side effects. Tears executes these asynchronously.
/// Each Cmd ultimately produces zero or more Messages.
pub enum Cmd {
    /// Fetch OHLCV data for a symbol.
    FetchKlines { symbol: String, interval: String, limit: u32 },

    /// Submit a trade signal.
    SubmitSignal { input: SignalInput, idempotency_key: Uuid },

    /// Read agent journal summary.
    ReadJournal { format: SummaryFormat, timeframe: String },

    /// Write a journal entry.
    WriteJournal { entry: JournalEntry },

    /// Check onboarding status.
    CheckOnboarding,

    /// Fetch open positions.
    FetchPositions,

    /// Fetch risk config.
    FetchRiskConfig,

    /// Call the LLM with current conversation.
    CallLlm { messages: Vec<LlmMessage>, tools: Vec<ToolDef> },

    /// Subscribe to WebSocket channels.
    SubscribeWs { channels: Vec<String> },

    /// Schedule a delayed message.
    Sleep(Duration, Box<Message>),

    /// Execute a batch of commands concurrently.
    Batch(Vec<Cmd>),

    /// No side effect. Used for pure state transitions.
    None,
}
```

### Tools

```rust
// src/tools/types.rs

/// A tool exposed to the LLM via OpenAI-compatible function calling.
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,  // JSON Schema
}

/// Result of executing a tool.
pub struct ToolResult {
    pub call_id: String,
    pub name: String,
    pub content: String,  // JSON string
}

/// All tools the harness provides.
pub fn all_tools() -> Vec<ToolDef> {
    vec![
        fetch_klines::tool_def(),
        submit_signal::tool_def(),
        read_journal::tool_def(),
        write_journal::tool_def(),
        list_positions::tool_def(),
        check_risk::tool_def(),
        check_onboarding::tool_def(),
    ]
}
```

```rust
// src/tools/fetch_klines.rs

pub fn tool_def() -> ToolDef {
    ToolDef {
        name: "fetch_klines".into(),
        description: "Fetch OHLCV candlestick data for a symbol. Use this to analyze price action before making trading decisions.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Trading pair, e.g. 'ETH_USDT' or 'BTC_USDT'"
                },
                "interval": {
                    "type": "string",
                    "enum": ["1m", "5m", "15m", "1h", "4h", "1d"],
                    "description": "Candle interval"
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 500,
                    "description": "Number of candles to fetch (max 500)"
                }
            },
            "required": ["symbol", "interval", "limit"]
        }),
    }
}

pub async fn execute(
    client: &ApiClient,
    args: FetchKlinesArgs,
) -> Result<ToolResult, ToolError> {
    let klines = client.get_klines(&args.symbol, &args.interval, args.limit).await?;

    let summary = format!(
        "Fetched {} {} candles for {}. Latest close: {}. High: {}, Low: {}.",
        klines.len(),
        args.interval,
        args.symbol,
        klines.last().map(|k| k.close).unwrap_or_default(),
        klines.iter().map(|k| k.high).fold(f64::NEG_INFINITY, f64::max),
        klines.iter().map(|k| k.low).fold(f64::INFINITY, f64::min),
    );

    Ok(ToolResult {
        call_id: args.call_id,
        name: "fetch_klines".into(),
        content: json!({
            "summary": summary,
            "candles": klines,
        }).to_string(),
    })
}
```

```rust
// src/tools/submit_signal.rs

pub fn tool_def() -> ToolDef {
    ToolDef {
        name: "submit_signal".into(),
        description: "Submit a trade signal. The harness will perform client-side risk checks before forwarding to the Testudo backend. Always include stop_loss, reasoning, and confidence. Start in SHADOW mode; switch to LIVE only after building a profitable track record.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "symbol": {"type": "string", "description": "Trading pair, e.g. 'ETH_USDT'"},
                "side": {"type": "string", "enum": ["LONG", "SHORT"]},
                "entry_price": {"type": "number"},
                "stop_loss": {"type": "number", "description": "Stop loss price. Required for risk management."},
                "take_profit": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "price": {"type": "number"},
                            "quantity": {"type": "number", "description": "Fraction of position to close (0.0–1.0)"}
                        }
                    }
                },
                "leverage": {"type": "integer", "minimum": 1, "maximum": 20},
                "execution_mode": {"type": "string", "enum": ["SHADOW", "LIVE"], "description": "Start with SHADOW. Switch to LIVE only after profitable paper trading."},
                "reasoning": {"type": "string", "description": "Why this trade? Include setup, confirmation signals, and risk assessment."},
                "confidence": {"type": "number", "minimum": 0, "maximum": 1, "description": "0.0–1.0. Be honest. Used for Kelly sizing."},
                "source": {"type": "string", "description": "Agent identifier, e.g. 'agent:mean-reversion:v1'"}
            },
            "required": ["symbol", "side", "entry_price", "stop_loss", "execution_mode", "reasoning", "confidence", "source"]
        }),
    }
}

pub async fn execute(
    client: &ApiClient,
    risk_cache: &RiskConfigCache,
    args: SubmitSignalArgs,
) -> Result<ToolResult, ToolError> {
    // ── Client-side risk pre-check ──
    let precheck = risk::precheck::validate_signal(&args, risk_cache);
    if !precheck.passed {
        return Ok(ToolResult {
            call_id: args.call_id,
            name: "submit_signal".into(),
            content: json!({
                "status": "rejected_client_side",
                "reason": precheck.reason,
                "suggestion": precheck.suggestion,
            }).to_string(),
        });
    }

    let idempotency_key = Uuid::new_v4();
    let result = client.submit_signal(&args.into_signal_input(), idempotency_key).await?;

    Ok(ToolResult {
        call_id: args.call_id,
        name: "submit_signal".into(),
        content: json!(result).to_string(),
    })
}
```

### Strategy Registry

```rust
// src/strategies/template.rs

/// A strategy is a prompt template + constraints loaded from TOML.
pub struct StrategyTemplate {
    pub name: String,
    pub version: String,
    pub description: String,

    /// System prompt injected at the start of every agent session.
    pub system_prompt: String,

    /// Default loop configuration.
    pub loop_config: LoopConfig,

    /// Allowed tools. If empty, all tools are available.
    pub allowed_tools: Vec<String>,

    /// Strategy parameters exposed to the LLM in the system prompt.
    pub parameters: HashMap<String, StrategyParam>,

    /// Risk constraints specific to this strategy.
    pub constraints: StrategyConstraints,
}

pub struct StrategyConstraints {
    pub max_leverage: Option<u8>,
    pub max_position_notional: Option<Decimal>,
    pub allowed_symbols: Option<Vec<String>>,
    pub shadow_only: bool,
}
```

```toml
# strategies/builtins/mean_reversion.toml

[meta]
name = "mean-reversion"
version = "1.0.0"
description = "Trades price deviations from a rolling SMA using Bollinger Bands."

[loop]
interval_secs = 60
shadow_only = true
max_signals_per_hour = 30

[prompt]
system = """
You are a disciplined mean-reversion trader.

## Strategy
- Compute Bollinger Bands: 20-period SMA ± 2σ.
- When price crosses below the lower band: enter LONG. Target: SMA midpoint.
- When price crosses above the upper band: enter SHORT. Target: SMA midpoint.
- Confirm with RSI(14): only enter if RSI < 30 (LONG) or RSI > 70 (SHORT).
- Stop loss: 2× ATR(14) from entry.
- Confidence: based on deviation magnitude and RSI extreme.

## Rules
- Always use SHADOW mode unless explicitly told otherwise.
- Never exceed 1 active position per symbol.
- If a coach warning is active for sizing_drift or frequency_spike, skip the trade.
- Write a pre-trade journal entry after every signal. Tag the trade with "mean-reversion".
- Every 6 hours, read the journal summary and note which setups are working.

## Risk
- Account risk per trade: 2% (handled by Testudo's sizing engine).
- Max leverage: 3×.
"""

[parameters]
lookback_periods = { type = "int", default = "20", description = "SMA period" }
std_dev_multiplier = { type = "float", default = "2.0", description = "Sigma multiplier for bands" }
rsi_period = { type = "int", default = "14", description = "RSI lookback" }

[constraints]
max_leverage = 3
shadow_only = true

[allowed_tools]
tools = ["fetch_klines", "submit_signal", "read_journal", "write_journal", "list_positions", "check_risk"]
```

### CLI

```rust
// src/main.rs

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tudo", about = "Testudo trading agent harness")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Authenticate and configure Testudo connection
    Init,

    /// Start the autonomous trading agent
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },

    /// Open the live TUI dashboard (read-only)
    Dashboard,

    /// Subscribe to WebSocket alerts and stream to stdout
    Listen,

    /// Fetch and display journal summary
    Journal,

    /// Manage strategies
    Strategy {
        #[command(subcommand)]
        action: StrategyAction,
    },

    /// Attach TUI to a running daemon
    Attach,
}

#[derive(Subcommand)]
pub enum AgentAction {
    /// Start the agent loop
    Start {
        /// Strategy name to use
        #[arg(short, long)]
        strategy: Option<String>,

        /// Run headless (no TUI)
        #[arg(long)]
        daemon: bool,
    },

    /// Stop a running agent
    Stop,

    /// Pause the agent loop
    Pause,

    /// Resume a paused agent
    Resume,
}

#[derive(Subcommand)]
pub enum StrategyAction {
    /// List registered strategies
    List,

    /// Add a strategy from a TOML file
    Add {
        name: String,
        #[arg(short, long)]
        from: PathBuf,
    },

    /// Show a strategy's details
    Show { name: String },

    /// Remove a strategy
    Remove { name: String },
}
```

### TUI Layout

```
┌─────────────────────────────────────────────────────────────────┐
│  Testudo Trading Harness v0.1.0    SHADOW    ETH: $3,214.50    │ ← Status bar
├───────────────────────────┬─────────────────────────────────────┤
│                           │                                     │
│   ┌─ Positions ─────────┐ │  ┌─ Agent Reasoning ─────────────┐ │
│   │                     │ │  │                                │ │
│   │ ETH_USDT LONG       │ │  │ ETH broke above 3-day         │ │
│   │ Entry: 3,100        │ │  │ resistance at 3,080. Volume   │ │
│   │ Current: 3,214      │ │  │ 2.3× average. BTC.D dropping  │ │
│   │ P&L: +$342.00       │ │  │ from 48.2 → 47.1.             │ │
│   │ R-multiple: 1.14R   │ │  │                                │ │
│   │                     │ │  │ RSI(14): 62 — not overbought  │ │
│   │ BTC_USDT SHORT      │ │  │ yet. Watching for divergence. │ │
│   │ Entry: 89,200       │ │  │                                │ │
│   │ Current: 88,950     │ │  │ Decision: HOLD. Trail stop    │ │
│   │ P&L: +$187.50       │ │  │ at 40% of move.               │ │
│   │ R-multiple: 0.75R   │ │  │                                │ │
│   │                     │ │  └────────────────────────────────┘ │
│   └─────────────────────┘ │                                     │
│                           │  ┌─ Signal Log ──────────────────┐ │
│   ┌─ P&L Chart ─────────┐ │  │ 14:22:05  LONG  ETH $3,100   │ │
│   │                     │ │  │   ✓ filled at $3,101.50      │ │
│   │    ╱╲               │ │  │ 14:08:12  SHORT BTC $89,200  │ │
│   │   ╱  ╲    ╱╲       │ │  │   ✓ filled at $89,195.00     │ │
│   │  ╱    ╲  ╱  ╲      │ │  │ 13:45:00  — no edge detected │ │
│   │ ╱      ╲╱    ╲     │ │  │ 13:44:00  — no edge detected │ │
│   │╱               ╲    │ │  │ 13:43:00  — no edge detected │ │
│   └─────────────────────┘ │  └────────────────────────────────┘ │
│                           │                                     │
│   ┌─ Journal Summary ───┐ │  ┌─ Risk ───────────────────────┐ │
│   │ 30d: 42 trades      │ │  │ Drawdown: ████░░░░ 3.2%     │ │
│   │ WR: 54.8%  PF: 1.83 │ │  │ Limit:    ░░░░░░░░ 5.0%     │ │
│   │ Avg R: 1.72          │ │  │                              │ │
│   │ Total P&L: +$2,450   │ │  │ Active: 2/5 positions       │ │
│   │ Best: breakout (61%) │ │  │ Session signals: 12/30      │ │
│   └──────────────────────┘ │  └──────────────────────────────┘ │
└───────────────────────────┴─────────────────────────────────────┘
 F1 Dashboard  F2 Journal  F3 Strategies  F4 Logs  ^C Quit   Help: ?
```

### Paved Roads

- `testudo-exchange/crates/common_utils/` — share types (`SignalInput`, `AgentAlert`, `RiskConfig`, `AgentSummary`) via a dedicated `tudo-types` crate or direct dependency. Avoids type drift between harness and backend.
- `AGENT_TRADING.md` — the harness embeds this verbatim as the default system prompt when no strategy is specified. The doc is the default agent.
- `tears` — strict TEA framework on top of `ratatui`. If `tears` is not mature enough, fall back to hand-rolled TEA loop using `ratatui` + `tokio::select!` for the event loop.
- pi.dev — reference architecture for tool system, LLM provider abstraction, skill registry. Reverse-engineer patterns, not code.
- `testudo-exchange/crates/router/src/routes/` — the harness API client mirrors each route handler's input/output types.

### Files

All in a new `tudo/` directory at project root:

- `tudo/Cargo.toml` — crate manifest
- `tudo/src/main.rs` — entry point, CLI parsing, app bootstrap
- `tudo/src/app.rs` — TEA wiring
- `tudo/src/model/*.rs` — state tree (6 files)
- `tudo/src/msg.rs` — message enum
- `tudo/src/update.rs` — pure update function
- `tudo/src/cmd.rs` — command enum + executors
- `tudo/src/view/*.rs` — TUI rendering (9 files)
- `tudo/src/tools/*.rs` — tool definitions + execution (8 files)
- `tudo/src/llm/*.rs` — LLM provider abstraction (5 files)
- `tudo/src/api/*.rs` — REST client (7 files)
- `tudo/src/ws/*.rs` — WebSocket client (2 files)
- `tudo/src/strategies/*.rs` — strategy registry (3 files)
- `tudo/src/risk/precheck.rs` — client-side risk validation
- `tudo/src/config.rs` — config loading
- `tudo/src/auth.rs` — credential management
- `tudo/src/daemon.rs` — headless mode
- `tudo/strategies/builtins/*.toml` — 3 built-in strategy templates
- `tudo/tests/integration/*.rs` — integration tests

### Dependencies

```toml
[dependencies]
# TUI
ratatui = "0.29"
crossterm = "0.28"
tears = "0.4"                  # TEA framework (or hand-rolled if not mature)

# Async
tokio = { version = "1", features = ["full"] }

# CLI
clap = { version = "4", features = ["derive"] }

# HTTP + WebSocket
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
tokio-tungstenite = "0.24"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Config
toml = "0.8"
directories = "5"             # XDG config paths

# Cryptography (for SIWE, agent key handling)
ring = "0.17"
sha2 = "0.10"
hex = "0.4"

# Observability
tracing = "0.1"
tracing-appender = "0.2"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# UUID for idempotency keys
uuid = { version = "1", features = ["v4"] }

# Shared types with backend
common-utils = { path = "../testudo-exchange/crates/common_utils" }
```

---

## Acceptance Criteria

- [ ] `tudo init` completes SIWE auth, exchange connection, and risk config in a single TUI flow
- [ ] `tudo agent start` runs the full loop: observe → think → act → journal → sleep → repeat
- [ ] `tudo agent start --strategy mean-reversion` loads the strategy's system prompt and constraints
- [ ] `tudo dashboard` renders all panes (positions, P&L chart, signal log, agent reasoning, journal, risk) at 60fps input / 1fps data refresh
- [ ] TUI never blocks — price ticks, LLM tokens, and keystrokes handled concurrently
- [ ] Client-side risk pre-check rejects signals that would fail server-side (drawdown, max positions)
- [ ] `tudo listen` outputs JSON Lines of WebSocket events to stdout, usable in pipes
- [ ] `tudo journal` prints the LLM-formatted journal summary
- [ ] `tudo strategy list` shows built-in + user-installed strategies
- [ ] `tudo strategy add my-strat --from ./my-strat.toml` registers and validates a strategy
- [ ] Agent key (`tudo_sk_...`) used for all API calls after initial SIWE auth
- [ ] Signal idempotency: retry with same `Idempotency-Key` on network failure
- [ ] Harness writes pre-trade journal entry and tags after every signal
- [ ] `cargo clippy && cargo test` passes in `tudo/`
- [ ] Integration test: full loop with shadow mode, mock backend, verifies tool call → signal → journal flow
- [ ] `tudo agent start --daemon` runs headless, `tudo attach` reconnects TUI

---

## Risks

1. **`tears` immaturity** — The crate may lack features or have bugs that block development. Mitigation: start with a hand-rolled TEA loop using `ratatui` + `tokio::select!`, which is ~200 lines of glue. Migrate to `tears` if/when it matures.
2. **Type sharing with backend** — The harness needs `SignalInput`, `AgentAlert`, `RiskConfig`, etc. from `common_utils`. If the workspace dependency becomes unwieldy, extract a `tudo-types` crate that both `common_utils` and `tudo` depend on. Mitigation: start with a direct path dependency on `common_utils`; extract only if needed.
3. **LLM provider proliferation** — Supporting 4 providers (Anthropic, OpenAI, Gemini, Ollama) means 4 API clients, 4 streaming formats, 4 tool-calling dialects. Mitigation: implement Anthropic first (best tool calling), add OpenAI second (largest user base). Gemini and Ollama are stretch goals.
4. **WebSocket reconnection** — The harness must survive network drops, n0x restarts, and Testudo deploys without crashing. Mitigation: exponential backoff reconnection in `ws::client.rs`, buffered event queue to avoid message loss during reconnect.
5. **Binary size** — `ratatui` + `tokio` + `reqwest` + `tungstenite` produces a non-trivial binary. Mitigation: acceptable for a tool deployed on n0x and dev machines. Optimize with LTO and `strip` in release builds.

---

## Completion Signal

This spec is complete when:
1. `tudo` binary passes all 15 acceptance criteria
2. Built-in strategies (mean-reversion, momentum-breakout, funding-arb) registered and executable
3. TUI dashboard renders live positions, P&L, signal log, and agent reasoning
4. Agent loop makes autonomous decisions, submits signals, writes journal entries
5. `cargo clippy && cargo test` passes in `tudo/`
6. Code committed to master under `tudo/`
