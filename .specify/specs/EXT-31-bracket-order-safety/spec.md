# Specification: Bracket Order Safety

**Spec ID:** EXT-31-bracket-order-safety
**Date:** 2026-03-14
**Status:** Draft
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

## Solution: Native Bracket Orders via CCXT

CCXT 4.5.39 (our installed version) supports the unified `createOrder` with attached `stopLoss` and `takeProfit` parameters. WOO X supports this on perpetual swap markets.

```javascript
exchange.createOrder('BTC/USDT:USDT', 'limit', 'sell', 0.01473, 71415, {
  stopLoss: { triggerPrice: 72500 },
  takeProfit: { triggerPrice: 70000 },
})
```

Per CCXT docs: *"These attached orders are triggered automatically once the primary order is filled and the market price reaches their respective trigger prices."*

The exchange handles the activation logic — SL/TP only become live after entry fills. Single API call, atomic, no race conditions.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Extend sidecar `/order` handler to accept optional `stopLoss` and `takeProfit` params and pass them to `exchange.createOrder()` | High | testudo-ccxt |
| FR-2 | Modify backend `create_trade` to send a single bracket order call instead of three sequential calls when live trading | High | router/routes |
| FR-3 | Parse bracket order response to extract entry, SL, and TP exchange order IDs | High | router/ccxt_client |
| FR-4 | Register all three exchange order IDs in the OrderGroup atomically | High | engine/order_group |
| FR-5 | Remove the three-sequential-call code path for live trades (keep for shadow engine) | Medium | router/routes |
| FR-6 | Stamp all three legs with `clientOrderId` convention: `testudo:{group_id}:{role}` | Medium | router/routes |
| FR-7 | Ensure fill detector handles bracket order fills correctly (already idempotent — verify) | Medium | router/fill_detector |

---

## Technical Implementation

### 1) CCXT Sidecar — Extend `/order` Handler (FR-1)

**File:** `testudo-ccxt/src/handlers.js`

**Current:** `handleOrder()` accepts `stopPrice`, `reduceOnly`, `clientOrderId` and makes a single `exchange.createOrder()` call per order.

**Change:** Accept optional `stopLoss` and `takeProfit` objects in the request body and forward them to CCXT:

```javascript
async function handleOrder(req, res) {
  const { exchange, params } = getExchangeAndParams(req.body);
  const { symbol, type, side, amount, price, stopPrice, leverage,
          reduceOnly, clientOrderId, stopLoss, takeProfit } = params;

  if (leverage && leverage > 0) {
    await exchange.setLeverage(leverage, symbol);
  }

  const orderParams = {};
  if (stopPrice !== undefined && stopPrice !== null) {
    orderParams.stopPrice = stopPrice;
  }
  if (reduceOnly) {
    orderParams.reduceOnly = true;
  }
  if (clientOrderId) {
    orderParams.clientOrderId = clientOrderId;
  }

  // Bracket order: attach SL/TP to entry (exchange activates on fill)
  if (stopLoss && stopLoss.triggerPrice) {
    orderParams.stopLoss = { triggerPrice: stopLoss.triggerPrice };
  }
  if (takeProfit && takeProfit.triggerPrice) {
    orderParams.takeProfit = { triggerPrice: takeProfit.triggerPrice };
  }

  const order = await exchange.createOrder(symbol, type, side, amount, price, orderParams);

  res.json({
    id: stringify(order.id),
    clientOrderId: order.clientOrderId || null,
    status: order.status,
    symbol: order.symbol,
    side: order.side,
    type: order.type,
    amount: stringify(order.amount),
    filled: stringify(order.filled),
    remaining: stringify(order.remaining),
    average: stringify(order.average),
    price: stringify(order.price),
    // Bracket order IDs (if exchange returns them)
    stopLossOrderId: order.info?.stopLossOrderId || null,
    takeProfitOrderId: order.info?.takeProfitOrderId || null,
  });
}
```

**Key:** The sidecar remains a thin proxy. No new endpoint needed — the existing `/order` endpoint gains optional bracket parameters.

### 2) Backend — CCXT Client Request/Response (FR-3)

**File:** `testudo-exchange/crates/router/src/services/ccxt_client.rs`

Extend the request envelope to include optional bracket fields:

```rust
// In the request body builder:
if let Some(sl_trigger) = req.stop_loss_trigger {
    body["stopLoss"] = json!({ "triggerPrice": sl_trigger.to_string() });
}
if let Some(tp_trigger) = req.take_profit_trigger {
    body["takeProfit"] = json!({ "triggerPrice": tp_trigger.to_string() });
}
```

Extend `SidecarOrderResponse`:

```rust
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SidecarOrderResponse {
    pub id: String,
    pub client_order_id: Option<String>,
    pub status: Option<String>,
    pub symbol: Option<String>,
    pub side: Option<String>,
    #[serde(rename = "type")]
    pub order_type: Option<String>,
    pub amount: Option<String>,
    pub filled: Option<String>,
    pub remaining: Option<String>,
    pub average: Option<String>,
    pub price: Option<String>,
    // NEW: bracket order child IDs
    pub stop_loss_order_id: Option<String>,
    pub take_profit_order_id: Option<String>,
}
```

### 3) Backend — Exchange API Trait Extension (FR-2)

**File:** `testudo-exchange/crates/router/src/services/exchange_api.rs`

Add optional bracket fields to `PlaceOrderRequest`:

```rust
pub struct PlaceOrderRequest {
    pub user_id: Uuid,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: ApiOrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub leverage: u8,
    pub exchange_account_id: Option<Uuid>,
    pub reduce_only: bool,
    pub client_order_id: Option<String>,
    // NEW: bracket order fields
    pub stop_loss_trigger: Option<Decimal>,
    pub take_profit_trigger: Option<Decimal>,
}
```

The `CcxtExchangeApi::place_order()` implementation passes these through to the sidecar.

### 4) Backend — Trade Route Refactor (FR-2, FR-5, FR-6)

**File:** `testudo-exchange/crates/router/src/routes/trade_management.rs`

Replace the three sequential exchange calls (lines ~812-970) with a single bracket call:

```rust
// For live trades: single bracket order
if is_authenticated {
    let entry_client_id = format!("testudo:{}:entry", group_id);

    let result = tm.place_order(ExchangePlaceOrderRequest {
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
        // Bracket: attach SL/TP
        stop_loss_trigger: Some(sl_price),
        take_profit_trigger: tp_price,
    }).await;

    match result {
        Ok(resp) => {
            // Register entry ID
            engine_handle.set_exchange_order_id(group_id, "entry", &resp.id).await;

            // Register SL/TP IDs if returned
            if let Some(sl_id) = &resp.stop_loss_order_id {
                engine_handle.set_exchange_order_id(group_id, "sl", sl_id).await;
            }
            if let Some(tp_id) = &resp.take_profit_order_id {
                engine_handle.set_exchange_order_id(group_id, "tp", tp_id).await;
            }
        }
        Err(e) => {
            // Rollback shadow order and return error
            engine_handle.cancel_order(user_id, placed_order.id).await;
            return Err(e);
        }
    }
}
```

**Rollback simplification:** One call to rollback instead of cascading rollbacks across three calls.

### 5) Fill Detector — Verification (FR-7)

**File:** `testudo-exchange/crates/router/src/services/fill_detector.rs`

The fill detector already handles fills idempotently:
- Entry fill → marks group Active
- SL fill → marks StoppedOut, cancels TP
- TP fill → marks TookProfit, cancels SL
- Terminal state fills → no-op

**Verify:** With bracket orders, the exchange may auto-cancel the opposite leg when SL/TP fills. The cancel attempt in fill_detector should handle "order not found" / "already cancelled" gracefully. Check that `cancel_order` errors are logged but don't crash.

### 6) OrderGroup Registration (FR-4)

**File:** `testudo-exchange/crates/engine/src/shadow/order_group.rs`

Verify the group can store all three exchange order IDs. The current fields:
- `exchange_order_id` (entry)
- `exchange_sl_order_id`
- `exchange_tp_order_id`

These are already present. The only change is populating them from a single response instead of three.

---

## Migration Strategy

1. **Add bracket fields as `Option`** — existing sequential path still works
2. **Deploy sidecar first** (backward compatible — new fields are optional)
3. **Deploy backend** with bracket order logic
4. **Test on WOO testnet** with a limit order that won't fill — verify SL/TP don't exist on exchange
5. **Remove sequential path** for live trades once bracket orders are verified

---

## Files Modified

| File | Change |
|------|--------|
| `testudo-ccxt/src/handlers.js` | Accept & forward `stopLoss`/`takeProfit` params, return child order IDs |
| `testudo-exchange/crates/router/src/services/ccxt_client.rs` | Add bracket fields to request builder, extend response struct |
| `testudo-exchange/crates/router/src/services/exchange_api.rs` | Add `stop_loss_trigger`/`take_profit_trigger` to `PlaceOrderRequest` |
| `testudo-exchange/crates/router/src/routes/trade_management.rs` | Single bracket call replaces three sequential calls for live trades |
| `testudo-exchange/crates/router/src/services/fill_detector.rs` | Verify graceful handling of "order already cancelled" on OCO cancel |

---

## Acceptance Criteria

- [ ] Live trade places entry+SL+TP in a single `createOrder` call with attached params
- [ ] SL and TP do NOT exist on the exchange as independent orders before entry fills
- [ ] If entry never fills and is cancelled, no SL/TP orders remain on exchange
- [ ] If entry fills, SL and TP activate and OCO logic works (one fills → other cancels)
- [ ] Sidecar `/order` endpoint is backward compatible (bracket fields are optional)
- [ ] Fill detector handles bracket fills without errors
- [ ] `cargo clippy --all-targets && cargo test` passes
- [ ] Shadow engine (paper trading) continues to work via existing path

---

## Testing Plan

1. **Unit test:** Sidecar handler correctly forwards `stopLoss`/`takeProfit` to CCXT
2. **Unit test:** Backend builds correct request body with bracket fields
3. **Integration test (testnet):** Place bracket order on WOO testnet, verify:
   - Only entry order visible in open orders before fill
   - After entry fills, SL/TP appear
   - Cancelling entry also cancels SL/TP
4. **Regression test:** Paper trading (shadow engine) still works with sequential path

---

## Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| WOO doesn't return SL/TP order IDs in response | Medium | Fall back to fetching open orders after placement; or use `fetchOpenOrders` to discover child IDs |
| Exchange rejects bracket syntax | High | Validate on testnet before production; keep sequential path as fallback |
| Fill detector double-cancels | Low | Already idempotent; verify cancel errors are non-fatal |
| CCXT version drift | Low | Pinned to ^4.4.0, currently 4.5.39; nested syntax stable since 4.x |

---

## Out of Scope

- Software OTO state machine (not needed — exchange handles activation natively)
- Multi-exchange bracket support matrix (WOO only for now; extend per exchange later)
- Partial fill handling for bracket legs (exchange manages this)
- Amendment of bracket orders after placement (separate spec)
