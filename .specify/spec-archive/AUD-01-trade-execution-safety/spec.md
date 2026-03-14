# Specification: Trade Execution Safety

**Spec ID:** AUD-01-trade-execution-safety
**Date:** 2026-03-07
**Status:** Complete
**Class:** Audit
**Phase:** 1 (Safety-Critical)
**Audit Refs:** RC-1, RC-2, RC-3

---

## Overview

Eliminate three race conditions in `trade_management.rs` that can cause double exposure, unprotected positions, and stale state during concurrent or sequential trade operations.

**Current state:**
- Two concurrent live trade requests from the same user can consume the same balance (RC-1: TOCTOU balance race).
- If SL placement fails on exchange after entry succeeds, user gets an unprotected live position with no stop-loss (RC-2: non-atomic SL+TP).
- `update_stop_loss` and `update_entry_price` read group status, drop the lock, then modify — status can change between read and write (RC-3: TOCTOU update race).

**Target state:**
- Live trade creation is serialized per user — no double exposure possible.
- Entry order is rolled back if SL placement fails — no unprotected positions.
- Group status is re-validated under write lock before modifications.

---

## Root Cause Analysis

### RC-1: TOCTOU Balance Race

`trade_management.rs:515-567` fetches live balance for position sizing. Orders are placed at lines 717-833, much later. No per-user serialization exists. Two concurrent requests read the same balance and both succeed on the exchange.

### RC-2: Non-Atomic SL+TP Placement

`trade_management.rs:716-842` places entry, SL, TP as three sequential API calls. Lines 788-794 catch SL failure with `warn!("Failed to place SL on exchange (will manage locally)")` and proceed. The "manage locally" claim is misleading — TradeManagerService only adjusts existing SL orders via price ticks, it cannot re-place a missing SL.

### RC-3: TOCTOU Update Race

`trade_management.rs:1067-1101` (update_stop_loss) and `1310-1351` (update_entry_price) read group info with a read lock, drop it, then cancel old order and place new one. Between read and write, FillDetectorService or PriceFeedService could transition the group to a terminal state, causing:
- Cancellation of an already-filled order (harmless but wasteful)
- Creation of a replacement order for a terminal group — second position opened

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add per-user `tokio::sync::Semaphore(1)` around live trade creation path (`create_trade` when execution_mode is Live) | Critical | Router / Trade Management |
| FR-2 | If SL placement on exchange fails, cancel the entry order (rollback). Return error to user with "SL placement failed, trade rolled back" | Critical | Router / Trade Management |
| FR-3 | If TP placement fails after entry+SL succeed, log warning but proceed (TP is non-critical — limit order, not stop) | High | Router / Trade Management |
| FR-4 | In `update_stop_loss`: acquire write lock, re-check group status is still Active, abort if terminal | High | Router / Trade Management |
| FR-5 | In `update_entry_price`: acquire write lock, re-check group status is still Pending, abort if changed | High | Router / Trade Management |
| FR-6 | Add test: concurrent `create_trade` requests for same user, verify only one succeeds or both get correct sizing | Critical | Test |
| FR-7 | Add test: SL placement failure triggers entry order cancellation | Critical | Test |
| FR-8 | Add test: `update_stop_loss` on a group that transitions to StoppedOut mid-operation returns error, not new order | High | Test |

---

## Technical Implementation

### 1) Per-User Trade Lock (FR-1)

Add a `DashMap<Uuid, Arc<Semaphore>>` to `AppState` (or trade management route data). Before the live trade path, acquire a permit:

```rust
// In AppState or route-level data
pub trade_locks: DashMap<Uuid, Arc<Semaphore>>,

// In create_trade handler, before balance fetch
let lock = state.trade_locks
    .entry(user_id)
    .or_insert_with(|| Arc::new(Semaphore::new(1)))
    .clone();
let _permit = lock.acquire().await?;
// ... rest of trade creation
```

The semaphore is per-user so different users are not blocked. The DashMap provides lock-free per-key access.

### 2) Atomic SL Rollback (FR-2, FR-3)

After entry order placement succeeds (line ~745), if SL placement fails:

```rust
match ccxt_client.create_order(sl_params).await {
    Ok(sl_response) => { /* store SL */ },
    Err(e) => {
        log::error!("SL placement failed, rolling back entry order: {}", e);
        // Cancel the entry order on exchange
        if let Some(ref entry_exchange_id) = entry_exchange_order_id {
            let _ = ccxt_client.cancel_order(cancel_params).await;
        }
        // Roll back shadow engine state
        engine.cancel_order(entry_order_id)?;
        return Err(TradeError::SlPlacementFailed(e.to_string()));
    }
}
```

TP failure is non-critical (it's a limit order, user can re-add). Log warning, proceed.

### 3) Status Re-Validation (FR-4, FR-5)

Replace the read-then-write pattern with write-lock-throughout:

```rust
// update_stop_loss — acquire write lock, validate, then operate
let mut order_groups = engine.order_groups.write().await;
let group = order_groups.get(&group_id)
    .ok_or(TradeError::GroupNotFound)?;
if group.status != OrderGroupStatus::Active {
    return Err(TradeError::GroupNotActive(group.status));
}
// Now safe to cancel old SL and place new one while holding lock
```

---

## Verification

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

- [ ] Per-user semaphore prevents concurrent live trade creation
- [ ] SL failure triggers entry rollback and returns error
- [ ] TP failure logs warning but trade proceeds
- [ ] update_stop_loss re-validates group status under write lock
- [ ] update_entry_price re-validates group status under write lock
- [ ] All existing tests still pass
