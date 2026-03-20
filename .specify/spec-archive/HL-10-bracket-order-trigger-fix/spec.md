# Specification: Fix Hyperliquid Bracket Order SL/TP Placement

**Spec ID:** HL-10-bracket-order-trigger-fix
**Date:** 2026-03-20
**Status:** Complete
**Class:** Bugfix / Trading
**Priority:** P0 — SL/TP never placed on Hyperliquid, leaving positions unprotected
**Depends on:** HL-09 (bracket order implementation)
**Series:** HL-10

---

## Problem Statement

After HL-09 implemented bracket orders (entry + SL/TP), the SL and TP trigger orders were silently rejected by Hyperliquid with "Order has invalid price." The entry order succeeded, but positions had no stop-loss or take-profit protection.

### Root Cause

The Rust SDK (`hyperliquid-sdk-rs 0.1.2`) sets `limit_px: "0"` in `OrderRequest::trigger()`. Hyperliquid's API requires a valid `limit_px` even when `isMarket: true`. The Python SDK documentation confirms that trigger orders need a real limit price (used as a slippage safety bound).

Evidence from Python SDK:
```python
# limit_px=1900.0 (NOT "0") with triggerPx="1950.0"
exchange.order("ETH", False, 0.1, 1900.0, {
    "trigger": {"isMarket": True, "triggerPx": "1950.0", "tpsl": "sl"}
})
```

### Related Fixes (Same Session)

1. **AW-06**: Agent wallet reuse — `init_agent_wallet` now checks for existing wallets before generating new keypairs
2. **Ghost position cleanup**: WS subscription manager bails on `NotFound` instead of infinite retry; reconciliation force-cancels orphaned groups for deactivated accounts
3. **HL reconciliation**: Reconciliation service now handles Hyperliquid accounts (was `return Ok(0)`) by cancelling groups with no exchange_order_id

---

## Functional Requirements

| ID | Requirement | Status |
|----|-------------|--------|
| FR-1 | Trigger orders (SL/TP) must have a valid `limit_px` — not "0" | Complete |
| FR-2 | `limit_px` uses 10% slippage from trigger price as safety bound | Complete |
| FR-3 | Fix applies to: bracket orders (`place_trigger_order`), standalone orders (`build_order_request`), and amend orders | Complete |
| FR-4 | For sell triggers (close long): `limit_px = trigger_px * 0.9` | Complete |
| FR-5 | For buy triggers (close short): `limit_px = trigger_px * 1.1` | Complete |
| FR-6 | WS subscription manager terminates on `RepoError::NotFound` instead of infinite retry | Complete |
| FR-7 | Reconciliation force-cancels all groups for deactivated accounts | Complete |
| FR-8 | Reconciliation handles Hyperliquid accounts (minimal: cancel orphaned groups with no exchange order ID) | Complete |

---

## Technical Implementation

### 1. `trigger_limit_px()` Helper

**File:** `crates/router/src/services/hyperliquid/exchange_api.rs`

```rust
fn trigger_limit_px(trigger_px: &Decimal, is_buy: bool) -> String {
    let slippage = if is_buy {
        *trigger_px * Decimal::new(11, 1) // 1.1x for buys
    } else {
        *trigger_px * Decimal::new(9, 1)  // 0.9x for sells
    };
    slippage.normalize().to_string()
}
```

Applied in 3 locations:
- `place_trigger_order()` — bracket SL/TP after entry (line ~707)
- `build_order_request()` — standalone StopLoss/TakeProfit (lines ~205, ~221)
- `amend_order()` — trigger order amendments (lines ~484, ~500)

### 2. WS Subscription Manager Fix

**File:** `crates/router/src/services/ws_subscription_manager.rs`

- Import `RepoError` from repository
- Match `Err(RepoError::NotFound)` explicitly — return immediately (terminate task)
- Other errors still retry with exponential backoff

### 3. Reconciliation Service Fixes

**File:** `crates/router/src/services/reconciliation.rs`

- `force_cancel_orphaned_groups()` — cancels all non-terminal groups for deactivated accounts, persists to DB
- `reconcile_hyperliquid_minimal()` — cancels HL groups in `AwaitingReconciliation`/`Pending` with no exchange_order_id
- `reconcile_account()` restructured: handles `RepoError::NotFound` → force-cancel, Hyperliquid → minimal reconciliation

### 4. Agent Wallet Reuse (AW-06)

**File:** `crates/router/src/repositories/exchange_account.rs`
- `find_agent_wallet()` — queries by `(user_id, wallet_address, auth_mode='agent_wallet')`, ordered by `is_active DESC, created_at DESC`

**File:** `crates/router/src/routes/exchanges.rs`
- `init_agent_wallet()` — checks `find_agent_wallet` before generating new keypair; falls through on decryption failure

**Migration:** `20260320000000_agent_wallet_unique.up.sql`
- Partial unique index on `(user_id, wallet_address)` WHERE `auth_mode = 'agent_wallet' AND is_active = true`

---

## Files Changed

| File | Change |
|------|--------|
| `crates/router/src/services/hyperliquid/exchange_api.rs` | `trigger_limit_px()` helper + apply to all trigger order construction |
| `crates/router/src/services/ws_subscription_manager.rs` | Bail on `NotFound` instead of infinite retry |
| `crates/router/src/services/reconciliation.rs` | Force-cancel orphaned groups + minimal HL reconciliation |
| `crates/router/src/repositories/exchange_account.rs` | `find_agent_wallet()` method |
| `crates/router/src/routes/exchanges.rs` | Reuse existing agent wallet in `init_agent_wallet()` |
| `crates/sqlx_postgres/migrations/20260320000000_agent_wallet_unique.up.sql` | Partial unique index |
| `crates/sqlx_postgres/migrations/20260320000000_agent_wallet_unique.down.sql` | Drop index |

---

## Acceptance Criteria

- [x] SL/TP trigger orders have valid `limit_px` (10% slippage band)
- [x] Bracket orders (entry + SL + TP) place all three on Hyperliquid
- [x] WS manager stops infinite retry for deactivated accounts
- [x] Reconciliation cleans up ghost positions for stale accounts
- [x] Reconciliation handles Hyperliquid groups (cancel orphans)
- [x] `init_agent_wallet` reuses existing agent wallets
- [x] Partial unique index prevents duplicate active agent wallets
- [x] `cargo clippy --all-targets && cargo test` passes (972 tests, 0 failures)

---

## Risks

1. **10% slippage band** — For extreme volatility events, 10% may not be enough. Hyperliquid's `isMarket: true` should execute at best available price regardless, but the limit_px acts as a circuit breaker.
2. **Rust SDK upstream** — The `limit_px: "0"` default in `hyperliquid-sdk-rs` is a bug. A PR should be submitted to set a reasonable default or require the caller to specify it for trigger orders.

---

## Completion Signal

This spec is complete:
1. All acceptance criteria met
2. All changes committed to master and pushed
3. `cargo clippy --all-targets && cargo test` passes
