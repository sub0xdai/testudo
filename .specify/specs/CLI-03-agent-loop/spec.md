# Specification: Agent Loop + LLM Providers + Tool System

**Spec ID:** CLI-03-agent-loop
**Date:** 2026-05-31
**Status:** Draft
**Class:** Feature / Application
**Priority:** P1 — this is the core of the harness; the reason `tudo` exists
**Depends on:** CLI-02-api-client (REST client, WS client, shared types)
**Series:** CLI-03 (Agent Loop)

---

## Problem Statement

The harness has a TUI and network layer but no brain. There's no LLM integration, no tool definitions, no autonomous decision loop. The `tudo agent start` command is a stub. An external LLM (Hermes, pi, OpenClaw) still has to manually call the REST API, manage its own context window, and track tool executions.

This spec builds the agent layer: LLM provider abstraction (Anthropic first, OpenAI second), 7 typed tool definitions with OpenAI-compatible function calling, and the autonomous `observe → think → act → journal → sleep → repeat` loop. After this spec, `tudo agent start` is a working command — the harness analyzes markets, decides on trades, submits signals, and writes journal entries, all without human intervention.

---

## User Stories

- **As a trader**, I run `tudo agent start` and the harness begins analyzing markets and submitting shadow signals every 60 seconds, so that I can test strategies without writing any code.
- **As a strategy developer**, I want the LLM to have access to typed tools (fetch klines, submit signal, read journal, etc.) with proper JSON Schema validation, so that tool calls are reliable and type-safe.
- **As a risk-conscious user**, I want journal entries automatically written after every signal (pre-trade thesis + post-trade P&L via WebSocket), so that I have a full audit trail without manual logging.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | LLM provider abstraction: `LlmClient` trait with `send_message(messages, tools) → LlmResponse`. Anthropic provider implemented first (Messages API with tool_use). OpenAI provider as second (Chat Completions with tool_calls). | High | LLM |
| FR-2 | LLM streaming: tokens received from the provider are emitted as `Message::LlmToken` for real-time TUI display. Complete responses emit `Message::LlmDone` with parsed tool calls. | High | LLM |
| FR-3 | 7 typed tool definitions: `fetch_klines`, `submit_signal`, `read_journal`, `write_journal`, `list_positions`, `check_risk`, `check_onboarding`. Each tool has a JSON Schema definition and an async executor. Tools serialize to OpenAI-compatible function calling format. | High | Tools |
| FR-4 | `fetch_klines` tool: calls `ApiClient::get_klines()`, returns structured OHLCV summary (count, latest close, high, low). | High | Tools |
| FR-5 | `submit_signal` tool: validates required fields (symbol, side, entry_price, stop_loss, reasoning, confidence, source), generates idempotency key, calls `ApiClient::submit_signal()`. Returns structured result from backend. | High | Tools |
| FR-6 | `tudo agent start` launches the autonomous loop: observe (read journal + list positions) → think (LLM with tools) → act (execute tool calls) → journal (write pre-trade entry, tag) → sleep (configurable interval, default 60s) → repeat. | High | Loop |
| FR-7 | Agent loop phase tracking: `AgentPhase::Observing` → `Thinking` → `Acting` → `Idle`. Phase transitions visible in TUI status bar. Loop respects `shadow_only` config — refuses to submit LIVE signals when set. | High | Loop |
| FR-8 | Signal idempotency: `Idempotency-Key` header generated per signal (UUIDv4). On network failure, retry with same key. Backend deduplicates. | High | Loop |
| FR-9 | Journal write-after-signal: after every `submit_signal` success, write a pre-trade journal entry via `POST /journal/agent/note` with thesis=reasoning, tag=strategy_name. Post-trade entry written on `ExecutionReport` WebSocket event when trade closes. | High | Journal |
| FR-10 | `cargo clippy && cargo test` passes in `tudo/`. | High | CI |

---

## Technical Implementation

### Crate Structure (additions)

```
tudo/src/
├── llm/
│   ├── mod.rs              // LlmClient trait + provider selection
│   ├── client.rs           // Trait: send_message(), stream_message()
│   ├── anthropic.rs        // Anthropic Messages API (tool_use blocks)
│   ├── openai.rs           // OpenAI Chat Completions (tool_calls)
│   ├── types.rs            // LlmMessage, LlmResponse, LlmToolCall, LlmToolResult
│   └── stream.rs           // SSE/streaming token parser → Message::LlmToken
├── tools/
│   ├── mod.rs              // all_tools(), execute_tool()
│   ├── types.rs            // ToolDef, ToolResult, OpenAIFunctionDef
│   ├── fetch_klines.rs     // tool_def() + execute()
│   ├── submit_signal.rs    // tool_def() + execute() + client-side risk stub
│   ├── read_journal.rs     // tool_def() + execute()
│   ├── write_journal.rs    // tool_def() + execute()
│   ├── list_positions.rs   // tool_def() + execute()
│   ├── check_risk.rs       // tool_def() + execute()
│   └── check_onboarding.rs // tool_def() + execute()
├── model/
│   └── agent.rs            // AgentState, AgentPhase, LoopConfig, AgentMode
├── cmd/
│   └── agent.rs            // tudo agent start handler
├── app.rs                  // Wire agent loop into TEA event loop
├── msg.rs                  // Add LlmToken, LlmDone, LlmError, ToolResults, PhaseChange
├── update.rs               // Handle agent lifecycle messages
├── cmd.rs                  // Add CallLlm, ExecuteTool, Sleep commands
└── main.rs                 // Wire agent start command
```

### LLM Provider Abstraction

```rust
// src/llm/client.rs

#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Send messages + tools, get a complete response.
    async fn send_message(
        &self,
        messages: &[LlmMessage],
        tools: &[ToolDef],
    ) -> Result<LlmResponse, LlmError>;

    /// Stream the response token-by-token. Each token → Message::LlmToken.
    /// Final result → Message::LlmDone.
    async fn stream_message(
        &self,
        messages: &[LlmMessage],
        tools: &[ToolDef],
        token_tx: tokio::sync::mpsc::UnboundedSender<Message>,
    ) -> Result<(), LlmError>;
}

pub fn create_client(config: &LlmConfig) -> Box<dyn LlmClient> {
    match config.provider.as_str() {
        "anthropic" => Box::new(AnthropicClient::new(config)),
        "openai" => Box::new(OpenAiClient::new(config)),
        other => panic!("Unknown LLM provider: {}. Supported: anthropic, openai", other),
    }
}
```

```rust
// src/llm/types.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,     // "system" | "user" | "assistant" | "tool"
    pub content: Option<String>,
    pub tool_calls: Option<Vec<LlmToolCall>>,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: Option<String>,          // Text response (can be None if tool_calls present)
    pub tool_calls: Vec<LlmToolCall>,     // Tool calls the LLM wants to execute
    pub finish_reason: String,            // "stop" | "tool_calls" | "length"
    pub usage: LlmUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct LlmToolResult {
    pub call_id: String,
    pub name: String,
    pub content: String,
}
```

### Anthropic Provider

```rust
// src/llm/anthropic.rs

pub struct AnthropicClient {
    api_key: String,
    model: String,
    http: reqwest::Client,
}

impl AnthropicClient {
    const API_URL: &str = "https://api.anthropic.com/v1/messages";

    /// Convert our ToolDef → Anthropic tool format.
    fn to_anthropic_tools(tools: &[ToolDef]) -> Vec<serde_json::Value> {
        tools.iter().map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.parameters,
            })
        }).collect()
    }

    /// Convert our LlmMessage → Anthropic content block.
    fn to_anthropic_message(msg: &LlmMessage) -> serde_json::Value {
        match msg.role.as_str() {
            "user" => json!({
                "role": "user",
                "content": msg.content,
            }),
            "assistant" => json!({
                "role": "assistant",
                "content": msg.content,
            }),
            "tool" => json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": msg.tool_call_id,
                    "content": msg.content,
                }],
            }),
            _ => json!({"role": "user", "content": msg.content}),
        }
    }
}

#[async_trait]
impl LlmClient for AnthropicClient {
    async fn send_message(
        &self,
        messages: &[LlmMessage],
        tools: &[ToolDef],
    ) -> Result<LlmResponse, LlmError> {
        let body = json!({
            "model": self.model,
            "max_tokens": 4096,
            "system": messages.iter().find(|m| m.role == "system").map(|m| m.content.clone()),
            "messages": messages.iter().filter(|m| m.role != "system").map(Self::to_anthropic_message).collect::<Vec<_>>(),
            "tools": Self::to_anthropic_tools(tools),
        });

        let resp = self.http
            .post(Self::API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        // Parse Anthropic response → LlmResponse
        let anthropic_resp: serde_json::Value = resp.json().await?;

        let mut tool_calls = vec![];
        if let Some(blocks) = anthropic_resp["content"].as_array() {
            for block in blocks {
                if block["type"] == "tool_use" {
                    tool_calls.push(LlmToolCall {
                        id: block["id"].as_str().unwrap_or("").to_string(),
                        name: block["name"].as_str().unwrap_or("").to_string(),
                        arguments: block["input"].clone(),
                    });
                }
            }
        }

        Ok(LlmResponse {
            content: anthropic_resp["content"][0]["text"].as_str().map(String::from),
            tool_calls,
            finish_reason: anthropic_resp["stop_reason"].as_str().unwrap_or("stop").to_string(),
            usage: LlmUsage { /* extract from response */ },
        })
    }

    // stream_message implementation omitted for brevity
}
```

### Tool Definitions

```rust
// src/tools/types.rs

#[derive(Debug, Clone, Serialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,  // JSON Schema object
}

/// Function signature for tool execution.
#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDef;
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError>;
}
```

```rust
// src/tools/fetch_klines.rs

pub struct FetchKlinesTool {
    api: Arc<ApiClient>,
}

impl FetchKlinesTool {
    pub fn new(api: Arc<ApiClient>) -> Self { Self { api } }
}

impl Tool for FetchKlinesTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "fetch_klines".into(),
            description: "Fetch OHLCV candlestick data for a symbol. Use before making trading decisions.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "description": "Trading pair, e.g. 'ETH_USDT'"},
                    "interval": {"type": "string", "enum": ["1m", "5m", "15m", "1h", "4h", "1d"]},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 500}
                },
                "required": ["symbol", "interval", "limit"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        let symbol = args["symbol"].as_str().ok_or(ToolError::MissingArg("symbol"))?;
        let interval = args["interval"].as_str().ok_or(ToolError::MissingArg("interval"))?;
        let limit = args["limit"].as_u64().unwrap_or(100) as u32;

        let klines = self.api.get_klines(symbol, interval, limit).await?;

        let summary = format!(
            "{} {} candles for {} | latest close: {} | high: {} | low: {}",
            klines.len(), interval, symbol,
            klines.last().map(|k| k.close).unwrap_or_default(),
            klines.iter().map(|k| k.high).fold(f64::NEG_INFINITY, f64::max),
            klines.iter().map(|k| k.low).fold(f64::INFINITY, f64::min),
        );

        Ok(ToolResult { content: json!({"summary": summary, "candles": klines}).to_string() })
    }
}
```

```rust
// src/tools/submit_signal.rs

pub struct SubmitSignalTool {
    api: Arc<ApiClient>,
}

impl Tool for SubmitSignalTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "submit_signal".into(),
            description: "Submit a trade signal. Always include stop_loss, reasoning, and confidence. Start in SHADOW mode.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string"},
                    "side": {"type": "string", "enum": ["LONG", "SHORT"]},
                    "entry_price": {"type": "number"},
                    "stop_loss": {"type": "number"},
                    "take_profit": {"type": "array", "items": {"type": "object", "properties": {"price": {"type": "number"}, "quantity": {"type": "number"}}}},
                    "leverage": {"type": "integer", "minimum": 1, "maximum": 20},
                    "execution_mode": {"type": "string", "enum": ["SHADOW", "LIVE"]},
                    "reasoning": {"type": "string"},
                    "confidence": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                    "source": {"type": "string"}
                },
                "required": ["symbol", "side", "entry_price", "stop_loss", "execution_mode", "reasoning", "confidence", "source"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
        // Validate required fields
        let execution_mode = args["execution_mode"].as_str().unwrap_or("SHADOW");

        if execution_mode == "LIVE" {
            // Placeholder: client-side risk pre-check (CLI-04 adds real implementation)
        }

        let input = SignalInput {
            symbol: args["symbol"].as_str().ok_or(ToolError::MissingArg("symbol"))?.to_string(),
            side: args["side"].as_str().ok_or(ToolError::MissingArg("side"))?.to_string(),
            entry_price: args["entry_price"].as_str().map(String::from),
            stop_loss: args["stop_loss"].as_str().map(String::from),
            execution_mode: execution_mode.to_string(),
            reasoning: args["reasoning"].as_str().map(String::from),
            confidence: args["confidence"].as_f64(),
            source: args["source"].as_str().map(String::from),
            idempotency_key: Some(Uuid::new_v4()),
            ..Default::default()
        };

        let idempotency_key = Uuid::new_v4();
        let result = self.api.submit_signal(&input, idempotency_key).await?;

        Ok(ToolResult {
            content: json!(result).to_string(),
        })
    }
}
```

### Agent Loop

```rust
// src/cmd/agent.rs

pub async fn run_agent(config: &Config, strategy_name: Option<String>) -> Result<(), Box<dyn Error>> {
    let api = Arc::new(ApiClient::new(&config.api));
    let llm = create_client(&config.llm);

    // --- Default system prompt (from AGENT_TRADING.md or strategy) ---
    let system_prompt = strategy_name
        .map(|name| load_strategy_prompt(&name))
        .unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string());

    let mut messages: Vec<LlmMessage> = vec![LlmMessage {
        role: "system".into(),
        content: Some(system_prompt),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }];

    let tools = all_tools(api.clone());
    let tool_defs: Vec<ToolDef> = tools.iter().map(|t| t.definition()).collect();

    // --- Main loop ---
    loop {
        // 1. Observe: read journal + list positions (pre-load context)
        let journal = api.get_summary("llm", "7d").await?;
        messages.push(LlmMessage {
            role: "user".into(),
            content: Some(format!(
                "Here is your current context:\n## Journal (7d)\n{}\n\nAnalyze the market and decide on your next action.",
                journal.markdown.unwrap_or_default()
            )),
            tool_calls: None, tool_call_id: None, name: None,
        });

        // 2. Think: call LLM with tools
        let response = llm.send_message(&messages, &tool_defs).await?;

        // 3. Act: execute tool calls
        if !response.tool_calls.is_empty() {
            messages.push(LlmMessage {
                role: "assistant".into(),
                content: response.content,
                tool_calls: Some(response.tool_calls.clone()),
                tool_call_id: None, name: None,
            });

            for tc in &response.tool_calls {
                let result = execute_tool(&tools, tc).await?;
                messages.push(LlmMessage {
                    role: "tool".into(),
                    content: Some(result.content),
                    tool_calls: None,
                    tool_call_id: Some(tc.id.clone()),
                    name: Some(tc.name.clone()),
                });

                // 4. Journal: if it was a submit_signal, write pre-trade entry
                if tc.name == "submit_signal" {
                    // Write journal entry (see FR-9)
                }
            }
        }

        // 5. Sleep
        tokio::time::sleep(Duration::from_secs(config.agent.loop_interval_secs)).await;
    }
}
```

### Model Extensions

```rust
// src/model/agent.rs (NEW)

pub struct AgentState {
    pub phase: AgentPhase,
    pub strategy: Option<String>,
    pub messages: Vec<LlmMessage>,
    pub pending_tool_calls: Vec<PendingToolCall>,
    pub recent_signals: Vec<SignalResult>,
    pub loop_config: LoopConfig,
    pub mode: AgentMode,
    pub stream_tokens: String,        // Accumulated LLM streaming tokens
}

pub enum AgentPhase {
    Observing,
    Thinking { tokens_received: usize },
    Acting,
    Idle,
}

pub enum AgentMode { Shadow, Live }

pub struct LoopConfig {
    pub interval_secs: u64,
    pub shadow_only: bool,
    pub max_signals_per_hour: u32,
}

pub struct PendingToolCall {
    pub id: String,
    pub name: String,
    pub started_at: Instant,
}
```

### Dependencies Added

```toml
# Added to tudo/Cargo.toml
async-trait = "0.1"
chrono = { version = "0.4", features = ["serde"] }
```

---

## Checkpoints

### CP-1: LLM provider trait + Anthropic implementation
- **Touches**: `tudo/src/llm/mod.rs`, `client.rs`, `types.rs`, `anthropic.rs`, `stream.rs` (all NEW)
- **Tasks**:
  1. Define `LlmClient` trait with `send_message()` and `stream_message()`.
  2. Define `LlmMessage`, `LlmResponse`, `LlmToolCall`, `LlmToolResult` types.
  3. Implement `AnthropicClient`: Messages API, tool_use block parsing, API key header, error mapping.
  4. Implement `AnthropicClient::stream_message()`: SSE parsing, token emission via mpsc channel.
  5. Unit test: mock Anthropic API (or use recorded responses), verify tool_use blocks parsed correctly.
  6. Unit test: response with text only → correct LlmResponse with no tool_calls.
- **Verification**: `cargo test -p tudo -- llm` passes. AnthropicClient sends correct request body structure. Tool call parsing extracts name + arguments.

### CP-2: 7 tool definitions
- **Touches**: `tudo/src/tools/mod.rs`, `types.rs`, `fetch_klines.rs`, `submit_signal.rs`, `read_journal.rs`, `write_journal.rs`, `list_positions.rs`, `check_risk.rs`, `check_onboarding.rs` (all NEW)
- **Tasks**:
  1. Define `Tool` trait: `fn definition() → ToolDef`, `async fn execute(args) → ToolResult`.
  2. Implement all 7 tools:
     - `FetchKlinesTool` → `ApiClient::get_klines()`
     - `SubmitSignalTool` → `ApiClient::submit_signal()`
     - `ReadJournalTool` → `ApiClient::get_summary()`
     - `WriteJournalTool` → `ApiClient::post_note()` (use `/journal/agent/note` endpoint)
     - `ListPositionsTool` → TODO (needs positions endpoint; for now returns placeholder)
     - `CheckRiskTool` → `ApiClient::get_risk_config()` + basic validation
     - `CheckOnboardingTool` → `ApiClient::get_onboarding_status()`
  3. `all_tools(api)` constructor function returns `Vec<Box<dyn Tool>>`.
  4. Each tool's `definition()` returns JSON Schema matching the original blueprint.
  5. Unit test: each tool definition validates against JSON Schema meta-schema (valid structure). Mock API responses so tools return expected data.
- **Verification**: `cargo test -p tudo -- tools` passes. All 7 tools defined and testable with mock API.

### CP-3: `tudo agent start` autonomous loop
- **Touches**: `tudo/src/cmd/agent.rs` (NEW), `tudo/src/model/agent.rs` (NEW), `tudo/src/main.rs`, `tudo/src/app.rs`
- **Tasks**:
  1. Implement `run_agent(config, strategy_name)`:
     - Load system prompt (default: embed `AGENT_TRADING.md` content, or load from strategy TOML if name provided — CLI-04 adds full strategy loading).
     - Initialize `AgentState` with `phase = Observing`.
     - Main loop: observe → think → act → journal → sleep → repeat.
  2. Phase tracking: update `AgentState.phase` at each step. Emit `Message::PhaseChange`.
  3. Tool call loop: after LLM returns tool calls, execute them sequentially, feed results back, call LLM again if needed (max 3 tool-call rounds per iteration).
  4. `shadow_only` enforcement: if config says `shadow_only = true` and a tool call tries `execution_mode = "LIVE"`, override to "SHADOW" and log warning.
  5. Wire `Command::Agent { action: AgentAction::Start { strategy, daemon } }` in main.rs. For now, daemon flag is accepted but ignored (CLI-05 implements daemon mode).
  6. Integration test: mock LLM (returns tool calls for fetch_klines + submit_signal) + mock HTTP backend → verify full loop executes without error.
- **Verification**: `cargo test -p tudo -- agent` passes. Agent loop runs 2 iterations without crash. Phase transitions logged correctly.

### CP-4: Journal integration + idempotency
- **Touches**: `tudo/src/cmd/agent.rs`, `tudo/src/tools/submit_signal.rs`
- **Tasks**:
  1. After `submit_signal` success, write pre-trade journal entry via `WriteJournalTool`: thesis = `reasoning`, tag = `source`.
  2. Listen for `ExecutionReport` WebSocket events during loop. When a trade closes (status = "filled" and is_close = true), write post-trade journal entry.
  3. Idempotency: `submit_signal` tool generates UUIDv4. On `reqwest::Error` (timeout, connection reset), retry up to 3 times with same key before failing.
  4. Signal count tracking: increment counter per iteration. If `max_signals_per_hour` exceeded, skip signal submission and log warning.
  5. Integration test: mock backend returns signal success → verify journal entry written with correct thesis and tag. Mock execution report → verify post-trade journal entry.
- **Verification**: `cargo test -p tudo -- agent` passes journal + idempotency tests. `cargo clippy -p tudo --all-targets` passes.

---

## Acceptance Criteria

- [ ] `AnthropicClient` implements `LlmClient` trait, sends correct API requests, parses tool_use blocks
- [ ] `OpenAiClient` implements `LlmClient` trait (can be thin — full parity with Anthropic not required yet)
- [ ] All 7 tools defined with JSON Schema, execute against `ApiClient`, return typed `ToolResult`
- [ ] `tudo agent start` runs the full observe→think→act→journal→sleep loop
- [ ] Agent loop enforces `shadow_only` — LIVE signals overridden to SHADOW with warning
- [ ] Journal entries written after signal submission (pre-trade) and execution reports (post-trade)
- [ ] Signal idempotency: retries with same UUID key on network failure
- [ ] LLM streaming renders tokens in real-time in TUI agent pane
- [ ] `cargo clippy --all-targets && cargo test` passes in `tudo/`

---

## Risks

1. **Anthropic tool_use format** — Anthropic uses `tool_use` content blocks with `input` (not `arguments` like OpenAI). The `LlmToolCall` struct normalizes both. Mitigation: the `AnthropicClient` maps Anthropic's format to our internal type; other providers map their format.
2. **Tool call retry storm** — If the LLM repeatedly calls tools, the loop could run indefinitely. Mitigation: max 3 tool-call rounds per iteration; after that, force an idle phase.
3. **`AGENT_TRADING.md` as system prompt** — The file is long and may change. Embedding the content at compile time (`include_str!`) means every doc change requires a rebuild. Mitigation: load at runtime from a known path (`~/.config/tudo/AGENT_TRADING.md`) with a compiled-in fallback.
4. **WebSocket event handling in agent loop** — The agent loop currently sleeps between iterations. Execution reports may arrive during sleep and need to be buffered. Mitigation: spawn a separate tokio task for WebSocket listener that sends events to a shared mpsc channel; the agent loop checks the channel before each iteration.

---

## Completion Signal

This spec is complete when:
1. `LlmClient` trait + Anthropic provider working
2. 7 tools defined and executable
3. `tudo agent start` runs autonomous loop (shadow mode)
4. Journal entries written after signals and fills
5. Signal idempotency works
6. `cargo clippy --all-targets && cargo test` passes in `tudo/`
7. Code committed to master

---

## Next Spec

**CLI-04-strategy-registry** — Adds strategy TOML loading, 3 built-in strategies, client-side risk pre-check, and `tudo init` onboarding flow. Depends on the agent loop and tool system from this spec.
