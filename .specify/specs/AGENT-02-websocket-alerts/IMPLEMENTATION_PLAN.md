# AGENT-02-websocket-alerts — Implementation Plan

## Current State Summary

The ws-stream crate already has a functioning WebSocket pub/sub system using PostgreSQL `LISTEN/NOTIFY`. The `PgWsManager` handles subscription lifecycle (subscribe, unsubscribe, send_to_ws_stream), the `ListenerService` manages dynamic LISTEN/UNLISTEN, and `WsResponse` routes messages to subscribers by `stream` field. Five channels exist: `depth.{symbol}`, `trade.{symbol}`, `ticker.{symbol}`, `balance.{user_id}`, `order.{user_id}`.

However, the subscription parser (`parse_subscription` in `types.rs`) assumes exactly 2-part channel names (`type.topic`). Agent channels require 3-part names: `agent.alert.{user_id}`, `agent.execution.{user_id}`, etc. This requires extending the parser to handle multi-part names and adding a new subscription category.

On the router side, `pg_notify` is already called via raw SQL (`sqlx::query("SELECT pg_notify($1, $2)")`) in `trade_event_writer.rs` and `hl_fill_journal.rs`. The `RiskService::validate()` already computes `ApproachingDrawdownLimit` warnings when drawdown ≥ 80% × limit, but these are only returned in API responses — never broadcast. There is no centralized alert emission service.

For agent wallet expiry, the `AuthCache` tracks `inserted_at` per entry, but this is in-memory only and resets on restart. The `exchange_accounts` table has no `agent_approved_at` timestamp, so CP-4 will need a migration + detection on auth load.

---

## Checkpoints

### CP-1: Agent channel types + ws-stream subscription handling ✅

Completed 2026-05-20 by /skill:vox build.

- **Touches**: `crates/ws-stream/src/types.rs`, `crates/ws-stream/src/pg_ws_manager.rs`
- **Tasks**:
  1. Add `AgentChannel` enum and `AgentAlert`, `ExecutionReport`, `AlertType`, `AlertSeverity` types to `ws-stream/src/types.rs`.
  2. Extend `WsMessage::parse_subscription()` to handle 3-part agent channel names (`agent.alert.{user_id}` → subscription ID).
  3. Add getter method to produce the channel name string for LISTEN compatibility.
  4. Unit test: subscription parse for `agent.alert.{user_id}` produces correct channel.
- **Verification**: `cargo test -p ws-stream` passes; SUBSCRIBE to `agent.alert.{uuid}` is recognized and routed.
- **Commit message**: `feat: agent channel types and ws-stream subscription parsing`

### CP-2: Risk breach alert emission via pg_notify

- **Touches**: `crates/router/src/services/agent_alert.rs` (NEW), `crates/router/src/routes/signal.rs`, `crates/router/src/services/trade_manager/evaluator.rs`
- **Tasks**:
  1. Create `crates/router/src/services/agent_alert.rs` with `emit_alert(user_id, alert)` and `emit_execution_report(user_id, report)` — calls `sqlx::query("SELECT pg_notify('agent_alert', $1)")`.
  2. In `signal.rs`, call `emit_alert` when `DecisionLoop` returns `ApproachingDrawdownLimit` or `DailyDrawdownExceeded` warnings.
  3. In `trade_manager/evaluator.rs`, call `emit_alert` when drawdown-based risk checks fire.
  4. Unit test: `ApproachingDrawdownLimit` at 80% limit produces alert with severity=Notable.
  5. Unit test: `DailyDrawdownExceeded` produces alert with severity=Concerning.
- **Verification**: `cargo test -p router -- agent_alert` passes 2 alert emission tests.
- **Commit message**: `feat: risk breach alerts via pg_notify agent_alert channel`

### CP-3: Execution report emission via pg_notify

- **Touches**: `crates/router/src/services/agent_alert.rs`, `crates/router/src/services/trade_manager/service.rs`, `crates/router/src/routes/signal.rs`
- **Tasks**:
  1. Add `emit_execution_report()` to `agent_alert.rs` — serializes `ExecutionReport` and calls `pg_notify('agent_execution', payload)`.
  2. Call `emit_execution_report` after successful order placement in `signal.rs` (both shadow and live paths).
  3. Call `emit_execution_report` after fill detection in `trade_manager/service.rs` when SL/TP fills are processed.
  4. Unit test: execution report JSON matches schema (trade_group_id, order_id, status, fill_price, exchange, latency_ms).
- **Verification**: `cargo test -p router -- agent_execution` passes structure validation test.
- **Commit message**: `feat: execution reports via pg_notify agent_execution channel`

### CP-4: Agent wallet expiry detection + alert emission

- **Touches**: `crates/sqlx_postgres/migrations/` (NEW), `crates/router/src/repositories/exchange_account.rs`, `crates/router/src/services/agent_alert.rs`, `crates/router/src/services/hyperliquid/auth.rs`
- **Tasks**:
  1. Create migration adding `agent_approved_at TIMESTAMPTZ` to `exchange_accounts`.
  2. Update `ExchangeAccountRepository` to set `agent_approved_at` on wallet approval (in `update_agent_approved()` and `insert_agent_wallet()`).
  3. In `hyperliquid/auth.rs`, on auth cache load for agent wallet, check `agent_approved_at`. If < 24h until 30 days from approval, emit `AgentWalletExpiring` alert. If expired (< 0), emit `AgentWalletExpired`.
  4. Unit test: wallet approved 29 days ago fires `AgentWalletExpiring` with `severity=Notable`, wallet approved 31 days ago fires `AgentWalletExpired` with `severity=Concerning`.
- **Verification**: `cargo test -p router -- agent_wallet` passes expiry detection tests.
- **Commit message**: `feat: agent wallet expiry detection and alert emission`

---

## Risks & Open Questions

1. **Subscription parser change is breaking** — Currently, `parse_subscription()` returns `None` for any non-2-part channel name. Agent channels are a new format, so no existing callers are affected. However, future channel formats (e.g., `agent.order.detail.{user_id}` with 4 parts) would need another extension. Design for `agent.*.*` prefix matching rather than fixed-arity parsing.
2. **Notification volume at scale** — If many agents trade simultaneously, pg_notify could become noisy. The spec mitigates this by emitting execution reports only on fills (not on every order status change). Alert emission is throttled by the drawdown check (once per trade evaluation, not per tick).
3. **Agent wallet expiry requires periodic check** — The spec asks for alerts at 24h and 1h before expiry, but there's no background scheduler. CP-4 checks on auth cache load, which may not catch the exact 1h window. A follow-up spec should add a cron-like task to periodically check all agent wallets, or rely on the ws-stream reconnection cycle (agent re-subscribes → auth reloaded → expiry check fires).
4. **ws-stream message routing** — `send_to_ws_stream` matches by `WsResponse.stream` field, which currently uses the `{:?}.{}` debug format for subscription IDs (e.g., `depth.BTC_USDT`). Agent channels will use `agent.alert.{user_id}` format. The `stream` field in `WsResponse` must match the subscription ID exactly.

---

Plan ready: 4 checkpoints, ~8–10 hours total. Run `/skill:vox build AGENT-02-websocket-alerts` to start CP-1.
