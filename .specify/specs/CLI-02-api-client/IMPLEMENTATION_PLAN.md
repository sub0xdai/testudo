# CLI-02-api-client — Implementation Plan

## Current State Summary

CLI-01 is complete — the `testudo` binary has a working TUI, config loading from `~/.config/testudo/config.toml`, and clap CLI with 7 stub subcommands. The `ApiConfig` struct already has `base_url` and `agent_key` fields ready for use. All `api/*.rs` and `ws/*.rs` files are empty stubs (anchor tags only). `Cargo.toml` has no network dependencies — `reqwest`, `tokio-tungstenite`, `futures-util`, and `common-utils` were all stripped during CLI-01 CP-1. The `cmd.rs` file handles CLI parsing but has no listen/journal dispatch.

The Testudo backend has a mature API surface under `/api/v1/` with JWT + `X-Agent-Key` auth. Key endpoints: `POST /signals`, `GET /journal/agent/summary`, `GET /journal/agent/insights`, `POST /journal/agent/compare`, `GET /klines`, `GET /onboarding/status`, `GET/PUT /risk-config`. The WebSocket layer is a separate `ws-stream` service using a JSON-RPC-style protocol: send `{"method":"SUBSCRIBE","params":["agent.alert.<user_id>"],"id":1}`, receive `{"stream":"agent.alert.<user_id>","data":{...}}`.

### Critical deviations from spec

| Spec says | Reality | Impact |
|-----------|---------|--------|
| Types from `common-utils` (SignalInput, SignalResult, etc.) | Types live in `router/src/models/` — not re-exported from common-utils | Define client-side types in `api/types.rs` mirroring backend JSON shapes. Use `common-utils` only for `AgentAlert`, `ExecutionReport`, `Candle`. |
| WebSocket at `ws://<base_url>/ws` | WS is a separate `ws-stream` service on its own port | Add `ws_url` field to `ApiConfig`. Default: derived from `base_url` by swapping port or using explicit config. |
| Subscribe to `agent.alert.*` (wildcard) | Subscription requires `agent.alert.<user_id>` (explicit user_id) | Must discover `user_id` first (from `/onboarding/status` or config). |
| `GET /klines?symbol=&interval=&limit=` | `GET /klines?symbol=&interval=&start_time=` (no `limit` param) | Use `start_time` parameter. |
| `GET/PUT /risk` | `GET/PUT /risk-config` | Use `/risk-config` path. |

### Gap Summary

| Requirement | Status | Detail |
|---|---|---|
| FR-1: Typed REST client (7 endpoints) | ❌ None | 8 stub files in `api/`, zero implementation |
| FR-2: X-Agent-Key header injection | ❌ None | No HTTP client exists |
| FR-3: WebSocket client | ❌ None | 2 stub files in `ws/` |
| FR-4: WS reconnection + backoff | ❌ None | No WS code |
| FR-5: `testudo listen` command | ❌ None | cmd.rs has no listen dispatch |
| FR-6: `testudo journal` command | ❌ None | cmd.rs has no journal dispatch |
| FR-7: Type sharing | ❌ Missing deps | No common-utils in Cargo.toml; client types undefined |
| FR-8: build/test | ✅ Pass | CLI-01 state is clean |
| Config: ws_url field | ❌ Missing | ApiConfig needs `ws_url` |

---

## Checkpoints

### CP-1: REST API client + types + wiring ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `testudo-cli/Cargo.toml`, `testudo-cli/src/api/types.rs` (NEW), `testudo-cli/src/api/client.rs`, `testudo-cli/src/api/signals.rs`, `testudo-cli/src/api/journal.rs`, `testudo-cli/src/api/klines.rs`, `testudo-cli/src/api/onboarding.rs`, `testudo-cli/src/api/risk.rs`, `testudo-cli/src/config.rs` (add ws_url), `testudo-cli/src/lib.rs` (make api public)
- **Tasks**:
  1. Add deps to `Cargo.toml`: `reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }`, `common_utils = { path = "../testudo-exchange/crates/common_utils" }`, `uuid = { version = "1", features = ["v4"] }`, `chrono = { version = "0.4", features = ["serde"] }`, `rust_decimal = { version = "1", features = ["serde"] }`, `thiserror = "2"`.
  2. Add `ws_url: String` to `ApiConfig` with default `"ws://localhost:8081"`. Update `default_base_url()` → `"http://localhost:8080/api/v1"`.
  3. Create `api/types.rs`: `ApiError` enum (Network, Unauthorized, NotFound, Deserialize, UnexpectedStatus), minimal request/response types mirroring backend JSON: `SignalInput`, `SignalResult`, `SignalRejection`, `AgentSummary`, `AgentInsight`, `CompareRequest`, `CompareResult`, `KlineData`, `OnboardingStatus`, `RiskConfigData`.
  4. Implement `api/client.rs`: `ApiClient` struct with reqwest client, `new(&ApiConfig)`, `request(method, path)` injecting `X-Agent-Key` + `Content-Type: application/json`. `get_json<T>(path)` and `post_json<T, B>(path, body)` helpers.
  5. Implement `api/signals.rs`: `submit_signal(&self, input: &SignalInput)` → `Result<SignalResult, ApiError>`. POST to `/signals`. Handle 401 (Unauthorized), 422 (SignalRejected body).
  6. Implement `api/journal.rs`: `get_summary(timeframe, format)` → `Result<String, ApiError>` (returns raw body — server returns markdown for `format=llm`), `get_insights()` → `Result<Vec<AgentInsight>, ApiError>`, `post_compare(req)` → `Result<CompareResult, ApiError>`.
  7. Implement `api/klines.rs`: `get_klines(symbol, interval, start_time)` → `Result<Vec<KlineData>, ApiError>`.
  8. Implement `api/onboarding.rs`: `get_status()` → `Result<OnboardingStatus, ApiError>`.
  9. Implement `api/risk.rs`: `get_config()` → `Result<RiskConfigData, ApiError>`, `put_config(update)` → `Result<RiskConfigData, ApiError>`.
  10. Unit test: mock HTTP server (httptest or wiremock) returns sample JSON → client deserializes correctly for signals, journal, klines.
- **Verification**: `cd testudo-cli && cargo build && cargo test -- api` passes. All 7 endpoint methods compile. Mock tests verify deserialization.
- **Commit message**: `feat: typed REST client for 7 Testudo API endpoints`

### CP-2: WebSocket client + reconnection ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `testudo-cli/Cargo.toml` (add tokio-tungstenite, futures-util), `testudo-cli/src/ws/client.rs`, `testudo-cli/src/ws/stream.rs`, `testudo-cli/src/lib.rs`
- **Tasks**:
  1. Add deps: `tokio-tungstenite = { version = "0.24", features = ["native-tls"] }`, `futures-util = "0.3"`.
  2. Implement `ws/client.rs`: `WsClient` struct with `ws_url`, `agent_key`. `connect(user_id, channels)` → opens tokio-tungstenite connection, sends `SUBSCRIBE` messages for each channel (format: `agent.alert.<user_id>`), spawns read task that deserializes `WsResponse` → `WsEvent::Alert(AgentAlert)` or `WsEvent::Execution(ExecutionReport)` into mpsc channel.
  3. Implement `ws/stream.rs`: `EventStream` wrapping `mpsc::UnboundedReceiver<WsEvent>`. `async fn recv() → Option<WsEvent>`. Returns None on channel close.
  4. Implement exponential backoff reconnection in `WsClient`: on disconnect, sleep `backoff` (starts 1s, doubles to max 60s), reconnect, resubscribe. Events flow through same mpsc after reconnect. Max 10 retry attempts, then return error.
  5. Unit test: connect to local echo/mock WS, verify subscription JSON format, verify reconnection loop.
- **Verification**: `cargo test -- ws` passes. WebSocket types compile. Reconnection logic tested.
- **Commit message**: `feat: WebSocket client with exponential backoff reconnection`

### CP-3: `testudo listen` + `testudo journal` commands ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `testudo-cli/src/cmd.rs` (add listen/journal dispatch), `testudo-cli/src/main.rs` (wire commands), `testudo-cli/src/config.rs` (add ws_url helper)
- **Tasks**:
  1. Wire `Command::Listen` and `Command::Journal` in `main.rs` to async handlers.
  2. Implement `cmd.rs` listen handler: creates `WsClient`, connects to agent channels, loops on `EventStream::recv()`, writes each event as JSON Line to stdout. On Ctrl-C/SIGINT, exits cleanly. If agent_key empty, prints error with `testudo init` hint.
  3. Implement `cmd.rs` journal handler: creates `ApiClient`, calls `get_summary("llm", "30d")`. Prints body to stdout. If agent_key empty, prints error. Handle 401 with clear auth error.
  4. Integration test: mock HTTP + WebSocket servers, verify `listen` produces JSON Lines, `journal` prints markdown.
- **Verification**: `cargo test` passes. Manual: `testudo journal` with no key → clear error. `testudo listen` against mock → JSON Lines output.
- **Commit message**: `feat: testudo listen and journal commands with live API integration`

### CP-4: Error handling + tracing instrumentation

- **Touches**: `testudo-cli/src/api/client.rs`, `testudo-cli/src/cmd.rs`, `testudo-cli/src/main.rs`
- **Tasks**:
  1. Polish `ApiError` Display impl: user-friendly messages for each variant ("Connection refused — is the Testudo backend running?", "Unauthorized — check your agent key", "No trades found for this period", etc.).
  2. Add `tracing` instrumentation: `info!("GET /journal/agent/summary timeframe=30d")`, `warn!("WebSocket reconnecting in {}s", backoff_secs)`, `error!("WebSocket permanently failed after {} attempts", max_retries)`.
  3. `listen` command: handle connection refused (retry message to stderr), auth failure (exit 1), graceful Ctrl-C.
  4. `journal` command: handle 401, 404 (no trades → friendly message), network timeout.
  5. Initialize tracing subscriber in `main.rs` for non-Dashboard commands (Dashboard already has TUI which owns the terminal).
- **Verification**: `cargo clippy --all-targets && cargo test` passes. Manual: error messages are clear and actionable.
- **Commit message**: `fix: user-friendly error messages and tracing for API commands`

---

## Risks & Open Questions

1. **`user_id` for WebSocket** — The WS subscription requires `agent.alert.<user_id>`, not a wildcard. Options: (a) add `user_id` to config, (b) call `/onboarding/status` first to discover it, (c) infer from agent_key. Recommendation: call `/onboarding/status` as part of connect flow — it's a lightweight GET that returns user context.
2. **`common-utils` path** — Package name is `common_utils` (underscore), not `common-utils`. Must use `common_utils = { path = "..." }` in Cargo.toml.
3. **`ws_url` default** — The ws-stream service defaults to a separate port. If not configured, we derive `ws://localhost:8081` (common default). The config file comment should explain this.
4. **reqwest TLS** — Using `rustls-tls` avoids OpenSSL linking issues. Self-signed certs in dev require explicit opt-in.
5. **Date types** — The backend uses `NaiveDate` from chrono in some response types. We mirror with chrono `NaiveDate` in client types.
6. **Klines response format** — The backend returns `Vec<Candle>` from common-utils for klines. We reuse `common_utils::Candle`.
7. **`rust_decimal::Decimal`** — Already in common-utils' public API. Our client types use it for financial values.
