# Specification: Agent WebSocket Alert Channels

**Spec ID:** AGENT-02-websocket-alerts
**Date:** 2026-05-20
**Status:** Draft
**Class:** Feature / Infrastructure
**Priority:** P1 — agents need real-time awareness; polling REST is too slow and expensive
**Depends on:** AGENT-01-signal-endpoint (alerts are most valuable once agents are trading)
**Series:** AGENT-01 through AGENT-03 (Agent Integration Blueprint)

---

## Problem Statement

Agents trading on Testudo have no real-time awareness of exchange state. To check if an order filled, if a stop-loss triggered, or if a drawdown limit was breached, the agent must poll REST endpoints (`GET /api/v1/trades/{id}`, `GET /api/v1/exchanges/accounts/{id}/positions`). This is both inefficient (wasted HTTP round-trips) and slow — an agent may react to a stop-loss fill 5–10 seconds after it happened, or miss a drawdown warning entirely between polls.

The WebSocket server on port 4000 already broadcasts real-time events to the browser extension and desk dashboard via PostgreSQL `LISTEN/NOTIFY`. Channels exist for `order.{user_id}`, `depth.{symbol}`, `trade.{symbol}`, `ticker.{symbol}`, and `balance.{user_id}`. But these are designed for human-facing UI updates — they lack agent-specific event types (risk breaches, execution latency reports, agent wallet expiry) and contextual metadata (confidence score, source attribution).

Adding agent-specific channels requires no new infrastructure. The `pg_queue` crate already provides pub/sub via `LISTEN/NOTIFY`. The `ws-stream` crate already handles subscription management and message routing. The router already calls `pg_notify()` after every order event. Agent channels are additional notification topics and message payloads — configuration, not architecture.

---

## User Stories

- **As a coding agent**, I want to receive real-time alerts when my risk limits are approached, so that I can pause trading or reduce position size before hitting a hard limit.
- **As a coding agent**, I want execution reports streamed to me with latency breakdowns, so that I can monitor exchange performance and detect anomalies.
- **As a user**, I want to know when my agent's wallet is about to expire or needs re-authorization, so that I don't lose trading time.
- **As a strategy developer**, I want streaming balance updates after each fill, so that my agent can dynamically adjust position sizing.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | WebSocket channel `agent.alert.{user_id}` pushes risk breaches, drawdown warnings, margin calls, and agent wallet expiry events | High | ws-stream |
| FR-2 | WebSocket channel `agent.execution.{user_id}` pushes fill confirmations, order rejections, cancellation reports, and latency breakdowns | High | ws-stream |
| FR-3 | WebSocket channel `agent.order.{user_id}` mirrors `order.{user_id}` but includes agent-specific fields (source, confidence, reasoning summary) | Medium | ws-stream |
| FR-4 | WebSocket channel `agent.balance.{user_id}` pushes balance snapshots after each fill, including daily PnL and drawdown percentage | Medium | ws-stream |
| FR-5 | All agent channels use the same `SUBSCRIBE`/`UNSUBSCRIBE` message format as existing channels | High | ws-stream |
| FR-6 | The router emits `pg_notify('agent_alert', payload)` alongside existing notifications — no new pub/sub mechanism | High | Router |
| FR-7 | Risk breach alerts include: severity (info/notable/concerning), message, current value, limit value, timestamp | High | Router |
| FR-8 | Execution reports include: trade_group_id, order_id, status, fill_price, exchange, latency_ms, timestamp | Medium | Router |
| FR-9 | Agent wallet expiry alerts fire 24h and 1h before expiry, and immediately on expiration | Medium | Router |
| FR-10 | Agent balance updates fire after every fill (not on every ticker update — throttled to fill events) | Medium | Router |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Add `agent.alert.{user_id}` channel to ws-stream, hardcode a test alert | Subscribe/unsubscribe works, alert payload received by client |
| CP-2 | Wire risk breach detection to `pg_notify('agent_alert', ...)` in router | Drawdown warning fires when `daily_drawdown_percent >= 0.8 * limit` |
| CP-3 | Wire execution reports to `pg_notify('agent_execution', ...)` in router | Fill events include latency_ms, status, fill_price |
| CP-4 | Add agent wallet expiry detection + alert emission | Expiry alert fires at 24h/1h/0h before wallet expires |

### Channel Definitions

```rust
// crates/ws-stream/src/types.rs — add to existing channel enum

pub enum AgentChannel {
    Alert,
    Execution,
    Order,
    Balance,
}

impl AgentChannel {
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Alert     => "agent.alert",
            Self::Execution => "agent.execution",
            Self::Order     => "agent.order",
            Self::Balance   => "agent.balance",
        }
    }

    pub fn channel_name(&self, user_id: Uuid) -> String {
        format!("{}.{}", self.prefix(), user_id)
    }
}
```

### Alert Payload Schema

```rust
// crates/ws-stream/src/types.rs

#[derive(Debug, Serialize)]
pub struct AgentAlert {
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub current_value: Option<Decimal>,
    pub limit_value: Option<Decimal>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertType {
    RiskBreach,
    DrawdownWarning,
    DrawdownLimit,
    MarginCall,
    AgentWalletExpiring,
    AgentWalletExpired,
    MaxPositionsReached,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Info,
    Notable,
    Concerning,
}

#[derive(Debug, Serialize)]
pub struct ExecutionReport {
    pub trade_group_id: Uuid,
    pub order_id: String,
    pub status: String,           // "filled", "rejected", "cancelled", "expired"
    pub fill_price: Option<Decimal>,
    pub exchange: String,         // "hyperliquid", "binance", "bybit", etc.
    pub latency_ms: u64,
    pub timestamp: DateTime<Utc>,
}
```

### Notification Wiring

The router already emits `pg_notify()` in these locations. AGENT-02 adds agent-specific notifications alongside:

| Router Location | Existing Notification | New Agent Notification |
|---|---|---|
| `execution_service.rs` after place_order | `pg_notify('order_events', ...)` | `pg_notify('agent_execution', execution_report)` |
| `trade_manager/evaluator.rs` after risk check | (none) | `pg_notify('agent_alert', alert)` when drawdown ≥ 80% limit |
| `trade_manager/service.rs` after fill detection | `pg_notify('order_events', ...)` | `pg_notify('agent_execution', ...)` with latency breakdown |
| `hyperliquid/auth.rs` on wallet load | (none) | `pg_notify('agent_alert', wallet_expiry)` when < 24h from expiry |

### Paved Roads

- `pg_queue::notify()` — already used across the router for all order/trade/balance events
- `ws-stream/src/pg_ws_manager.rs` — `PgWsManager` already handles LISTEN subscription lifecycle
- `ws-stream/src/user.rs` — user session management, subscription tracking
- `RiskService::validate()` in `common_utils/src/risk/service.rs` — already emits `RiskWarning::ApproachingDrawdownLimit` when drawdown ≥ 80% limit
- `CexExchangeApi::place_order()` and `HyperliquidExchangeApi::place_order()` — order results already include status, fill price; just need to compute latency

### Files

- `crates/ws-stream/src/types.rs` — add `AgentAlert`, `ExecutionReport`, `AlertType`, `AlertSeverity` types + `AgentChannel` enum
- `crates/ws-stream/src/pg_ws_manager.rs` — add LISTEN for `agent_alert`, `agent_execution` PostgreSQL channels
- `crates/ws-stream/src/user.rs` — extend subscription handling to parse `agent.alert.*`, `agent.execution.*`, etc.
- `crates/router/src/services/agent_alert.rs` — **NEW** — centralized alert emission: `emit_alert(user_id, alert)`, `emit_execution_report(user_id, report)`
- `crates/router/src/services/execution_service.rs` — call `emit_execution_report` after order placement
- `crates/router/src/services/trade_manager/evaluator.rs` — call `emit_alert` on drawdown approach
- `crates/router/src/services/hyperliquid/auth.rs` — call `emit_alert` on wallet expiry detection

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `agent.alert.{user_id}` channel subscribable via WebSocket `SUBSCRIBE` message
- [ ] `agent.execution.{user_id}` channel subscribable via WebSocket `SUBSCRIBE` message
- [ ] Drawdown warning alert fires when `daily_drawdown_percent >= 0.8 * daily_max_drawdown_percent`
- [ ] Drawdown limit alert fires when `daily_drawdown_percent >= daily_max_drawdown_percent`
- [ ] Execution report fires after every order placement with `status`, `fill_price` (if filled), `latency_ms`
- [ ] Execution report fires after fill detection (SL/TP hits) via trade manager
- [ ] Agent wallet expiry alert fires when `expires_at - now() < 24h` and `expires_at - now() < 1h`
- [ ] All alert/execution messages serialize as valid JSON matching the type schemas above
- [ ] Existing channels (`order.{user_id}`, `balance.{user_id}`, etc.) continue to work unchanged
- [ ] `UNSUBSCRIBE` stops delivery for the specified agent channels
- [ ] WebSocket server does not crash on malformed agent channel subscription requests
- [ ] `cargo clippy --all-targets && cargo test` passes in testudo-exchange
- [ ] Unit tests cover: alert emission on drawdown, execution report on fill, wallet expiry detection

---

## Risks

1. **Notification volume** — If an agent trades frequently (scalping), execution reports could be noisy. Mitigation: execution reports fire on fill events only (not on every order status change). Scalping agents subscribe to `agent.order.{user_id}` for full lifecycle.
2. **pg_notify payload size** — PostgreSQL `NOTIFY` payloads have a practical limit of ~8000 bytes. Mitigation: alerts are small JSON payloads (< 500 bytes). Execution reports include only essential fields.
3. **Client desync** — If a WebSocket client disconnects, it misses events. Mitigation: agents should reconcile on reconnect by calling `GET /api/v1/trades` and `GET /api/v1/exchanges/accounts/{id}/positions`. Document this in agent SDK.

---

## Completion Signal

This spec is complete when:
1. Four agent WebSocket channels are subscribable and deliver events correctly
2. Router emits risk alerts, execution reports, and wallet expiry notifications via pg_notify
3. All 13 acceptance criteria met
4. `cargo clippy --all-targets && cargo test` passes
5. Code committed to master
