
- [ ] Break-even trigger, trailing stop, partial tp (perhaps remove that) not important for now
- [ ] change pw in env on n0x

## Strategy Verification Series (STRAT-01)

Blueprint: `strat-lean-proofs.md` (project root)

Workflow:
```
/skill:vox plan STRAT-01-lean-proofs    # gap analysis → IMPLEMENTATION_PLAN.md
/skill:vox build STRAT-01-lean-proofs   # CP-1 (repeat for each checkpoint)
```

- [ ] **STRAT-01-lean-proofs** — `testudo-proofs/` with 7 Lean 4 theorems (Wasserstein, Kelly, OU, momentum, funding arb, delta-neutral, gambler's ruin). Fix syntax error so `lake build` exits 0. Defines **strategy artifact format**: each proof ships with a matching `.toml` artifact (`[meta]`, `[theorem]`, `[constraints]`, `[prompt]`). Constraints are mathematically derived from the proof (e.g., `max_leverage = 5` from Quarter-Kelly). `verify-artifacts.py` cross-references `.lean` ↔ `.toml`. Consumed by AGENT-09.

## Agent Integration Series (AGENT-01 → AGENT-03)

Blueprint: `agent-integration-blueprint.md` (project root)

Workflow:
```
/skill:vox plan AGENT-01-signal-endpoint    # gap analysis → IMPLEMENTATION_PLAN.md
/skill:vox build AGENT-01-signal-endpoint   # CP-1 (repeat for each checkpoint)
# ... repeat build until all CPs complete ...
/skill:vox plan AGENT-02-websocket-alerts
/skill:vox build AGENT-02-websocket-alerts
# ... etc.
```

- [ ] **AGENT-01-signal-endpoint** — `POST /api/v1/signals`, programmatic trade execution via DecisionLoop. Shadow + live modes, agent attribution in journal.
- [ ] **AGENT-02-websocket-alerts** — `agent.alert.*`, `agent.execution.*` WebSocket channels. Risk breaches, execution reports, wallet expiry via pg_notify.
- [ ] **AGENT-03-journal-memory** — Bidirectional journal API. **Read**: `GET /journal/agent/summary` (JSON + LLM markdown), `/insights` (coach patterns), `POST /compare` (period-over-period). **Write**: `POST /journal/agent/note` (thesis/postmortem/observation), `/tag` (strategy labels), `/strategy` (named strategy persistence). Agents build persistent memory across sessions.

## Deprecated

- ~~**AGENT-04-circle-adapter**~~ — removed (Circle/Arc testnet, Agora hackathon). Slot available for reuse.

## Strategy Agent (AGENT-05)

- [ ] **AGENT-05-strategy-agent** — Autonomous mean-reversion agent (Bollinger Bands, 20-period SMA, 2σ). Polls price feed, submits signals via AGENT-01, reads journal memory via AGENT-03, respects risk alerts via AGENT-02. **Deferred** — currently hardcoded for Circle/Arc USDC settlement (FR-9 depends on removed AGENT-04). Will refactor to target Hyperliquid/CEX shadow mode when circling back.

## Agent Onboarding UX Series (AGENT-06 → AGENT-07)

Blueprint: msg-pi.md on n0x-server ("Message to Pi — Agent Onboarding UX v2").
Goal: make "set up my agent" one action — one conversation, one paste, agent trades autonomously.

Workflow:
```
/skill:vox plan AGENT-06-onboarding-status
/skill:vox build AGENT-06-onboarding-status
/skill:vox plan AGENT-07-agent-api-keys
/skill:vox build AGENT-07-agent-api-keys
```

- [ ] **AGENT-06-onboarding-status** — `GET /api/v1/onboarding/status` collapses 3-call agent discovery dance into 1 call. Returns `{is_ready, next_step, missing, available_exchanges, pending_agent_wallet, has_trades, risk_config}`. Agent gets prescriptive `next_step` enum (`authenticate`, `connect_exchange`, `approve_agent_wallet`, `configure_risk`, `ready_to_trade`) instead of interpreting raw empty arrays.
- [ ] **AGENT-07-agent-api-keys** — `POST /api/v1/agent-keys` creates scoped agent API keys (`tudo_sk_...`). `X-Agent-Key` auth middleware decouples agent identity from user SIWE. Per-key permissions (trade_execute, journal_read, journal_write, exchange_manage, risk_configure, account_read). Hashed at rest, irrecoverable after creation, independently revocable. `agent_key_id` recorded in trade_groups and journal_entries for audit trail. Makes the CEX path truly "one action" — create key once, paste into agent config, agent trades autonomously.

## Trading Harness (AGENT-08)

- [ ] **AGENT-08-trading-harness** — Purpose-built Rust TUI harness (`tudo` binary). ratatui + crossterm rendering, Elm Architecture (TEA) via `tears` (fallback: hand-rolled `tokio::select!` loop). Replaces the need for external LLM agent scaffolding. CLI: `tudo init` (onboarding via AGENT-06), `tudo agent start --strategy mean-reversion` (autonomous loop via AGENT-01/02/03), `tudo dashboard` (live P&L/positions/signals/alerts TUI), `tudo listen` (WebSocket pipe), `tudo journal` (summary). Strategy registry in TOML. 4 LLM providers (Anthropic first). Agent keys from AGENT-07 for all API calls. Client-side risk pre-check before signal submission.

## Strategy System Bridge (AGENT-09)

- [ ] **AGENT-09-strategy-system** — Bridge connecting Lean proofs (STRAT-01) to trading harness (AGENT-08). `StrategyLoader` parses proof artifacts, `ConstraintMerger` combines constraints (most conservative wins) and intersects with user risk config (user can only tighten). `ToolConstrainer` bakes proof-derived bounds into LLM tool JSON Schemas (e.g., `submit_signal.leverage.maximum = 5` from Kelly). `StrategyValidator` checks strategy constraints don't violate proven bounds. `tudo strategy validate <name>` CLI prints constraint summary with proof sources. Without this bridge, proofs are inert math.


