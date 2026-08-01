# Specification: Agent Signal Endpoint — Programmatic Trade Execution

**Spec ID:** AGENT-01-signal-endpoint
**Date:** 2026-05-20
**Status:** Draft
**Class:** Feature / API
**Priority:** P0 — unlocks all coding-agent integrations; no programmatic trade path exists today
**Depends on:** None (first in series)
**Series:** AGENT-01 through AGENT-03 (Agent Integration Blueprint)

---

## Problem Statement

Trading on Testudo today requires the browser extension: a user draws a position tool on TradingView, hits Alt+X, the extension scrapes the DOM, shows a modal, and after double-Enter confirmation, POSTs to `/api/v1/trades`. There is no programmatic entry point for external agents — no REST endpoint, no SDK method, no headless path.

A coding agent (Hermes, OpenClaw, pi-harnessed Claude) cannot submit a trade signal. The agent may have analyzed charts, processed on-chain events, evaluated sentiment, or run a strategy backtest — but it has no API to act on its conclusions. The extension's DOM-scraping path is inherently browser-bound, fragile across TradingView updates, and not designed for machine-to-machine communication.

The exchange already has the complete infrastructure for risk-managed trade execution:
- `DecisionLoop` (orchestrates validation + sizing)
- `RiskService` (8 safety checks, 4 sizing methods)
- `CexExchangeApi` / `HyperliquidExchangeApi` / `ShadowExchangeApi` (order routing)
- `journal_service` (trade recording with full metadata)

What's missing is a thin API layer that exposes this pipeline to external agents with proper attribution.

---

## User Stories

- **As a coding agent**, I want to submit a trade signal via REST API, so that I can execute trades programmatically without browser automation.
- **As a strategy developer**, I want to run my strategy in shadow (paper) mode first, so that I can validate performance before risking real capital.
- **As a user**, I want agent trades recorded in my journal with source attribution, so that I can audit what my agents are doing.
- **As a platform operator**, I want all agent signals to pass through the same risk engine as human trades, so that no trade bypasses safety checks.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `POST /api/v1/signals` accepts a trade signal with symbol, side, entry price, stop loss, take profit targets, execution mode, reasoning, source, confidence, and idempotency key | High | Router |
| FR-2 | Signal passes through `DecisionLoop::execute()` — never bypasses the risk engine | High | Router |
| FR-3 | Signal supports `execution_mode: "shadow"` (paper trading via ShadowExchangeApi) and `"live"` (real execution via CexExchangeApi or HyperliquidExchangeApi) | High | Router |
| FR-4 | `reasoning` (free-text) and `source` (agent identifier, e.g. `"agent:hermes_v1.2"`) fields stored in journal alongside the trade | High | Router |
| FR-5 | `confidence` (float 0–1) stored in journal for future Kelly criterion calibration | Medium | Router |
| FR-6 | Idempotency key prevents double-execution (same key → 409 Conflict if already processed) | High | Router |
| FR-7 | Response includes `trade_group_id`, `entry_order_id`, `sizing_method`, `position_size`, `warnings`, and `rejection` reason (if any) | High | Router |
| FR-8 | Signal endpoint requires authentication (SIWE bearer token) — same auth as all other trade endpoints | High | Router |
| FR-9 | Signal validates `exchange_account_id` exists and belongs to the authenticated user | Medium | Router |
| FR-10 | Agent signals emit the same `pg_notify` events as human trades, so WebSocket subscribers receive order updates | Medium | Router |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | `POST /api/v1/signals` with shadow mode, minimal payload (symbol, side, entry, stop, tp) | 200 with trade_group_id, decision loop runs, shadow order created |
| CP-2 | Add `reasoning`, `source`, `confidence` to journal writes | Journal records show agent attribution |
| CP-3 | Live mode — route to CexExchangeApi or HyperliquidExchangeApi based on account | Real order on exchange, bracket SL/TP placed |
| CP-4 | Idempotency key dedup + error paths | 409 on duplicate key, 400 on missing fields, 422 on risk rejection |

### Key Types

```rust
// crates/router/src/models/agent_signal.rs

/// Signal submitted by an external agent.
#[derive(Debug, Deserialize)]
pub struct SignalInput {
    pub symbol: String,
    pub side: SignalSide,
    pub entry_price: Decimal,
    pub stop_loss: Option<Decimal>,
    pub take_profit: Vec<TakeProfitTarget>,
    pub exchange_account_id: Option<Uuid>,
    pub execution_mode: ExecutionMode,
    pub reasoning: Option<String>,
    pub source: String,              // e.g. "agent:hermes_v1.2"
    pub confidence: Option<Decimal>, // 0.0–1.0
    pub idempotency_key: Uuid,
    pub leverage: Option<u8>,
    pub management: Option<SignalManagement>,
}

#[derive(Debug, Deserialize)]
pub struct TakeProfitTarget {
    pub price: Decimal,
    pub quantity: Decimal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SignalSide {
    Long,
    Short,
}

#[derive(Debug, Deserialize)]
pub struct SignalManagement {
    pub break_even_enabled: Option<bool>,
    pub break_even_at: Option<Decimal>,
    pub trailing_stop: Option<TrailingStopConfig>,
    pub partial_tp: Option<PartialTpConfig>,
}

/// Result returned to the agent after signal processing.
#[derive(Debug, Serialize)]
pub struct SignalResult {
    pub success: bool,
    pub trade_group_id: Option<Uuid>,
    pub entry_order_id: Option<String>,
    pub position_size: Option<Decimal>,
    pub sizing_method: Option<SizingMethod>,
    pub execution_mode: ExecutionMode,
    pub warnings: Vec<String>,
    pub rejection: Option<SignalRejection>,
}

#[derive(Debug, Serialize)]
pub struct SignalRejection {
    pub reason: String,
    pub code: String,
}
```

### Signal Processing Pipeline

```
POST /api/v1/signals
    │
    ▼
1. Validate input (all required fields present, Decimal parsing)
    │
    ▼
2. Check idempotency — query signal_events table for key
    │  → 409 Conflict if already processed (return cached result)
    ▼
3. Load account → verify user owns account, fetch balance + positions
    │
    ▼
4. Build DecisionInput via DecisionInputBuilder
    │  · symbol, side, entry_price, stop_loss, leverage, execution_mode
    ▼
5. DecisionLoop::execute(input, account, market_data)
    │  · RiskService::validate() → 8 checks + 4 sizing methods
    │  · Returns DecisionResult { approved, position_size, sizing_method, rejection, warnings }
    ▼
6a. REJECTED → return 422 with SignalResult.rejection
6b. APPROVED + shadow → ShadowExchangeApi::place_order()
6c. APPROVED + live → CexExchangeApi::place_order() or HyperliquidExchangeApi::place_order()
    │
    ▼
7. Record in journal with reasoning + source + confidence fields
    │
    ▼
8. pg_notify → WebSocket broadcasts to subscribed clients
    │
    ▼
9. Return 200 with SignalResult
```

### Route Wiring

```rust
// crates/router/src/routes/mod.rs — add to configure_routes()
cfg.route("/api/v1/signals", web::post().to(signal::create_signal));
```

### Paved Roads

- `DecisionLoop::execute()` in `decision_loop.rs` — already accepts `DecisionInput`, returns `DecisionResult`. Signal handler converts `SignalInput` → `DecisionInput` via `DecisionInputBuilder`.
- `ExchangeApi` trait in `services/exchange_api.rs` — `place_order()`, `get_balance()`, `get_position()` already implemented for Shadow, CEX, and HL.
- `journal_service.rs` — `record_trade_close()` and trade_event writing already handle metadata. Add `reasoning` and `source` columns.
- `TradePayloadSchema` in `testudo-extension/src/schemas.ts` — Zod schema for trade payloads. Signal schema is a superset.
- Auth middleware (`middleware/auth.rs`) — `AuthenticatedUser` extractor already guards trade routes.

### Files

- `crates/router/src/routes/signal.rs` — **NEW** — POST handler, input validation, idempotency check
- `crates/router/src/services/agent_signal.rs` — **NEW** — orchestrates SignalInput → DecisionInput → ExchangeApi → journal → response
- `crates/router/src/models/agent_signal.rs` — **NEW** — SignalInput, SignalResult, SignalRejection types
- `crates/router/src/routes/mod.rs` — route registration
- `crates/router/src/types/routes.rs` — add SignalInput query/body types
- `crates/db-processor/src/query.rs` — idempotency key lookup query
- `crates/sqlx_postgres/migrations/` — add `reasoning`, `source`, `confidence` columns to trade tables (or journal events table)

### Database Changes

```sql
-- Add agent attribution columns to trade_events or trade_groups
ALTER TABLE trade_groups ADD COLUMN reasoning TEXT;
ALTER TABLE trade_groups ADD COLUMN source VARCHAR(128);   -- e.g. "agent:hermes_v1.2"
ALTER TABLE trade_groups ADD COLUMN confidence NUMERIC(3,2); -- 0.00–1.00
```

### Dependencies Added

None. Reuses existing crates exclusively.

---

## Acceptance Criteria

- [ ] `POST /api/v1/signals` accepts a valid signal and returns 200 with trade_group_id
- [ ] Signal rejected with 422 when stop loss missing and `require_stop_loss: true` in risk config
- [ ] Signal rejected with 422 when max positions reached
- [ ] Signal rejected with 422 when drawdown limit exceeded
- [ ] Shadow mode creates a shadow order, no real exchange API call
- [ ] Live mode routes to correct exchange (CEX or HL) based on exchange_account_id
- [ ] Idempotency: duplicate key returns 409 with original result (no double-execution)
- [ ] Journal records show `reasoning` and `source` fields after signal execution
- [ ] `pg_notify` fires after signal execution, WebSocket subscribers receive update
- [ ] Unauthenticated requests return 401
- [ ] Invalid symbol or missing required fields return 400 with clear error message
- [ ] `cargo clippy --all-targets && cargo test` passes in testudo-exchange
- [ ] Unit tests cover: valid signal approval, rejection paths (all 8 risk checks), idempotency, invalid input

---

## Risks

1. **Agent floods signals** — An agent could submit hundreds of signals per second, overwhelming the exchange. Mitigation: rate limiting on `/api/v1/signals` (reuse existing rate limiter if present, or add per-user token bucket).
2. **Risk config bypass** — Agents might try to set per-signal risk overrides. Mitigation: `execution_mode` is the only config field agents control; risk params are pulled from the user's stored `RiskConfig` via `PUT /api/v1/risk-config`.
3. **Journal table bloat** — `reasoning` text could be large. Mitigation: `TEXT` column handles this. Consider truncation at 2000 chars if needed later.
4. **Agent auth scope** — An agent using the user's full bearer token has the same permissions as the user. Mitigation: future spec could add scoped API keys with per-agent limits.

---

## Completion Signal

This spec is complete when:
1. `POST /api/v1/signals` accepts, validates, and executes signals in shadow and live modes
2. All 13 acceptance criteria met
3. `cargo clippy --all-targets && cargo test` passes with new unit tests covering all code paths
4. Agent attribution (reasoning, source, confidence) persisted in journal
5. Code committed to master
