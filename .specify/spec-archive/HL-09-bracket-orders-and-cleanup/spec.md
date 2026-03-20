# Specification: Hyperliquid Bracket Orders, Close Position Fix, Ghost Order Cleanup

**Spec ID:** HL-09-bracket-orders-and-cleanup
**Date:** 2026-03-19
**Status:** Complete
**Class:** Feature / Exchange Integration
**Priority:** P0 — Live trading incomplete without SL/TP protection
**Depends on:** HL-08 (order 422 fix — complete)
**Series:** HL-01 through HL-09 (Hyperliquid native integration)

---

## Problem Statement

Three related issues prevent full Hyperliquid live trading parity with the WOO/CCXT path:

### Issue 1: SL/TP orders not placed (Critical)

`HyperliquidExchangeApi::place_order()` completely ignores the `req.stop_loss_trigger` and `req.take_profit_trigger` fields. It places only the entry order and returns `stop_loss_order_id: None, take_profit_order_id: None`. The trade route handler (`trade_management.rs:876-877`) passes these values correctly, but they are silently dropped.

On the CCXT path, `CexExchangeApi::place_order()` passes these to `cex_client.create_order()` which handles bracket orders atomically. For Hyperliquid, SL and TP must be placed as **separate trigger orders** via the Rust SDK after the entry order.

**Root cause**: Feature gap — SL/TP placement was never implemented for the HL SDK path.

### Issue 2: Close position fails with "No order ID" (High)

`close_hyperliquid_position()` places a reduce-only market order with `client_order_id: None`. When the order fills instantly, HL returns `ExchangeDataStatus::Success` (no OID field) instead of `ExchangeDataStatus::Filled(FilledOrder)`. Since there's no CLOID fallback either, `extract_order_id` returns `None` and the code hits:

```
"No order ID in response and no CLOID available"
```

The `ExchangeDataStatus` enum has 6 variants: `Success`, `WaitingForFill`, `WaitingForTrigger`, `Error(String)`, `Resting(RestingOrder)`, `Filled(FilledOrder)`. Only `Resting` and `Filled` carry OIDs. Market orders that fill atomically may return `Success` without an OID.

**Root cause**: `place_order()` requires an OID but some HL response types don't provide one.

### Issue 3: 17 ghost orders in extension (Medium)

The HL-08 debugging session generated ~17 failed order attempts (HTTP 422 errors). Each created a shadow order group in the engine/DB but was classified as an "ambiguous error" (not rolled back). The extension shows these as 17 pending orders that don't exist on the exchange.

`is_definitive_rejection()` checks for keywords like "insufficient", "authentication", "invalid", "not allowed". The HTTP 422 "Failed to deserialize the JSON body" errors match "invalid", so some should have been rolled back. Others may have been timeouts or parse errors.

**Root cause**: Stale order groups from debugging that were never cleaned up.

---

## User Stories

- **As a trader**, I want SL and TP orders placed automatically with my entry, so that my position is protected from the moment it opens.
- **As a trader**, I want to close positions from the extension, so that I can exit trades quickly without using the HL web UI.
- **As a trader**, I want stale ghost orders cleaned up, so that the extension shows only real positions.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | After placing entry order, place SL trigger order if `stop_loss_trigger` is Some | High | exchange_api.rs |
| FR-2 | After placing entry order, place TP trigger order if `take_profit_trigger` is Some | High | exchange_api.rs |
| FR-3 | SL trigger uses `tpsl: "sl"`, `is_market: true`, `reduce_only: true`, close side | High | exchange_api.rs |
| FR-4 | TP trigger uses `tpsl: "tp"`, `is_market: true`, `reduce_only: true`, close side | High | exchange_api.rs |
| FR-5 | Return SL/TP order IDs in `PlaceOrderResult` for engine registration | High | exchange_api.rs |
| FR-6 | Handle `WaitingForTrigger` response for SL/TP by querying OID via CLOID | High | exchange_api.rs |
| FR-7 | Handle `ExchangeDataStatus::Success` for market orders — don't error on missing OID | High | exchange_api.rs |
| FR-8 | Log SL/TP failure as warning but don't fail the entire trade | Medium | exchange_api.rs |
| FR-9 | Add `TakeProfit` variant to `ApiOrderType` enum | Medium | exchange_api.rs |
| FR-10 | Add TP case to `build_order_request()` | Medium | exchange_api.rs |
| FR-11 | Provide a mechanism to clear ghost order groups from the DB | Medium | trade_management.rs |

---

## Technical Implementation

### 1. SL/TP Trigger Order Placement

Add a helper method to `HyperliquidExchangeApi`:

```rust
/// Place a trigger order (SL or TP) and return the exchange order ID.
async fn place_trigger_order(
    &self,
    exchange: &ExchangeProvider<PrivateKeySigner>,
    auth: &HyperliquidAuth,
    asset_index: u32,
    close_is_buy: bool,
    trigger_px: Decimal,
    sz: &str,
    tpsl: &str,  // "sl" or "tp"
    client_order_id_base: Option<&str>,
) -> Option<String> {
    // Generate CLOID: "{base}:sl" or "{base}:tp"
    let cloid = client_order_id_base
        .map(|base| generate_cloid(&format!("{}:{}", base, tpsl)));

    let mut order = HlOrderRequest::trigger(
        asset_index, close_is_buy, trigger_px.to_string(), sz, tpsl, true,
    )
    .reduce_only(true)
    .with_cloid(cloid);

    // Fix CLOID 0x prefix
    if let Some(ref s) = order.cloid {
        if !s.starts_with("0x") {
            order.cloid = Some(format!("0x{}", s));
        }
    }

    tracing::info!(tpsl, trigger_px = %trigger_px, "Placing {} trigger order", tpsl);

    match exchange.place_order(&order).await {
        Ok(response) => match response.into_result() {
            Ok(resp) => {
                let statuses = resp.data.as_ref()
                    .map(|d| d.statuses.as_slice()).unwrap_or(&[]);
                // Try OID from response, then CLOID lookup
                if let Some(oid) = extract_order_id(statuses) {
                    return Some(oid.to_string());
                }
                if let Some(cloid_uuid) = cloid {
                    if let Ok(oid) = self.find_oid_by_cloid(auth, cloid_uuid).await {
                        return Some(oid.to_string());
                    }
                    return Some(format!("cloid:{:032x}", cloid_uuid.as_u128()));
                }
                None
            }
            Err(e) => { tracing::warn!("{} trigger rejected: {}", tpsl, e); None }
        },
        Err(e) => { tracing::warn!("{} trigger failed: {}", tpsl, e); None }
    }
}
```

Then in `place_order()`, after the entry order succeeds:

```rust
// Place SL/TP as separate trigger orders
let close_is_buy = !matches!(req.side, OrderSide::Buy);
let mut sl_order_id = None;
let mut tp_order_id = None;

if let Some(sl_trigger) = req.stop_loss_trigger {
    sl_order_id = self.place_trigger_order(
        &exchange, &auth, asset_index, close_is_buy,
        sl_trigger, &sz, "sl", req.client_order_id.as_deref(),
    ).await;
}

if let Some(tp_trigger) = req.take_profit_trigger {
    tp_order_id = self.place_trigger_order(
        &exchange, &auth, asset_index, close_is_buy,
        tp_trigger, &sz, "tp", req.client_order_id.as_deref(),
    ).await;
}

Ok(PlaceOrderResult {
    id: order_id,
    status,
    average: avg_price,
    stop_loss_order_id: sl_order_id,
    take_profit_order_id: tp_order_id,
})
```

### 2. Close Position Fix — Handle `Success` Status

Modify `extract_order_id` or the OID extraction logic to handle `Success`:

```rust
// In place_order(), replace the strict error path:
let order_id = if let Some(oid) = extract_order_id(statuses) {
    oid.to_string()
} else if let Some(cloid_uuid) = cloid {
    // CLOID fallback (existing logic)
    match self.find_oid_by_cloid(&auth, cloid_uuid).await {
        Ok(oid) => oid.to_string(),
        Err(_) => format!("cloid:{:032x}", cloid_uuid.as_u128()),
    }
} else {
    // No OID and no CLOID — check if the order succeeded anyway
    // ExchangeDataStatus::Success means the action completed but has no OID
    let has_success = statuses.iter().any(|s| matches!(s, ExchangeDataStatus::Success));
    if has_success {
        "success".to_string()  // Synthetic ID — order completed
    } else {
        return Err(ExchangeApiError::Exchange(
            "No order ID in response and no CLOID available".into(),
        ));
    }
};
```

### 3. ApiOrderType — Add TakeProfit Variant

```rust
pub enum ApiOrderType {
    Market,
    Limit,
    StopLoss,
    TakeProfit,  // NEW
}
```

Add TP case to `build_order_request()`:

```rust
ApiOrderType::TakeProfit => {
    let trigger_px = req.stop_price.ok_or_else(|| {
        ExchangeApiError::Internal("TakeProfit order requires stop_price".into())
    })?;
    HlOrderRequest::trigger(
        asset_index, is_buy, trigger_px.to_string(), &sz, "tp", true,
    )
    .reduce_only(true)
    .with_cloid(cloid)
}
```

### 4. Ghost Order Cleanup

Two approaches (pick one during implementation):

**Option A: Database cleanup** — SQL query to mark all non-exchange-confirmed order groups as cancelled for the user.

**Option B: Cancel via extension** — User clicks cancel on each ghost order. This already works via `CANCEL_TRADE` → `cancel_trade` endpoint. Tedious but requires no new code.

Recommendation: Option A — add a one-time cleanup endpoint or admin SQL script.

### Files

| File | Change |
|------|--------|
| `crates/router/src/services/hyperliquid/exchange_api.rs` | FR-1 through FR-8: Add `place_trigger_order()` helper, modify `place_order()` to place SL/TP, handle `Success` status |
| `crates/router/src/services/exchange_api.rs` | FR-9: Add `TakeProfit` to `ApiOrderType` |

### Dependencies Added

None — uses existing `hyperliquid-sdk-rs` types.

---

## Acceptance Criteria

- [ ] Entry + SL + TP all placed on Hyperliquid when trade includes SL/TP prices
- [ ] SL order uses `tpsl: "sl"`, reduce-only, opposite side of entry
- [ ] TP order uses `tpsl: "tp"`, reduce-only, opposite side of entry
- [ ] SL/TP order IDs returned in `PlaceOrderResult` and registered in engine
- [ ] SL/TP failure logs warning but doesn't fail the entry order
- [ ] Close position from extension works without "No order ID" error
- [ ] Ghost orders cleared from extension view
- [ ] `cargo clippy --all-targets && cargo test` passes
- [ ] `cd testudo-extension && bun run build` passes

---

## Risks

1. **SL/TP placed before entry fills** — On CCXT exchanges, bracket orders are atomic (exchange handles fill dependency). On HL, SL/TP trigger orders exist independently. If entry is a limit order that hasn't filled yet, the SL/TP reduce-only orders may be rejected because there's no position to reduce. Mitigation: HL trigger orders (`tpsl: "sl"/"tp"`) are specifically designed for this — they only activate when a matching position exists. If entry is market and fills instantly, this is not an issue.

2. **Race condition between SL/TP placement** — If entry fills and price moves rapidly, the SL/TP placement could be at stale prices. Mitigation: Trigger prices are set at order time and don't change — only the trigger condition matters, not the current price.

3. **CLOID collisions for SL/TP** — UUID v5 with `"{base}:sl"` and `"{base}:tp"` suffixes are deterministic and unique per group/role. No collision risk.

---

## Completion Signal

This spec is complete when:
1. Live trade on HL places entry + SL + TP (3 orders visible on HL web UI)
2. Close position from extension succeeds without errors
3. Extension shows only real positions (ghost orders cleared)
4. All acceptance criteria met
5. Code committed to master
