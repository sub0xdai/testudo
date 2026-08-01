# Specification: API Client + Network Layer

**Spec ID:** CLI-02-api-client
**Date:** 2026-05-31
**Status:** Draft
**Class:** Feature / Application
**Priority:** P1 — the harness needs to talk to the backend before any trading can happen
**Depends on:** CLI-01-core-tui (config, TUI loop)
**Series:** CLI-02 (API Client)

---

## Problem Statement

The `tudo` binary has a TUI and config but can't talk to the Testudo backend. There's no HTTP client, no WebSocket connection, no auth header injection. The `tudo listen` and `tudo journal` commands are stubs. Without a network layer, the harness is a pretty frame with no picture.

This spec builds the API client — a typed REST client for all 7 backend endpoints, a WebSocket client with reconnection, agent key auth, and two working CLI commands (`listen` and `journal`) that prove the network layer works end-to-end against a real Testudo backend.

---

## User Stories

- **As a user**, I run `tudo listen` and see real-time WebSocket events streaming to stdout in JSON Lines format, so that I can pipe agent alerts and execution reports into other tools.
- **As a user**, I run `tudo journal` and see my 30-day trading summary printed to the terminal, so that I can check performance without opening a browser.
- **As a developer**, I call `api.signal().submit(input)` from any part of the harness and get a typed `SignalResult` back, so that subsequent specs don't need to deal with HTTP plumbing.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Typed REST client for all 7 Testudo endpoints: signals (`POST`), journal summary (`GET`), journal insights (`GET`), journal compare (`POST`), klines (`GET`), onboarding status (`GET`), risk config (`GET`/`PUT`). Each endpoint returns a typed Rust struct. | High | API |
| FR-2 | `X-Agent-Key` header injected into every request. Read from `Config.api.agent_key` (loaded by CLI-01). If key is empty or missing, commands that require auth print a clear error pointing to `tudo init`. | High | Auth |
| FR-3 | WebSocket client using `tokio-tungstenite`. Connects to `ws://<base_url>/ws`. Subscribes to `agent.alert.{user_id}` and `agent.execution.{user_id}` channels. Parses incoming JSON into `AgentAlert` and `ExecutionReport` enums. | High | WS |
| FR-4 | WebSocket automatic reconnection with exponential backoff (1s, 2s, 4s, 8s, max 60s). Buffered event queue to avoid message loss during reconnect. | High | WS |
| FR-5 | `tudo listen` command: opens WebSocket, subscribes to agent channels, writes each received event as a JSON Line to stdout. Runs until SIGINT/Ctrl-C. Exits cleanly on signal. | Medium | CLI |
| FR-6 | `tudo journal` command: calls `GET /journal/agent/summary?format=llm&timeframe=30d`, prints markdown to stdout. Passes `X-Agent-Key` header from config. | Medium | CLI |
| FR-7 | Type sharing with backend via `common-utils` path dependency. Reuse `SignalInput`, `SignalResult`, `AgentAlert`, `ExecutionReport`, `AgentSummary`, `OnboardingStatus`, `RiskConfigSummary`, `Kline` types. | High | Types |
| FR-8 | `cargo clippy && cargo test` passes in `tudo/`. | High | CI |

---

## Technical Implementation

### Crate Structure (additions)

```
tudo/src/
├── api/
│   ├── mod.rs              // ApiClient struct
│   ├── client.rs           // Reqwest client builder + auth header injection
│   ├── signals.rs          // submit_signal(input) → SignalResult
│   ├── journal.rs          // get_summary(), get_insights(), compare()
│   ├── klines.rs           // get_klines(symbol, interval, limit) → Vec<Kline>
│   ├── onboarding.rs       // get_status() → OnboardingStatus
│   ├── risk.rs             // get_config(), put_config()
│   └── types.rs            // Shared types re-exported from common-utils
├── ws/
│   ├── mod.rs
│   ├── client.rs           // WsClient: connect, subscribe, reconnect
│   └── stream.rs           // EventStream → AgentAlert | ExecutionReport
├── cmd/ (NEW commands wired)
│   ├── listen.rs           // tudo listen handler
│   └── journal.rs          // tudo journal handler
└── main.rs                 // Wire ApiClient into AppState; wire listen/journal commands
```

### Key Types

```rust
// src/api/client.rs

pub struct ApiClient {
    http: reqwest::Client,
    base_url: String,
    agent_key: String,
}

impl ApiClient {
    pub fn new(config: &ApiConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client build");

        Self {
            http,
            base_url: config.base_url.trim_end_matches('/').to_string(),
            agent_key: config.agent_key.clone(),
        }
    }

    /// All HTTP methods use this — injects X-Agent-Key and Content-Type.
    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}{}", self.base_url, path))
            .header("X-Agent-Key", &self.agent_key)
            .header("Content-Type", "application/json")
    }
}
```

```rust
// src/api/signals.rs

impl ApiClient {
    pub async fn submit_signal(
        &self,
        input: &SignalInput,
        idempotency_key: Uuid,
    ) -> Result<SignalResult, ApiError> {
        let resp = self
            .request(Method::POST, "/signals")
            .header("Idempotency-Key", idempotency_key.to_string())
            .json(input)
            .send()
            .await?;

        match resp.status() {
            StatusCode::OK | StatusCode::CREATED => Ok(resp.json().await?),
            StatusCode::UNAUTHORIZED => Err(ApiError::Unauthorized),
            StatusCode::UNPROCESSABLE_ENTITY => {
                let body: SignalRejection = resp.json().await?;
                Err(ApiError::SignalRejected(body))
            }
            s => Err(ApiError::UnexpectedStatus(s, resp.text().await?)),
        }
    }
}
```

```rust
// src/api/journal.rs

impl ApiClient {
    pub async fn get_summary(
        &self,
        format: &str,     // "json" or "llm"
        timeframe: &str,  // "30d", "90d", etc.
    ) -> Result<AgentSummary, ApiError> {
        let resp = self
            .request(Method::GET, &format!("/journal/agent/summary?format={}&timeframe={}", format, timeframe))
            .send()
            .await?;

        match resp.status() {
            StatusCode::OK => Ok(resp.json().await?),
            StatusCode::UNAUTHORIZED => Err(ApiError::Unauthorized),
            s => Err(ApiError::UnexpectedStatus(s, resp.text().await?)),
        }
    }

    pub async fn get_insights(&self) -> Result<PaginatedInsights, ApiError> { /* ... */ }

    pub async fn compare(
        &self,
        request: &CompareRequest,
    ) -> Result<ComparisonResult, ApiError> { /* ... */ }
}
```

```rust
// src/ws/client.rs

pub struct WsClient {
    base_url: String,
    agent_key: String,
    reconnect_backoff: Duration,
}

impl WsClient {
    /// Connect and subscribe to channels. Returns an EventStream.
    pub async fn connect(&self, channels: &[&str]) -> Result<EventStream, WsError> {
        let ws_url = self.base_url
            .replace("http://", "ws://")
            .replace("https://", "wss://");

        let (ws_stream, _) = tokio_tungstenite::connect_async(&format!("{}/ws", ws_url)).await?;

        // Send SUBSCRIBE messages for each channel
        let (write, read) = ws_stream.split();
        // ... subscription + read loop producing Events ...

        Ok(EventStream { rx })
    }
}

pub enum WsEvent {
    Alert(AgentAlert),
    Execution(ExecutionReport),
    Unknown(String),
}

pub struct EventStream {
    rx: tokio::sync::mpsc::UnboundedReceiver<WsEvent>,
}

impl EventStream {
    pub async fn recv(&mut self) -> Option<WsEvent> {
        self.rx.recv().await
    }
}
```

```rust
// src/cmd/listen.rs

pub async fn run_listen(config: &Config) -> Result<(), Box<dyn Error>> {
    let client = WsClient::new(&config.api);

    println!("Connecting to {}...", config.api.base_url);
    let mut stream = client
        .connect(&["agent.alert.*", "agent.execution.*"])
        .await?;

    println!("Listening for agent events (Ctrl-C to stop)...\n");

    while let Some(event) = stream.recv().await {
        match serde_json::to_string(&event) {
            Ok(line) => println!("{}", line),
            Err(e) => eprintln!("json error: {}", e),
        }
    }

    Ok(())
}
```

```rust
// src/cmd/journal.rs

pub async fn run_journal(config: &Config) -> Result<(), Box<dyn Error>> {
    let api = ApiClient::new(&config.api);

    if config.api.agent_key.is_empty() {
        eprintln!("Error: No agent key configured.");
        eprintln!("Run 'tudo init' first, or set api.agent_key in ~/.config/tudo/config.toml");
        std::process::exit(1);
    }

    let summary = api.get_summary("llm", "30d").await?;

    // If the server returns markdown (Content-Type: text/markdown),
    // the body is already the formatted string.
    println!("{}", summary.markdown.unwrap_or_else(|| "No data available.".into()));

    Ok(())
}
```

### Dependencies Added

```toml
# Added to tudo/Cargo.toml
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
tokio-tungstenite = "0.24"
futures-util = "0.3"                    # StreamExt for WebSocket
common-utils = { path = "../testudo-exchange/crates/common_utils" }
uuid = { version = "1", features = ["v4"] }
```

### Model Extensions

```rust
// Added to src/model/state.rs AppState
pub struct AppState {
    // ... existing fields from CLI-01 ...
    pub api: Option<ApiClient>,          // None when config has no key
}
```

---

## Checkpoints

### CP-1: REST API client + type integration
- **Touches**: `tudo/src/api/mod.rs`, `client.rs`, `signals.rs`, `journal.rs`, `klines.rs`, `onboarding.rs`, `risk.rs`, `types.rs` (all NEW), `tudo/Cargo.toml`
- **Tasks**:
  1. Create `src/api/client.rs` with `ApiClient::new()` — reqwest Client with 30s timeout and `X-Agent-Key` header injection.
  2. Create `src/api/signals.rs` — `submit_signal()` mapping to `POST /api/v1/signals`. Error handling for 401, 422 (rejection body), 409 (idempotent duplicate).
  3. Create `src/api/journal.rs` — `get_summary()`, `get_insights()`, `post_compare()`.
  4. Create `src/api/klines.rs` — `get_klines(symbol, interval, limit)`.
  5. Create `src/api/onboarding.rs` — `get_status()`.
  6. Create `src/api/risk.rs` — `get_config()`, `put_config()`.
  7. Add `common-utils` as path dependency. Verify types compile.
  8. Unit test: mock HTTP server (using `wiremock` or a simple `httptest` server) returns 200 with sample JSON → client deserializes correctly.
- **Verification**: `cargo test -p tudo -- api` passes. All 7 endpoint methods exist and compile. Mock-based tests verify deserialization.

### CP-2: WebSocket client + reconnection
- **Touches**: `tudo/src/ws/mod.rs`, `client.rs`, `stream.rs` (NEW), `tudo/Cargo.toml`
- **Tasks**:
  1. Create `src/ws/client.rs` — `WsClient` connects via `tokio-tungstenite`, sends SUBSCRIBE JSON for provided channels, spawns read task that sends `WsEvent` variants through an mpsc channel.
  2. Implement exponential backoff reconnection: on disconnect, wait `backoff` (starts 1s, doubles to max 60s), reconnect, resubscribe. Emit events into same mpsc channel after reconnect.
  3. Create `src/ws/stream.rs` — `EventStream` wraps the mpsc receiver with `async fn recv()`. On channel close (sender dropped), returns `None`.
  4. Unit test: connect to a local echo WebSocket server (or mock), verify subscribe message format, verify reconnection after server drop.
- **Verification**: `cargo test -p tudo -- ws` passes. WebSocket connects, subscribes, receives events.

### CP-3: `tudo listen` + `tudo journal` commands
- **Touches**: `tudo/src/cmd/listen.rs` (NEW), `tudo/src/cmd/journal.rs` (NEW), `tudo/src/main.rs`
- **Tasks**:
  1. Create `src/cmd/listen.rs` — `run_listen()`: creates `WsClient`, connects to agent channels, loops on `EventStream::recv()`, writes each event as JSON Line to stdout. On `None` (stream closed), exit with message.
  2. Create `src/cmd/journal.rs` — `run_journal()`: creates `ApiClient`, calls `get_summary("llm", "30d")`. Prints markdown to stdout. If agent_key empty, prints error with `tudo init` hint.
  3. Wire both commands in `main.rs`: `Command::Listen` → `run_listen()`, `Command::Journal` → `run_journal()`.
  4. Integration test: mock HTTP + WebSocket server, run `listen` → receives events → JSON Lines output verified. Run `journal` → prints markdown output.
- **Verification**: `cargo test -p tudo` passes with new command tests. Manual test against running Testudo backend: `tudo listen` streams events, `tudo journal` prints summary.

### CP-4: Error handling + polish
- **Touches**: `tudo/src/api/client.rs`, `tudo/src/cmd/listen.rs`, `tudo/src/cmd/journal.rs`
- **Tasks**:
  1. Implement `ApiError` enum: `Unauthorized`, `SignalRejected(SignalRejection)`, `Network(reqwest::Error)`, `Deserialize(serde_json::Error)`, `UnexpectedStatus(StatusCode, String)`. Implement `Display` with user-friendly messages.
  2. `listen` command: handle connection refused (retry with backoff message printed to stderr). Handle auth failure (exit 1 with "check agent key" message).
  3. `journal` command: handle 401, 404 (no trades yet → friendly message), network timeout.
  4. Add `tracing` instrumentation: `info!("GET /journal/agent/summary")`, `error!("WebSocket disconnected: {}", e)`.
- **Verification**: `cargo clippy -p tudo --all-targets && cargo test -p tudo` passes. Manual: `tudo journal` with no key → clear error. `tudo listen` with bad URL → graceful retry message.

---

## Acceptance Criteria

- [ ] `ApiClient` supports all 7 Testudo endpoints with typed return values
- [ ] `X-Agent-Key` header sent on every request from config
- [ ] WebSocket client connects, subscribes to agent channels, receives typed events
- [ ] WebSocket reconnects with exponential backoff on disconnect
- [ ] `tudo listen` streams JSON Lines to stdout until Ctrl-C
- [ ] `tudo journal` prints markdown summary to stdout
- [ ] Missing agent key → clear error message pointing to `tudo init`
- [ ] Type sharing with `common-utils` works — no duplicate type definitions
- [ ] `cargo clippy --all-targets && cargo test` passes in `tudo/`

---

## Risks

1. **`common-utils` type drift** — The harness depends on types from the backend crate. If backend types change, the harness breaks at compile time. Mitigation: this is a feature — compile-time safety. The harness is in the same monorepo so type changes are caught in CI.
2. **WebSocket URL construction** — The spec assumes the WS endpoint is at `<base_url>/ws`. If the backend uses a different path or port, this breaks. Mitigation: add `ws_url` to config (with sensible default derived from `base_url`).
3. **reqwest TLS on Linux** — Using `rustls-tls` avoids OpenSSL linking issues. If the user's Testudo backend uses a self-signed cert, `rustls` will reject it. Mitigation: add `danger_accept_invalid_certs` config option (default false, documented as dev-only).

---

## Completion Signal

This spec is complete when:
1. `ApiClient` has typed methods for all 7 endpoints
2. `WsClient` connects, subscribes, reconnects, and streams events
3. `tudo listen` produces JSON Lines output from a running Testudo backend
4. `tudo journal` prints markdown summary from a running Testudo backend
5. `cargo clippy --all-targets && cargo test` passes in `tudo/`
6. Code committed to master

---

## Next Spec

**CLI-03-agent-loop** — Adds LLM provider abstraction, 7 tool definitions, and the autonomous agent loop (`tudo agent start`). Depends on the API client and WebSocket client from this spec.
