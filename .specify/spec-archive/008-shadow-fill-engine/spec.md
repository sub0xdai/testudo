# Feature: Shadow Engine Price Feed & Order Fill Integration

> Spec ID: 008-shadow-fill-engine
> Created: 2026-02-07
> Status: Complete
> Priority: P0 (Core Functionality)

---

## Overview

The Shadow Engine's `process_price_update()` method — which evaluates whether pending limit orders should fill against live market prices — is fully implemented and tested but **never called from production code**. No service bridges live price data to the engine. This means every order placed via the position tool sits at "Pending" forever: the engine never knows the price moved.

**Current:** User draws position -> limit order created in Shadow Engine -> order sits at "Pending" permanently.

**Target:** User draws position -> limit order created -> background service feeds live Binance prices -> order fills when price hits entry -> SL/TP auto-created -> position shown as "Active".

Additionally:
- The OPEN ORDERS UI shows "LONG"/"SHORT" for all orders regardless of fill status, when pending orders should show "BUY LIMIT"/"SELL LIMIT".
- The Decision Loop hardcodes a $10,000 account balance instead of querying the Shadow Engine's actual balance tracker.

---

## User Stories

- [ ] As a paper trader, I want my limit orders to fill when the market price reaches my entry level so that I can simulate real trading conditions.
- [ ] As a user, I want to see "BUY LIMIT" for my pending orders and "LONG" only after the order fills, so that I understand my order status at a glance.
- [ ] As a user, I want my risk calculations to use my actual paper trading balance, not a hardcoded $10,000, so that position sizing reflects my current equity.

---

## Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | **Price Feed Service**: A background service polls Binance ticker data for symbols with open orders and feeds bid/ask/high/low to `ShadowEngine::process_price_update()`. | High |
| FR-2 | **Active Symbol Tracking**: The Shadow Engine exposes which symbols currently have open orders, so the price feed only polls relevant markets. | High |
| FR-3 | **Order Fill Display**: Pending orders show "BUY LIMIT" / "SELL LIMIT". Filled/active positions show "LONG" / "SHORT". | Medium |
| FR-4 | **Live Balance in Decision Loop**: Risk validation uses the user's actual Shadow Engine balance instead of hardcoded $10,000. | Medium |
| FR-5 | **Graceful Degradation**: If Binance API is unreachable, the price feed logs the error and retries on the next tick without crashing. | Medium |

---

## Acceptance Criteria

- [ ] Limit orders fill within 5 seconds of market price crossing the entry level (2-second poll interval + processing).
- [ ] SL/TP orders auto-created immediately after entry order fills (existing cascade logic).
- [ ] OPEN ORDERS shows "BUY LIMIT" for pending buy orders, "LONG" for active positions.
- [ ] Position sizing uses actual USDT available balance from Shadow Engine.
- [ ] Price feed only polls symbols that have open orders (no unnecessary API calls).
- [ ] `cargo clippy --all-targets` passes with no new warnings.
- [ ] `cargo test` passes including new integration tests.

---

## Technical Notes

### Files to Create

| File | Purpose |
|------|---------|
| `crates/router/src/services/price_feed.rs` | Background price feed service |

### Files to Modify

| File | Change |
|------|--------|
| `crates/engine/src/shadow/mod.rs` | Add `get_active_symbols()` method |
| `crates/router/src/services/mod.rs` | Export `price_feed` module |
| `crates/router/src/main.rs` | Spawn price feed task at startup |
| `crates/router/src/routes/trade_management.rs` | Wire actual balance to Decision Loop |
| `testudo-web/apps/web/src/components/OpenOrders.tsx` | Status-aware order type display |

### Architecture

```
Binance REST API (existing BinanceDataService)
    | get_ticker() every 2 seconds
    |
+---v------------------------------+
| PriceFeedService (new)            |
| - Queries active symbols          |
| - Polls tickers                   |
| - Calls process_price_update()    |
+---+------------------------------+
    |
+---v------------------------------+
| ShadowEngine (existing)           |
| - process_price_update()          |
| - Fill matching (3-phase RCW)     |
| - Auto SL/TP creation            |
| - Balance updates                 |
+----------------------------------+
```

### Existing Code to Reuse

- `BinanceDataService::get_ticker(symbol)` -- `common_utils/src/services/binance_data.rs:115` -- returns `CCXTTicker` with bid, ask, high, low
- `ShadowEngine::process_price_update(symbol, bid, ask, high, low)` -- `engine/src/shadow/mod.rs:296` -- 3-phase Read-Compute-Write fill engine
- `ShadowEngine::get_balances(user_id)` -- `engine/src/shadow/mod.rs:140` -- lock-free balance query
- `ShadowEngine::get_positions(user_id)` -- `engine/src/shadow/mod.rs:151` -- position count
- `ShadowOrderManager::open_orders_by_symbol` -- `engine/src/shadow/orders.rs` -- index for active symbol detection
- `BinanceDataService::from_binance_symbol()` -- `common_utils/src/services/binance_data.rs:101` -- symbol format conversion

### Price Feed Design

```rust
// crates/router/src/services/price_feed.rs
pub struct PriceFeedService {
    engine: Arc<RwLock<ShadowEngine>>,
    binance: BinanceDataService,
    poll_interval: Duration,  // 2 seconds
}

impl PriceFeedService {
    pub async fn run(&self) {
        loop {
            let symbols = self.engine.read().await.get_active_symbols().await;

            for symbol in symbols {
                match self.binance.get_ticker(&symbol).await {
                    Ok(ticker) => {
                        let bid = ticker.bid.unwrap_or_default();
                        let ask = ticker.ask.unwrap_or_default();
                        let high = ticker.high.unwrap_or(ask);
                        let low = ticker.low.unwrap_or(bid);

                        let filled = self.engine.read().await
                            .process_price_update(&symbol, bid, ask, high, low)
                            .await;

                        if !filled.is_empty() {
                            log::info!("{} orders filled for {}", filled.len(), symbol);
                        }
                    }
                    Err(e) => {
                        log::warn!("Price feed error for {}: {}", symbol, e);
                    }
                }
            }

            tokio::time::sleep(self.poll_interval).await;
        }
    }
}
```

### Balance Wiring

```rust
// In trade_management.rs, replace lines 294-300:
let balances = engine.get_balances(user_id).await;
let usdt_balance = balances.iter()
    .find(|b| b.asset == "USDT")
    .map(|b| b.available + b.reserved)
    .unwrap_or(Decimal::from(10000));

let positions = engine.get_positions(user_id).await;

let account_state = AccountState {
    balance: usdt_balance,
    open_position_count: positions.len() as u32,
    daily_pnl: Decimal::ZERO,
    starting_balance: Decimal::from(10000),
};
```

### Display Logic

```typescript
// OpenOrders.tsx line 180, replace badge text:
{order.status === 'Pending'
  ? (isBuy ? 'BUY LIMIT' : 'SELL LIMIT')
  : (isBuy ? 'LONG' : 'SHORT')
}
```

### Dependencies

- Existing `BinanceDataService` (no new dependencies)
- `tokio::time::sleep` for polling interval
- Shared `Arc<RwLock<ShadowEngine>>` from router startup

### Assumptions

- Binance Futures API is accessible from the development environment.
- 2-second poll interval is sufficient for paper trading accuracy (not HFT).
- The 24hr high/low from ticker API is an acceptable proxy between polls.
  For more precise fills, a future spec can add Binance WebSocket streaming.

---

## Implementation Tasks

### Phase 1: Shadow Engine Extension

| Task | File | Status |
|------|------|--------|
| T1.1 | Add `get_active_symbols()` to ShadowEngine | complete |
| T1.2 | Add test for `get_active_symbols()` | complete |

### Phase 2: Price Feed Service

| Task | File | Status |
|------|------|--------|
| T2.1 | Create `PriceFeedService` in `services/price_feed.rs` | complete |
| T2.2 | Export from `services/mod.rs` | complete |
| T2.3 | Spawn in `router/main.rs` at startup | complete |
| T2.4 | Add integration test (mock ticker -> order fills) | complete |

### Phase 3: Balance Wiring

| Task | File | Status |
|------|------|--------|
| T3.1 | Replace hardcoded balance in `trade_management.rs` | complete |
| T3.2 | Verify risk calculation uses actual balance | complete |

### Phase 4: Display Fix

| Task | File | Status |
|------|------|--------|
| T4.1 | Update `OpenOrders.tsx` badge to show order type for pending | complete |

---

## Verification Commands

```bash
# Backend
cd testudo-exchange && cargo clippy --all-targets && cargo test

# Frontend
cd testudo-web/apps/web && bun run lint && bun run build
```

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Binance rate limiting | Only poll symbols with open orders; 2s interval well within limits |
| Ticker API latency (2s timeout) | Graceful error handling; skip tick on timeout |
| 24hr high/low inaccuracy | Acceptable for paper trading; WebSocket upgrade in future spec |
| Lock contention on ShadowEngine | process_price_update uses 3-phase RCW pattern; read lock for get_active_symbols |

---

## Completion Signal

### Implementation Checklist
- [ ] `PriceFeedService` runs as background task in router
- [ ] `get_active_symbols()` returns correct symbols
- [ ] Limit orders fill when market price crosses entry
- [ ] SL/TP auto-created after entry fill
- [ ] Balance query wired to Decision Loop
- [ ] OPEN ORDERS shows correct order type labels

### Testing Requirements
- [ ] `cargo clippy --all-targets && cargo test` passes
- [ ] Manual test: place order, wait for fill, verify SL/TP created
- [ ] Frontend lint and build pass

### Done Signal
When ALL above criteria are satisfied, output:
<promise>DONE</promise>

---

*Template version: 1.0*
