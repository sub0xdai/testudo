# CLI-05-daemon-polish — Implementation Plan

## Current State Summary

CLI-01 through CLI-04 are complete — 123 tests, full agent loop with strategies, risk pre-check, init wizard. The daemon and TUI view panes exist as stubs: `daemon.rs` (4 lines), all 6 pane files except `dashboard.rs` and `status_bar.rs` are empty. The `Cmd::Attach` routes to "not yet implemented." No integration test directory exists. `AGENT_TRADING.md` still documents raw curl commands, not `testudo` CLI workflow.

The backend has the endpoints needed for live data: `GET /journal/agent/summary` (equity curve + stats), `GET /klines`, `POST /signals`. WebSocket provides real-time execution reports. The `ApiClient` already has typed methods for all of these.

### Gap Summary

| Requirement | Status | Detail |
|---|---|---|
| FR-1: Daemon mode (--daemon, PID, logs) | ❌ None | daemon.rs is a 4-line stub |
| FR-2: Unix socket + JSON-RPC | ❌ None | No socket code |
| FR-3: `testudo attach` command | ❌ None | Stub dispatch |
| FR-4: Live TUI panes (6 panes) | ❌ None | All pane files are stubs |
| FR-5: P&L sparkline | ❌ None | pnl_chart.rs empty |
| FR-6: Risk pane with gauges | ❌ None | risk_pane.rs empty |
| FR-7: Integration test suite | ❌ None | No directory |
| FR-8: Daemon lifecycle test | ❌ None | No directory |
| FR-9: AGENT_TRADING.md update | ❌ None | Still raw curl docs |
| FR-10: build/test | ✅ Pass | 123 tests, clean clippy |

---

## Checkpoints

### CP-1: Daemon mode + Unix socket control ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `src/daemon.rs`, `src/cmd.rs` (wire --daemon), `src/main.rs`, `Cargo.toml` (add daemonize or tokio::process)
- **Tasks**:
  1. Implement `run_daemon(config, strategy_name)`: 
     - Skip true daemonization (fork is platform-dependent). Instead: print PID to stdout, write `tudo.pid` to `~/.config/testudo/`, set up file logging via `tracing_appender` (daily rotation, structured format).
     - Start agent loop in a tokio task.
     - Bind Unix socket at `~/.config/testudo/tudo.sock`.
  2. Implement Unix socket JSON-RPC handler: accept connections, parse `{"method":"status"}` → return `DaemonState` as JSON. `{"method":"stop"}` → graceful shutdown. `{"method":"ping"}` → `{"result":"pong"}`.
  3. `DaemonState` struct: phase, signal_count, uptime_secs, last_error. Published via `tokio::sync::watch` from agent loop.
  4. Wire `--daemon` flag in `AgentAction::Start`. CLI-01 already has the clap flag (accepted but ignored). Now implement it.
  5. Unit test: daemon state struct serializes correctly. Socket path resolution. PID file write/read.
- **Verification**: `cargo test -- daemon` passes. Manual: `testudo agent start --daemon`, PID file appears, `echo '{"method":"status"}' | nc -U ~/.config/testudo/tudo.sock` returns JSON.
- **Commit message**: `feat: daemon mode with Unix socket JSON-RPC control`

### CP-2: `testudo attach` read-only TUI ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `src/cmd.rs` (run_attach), `src/main.rs`, `src/app.rs` (attach TUI loop)
- **Tasks**:
  1. Implement `run_attach()`: connect to Unix socket, send `attach` command, enter TUI loop that polls daemon state every 500ms via `status` RPC.
  2. Attach TUI renders: uses existing dashboard layout (6 panes from CLI-01) but fills panes with daemon state data instead of placeholders.
  3. `q` detaches (TUI closes, daemon continues). `Esc` also detaches.
  4. No daemon running → clear error message with `testudo agent start --daemon` hint.
  5. Unit test: attach handler exists, socket-not-found error message is clear.
- **Verification**: `cargo test -- attach` passes. Manual: start daemon, `testudo attach` opens TUI, `q` detaches.
- **Commit message**: `feat: testudo attach for read-only daemon TUI reconnection`

### CP-3: Live TUI panes (positions, signal log, agent reasoning) ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `src/view/positions_pane.rs`, `src/view/signal_log.rs`, `src/view/agent_pane.rs`, `src/app.rs`, `src/model/state.rs`
- **Tasks**:
  1. Rewrite `positions_pane.rs`: render `Vec<Position>` as a table (symbol, side, entry, current, unrealized_pnl with colors). Handles empty state.
  2. Rewrite `signal_log.rs`: render `Vec<SignalEntry>` as scrollable list with timestamp, symbol, side, status (✓/✗/⟳), P&L.
  3. Rewrite `agent_pane.rs`: render `AgentStream` — accumulated LLM tokens as wrapped text. Scrollable.
  4. Add `Position`, `SignalEntry`, `AgentStream` types to `model/state.rs` (or new model files). `AppState` gains `positions`, `signal_log`, `agent_stream` fields.
  5. `app.rs` `run_app()` updated: when running in dashboard mode against a real backend, spawn background tasks that fetch positions, signals, and journal data every 30s and update AppState.
  6. Fallback: if no data (no backend), panes show "No data — run testudo agent start" guidance — better than empty placeholders.
- **Verification**: Run `testudo dashboard` against live backend. Positions pane shows real data. Signal log populates. 3 panes show live data, 3 show "coming soon" guidance.
- **Commit message**: `feat: live positions, signal log, and agent reasoning panes`

### CP-4: P&L sparkline + risk pane + journal pane

- **Touches**: `src/view/pnl_chart.rs`, `src/view/risk_pane.rs`, `src/view/journal_pane.rs`, `src/app.rs`
- **Tasks**:
  1. Implement `pnl_chart.rs`: ASCII sparkline from `Vec<PnlPoint>` (date + cumulative_pnl). Min/max scaling, fills pane height. Triggers on 60s refresh. If < 2 data points, show "Insufficient data."
  2. Implement `risk_pane.rs`: drawdown progress bar (███░░░), active positions counter, session signal counter. Data from `RiskSnapshot` struct.
  3. Implement `journal_pane.rs`: summary stats from `AgentSummary` — 30d trade count, win rate, profit factor, avg R, total P&L.
  4. Wire data sources: `AppState` adds `equity_curve`, `risk_snapshot`, `journal_summary` fields. Background refresh tasks populate these.
  5. Sparkline algorithm: normalize daily P&L to 0-1, map to pane rows, render '█' at the correct row position.
- **Verification**: Run `testudo dashboard`. All 6 panes show real or guidance data. Sparkline renders with ≥ 2 data points. Risk gauge shows drawdown %.
- **Commit message**: `feat: P&L sparkline, risk gauge, and journal summary panes`

### CP-5: Integration tests + docs

- **Touches**: `tests/integration/loop.rs` (NEW), `AGENT_TRADING.md`, `Cargo.toml` (wiremock dev-dep)
- **Tasks**:
  1. Add `wiremock` dev-dependency.
  2. Write `tests/integration/loop.rs`: mock HTTP backend (klines, signals, journal, onboarding endpoints) + mock LLM (returns predetermined tool calls) → run agent loop for 2 iterations → assert correct API calls made, shadow_only enforced.
  3. Write edge cases: signal rejected by backend → loop continues. Network timeout → retry with same idempotency key. No agent key → clear error.
  4. Update `AGENT_TRADING.md` Section 0: add `testudo init` as first step. Add quick-start section with `testudo` commands. Keep raw API docs as reference.
  5. Manual verification checklist in README.
- **Verification**: `cargo test -- integration` passes mock tests. `cargo clippy --all-targets && cargo test` passes. AGENT_TRADING.md reads clearly.
- **Commit message**: `test: integration suite with mock backend + LLM; docs: testudo-first workflow`

---

## Risks & Open Questions

1. **No true fork/daemonize** — CLI-05 won't do `fork()` + `setsid()`. Instead, the process stays foreground but writes PID + socket files. Users can run with `nohup` or `systemd`. This is simpler and cross-platform. Real daemonization can come later.
2. **Unix socket on macOS CI** — CI environments may not support Unix sockets. Integration tests use loopback TCP or skip on unsupported platforms.
3. **Live data refresh** — Fetching API data every 30s in the TUI loop adds complexity. Keep it simple: spawn a single background task that fetches all data sources in sequence, updates AppState via Arc<Mutex<>>. The TUI renders from the shared state.
4. **`tracing-appender` dep** — Was removed in CLI-01 CP-1. Need to re-add it for daemon file logging. Was: `tracing-appender = "0.2"`.
5. **`nix` or `libc` crate** — Not needed since we skip fork(). Unix socket uses `tokio::net::UnixListener` which is in tokio with `full` features. Already present.
