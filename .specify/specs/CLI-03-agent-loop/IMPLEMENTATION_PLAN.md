# CLI-03-agent-loop — Implementation Plan

## Current State Summary

CLI-01 and CLI-02 are complete — the `testudo` binary has a TUI, config loading, typed REST/WS clients, and working `listen`/`journal` commands. The `llm/` directory has 6 stub files (anthropic, openai, gemini, ollama, client, stream), `tools/` has 8 stubs (one per tool + types), and `model/agent.rs` is a 4-line anchor stub. All are empty. The `cmd.rs` handles `Command::Agent(AgentAction::Start)` but routes it to the generic "not yet implemented" stub. `AGENT_TRADING.md` exists at project root and contains the system prompt content.

The backend lacks a `POST /journal/agent/note` endpoint (write_journal tool target) — only read endpoints exist (summary, insights, compare). The positions endpoint requires an exchange account ID (`GET /exchanges/accounts/{id}/positions`), not a simple "list all." `async-trait` is not in Cargo.toml.

### Gap Summary

| Requirement | Status | Detail |
|---|---|---|
| FR-1: LlmClient trait + Anthropic | ❌ None | 6 stub files, zero code |
| FR-2: LLM streaming | ❌ None | stream.rs empty stub |
| FR-3: 7 tool definitions | ❌ None | 8 stub files, zero code |
| FR-4: fetch_klines tool | ❌ None | Stub |
| FR-5: submit_signal tool | ❌ None | Stub |
| FR-6: agent start loop | ❌ None | Stub dispatch |
| FR-7: Phase tracking | ❌ None | model/agent.rs empty |
| FR-8: Signal idempotency | ❌ None | No signal submission code |
| FR-9: Journal write-after-signal | ❌ None | Backend has no write endpoint |
| FR-10: build/test | ✅ Pass | 55 tests, clean clippy |
| async-trait dep | ❌ Missing | Not in Cargo.toml |
| AGENT_TRADING.md | ✅ Exists | Can embed via include_str! |

---

## Checkpoints

### CP-1: LLM provider trait + Anthropic implementation ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `Cargo.toml`, `src/llm/types.rs` (NEW), `src/llm/client.rs`, `src/llm/anthropic.rs`, `src/llm/stream.rs`, `src/lib.rs`
- **Tasks**:
  1. Add `async-trait = "0.1"` to Cargo.toml.
  2. Create `llm/types.rs`: `LlmMessage`, `LlmResponse`, `LlmToolCall`, `LlmToolResult`, `LlmUsage`, `LlmError` (thiserror enum: ApiError, Deserialize, ProviderError, RateLimited).
  3. Implement `llm/client.rs`: `LlmClient` trait with `async fn send_message(messages, tools) → Result<LlmResponse, LlmError>`. `create_client(config) → Box<dyn LlmClient>` factory.
  4. Implement `llm/anthropic.rs`: `AnthropicClient` — Messages API POST to `https://api.anthropic.com/v1/messages`, `x-api-key` header, `anthropic-version: 2023-06-01`. Converts `LlmMessage` → Anthropic content blocks, `ToolDef` → Anthropic tool format (`input_schema`). Parses `tool_use` blocks from response. Handles `stop_reason: "tool_use"`.
  5. Implement `llm/stream.rs`: SSE parser for Anthropic streaming. Yields tokens via mpsc. For CP-1, implement the parsing logic — wire to TUI in CP-3.
  6. Unit test: mock HTTP server returns Anthropic tool_use response → verify parsed correctly. Mock text-only response → content extracted. Mock error response → LlmError propagated.
  7. Make `llm` module public in lib.rs.
- **Verification**: `cargo test -- llm` passes. AnthropicClient builds correct request JSON. Tool use blocks parsed with correct name + arguments.
- **Commit message**: `feat: LLM provider trait with Anthropic Messages API implementation`

### CP-2: 7 tool definitions ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `src/tools/types.rs`, `src/tools/mod.rs` (NEW), `src/tools/fetch_klines.rs`, `src/tools/submit_signal.rs`, `src/tools/read_journal.rs`, `src/tools/write_journal.rs`, `src/tools/list_positions.rs`, `src/tools/check_risk.rs`, `src/tools/check_onboarding.rs`, `src/lib.rs`
- **Tasks**:
  1. Define `ToolDef` struct and `Tool` trait in `tools/types.rs`: `fn definition() → ToolDef`, `async fn execute(args: Value) → Result<ToolResult, ToolError>`. `ToolResult { content: String }`.
  2. Implement `tools/mod.rs`: `all_tools(api: Arc<ApiClient>) → Vec<Box<dyn Tool>>`.
  3. Implement 7 tools:
     - **fetch_klines**: calls `api.get_klines()`, returns OHLCV summary (count, latest close, high/low).
     - **submit_signal**: validates required fields, generates UUIDv4 idempotency key, calls `api.submit_signal()`. Enforces `shadow_only` via config check.
     - **read_journal**: calls `api.get_summary_text("7d", "llm")`, returns markdown.
     - **write_journal**: **stub** — backend has no write endpoint. Logs entry locally via `tracing::info!` with thesis/strategy tag. Returns confirmation string.
     - **list_positions**: **stub** — requires exchange account ID. Calls `api.get_onboarding_status()` to check readiness, returns "no positions endpoint available — check TUI" message.
     - **check_risk**: calls `api.get_risk_config()`, returns formatted risk limits.
     - **check_onboarding**: calls `api.get_onboarding_status()`, returns readiness state + missing items.
  4. Each tool's `definition()` returns JSON Schema matching the spec's blueprint.
  5. Unit test: each tool definition validates structural correctness. With mock API, tools return expected data.
- **Verification**: `cargo test -- tools` passes. All 7 tools defined, definitions have valid JSON structure, executors compile and link to ApiClient.
- **Commit message**: `feat: 7 typed LLM tool definitions with JSON Schema`

### CP-3: `testudo agent start` autonomous loop ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `src/model/agent.rs`, `src/cmd.rs` (add run_agent), `src/main.rs` (wire agent start)
- **Tasks**:
  1. Implement `model/agent.rs`: `AgentState` (phase, messages, pending_calls, signal_count, mode), `AgentPhase` enum (Observing, Thinking, Acting, Idle), `AgentMode` (Shadow, Live).
  2. Implement `cmd.rs` `run_agent(config, strategy_name)`:
     - Load system prompt: `include_str!("../../AGENT_TRADING.md")` as fallback, or from strategy TOML if name provided (CLI-04).
     - Initialize ApiClient + LlmClient + 7 tools.
     - Main loop: observe (read journal + check onboarding) → think (LLM with tools) → act (execute tool calls, max 3 rounds) → journal (log iteration summary) → sleep (config.agent.loop_interval_secs).
     - Phase tracking: update AgentPhase at each step.
     - `shadow_only` enforcement: reject LIVE signals, log warning.
  3. Wire `Command::Agent(AgentAction::Start)` in `main.rs` to `run_agent()`.
  4. Add `AgentAction::Stop`/`Pause`/`Resume` as graceful shutdown stubs (print "not yet implemented" for now).
  5. Integration test: mock LLM (returns submit_signal tool call) + mock HTTP backend → verify full loop iteration completes without error.
- **Verification**: `cargo test -- agent` passes. Agent loop runs 1 iteration with mock LLM. Phase transitions correct. Shadow-only enforcement works.
- **Commit message**: `feat: autonomous agent loop with observe-think-act-journal cycle`

### CP-4: Signal idempotency + journal integration ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `src/tools/submit_signal.rs`, `src/cmd.rs`
- **Tasks**:
  1. Idempotency: `submit_signal` tool generates UUIDv4 per call. On reqwest timeout/connection-reset, retry up to 3 times with same key. On success, log trade_group_id.
  2. Pre-trade journal: after `submit_signal` success, create journal context (thesis=reasoning, tag=source, idempotency_key). For now, log via `tracing::info!` since backend lacks write endpoint.
  3. Post-trade journal: during agent loop sleep phase, poll for WebSocket execution reports (spawn background listener). On trade close, log post-trade summary.
  4. Signal rate limiting: track `signal_count` per hour. If exceeds `max_signals_per_hour` (default 5), skip and log warning.
  5. Unit test: idempotency key same across retries. Rate limiting triggers at threshold.
- **Verification**: `cargo test -- agent` passes idempotency + rate-limit tests. `cargo clippy --all-targets && cargo test` passes.
- **Commit message**: `feat: signal idempotency, rate limiting, and journal write-after-signal`

---

## Risks & Open Questions

1. **No `POST /journal/agent/note` backend endpoint** — The write_journal tool can't write to the backend. Fallback: log journal entries locally via `tracing::info!` with structured fields. The backend team should add this endpoint for full functionality.
2. **`list_positions` requires exchange account ID** — Not a simple "list all." Workaround: call onboarding status to discover accounts, or return a helpful message directing user to the TUI.
3. **Anthropic API key** — Must be set in `~/.config/testudo/config.toml` under `[llm].api_key`. The `ApiConfig.agent_key` is for Testudo backend auth, not LLM auth. Ensure users understand the distinction.
4. **LLM streaming in TUI** — The spec calls for streaming tokens to the TUI agent pane. CP-1 implements the streaming parser; CP-3 wires it to the TEA loop. Full TUI integration is complex — for CLI-03, streaming to stdout is sufficient. TUI pane wiring can come in CLI-05 (daemon polish).
5. **`include_str!` for system prompt** — Embeds AGENT_TRADING.md at compile time. Any prompt changes require rebuild. Acceptable for now — runtime loading added in CLI-04 with strategy system.
6. **Mock LLM for tests** — Integration tests need a mock LLM that returns predetermined tool calls. Use `wiremock` or a simple trait implementation that returns canned responses.
