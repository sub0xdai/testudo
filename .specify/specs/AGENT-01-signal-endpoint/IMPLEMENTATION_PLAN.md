# AGENT-01-signal-endpoint — Implementation Plan

## Current State Summary

The exchange has a complete risk-managed trade execution pipeline (`DecisionLoop` → `RiskService` → `ExchangeApi` → `JournalService`) but only one programmatic entry point: `POST /api/v1/trades` (in `routes/trade_management.rs`), which is tightly coupled to the browser extension's `CreateTradeRequest` payload and the shadow-engine order group abstraction.

The `DecisionLoop` in `decision_loop.rs` already accepts `DecisionInput` and returns `DecisionResult`, and the `DecisionInputBuilder` provides a clean construction API. The `ExchangeApi` trait (in `services/exchange_api.rs`) is implemented for `ShadowExchangeApi`, `CexExchangeApi`, and `HyperliquidExchangeApi`. The `JournalService` (in `services/journal_service.rs`) persists `TradeCloseEvent` values, and the `journal_trades` table already has a `source` column.

What's missing is a thin API layer (`POST /api/v1/signals`) that maps an agent-native `SignalInput` payload through the existing pipeline, plus a database migration for two new journal columns (`reasoning`, `confidence`). No new dependencies are needed — all building blocks exist.

The `trade_management.rs` handler provides a useful template: it runs `DecisionLoop::execute()`, routes to the correct exchange API based on execution mode, and records in the journal. The signal endpoint will follow the same pattern but with a flat JSON body instead of the shadow-order-group-centric `CreateTradeRequest`.

---

## Checkpoints

### CP-1: Endpoint scaffold + shadow-mode execution ✅

Completed 2026-05-20 by /skill:vox build.

- **Touches**: `crates/router/src/routes/signal.rs` (NEW), `crates/router/src/models/agent_signal.rs` (NEW), `crates/router/src/types/routes.rs`, `crates/router/src/routes/mod.rs`, `crates/router/src/main.rs`
- **Tasks**:
  1. Create `crates/router/src/models/agent_signal.rs` with `SignalInput`, `SignalResult`, `SignalRejection`, `SignalSide`, `TakeProfitTarget`, `SignalManagement`, `TrailingStopConfig`, `PartialTpConfig`, and `ExecutionMode` (wire-level).
  2. Create `crates/router/src/routes/signal.rs` with the `create_signal` handler — validate input, convert `SignalInput` → `DecisionInput` via `DecisionInputBuilder`, run `DecisionLoop::execute()`, on approval place a shadow order via `ShadowExchangeApi::place_order()`, return `SignalResult::<200>`.
  3. Register `pub mod signal;` in `crates/router/src/routes/mod.rs`.
  4. Wire `POST /api/v1/signals` in `crates/router/src/main.rs` with JWT middleware.
- **Verification**: `cargo test -p router -- signal` produces passing tests for: valid shadow signal → 200 with trade_group_id, signal rejected 422 when stop loss missing + `require_stop_loss: true`, signal rejected 422 when max positions reached, unauthenticated → 401.
- **Commit message**: `feat: POST /api/v1/signals with shadow mode, decision loop, and unit tests`

### CP-2: Journal attribution — reasoning, source, confidence ✅

Completed 2026-05-20 by /skill:vox build.

- **Touches**: `crates/sqlx_postgres/migrations/` (NEW migration), `crates/router/src/models/journal.rs`, `crates/router/src/services/journal_service.rs`, `crates/router/src/routes/signal.rs`
- **Tasks**:
  1. Create migration `YYYYMMDD000000_add_agent_attribution_columns` with `ALTER TABLE journal_trades ADD COLUMN reasoning TEXT`, `ALTER TABLE journal_trades ADD COLUMN confidence NUMERIC(3,2)` (source already exists).
  2. Add `reasoning: Option<String>` and `confidence: Option<Decimal>` to `JournalTrade` struct in `crates/router/src/models/journal.rs`.
  3. Add `reasoning` to the `SELECT` and `INSERT` lists in `JournalService::record_trade_close()`.
  4. Pass `reasoning` and `confidence` through `TradeCloseEvent` when recording a signal-created trade.
  5. Update the signal route to write `reasoning`, `source`, and `confidence` to the journal after order placement.
- **Verification**: `cargo test -p router` passes; manual test: POST a signal with reasoning/source/confidence, read the trade back via GET /api/v1/journal/trades and confirm the three fields are present.
- **Commit message**: `feat: agent attribution (reasoning, source, confidence) in journal`

### CP-3: Live-mode routing (CEX + Hyperliquid)

- **Touches**: `crates/router/src/routes/signal.rs`, `crates/router/src/main.rs` (AppState wiring)
- **Tasks**:
  1. Add `cex_client`, `hl_auth_cache`, `hl_universe`, `hl_network`, and `exchange_account_repo` access to the signal handler via `AppState`.
  2. When `execution_mode = "live"` and account is CEX: call `CexExchangeApi::place_order()` with bracket SL/TP.
  3. When `execution_mode = "live"` and account is Hyperliquid: call `HyperliquidExchangeApi::place_order()`.
  4. On definitive exchange rejection, attempt rollback of the shadow order. On ambiguous errors, keep shadow order tracked with a warning.
  5. Wire `pg_notify` broadcast after signal execution (same channel as human trades).
- **Verification**: `cargo test -p router -- signal` passes tests for: live CEX signal routed to correct exchange, live HL signal routed to correct exchange, account ownership validation, pg_notify fires after execution. Integration manually verifiable with sandbox accounts.
- **Commit message**: `feat: live-mode signal execution via CEX and Hyperliquid exchange APIs`

### CP-4: Idempotency + error handling polish ✅

- **Touches**: `crates/router/src/routes/signal.rs`, `crates/sqlx_postgres/migrations/` (NEW migration), `crates/db-processor/src/query.rs`
- **Tasks**:
  1. Create migration for `signal_events` table: `id UUID PK`, `idempotency_key UUID UNIQUE`, `user_id UUID`, `response JSONB`, `created_at TIMESTAMPTZ`.
  2. On signal POST, check `signal_events` for the idempotency key. If found → 409 with cached result.
  3. On first processing, INSERT into signal_events after successful execution.
  4. Add validation error responses: missing required fields → 400 with clear error message, invalid symbol format → 400, invalid side → 400.
  5. Map all 8 risk rejection variants to 422 with human-readable `SignalRejection { code, reason }`.
  6. Add unit tests for: duplicate key → 409, missing `symbol` → 400, invalid `side` → 400, drawdown exceeded → 422, max positions reached → 422.
- **Verification**: `cargo test -p router -- signal` passes all idempotency and error-path tests. Run `cargo clippy --all-targets && cargo test` in testudo-exchange — all green.
- **Commit message**: `feat: idempotency key dedup, signal_events table, error path hardening`

---

## Risks & Open Questions

1. **Agent auth scope** — The user's full bearer token gives the agent the same permissions as the user. This is acceptable for v1; a follow-up spec (AGENT-03 or later) should add scoped API keys with per-agent limits.
2. **Rate limiting** — Not implemented in this spec. The existing JWT middleware rate limiter (10 req / 15 min per IP) provides baseline protection. A dedicated per-user token bucket on `/api/v1/signals` should be added in a follow-up.
3. **Signal events table TTL** — The spec mentions a `signal_events` table for idempotency but doesn't specify a retention window. CP-4 will default to no TTL (keep forever); a follow-up should add a cleanup job for entries older than 90 days.
4. **Journal recording for live signals** — The journal is currently written by the `TradeEventWriter` after fills. For signal-initiated trades that fill asynchronously, the journal write may happen at fill time (via the existing pipeline) rather than immediately. CP-2 should record a placeholder journal entry at signal creation, with full P&L filled in later by the TradeEventWriter.
5. **Hyperliquid auth re-authorization** — Agent wallet expiry is handled by the existing `ExchangeApiError::AgentWalletInactive` error path. No new auth logic needed.

---

Plan ready: 4 checkpoints, ~8–12 hours total. Run `/skill:vox build AGENT-01-signal-endpoint` to start CP-1.
