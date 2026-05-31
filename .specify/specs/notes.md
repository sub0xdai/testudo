
- [ ] Break-even trigger, trailing stop, partial tp (perhaps remove that) not important for now
- [ ] change pw in env on n0x

## Strategy Verification Series (STRAT-01)

Blueprint: `strat-lean-proofs.md` (project root)

- [x] **STRAT-01-lean-proofs** — `testudo-proofs/` with 7 Lean 4 theorems, TOML artifacts, `verify-artifacts.py`. Consumed by CLI-06.

---

## Trading Harness Series (CLI-01 → CLI-06)

Blueprint: `agent-integration-blueprint.md` (project root)
Goal: single binary (`testudo`) that onboards, trades, journals, and monitors. pi.dev for trading.

Workflow:
```
/skill:vox plan CLI-01-core-tui          # gap analysis → IMPLEMENTATION_PLAN.md
/skill:vox build CLI-01-core-tui         # CP-1 (repeat for each checkpoint)
# ... repeat through CLI-06 ...
```

### CLI-01 — Core TUI Scaffold
- [x] **CLI-01-core-tui** — `testudo-cli/` crate, clap CLI (7 subcommands, 6 stubs), TEA event loop (`tokio::select!`), ratatui dashboard with 6 panes + status bar, config loading from `~/.config/testudo/config.toml` (auto-creates defaults), screen navigation (F1-F4). `testudo dashboard` opens a working TUI.

### CLI-02 — API Client + Network
- [ ] **CLI-02-api-client** — Typed REST client for all 7 Testudo endpoints (signals, journal, klines, exchanges, onboarding, risk, agent-keys). WebSocket client with exponential backoff reconnection. `testudo listen` streams JSON Lines to stdout. `testudo journal` prints markdown summary. `X-Agent-Key` header injection. Type sharing via `common-utils` path dependency.

### CLI-03 — Agent Loop + LLM + Tools
- [ ] **CLI-03-agent-loop** — LLM provider trait + Anthropic implementation (OpenAI second). 7 typed tools (fetch_klines, submit_signal, read_journal, write_journal, list_positions, check_risk, check_onboarding) with JSON Schema + OpenAI function calling. `testudo agent start` autonomous loop: observe → think → act → journal → sleep. Signal idempotency (UUIDv4). Journal write-after-signal (pre-trade thesis + post-trade from execution reports). Agent phase tracking.

### CLI-04 — Strategy Registry + Risk + Init
- [ ] **CLI-04-strategy-registry** — Strategy TOML loading from `~/.config/testudo/strategies/`. 3 built-in strategies (mean-reversion, momentum-breakout, funding-arb). `testudo strategy list/add/show/remove`. Client-side risk pre-check (leverage, positions, symbols, drawdown). `testudo agent start --strategy <name>` loads strategy prompt + constraints + tool filter. `testudo init` 5-step TUI onboarding wizard (URL → auth → exchange → risk → save).

### CLI-05 — Daemon + TUI Polish + Integration
- [ ] **CLI-05-daemon-polish** — `testudo agent start --daemon` headless mode with file logging (daily rotation, JSON). Unix domain socket for control (`status`, `stop`, `attach`). `testudo attach` reconnects TUI to running daemon (read-only, `q` to detach). All 6 TUI panes wired to live data (positions, P&L sparkline, signal log, risk gauge, agent stream, journal stats). Integration test suite with mock LLM + mock HTTP backend. `AGENT_TRADING.md` updated with `testudo`-first workflow.

### CLI-06 — Strategy System Bridge
- [ ] **CLI-06-strategy-system** — Bridge connecting STRAT-01 Lean proofs to the harness. `StrategyLoader` loads `.toml` artifacts, `ConstraintMerger` combines constraints (most conservative wins) + intersects with user risk config (user can only tighten), `ToolConstrainer` bakes proof-derived bounds into LLM tool JSON Schemas, `StrategyValidator` cross-references strategies against proofs. `testudo strategy validate <name>` CLI.

---

## Completed (Backend Agent Infrastructure)

All backend endpoints the harness depends on are implemented and tested:

- [x] **AGENT-01-signal-endpoint** — `POST /api/v1/signals`, shadow + live execution, journal attribution
- [x] **AGENT-02-websocket-alerts** — `agent.alert.*`, `agent.execution.*` WebSocket channels
- [x] **AGENT-03-journal-memory** — Bidirectional journal API (summary/insights/compare)
- [x] **AGENT-06-onboarding-status** — `GET /onboarding/status` single-call discovery
- [x] **AGENT-07-agent-api-keys** — Scoped `testudo_sk_...` keys, `X-Agent-Key` auth middleware


