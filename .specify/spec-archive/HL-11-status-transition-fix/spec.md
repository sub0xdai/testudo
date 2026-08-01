# Specification: Fix Hyperliquid OrderGroup Status Transitions and Ghost Cleanup

**Spec ID:** HL-11-status-transition-fix
**Date:** 2026-03-21
**Status:** Draft
**Class:** Core / Bug Fix
**Priority:** P0 — Live positions display as "Pending" forever; CLEAR ALL fails to cancel exchange orders
**Depends on:** HL-09-bracket-orders-and-cleanup, HL-10-bracket-order-trigger-fix
**Series:** HL-01 through HL-11 (Hyperliquid native integration)

---

## Problem Statement

Hyperliquid OrderGroups never transition from `Pending` to `Active` after entry fill. The root cause is a status string mismatch in `trade_management.rs:886`:

```rust
let is_filled = result.status.as_deref() == Some("closed");
```

This comparison uses the CCXT convention (`"closed"` = filled), inherited from the WooX/Binance sidecar path. Hyperliquid's `place_order` returns `format!("{:?}", ExchangeDataStatus)` at `exchange_api.rs:390`, producing Debug-formatted strings like `"Filled(FilledOrder { oid: 42, ... })"`, `"Success"`, or `"Resting(RestingOrder { ... })"`. None of these match `"closed"`, so `on_entry_filled()` is never called during placement. The OrderGroup stays `Pending` permanently.

The WS fill subscriber (`ws_fills.rs:285-288`) does correctly translate `"filled" → "closed"`, so fills detected via WebSocket do trigger `on_entry_filled`. But for limit orders that cross immediately (common with market-like entries), the placement response returns before the WS event is processed, and the OID may not yet be registered via `register_exchange_order_id` — causing the WS fill detector to drop the event as "unknown exchange order ID".

A secondary bug exists in `cleanup_stale_trades()` (`trade_management.rs:1909-1931`): it only cancels shadow engine orders via `engine_handle.cancel_order()` but does NOT cancel exchange orders or close positions. Compare with `cancel_trade()` (`trade_management.rs:1703-1840`) which properly cancels all exchange orders (entry + SL + TP) and closes positions via reduce-only market orders. This means CLEAR ALL marks groups as `Cancelled` in the engine but leaves orphaned orders and positions on Hyperliquid.

---

## User Stories

- **As a trader**, I want my filled entry orders to immediately show as "Active" in the extension, so that I can see my real position state at a glance.
- **As a trader**, I want CLEAR ALL to purge stale ghost Pending groups without touching my Active positions, so that cleanup doesn't accidentally close real trades.
- **As a trader**, I want CLEAR ALL to cancel the exchange orders associated with ghost Pending groups (if any exist on the exchange), so that orphaned trigger orders don't linger.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Normalize `PlaceOrderResult.status` from Hyperliquid SDK `ExchangeDataStatus` to CCXT-compatible strings (`"closed"`, `"open"`) so that `is_filled` evaluates correctly for immediate fills | High | exchange_api |
| FR-2 | When `ExchangeDataStatus::Filled` or `ExchangeDataStatus::Success` is returned from `place_order`, `on_entry_filled()` must be called, transitioning the OrderGroup from `Pending` to `Active` | High | trade_management |
| FR-3 | `cleanup_stale_trades()` must only target `Pending` groups — `Active` groups are untouched (use individual CANCEL for those) | High | trade_management |
| FR-4 | `cleanup_stale_trades()` must cancel linked exchange orders (entry, SL, TP) on the exchange for each `Pending` group being purged, to prevent orphaned orders | High | trade_management |
| FR-5 | Add unit test confirming `Filled(...)` status maps to `"closed"` and triggers `on_entry_filled` | Medium | exchange_api |
| FR-6 | Add unit test confirming `Resting(...)` status maps to `"open"` and does NOT trigger `on_entry_filled` | Medium | exchange_api |

---

## Technical Implementation

### Status Normalization (FR-1)

Replace the raw Debug formatting in `HyperliquidExchangeApi::place_order`:

```rust
// File: crates/router/src/services/hyperliquid/exchange_api.rs
// Current (line 390):
let status = statuses.first().map(|s| format!("{:?}", s));

// Fixed — normalize to CCXT convention:
let status = statuses.first().map(|s| match s {
    ExchangeDataStatus::Filled(_) => "closed".to_string(),
    ExchangeDataStatus::Success => "closed".to_string(),
    ExchangeDataStatus::Resting(_) => "open".to_string(),
    ExchangeDataStatus::WaitingForTrigger => "open".to_string(),
    ExchangeDataStatus::WaitingForFill => "open".to_string(),
    ExchangeDataStatus::Error(msg) => format!("error:{}", msg),
});
```

This aligns with the existing downstream contract:
- `trade_management.rs:886` checks `Some("closed")` for instant fill detection
- `fill_detector.rs:179` matches `"closed"` for WS fill handling

### Status Mapping Table

| ExchangeDataStatus | Normalized String | Effect on OrderGroup |
|---------------------|-------------------|---------------------|
| `Filled(...)` | `"closed"` | `on_entry_filled()` called → `Pending → Active` |
| `Success` | `"closed"` | `on_entry_filled()` called → `Pending → Active` |
| `Resting(...)` | `"open"` | Stays `Pending`, awaits WS fill event |
| `WaitingForTrigger` | `"open"` | Stays `Pending` (trigger orders) |
| `WaitingForFill` | `"open"` | Stays `Pending`, awaits WS fill event |
| `Error(msg)` | `"error:{msg}"` | Rejected, group rolled back |

### Cleanup Fix (FR-3, FR-4)

Refactor `cleanup_stale_trades()` to:

1. **Only target `Pending` groups** — skip `Active` and terminal groups entirely
2. Cancel linked exchange orders (entry, SL, TP) via the live trade manager for each pending group
3. Mark the group as `Cancelled` in the engine and persist to DB

Active positions are never touched by cleanup. The user must use the individual CANCEL button on a specific trade to cancel an Active position (which triggers `cancel_trade()` with position close logic).

```rust
// File: crates/router/src/routes/trade_management.rs
// Replace the current loop body in cleanup_stale_trades():

for group in &groups {
    // Only purge Pending (ghost) groups — leave Active untouched
    if group.status != OrderGroupStatus::Pending {
        continue;
    }

    // Cancel any exchange orders that may exist for this ghost group
    if let Some(tm) = state.trade_manager_live.as_ref() {
        let ids_to_cancel = [
            ("entry", group.exchange_order_id.as_deref()),
            ("sl", group.exchange_sl_order_id.as_deref()),
            ("tp", group.exchange_tp_order_id.as_deref()),
        ];
        for (role, maybe_id) in &ids_to_cancel {
            if let Some(exch_oid) = maybe_id {
                match tm
                    .cancel_order(user_id, exch_oid, &group.symbol, group.exchange_account_id)
                    .await
                {
                    Ok(()) => tracing::debug!(
                        group_id = %group.id, role = %role,
                        "cleanup: cancelled exchange order"
                    ),
                    Err(ExchangeApiError::OrderNotFound(_)) => {} // already gone
                    Err(e) => tracing::warn!(
                        group_id = %group.id, role = %role, error = %e,
                        "cleanup: exchange cancel failed"
                    ),
                }
            }
        }
    }

    // Cancel shadow orders
    let linked_ids = group.get_linked_order_ids();
    for order_id in linked_ids {
        let _ = state.engine_handle.cancel_order(user_id, order_id).await;
    }

    // Force terminal status
    let _ = state
        .engine_handle
        .update_group_status(group.id, OrderGroupStatus::Cancelled)
        .await;

    if let Some(ref tm) = state.trade_manager_live {
        let _ = tm.mark_position_closed(group.id).await;
    }

    cancelled_count += 1;
}
```

### Files

- `crates/router/src/services/hyperliquid/exchange_api.rs` — Normalize `ExchangeDataStatus` to CCXT strings (line 390), add tests
- `crates/router/src/routes/trade_management.rs` — Fix `cleanup_stale_trades()` to cancel exchange orders and close positions (lines 1909-1931)

### Dependencies Added

None — all types already imported.

---

## Acceptance Criteria

- [ ] A Hyperliquid limit order that fills immediately returns `status: Some("closed")` from `place_order`, causing `on_entry_filled()` to be called and the OrderGroup to transition to `Active`
- [ ] A Hyperliquid limit order that rests returns `status: Some("open")` and the OrderGroup remains `Pending` until WS fill event
- [ ] `cleanup_stale_trades()` only targets `Pending` groups — `Active` groups are untouched
- [ ] `cleanup_stale_trades()` cancels linked exchange orders (entry, SL, TP) on the exchange for each purged `Pending` group
- [ ] Unit test: `ExchangeDataStatus::Filled` maps to `"closed"`
- [ ] Unit test: `ExchangeDataStatus::Success` maps to `"closed"`
- [ ] Unit test: `ExchangeDataStatus::Resting` maps to `"open"`
- [ ] Existing tests remain green: `cargo clippy --all-targets && cargo test`

---

## Risks

1. **Filled status but no avg_px** — If `ExchangeDataStatus::Filled` is returned but `extract_avg_price` fails to parse, `on_entry_filled()` would be called with `entry_price` as fallback (line 927: `result.average.unwrap_or(req.entry_price)`). This is acceptable — the WS fill subscriber will later enrich via REST (FIX-02). Mitigation: Log when falling back to entry price.

2. **Ghost Pending group has exchange orders that are actually wanted** — A `Pending` group may have SL/TP orders on the exchange that are protecting a real position (because `on_entry_filled` never fired). After this fix, new trades will transition correctly, so this scenario only applies to legacy ghost groups. Mitigation: The status fix (FR-1/FR-2) prevents future ghost groups; cleanup only affects existing stale data.

3. **Race between placement response and WS fill** — After normalizing status, both the placement path and the WS fill path may try to call `on_entry_filled()` for the same group. Mitigation: `on_entry_filled()` is idempotent — it sets status to `Active` and updates `entry_price`, which is safe to call twice with the same data.

---

## Completion Signal

This spec is complete when:
1. `place_order` returns normalized status strings and `on_entry_filled()` fires for immediate fills
2. `cleanup_stale_trades()` only purges `Pending` groups and cancels their exchange orders
3. All acceptance criteria met
4. `cargo clippy --all-targets && cargo test` passes
5. Code committed to master
