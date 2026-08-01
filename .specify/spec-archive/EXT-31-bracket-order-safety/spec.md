# Specification: Deferred SL/TP Placement (Bracket Order Safety)

**Spec ID:** EXT-31-bracket-order-safety
**Date:** 2026-03-14 (revised 2026-03-14)
**Status:** Draft (Revision 2 — bracket approach failed, deferred placement)
**Class:** Safety / Critical Bug Fix
**Priority:** P0 — live trading safety
**Depends on:** EXT-21 (live trade execution), EXT-22 (WebSocket fill detection)

---

## Problem Statement

When Testudo places a trade, it sends **three separate `createOrder` calls** to the exchange: entry, SL, then TP. All three orders become live on the exchange immediately, regardless of whether the entry has filled.

**Consequence:** If the entry limit order is NOT filled but price sweeps through the TP level, the TP executes as a **new position in the opposite direction** (because TP is placed as a non-reduce-only limit order — WOO rejects reduce-only before a position exists).

This was observed in production: a short entry missed by a tick, price swept down, TP limit buy filled, user ended up with an unwanted long position.

**Root cause:** SL and TP exist on the exchange before the entry fills. They are not conditional on the entry.

---

## Revision History

### Rev 1 (bracket orders via CCXT) — FAILED

Attempted to use CCXT's `createOrder` with attached `stopLoss`/`takeProfit` parameters. WOO X rejected the order — CCXT WOO driver has bracket order code but `attachedStopLossTakeProfit: undefined` (unverified feature). The request routes to WOO's `/v3/trade/algoOrder` with `algoType: 'BRACKET'` which the exchange rejects.

Commit 1e2670d implemented this approach. The sidecar and backend changes work structurally but the exchange itself rejects the bracket format.

### Rev 2 (deferred placement) — CURRENT

Place only entry order on trade submission. FillDetectorService places SL/TP **after** WebSocket entry fill confirmation. SL/TP cannot fire before entry because they don't exist on the exchange until entry fills.

---

## Solution: Deferred SL/TP Placement

Instead of placing all orders upfront (broken) or using exchange-native bracket orders (rejected), defer SL/TP placement to the FillDetectorService:

1. **Trade submission** → place ONLY the entry order on the exchange
2. **Entry fills** → WebSocket event arrives at FillDetectorService
3. **FillDetectorService** → reads SL/TP prices from OrderGroup, places them on the exchange
4. **OCO logic** → existing fill detection handles SL/TP fills as before

The infrastructure already exists:
- `FillDetectorService` detects entry fills (`FillKind::Entry`, line 331 of fill_detector.rs)
- `OrderGroup` stores `stop_loss_price` and `take_profit_targets`
- `ExchangeApi` trait supports `place_order()` for SL/TP order types
- `EngineHandle` supports `register_exchange_order_id()` for order tracking

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `create_trade` places ONLY the entry order on the exchange (no SL/TP) | High | router/routes |
| FR-2 | FillDetectorService places SL order on exchange after entry fill | High | router/fill_detector |
| FR-3 | FillDetectorService places TP order on exchange after entry fill | High | router/fill_detector |
| FR-4 | FillDetectorService registers SL/TP exchange order IDs with the OrderGroup | High | router/fill_detector |
| FR-5 | Close side inferred from SL price vs entry fill price | Medium | router/fill_detector |
| FR-6 | Stamp SL/TP with `clientOrderId` convention: `testudo:{group_id}:{sl|tp}` | Medium | router/fill_detector |
| FR-7 | SL placement failure logs critical error (position live but unprotected) | Medium | router/fill_detector |
| FR-8 | TP placement failure logs warning (non-critical, matches pre-EXT-31 behavior) | Medium | router/fill_detector |
| FR-9 | Revert bracket params in sidecar handler (remove dead code) | Low | testudo-ccxt |
| FR-10 | Revert bracket fields in ccxt_client.rs and exchange_api.rs | Low | router/ccxt_client |

---

## Technical Implementation

### 1) Trade Route — Entry Only (FR-1)

**File:** `testudo-exchange/crates/router/src/routes/trade_management.rs` (~line 821-870)

Change the `place_order` call to pass `None` for bracket params:

```rust
// EXT-31 FR-1: Entry-only placement — SL/TP deferred to FillDetectorService
match tm
    .place_order(ExchangePlaceOrderRequest {
        user_id,
        symbol: req.symbol.clone(),
        side: exchange_side,
        order_type: ApiOrderType::Limit,
        quantity,
        price: Some(req.entry_price),
        stop_price: None,
        leverage,
        exchange_account_id,
        reduce_only: false,
        client_order_id: Some(entry_client_id),
        stop_loss_trigger: None,    // Deferred to fill detector
        take_profit_trigger: None,  // Deferred to fill detector
    })
    .await
```

The existing `if let Some(sl_id)` and `if let Some(tp_id)` registration blocks become harmless no-ops since the bracket result always returns `None` for those fields.

### 2) Fill Detector — Deferred SL/TP Placement (FR-2 through FR-8)

**File:** `testudo-exchange/crates/router/src/services/fill_detector.rs`

**a) Extend FillAction struct** with fields needed for deferred placement:

```rust
struct FillAction {
    group_id: Uuid,
    user_id: Uuid,
    symbol: String,
    exchange_account_id: Option<Uuid>,
    exchange_sl_order_id: Option<String>,
    exchange_tp_order_id: Option<String>,
    event_timestamp: Option<i64>,
    kind: FillKind,
    // EXT-31: Deferred placement fields
    stop_loss_price: Option<Decimal>,
    take_profit_price: Option<Decimal>,
    entry_fill_price: Decimal,
    entry_quantity: Decimal,
}
```

**b) Populate deferred fields** in the `is_entry` branch (~line 243):

```rust
} else if is_entry {
    let fill_price = event.average.or(event.price).unwrap_or(0.0);
    let fill_dec = rust_decimal::Decimal::from_f64_retain(fill_price).unwrap_or_default();
    if let Err(e) = self.engine_handle.on_entry_filled(group_id, fill_dec).await {
        tracing::error!("...");
        return;
    }
    FillAction {
        group_id,
        user_id,
        symbol,
        exchange_account_id,
        exchange_sl_order_id,
        exchange_tp_order_id,
        event_timestamp: event.timestamp,
        kind: FillKind::Entry,
        // EXT-31: Capture group prices for deferred placement
        stop_loss_price: group.stop_loss_price,
        take_profit_price: group.take_profit_targets.first().map(|t| t.price),
        entry_fill_price: fill_dec,
        entry_quantity: group.entry_quantity,
    }
```

**c) Add deferred SL/TP placement** in the `FillKind::Entry` handler (~line 331):

```rust
FillKind::Entry => {
    tracing::info!(...);

    // EXT-31 FR-5: Determine close side from SL price vs entry fill price
    let close_side = if let Some(sl) = action.stop_loss_price {
        if sl < action.entry_fill_price { OrderSide::Sell } else { OrderSide::Buy }
    } else if let Some(tp) = action.take_profit_price {
        if tp > action.entry_fill_price { OrderSide::Sell } else { OrderSide::Buy }
    } else {
        // No SL or TP — just broadcast and continue
        self.broadcast_fill_event(...);
        tracing::info!(...);
        return; // or use a labeled block
    };

    // EXT-31 FR-2: Place SL (stop-market, reduce-only)
    if let Some(sl_price) = action.stop_loss_price {
        let sl_client_id = format!("testudo:{}:sl", action.group_id);
        match self.exchange_api.place_order(PlaceOrderRequest {
            user_id: action.user_id,
            symbol: action.symbol.clone(),
            side: close_side.clone(),
            order_type: ApiOrderType::StopLoss,
            quantity: action.entry_quantity,
            price: None,
            stop_price: Some(sl_price),
            leverage: 0,
            exchange_account_id: action.exchange_account_id,
            reduce_only: true,
            client_order_id: Some(sl_client_id),
            stop_loss_trigger: None,
            take_profit_trigger: None,
        }).await {
            Ok(result) => {
                tracing::info!(
                    "FillDetector: deferred SL placed: order_id={}, sl_price={}, group={}",
                    result.id, sl_price, action.group_id
                );
                let _ = self.engine_handle
                    .register_exchange_order_id(action.group_id, OrderRole::StopLoss, result.id)
                    .await;
            }
            Err(e) => {
                // FR-7: Critical — position is live but unprotected
                tracing::error!(
                    "FillDetector: CRITICAL — deferred SL placement failed for group {}: {}. Position is UNPROTECTED.",
                    action.group_id, e
                );
            }
        }
    }

    // EXT-31 FR-3: Place TP (limit, NOT reduce-only per trading.md convention)
    if let Some(tp_price) = action.take_profit_price {
        let tp_client_id = format!("testudo:{}:tp", action.group_id);
        match self.exchange_api.place_order(PlaceOrderRequest {
            user_id: action.user_id,
            symbol: action.symbol.clone(),
            side: close_side,
            order_type: ApiOrderType::Limit,
            quantity: action.entry_quantity,
            price: Some(tp_price),
            stop_price: None,
            leverage: 0,
            exchange_account_id: action.exchange_account_id,
            reduce_only: false,
            client_order_id: Some(tp_client_id),
            stop_loss_trigger: None,
            take_profit_trigger: None,
        }).await {
            Ok(result) => {
                tracing::info!(
                    "FillDetector: deferred TP placed: order_id={}, tp_price={}, group={}",
                    result.id, tp_price, action.group_id
                );
                let _ = self.engine_handle
                    .register_exchange_order_id(action.group_id, OrderRole::TakeProfit, result.id)
                    .await;
            }
            Err(e) => {
                // FR-8: Non-critical — proceed without TP
                tracing::warn!(
                    "FillDetector: deferred TP placement failed for group {} (non-critical): {}",
                    action.group_id, e
                );
            }
        }
    }

    self.broadcast_fill_event(...);
    tracing::info!(...);
}
```

**New imports needed in fill_detector.rs:**
```rust
use crate::services::exchange_api::{PlaceOrderRequest, ApiOrderType, OrderSide};
use engine::OrderRole;
use rust_decimal::Decimal;
```

### 3) Revert Bracket Code (FR-9, FR-10) — LOW PRIORITY

These are optional cleanup tasks. The bracket fields (`stop_loss_trigger`, `take_profit_trigger` on `PlaceOrderRequest`; bracket forwarding in `handlers.js`) are harmless dead code when always set to `None`. Can be cleaned up in a separate commit.

---

## Side Inference Logic (FR-5)

Determine close side from SL/TP prices relative to entry fill price:
- `sl_price < entry_price` → long position → close side = Sell
- `sl_price > entry_price` → short position → close side = Buy
- Fallback to TP comparison if no SL: `tp_price > entry_price` → long → Sell

This is 100% reliable because a long SL is always below entry, short SL always above.

---

## Failure Modes

| Scenario | Behavior |
|----------|----------|
| SL placement fails after entry fill | Log CRITICAL error. Position is live but unprotected. User must manually manage. |
| TP placement fails after entry fill | Log warning (non-critical), proceed. Matches pre-EXT-31 behavior. |
| Entry cancelled before fill | Existing `handle_cancelled_event` cleans up group (line 405-470). No SL/TP to place. |
| Shadow-only trade (not authenticated) | No exchange orders placed. Deferred placement doesn't trigger — no WebSocket events for shadow trades. |
| Partial fill (entry partially filled) | FillDetectorService only acts on `status == "closed"` (fully filled). Partial fills ignored. |
| Network partition during SL/TP placement | `exchange_api.place_order()` returns timeout error. SL/TP not placed. Same as SL failure mode. |

---

## Files Modified

| File | Change | Lines |
|------|--------|-------|
| `testudo-exchange/crates/router/src/routes/trade_management.rs` | Set bracket params to `None` (entry-only) | ~2 lines |
| `testudo-exchange/crates/router/src/services/fill_detector.rs` | Add deferred SL/TP placement on entry fill | ~60 lines |

---

## Acceptance Criteria

- [ ] Live trade places ONLY entry order on the exchange (no SL/TP until fill)
- [ ] SL and TP do NOT exist on the exchange before entry fills
- [ ] After entry fill via WebSocket, SL is placed as stop-market reduce-only
- [ ] After entry fill via WebSocket, TP is placed as limit (non-reduce-only)
- [ ] SL/TP exchange order IDs are registered in OrderGroup for OCO tracking
- [ ] If entry never fills and is cancelled, no SL/TP orders exist on exchange
- [ ] If entry fills, OCO logic works (SL fills → TP cancelled, TP fills → SL cancelled)
- [ ] SL failure logs CRITICAL error with group ID
- [ ] TP failure logs warning (non-critical)
- [ ] Shadow engine (paper trading) continues to work unaffected
- [ ] `cargo clippy --all-targets && cargo test` passes
- [ ] `cd testudo-extension && bun run build` passes (no extension changes)

---

## Testing Plan

1. **Unit test:** `test_entry_fill_places_deferred_sl_tp` — verify SL/TP placement on entry fill using MockExchangeApi
2. **Unit test:** `test_entry_fill_no_sl_tp_when_prices_absent` — verify no SL/TP placed when group has no prices
3. **Unit test:** `test_deferred_sl_failure_logs_critical` — verify critical log on SL failure
4. **Regression:** All existing fill_detector tests pass (OCO, idempotency, entry cancel)
5. **Regression:** `cargo test` across all crates passes
6. **Manual (testnet):** Place live trade on WOO, verify:
   - Only entry order visible in open orders before fill
   - After entry fills, SL and TP appear as separate orders
   - Cancel/fill one → other is cancelled via OCO

---

## Completion Signal

All acceptance criteria checked. `cargo clippy --all-targets && cargo test` passes. Extension builds clean.
