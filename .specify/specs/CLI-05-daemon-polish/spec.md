# Specification: Daemon Mode + TUI Polish + Integration

**Spec ID:** CLI-05-daemon-polish
**Date:** 2026-05-31
**Status:** Draft
**Class:** Feature / Application
**Priority:** P1 — makes the harness production-ready for 24/7 operation and visually complete
**Depends on:** CLI-04-strategy-registry (all harness systems)
**Series:** CLI-05 (Daemon + Polish)

---

## Problem Statement

The harness works interactively but can't run headless on a server. The TUI panes show empty placeholders — no live P&L data, no sparkline, no real signal log, no agent reasoning stream. There's no integration test that exercises the full loop end-to-end against a mock backend. Without daemon mode, the harness can't trade 24/7 on n0x. Without TUI polish, it's a dev tool, not a product.

This spec is the finishing work: daemon mode with TUI reattachment, live data in all 6 TUI panes, an integration test suite, and the final `AGENT_TRADING.md` update that makes `tudo` the recommended agent interface.

---

## User Stories

- **As an n0x operator**, I run `tudo agent start --daemon` on my server and the agent trades autonomously 24/7, logging to files, so that I don't need a terminal open.
- **As a developer**, I run `tudo attach` from any terminal and reconnect to the running daemon's TUI, so that I can check on my agents without stopping them.
- **As a trader**, I open `tudo dashboard` and see live P&L, actual positions, real signal history, and the LLM's reasoning stream, so that the TUI is a genuine operational tool.
- **As a QA engineer**, I run `cargo test` and the integration suite exercises the full observe→think→act→journal loop against a mock backend, so that regressions are caught before deployment.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `tudo agent start --daemon` runs headless: no TUI, no terminal. Agent loop runs in background. Logs to `~/.config/tudo/logs/tudo.log` with tracing (JSON format, rotated daily). Writes PID to `~/.config/tudo/tudo.pid`. | High | Daemon |
| FR-2 | Daemon exposes a Unix domain socket at `~/.config/tudo/tudo.sock`. Accepts JSON-RPC commands: `{"method": "status"}`, `{"method": "stop"}`, `{"method": "attach"}`. Status returns current phase, positions, signal count, uptime. | High | Daemon |
| FR-3 | `tudo attach` connects to the daemon's Unix socket. Opens TUI that streams the daemon's state in real-time (positions, P&L, signal log, agent reasoning, risk). Read-only — can't modify the running agent, but can view all panes. Press `q` to detach (agent keeps running). | Medium | Daemon |
| FR-4 | TUI panes wired to real data: positions pane shows live positions from API + execution reports. P&L chart renders sparkline from equity curve data. Signal log shows recent signals with status (filled/rejected/pending). Risk pane shows drawdown gauge + limits. Agent pane shows LLM reasoning stream. Journal pane shows summary stats. | High | TUI |
| FR-5 | P&L sparkline: renders ASCII line chart in the P&L pane using daily equity curve data from `GET /journal/agent/summary?format=json`. Updates every 60 seconds. | Medium | TUI |
| FR-6 | Risk pane: drawdown progress bar (████░░░░ 3.2% / 5.0%), active positions counter (2/5), session signal counter (12/30), worst-case exposure. | Medium | TUI |
| FR-7 | Integration test: full loop with mock LLM (returns predetermined tool calls) + mock HTTP backend (wiremock or httptest). Verifies: klines fetch → LLM call → submit_signal → journal write → execution report → post-trade journal. | High | Test |
| FR-8 | Integration test: daemon lifecycle — start daemon, attach TUI, verify state streaming, detach, stop daemon via socket. | Medium | Test |
| FR-9 | Update `AGENT_TRADING.md` Section 0: document `tudo init` as the recommended first step. Add `tudo`-first workflow replacing raw curl examples. Keep raw API docs as reference. | Medium | Docs |
| FR-10 | `cargo clippy && cargo test` passes in `tudo/`. Integration tests run in CI. | High | CI |

---

## Technical Implementation

### Crate Structure (additions)

```
tudo/src/
├── daemon.rs              // Daemon mode: fork/background, PID file, Unix socket, JSON-RPC
├── cmd/
│   └── attach.rs          // tudo attach handler
├── view/
│   ├── positions_pane.rs  // Live positions table with entry/current/P&L/R-multiple
│   ├── pnl_chart.rs       // ASCII sparkline from equity curve
│   ├── signal_log.rs      // Recent signals with timestamps and status icons
│   ├── risk_pane.rs       // Drawdown gauge, position counter, exposure
│   ├── agent_pane.rs      // LLM reasoning stream (tokens accumulate live)
│   └── journal_pane.rs    // Journal summary stats
├── app.rs                 // Wire daemon attach mode; route state updates to panes
└── model/
    └── state.rs           // Add daemon_state, live data fields
```

### Daemon Mode

```rust
// src/daemon.rs

/// Start the agent in daemon mode: background the process, write PID file,
/// open Unix socket, run agent loop, log to file.
pub async fn run_daemon(config: &Config, strategy_name: Option<String>) -> Result<(), Box<dyn Error>> {
    // 1. Daemonize: fork + setsid (Linux) or just background (macOS fallback)
    daemonize()?;

    // 2. Write PID file
    let pid_path = config_dir().join("tudo.pid");
    std::fs::write(&pid_path, std::process::id().to_string())?;

    // 3. Set up file logging
    let log_dir = config_dir().join("logs");
    std::fs::create_dir_all(&log_dir)?;
    let file_appender = tracing_appender::rolling::daily(&log_dir, "tudo.log");
    tracing_subscriber::fmt()
        .json()
        .with_writer(file_appender)
        .init();

    tracing::info!("Daemon started. PID: {}", std::process::id());

    // 4. Set up Unix socket for control commands
    let socket_path = config_dir().join("tudo.sock");
    let listener = UnixListener::bind(&socket_path)?;

    // 5. Start agent loop in a task
    let (state_tx, state_rx) = tokio::sync::watch::channel(DaemonState::default());
    let agent_config = config.clone();
    let agent_tx = state_tx.clone();

    tokio::spawn(async move {
        run_agent_loop(&agent_config, strategy_name, agent_tx).await;
    });

    // 6. Accept control connections
    loop {
        let (stream, _) = listener.accept().await?;
        let state_rx = state_rx.clone();
        tokio::spawn(handle_control_connection(stream, state_rx));
    }
}

/// Shared state that the daemon publishes via watch channel.
#[derive(Debug, Clone, Serialize)]
pub struct DaemonState {
    pub phase: String,
    pub positions: Vec<PositionSnapshot>,
    pub signal_count: u64,
    pub uptime_secs: u64,
    pub last_signal: Option<SignalSnapshot>,
    pub risk: RiskSnapshot,
    pub reasoning: String,
    pub pnl_history: Vec<PnlPoint>,
}
```

```rust
// src/daemon.rs (continued)

/// Handle a control connection: parse JSON-RPC commands.
async fn handle_control_connection(
    stream: UnixStream,
    state_rx: tokio::sync::watch::Receiver<DaemonState>,
) {
    let (reader, writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    // Read JSON-RPC request
    while let Ok(Some(line)) = lines.next_line().await {
        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let response = match req.method.as_str() {
            "status" => {
                let state = state_rx.borrow().clone();
                json!({"jsonrpc": "2.0", "id": req.id, "result": state})
            }
            "stop" => {
                tracing::info!("Stop command received via socket");
                json!({"jsonrpc": "2.0", "id": req.id, "result": "stopping"})
                // Signal shutdown...
            }
            "attach" => {
                // Switch to streaming mode: send state updates every second
                handle_attach_stream(writer, state_rx).await;
                return;
            }
            _ => {
                json!({"jsonrpc": "2.0", "id": req.id, "error": {"code": -32601, "message": "Method not found"}})
            }
        };

        // Send response (one-shot for non-attach commands)
        // ...
    }
}
```

### TUI Attach

```rust
// src/cmd/attach.rs

pub async fn run_attach(config: &Config) -> Result<(), Box<dyn Error>> {
    let socket_path = config_dir().join("tudo.sock");

    if !socket_path.exists() {
        eprintln!("Error: No daemon running. Start one with 'tudo agent start --daemon'");
        std::process::exit(1);
    }

    let stream = UnixStream::connect(&socket_path).await?;

    // Send attach command
    let attach_msg = json!({"jsonrpc": "2.0", "id": 1, "method": "attach"});
    // ... write to stream ...

    // Enter TUI loop, reading state updates from stream
    run_attach_tui(stream).await?;

    Ok(())
}

async fn run_attach_tui(stream: UnixStream) -> Result<(), Box<dyn Error>> {
    // Initialize terminal
    // Spawn reader task: reads DaemonState JSON from stream, sends to mpsc channel
    // TEA loop: on state update → update model → render TUI
    // Key 'q' → close TUI, keep daemon running

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<DaemonState>();

    // Reader task
    let (read, _write) = stream.into_split();
    tokio::spawn(async move {
        let mut lines = BufReader::new(read).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(state) = serde_json::from_str::<DaemonState>(&line) {
                let _ = tx.send(state);
            }
        }
    });

    // TUI loop
    let mut model = AttachModel::default();
    loop {
        tokio::select! {
            Some(state) = rx.recv() => {
                model.state = state;
                model.dirty = true;
            }
            Some(key) = key_rx.recv() => {
                if key.code == KeyCode::Char('q') {
                    break; // Detach, daemon keeps running
                }
            }
            _ = tick.tick() => {
                if model.dirty {
                    view_attach(&terminal, &model)?;
                    model.dirty = false;
                }
            }
        }
    }

    Ok(())
}
```

### Live TUI Panes

```rust
// src/view/positions_pane.rs

pub fn render_positions(f: &mut Frame, area: Rect, positions: &[Position]) {
    let block = Block::default()
        .title(" Positions ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    if positions.is_empty() {
        let text = "No open positions";
        f.render_widget(Paragraph::new(text).block(block), area);
        return;
    }

    let rows: Vec<Row> = positions.iter().map(|p| {
        let pnl_color = if p.unrealized_pnl >= 0.0 { Color::Green } else { Color::Red };
        Row::new(vec![
            format!("{} {}", p.symbol, p.side),
            format!("${:.2}", p.entry_price),
            format!("${:.2}", p.current_price),
            Span::styled(format!("${:+.2}", p.unrealized_pnl), Style::default().fg(pnl_color)),
            format!("{:.2}R", p.r_multiple.unwrap_or(0.0)),
        ])
    }).collect();

    let table = Table::new(rows, &[
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Length(8),
    ])
    .header(Row::new(vec!["Symbol", "Entry", "Current", "P&L", "R"]))
    .block(block);

    f.render_widget(table, area);
}
```

```rust
// src/view/pnl_chart.rs

/// Render a simple ASCII sparkline from daily P&L data points.
pub fn render_pnl_chart(f: &mut Frame, area: Rect, points: &[PnlPoint]) {
    let block = Block::default()
        .title(" P&L Chart ")
        .borders(Borders::ALL);

    if points.len() < 2 {
        f.render_widget(Paragraph::new("Insufficient data").block(block), area);
        return;
    }

    // Find min/max for scaling
    let min = points.iter().map(|p| p.equity).fold(f64::INFINITY, f64::min);
    let max = points.iter().map(|p| p.equity).fold(f64::NEG_INFINITY, f64::max);
    let range = (max - min).max(1.0);

    // Render sparkline chars: ╱ ╲ ▄ ▀ █
    let height = area.height.saturating_sub(2) as usize; // minus borders
    let width = area.width.saturating_sub(2) as usize;

    let mut lines: Vec<String> = vec![String::new(); height];
    for i in 0..points.len().min(width) {
        let normalized = ((points[i].equity - min) / range * (height - 1) as f64) as usize;
        for row in 0..height {
            if row == (height - 1 - normalized) {
                lines[row].push('█');
            } else {
                lines[row].push(' ');
            }
        }
    }

    let text: Vec<Line> = lines.into_iter()
        .map(|l| Line::from(l))
        .collect();

    f.render_widget(Paragraph::new(text).block(block), area);
}
```

```rust
// src/view/risk_pane.rs

pub fn render_risk(f: &mut Frame, area: Rect, risk: &RiskSnapshot) {
    let block = Block::default()
        .title(" Risk ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let drawdown_bar = progress_bar(risk.drawdown_pct, risk.drawdown_limit_pct, 20);
    let drawdown_line = Line::from(vec![
        Span::raw("Drawdown: "),
        Span::styled(drawdown_bar, drawdown_style(risk.drawdown_pct, risk.drawdown_limit_pct)),
        Span::raw(format!(" {:.1}% / {:.1}%", risk.drawdown_pct, risk.drawdown_limit_pct)),
    ]);

    let positions_line = format!("Active: {}/{} positions", risk.active_positions, risk.max_positions);
    let signals_line = format!("Session signals: {}/{}", risk.session_signals, risk.max_signals_per_hour);
    let exposure_line = format!("Exposure: ${:.0}", risk.total_exposure);

    let text = vec![
        drawdown_line,
        Line::from(positions_line),
        Line::from(signals_line),
        Line::from(exposure_line),
    ];

    f.render_widget(Paragraph::new(text).block(block), area);
}

fn progress_bar(value: f64, max: f64, width: usize) -> String {
    let filled = ((value / max) * width as f64).min(width as f64) as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}
```

### Integration Tests

```rust
// tests/integration/loop.rs

use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn full_agent_loop_shadow_mode() {
    // 1. Start mock HTTP server
    let mock_server = MockServer::start().await;

    // 2. Mock klines endpoint
    Mock::given(method("GET"))
        .and(path("/klines"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "klines": [
                {"open": "100", "high": "105", "low": "99", "close": "102", "volume": "1000"},
                {"open": "102", "high": "108", "low": "101", "close": "107", "volume": "1200"},
            ]
        })))
        .mount(&mock_server)
        .await;

    // 3. Mock signals endpoint
    Mock::given(method("POST"))
        .and(path("/signals"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "trade_group_id": "550e8400-e29b-41d4-a716-446655440000",
            "status": "accepted",
            "execution_mode": "shadow"
        })))
        .mount(&mock_server)
        .await;

    // 4. Mock journal endpoints
    Mock::given(method("GET"))
        .and(path("/journal/agent/summary"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "overall": {"trade_count": 0, "win_rate": 0.0},
            "by_setup": [],
            "top_trades": [],
            "equity": []
        })))
        .mount(&mock_server)
        .await;

    // 5. Create config pointing to mock server
    let config = test_config(mock_server.uri());

    // 6. Run agent loop for 2 iterations with mock LLM
    let mock_llm = MockLlmClient::new()
        .with_response(LlmResponse {
            content: None,
            tool_calls: vec![
                LlmToolCall {
                    id: "toolu_001".into(),
                    name: "fetch_klines".into(),
                    arguments: json!({"symbol": "ETH_USDT", "interval": "1h", "limit": 50}),
                },
                LlmToolCall {
                    id: "toolu_002".into(),
                    name: "submit_signal".into(),
                    arguments: json!({
                        "symbol": "ETH_USDT",
                        "side": "LONG",
                        "entry_price": 3200,
                        "stop_loss": 3100,
                        "execution_mode": "SHADOW",
                        "reasoning": "Price broke above 50 SMA with volume confirmation",
                        "confidence": 0.72,
                        "source": "agent:test:v1"
                    }),
                },
            ],
            finish_reason: "tool_calls".into(),
            usage: LlmUsage::default(),
        })
        .with_response(LlmResponse {
            content: Some("Trade submitted. Waiting for next analysis window.".into()),
            tool_calls: vec![],
            finish_reason: "stop".into(),
            usage: LlmUsage::default(),
        });

    // Run the loop
    let result = run_agent_loop_with_llm(&config, None, mock_llm, 2).await;
    assert!(result.is_ok(), "Agent loop should complete without error");

    // Verify signal was submitted
    // (wiremock records all received requests — we can assert on them)
    let signal_requests = mock_server.received_requests()
        .filter(|r| r.method == Method::POST && r.url.path() == "/signals")
        .count();
    assert_eq!(signal_requests, 1, "Exactly one signal should be submitted");
}

#[tokio::test]
async fn agent_loop_respects_shadow_only() {
    // Mock LLM tries to submit LIVE signal → harness should override to SHADOW
    // ... similar setup ...

    // Verify the submitted signal has execution_mode = "SHADOW"
}
```

---

## Checkpoints

### CP-1: Daemon mode + Unix socket
- **Touches**: `tudo/src/daemon.rs` (NEW), `tudo/src/cmd/agent.rs`, `tudo/Cargo.toml`
- **Tasks**:
  1. Implement `run_daemon()`: daemonize process (fork + setsid on Linux, simple background on macOS), write PID file, set up file logging (daily rotation, JSON format), bind Unix socket.
  2. Implement Unix socket listener: accept connections, parse JSON-RPC, handle `status` (return `DaemonState`), `stop` (graceful shutdown), `attach` (streaming mode).
  3. `DaemonState` watch channel: agent loop publishes state updates (phase, positions, signals, risk) at configurable interval (default 2s).
  4. Wire `--daemon` flag in `agent start`: when set, call `run_daemon()` instead of interactive `run_agent()`.
  5. Unit test: daemon starts, writes PID file, socket appears at expected path. `status` command returns valid JSON.
  6. Unit test: `stop` command triggers graceful shutdown (loop exits, PID file removed, socket removed).
- **Verification**: `cargo test -p tudo -- daemon` passes. Manual: `tudo agent start --daemon`, check logs at `~/.config/tudo/logs/tudo.log`, `echo '{"jsonrpc":"2.0","id":1,"method":"status"}' | nc -U ~/.config/tudo/tudo.sock` returns JSON.

### CP-2: `tudo attach` + read-only TUI
- **Touches**: `tudo/src/cmd/attach.rs` (NEW), `tudo/src/daemon.rs`, `tudo/src/app.rs`
- **Tasks**:
  1. Implement `run_attach()`: connect to Unix socket, send `attach` command, enter TUI loop that receives `DaemonState` updates and renders all 6 panes.
  2. Attach TUI renders: positions (from `DaemonState.positions`), signal log (from `DaemonState.last_signal`), agent reasoning (from `DaemonState.reasoning`), risk (from `DaemonState.risk`), P&L sparkline (from `DaemonState.pnl_history`), journal (from `DaemonState.journal_summary`).
  3. `q` key detaches (TUI closes, daemon keeps running). Terminal restored.
  4. Handle daemon restart: `tudo attach` with no socket running → error message "No daemon running. Start with `tudo agent start --daemon`".
- **Verification**: Start daemon, attach TUI, verify all panes show real data, press `q` to detach, daemon still running (check PID file and logs).

### CP-3: Live TUI panes + P&L sparkline
- **Touches**: `tudo/src/view/positions_pane.rs`, `pnl_chart.rs`, `signal_log.rs`, `risk_pane.rs`, `agent_pane.rs`, `journal_pane.rs` (NEW/REWRITE), `tudo/src/app.rs`, `tudo/src/model/state.rs`
- **Tasks**:
  1. Rewrite `positions_pane.rs`: renders `Position` data from `AppState.positions`. Shows symbol, side, entry, current, P&L (colored), R-multiple. Handles empty state gracefully.
  2. Implement `pnl_chart.rs`: ASCII sparkline from `AppState.equity_curve` data. Scales to pane height. 60s refresh.
  3. Implement `signal_log.rs`: scrollable list of recent signals from `AppState.event_log`. Shows timestamp, symbol, side, status (✓ filled, ✗ rejected, ⟳ pending).
  4. Implement `risk_pane.rs`: drawdown progress bar, active positions counter, session signal counter, total exposure.
  5. Implement `agent_pane.rs`: shows `AgentState.stream_tokens` — accumulated LLM streaming tokens. Scrollable. Wraps at pane width.
  6. Implement `journal_pane.rs`: shows summary stats from `JournalCache`: 30d trade count, win rate, profit factor, avg R, total P&L, best setup.
  7. Wire data sources: `AppState` gets `positions`, `equity_curve`, `event_log`, `risk`, `journal_cache` fields. Agent loop and WebSocket client populate these.
- **Verification**: Run `tudo dashboard` against live Testudo backend. All 6 panes show real data. P&L sparkline updates over time. Signal log scrolls.

### CP-4: Integration tests
- **Touches**: `tudo/tests/integration/loop.rs` (NEW), `tudo/tests/integration/daemon.rs` (NEW), `tudo/tests/fixtures/` (NEW), `tudo/Cargo.toml`
- **Tasks**:
  1. Add `wiremock` (or `httptest`) as dev-dependency.
  2. Write `loop.rs`: mock HTTP backend + mock LLM → full observe→think→act→journal→sleep→repeat cycle. Assert: klines fetched, signal submitted with correct payload, journal entry written.
  3. Write `daemon.rs`: start daemon against mock backend, attach TUI (in test mode — pipe-based, no real terminal), verify state streaming, stop daemon.
  4. Write edge cases: shadow_only enforcement, signal rejection handling, max positions reached, network timeout + retry, config with missing agent_key.
  5. All integration tests run in CI (`cargo test`).
- **Verification**: `cargo test -p tudo` passes all integration tests. Mock server assertions verify correct API calls.

### CP-5: Documentation update
- **Touches**: `AGENT_TRADING.md`, `tudo/README.md` (NEW)
- **Tasks**:
  1. Update `AGENT_TRADING.md` Section 0 ("First Contact"): replace raw curl examples with `tudo init` as the recommended first step. Keep API reference docs for advanced users.
  2. Add Section 0.1: "Using the tudo Harness" — quick start: `tudo init` → `tudo agent start --strategy mean-reversion` → `tudo dashboard`.
  3. Add Section 0.2: "Daemon Mode" — `tudo agent start --daemon` + `tudo attach` + `tudo stop`.
  4. Add Section 0.3: "Strategy Management" — `tudo strategy list/add/remove`.
  5. Create `tudo/README.md` with build instructions, config reference, and troubleshooting.
  6. Update Quick Reference table to include `tudo` commands.
- **Verification**: Documentation reads clearly for a new user. `tudo init` flow matches docs.

---

## Acceptance Criteria

- [ ] `tudo agent start --daemon` backgrounds the process, writes PID file, opens Unix socket
- [ ] `echo '{"jsonrpc":"2.0","id":1,"method":"status"}' | nc -U ~/.config/tudo/tudo.sock` returns live state
- [ ] `tudo attach` opens TUI showing real-time daemon state, `q` detaches cleanly
- [ ] All 6 TUI panes render live data from API/WebSocket (not placeholders)
- [ ] P&L sparkline renders correctly with min/max scaling
- [ ] Risk pane shows drawdown gauge, position count, signal count
- [ ] Integration test exercises full agent loop with mock backend
- [ ] Integration test verifies daemon lifecycle (start → attach → detach → stop)
- [ ] `AGENT_TRADING.md` updated with `tudo`-first workflow
- [ ] `tudo/README.md` exists with build + config docs
- [ ] `cargo clippy --all-targets && cargo test` passes in `tudo/`

---

## Risks

1. **Unix socket on macOS** — macOS supports Unix domain sockets but some Docker/CI environments don't. Mitigation: integration tests use abstract sockets or TCP loopback in CI, with Unix socket as the default for production.
2. **Daemonization on macOS** — `fork()` is deprecated on macOS. Mitigation: use `daemonize` crate or simple backgrounding (`setsid` equivalent via `libc`). The fallback is to spawn a child process and have the parent exit.
3. **TUI performance with streaming** — Rendering the full TUI at 60fps while receiving WebSocket events and LLM tokens could cause flicker. Mitigation: ratatui differential rendering (only redraw changed cells), batch state updates (collect 100ms of tokens before redraw).

---

## Completion Signal

This spec is complete when:
1. Daemon mode works (start, status, attach, stop)
2. `tudo attach` reconnects TUI to running daemon
3. All 6 TUI panes show real live data
4. P&L sparkline, risk gauge, signal log, agent stream all functional
5. Integration tests pass against mock backend
6. `AGENT_TRADING.md` and `tudo/README.md` updated
7. `cargo clippy --all-targets && cargo test` passes in `tudo/`
8. Code committed to master

---

## Next Spec

**CLI-06-strategy-system** — Bridge connecting STRAT-01 Lean proofs to the harness. `StrategyLoader` loads proof artifacts, `ConstraintMerger` combines constraints (most conservative wins), `ToolConstrainer` bakes proof-derived bounds into LLM tool JSON Schemas, `StrategyValidator` cross-references strategies against proofs. Makes the proofs operational.
