# Specification: Ghost Pending Position Cleanup

**Spec ID:** 016-ghost-position-cleanup
**Date:** 2026-03-06
**Status:** Complete

---

## Overview

Eliminate persistent "ghost" pending positions caused by two confirmed backend bugs that prevent both real-time fill promotion and startup cleanup of live trades.

**Current state:**
- Live trades placed on exchange fill successfully, but the backend never detects the fill because PriceFeedService doesn't poll symbols without paper trades (Bug #2).
- On restart, RehydrationService attempts to verify stale pending positions against the exchange but passes the wrong symbol format (`BTC_USDT` instead of `BTC/USDT:USDT`), causing verification to fail silently (Bug #1).
- Users see phantom positions with $0 exposure and PENDING status indefinitely.

**Target state:**
- All live trade fills are detected regardless of whether a paper trade exists for that symbol.
- Rehydration correctly verifies pending positions against the exchange using the CCXT unified symbol format.
- Persistent verification failures result in explicit error handling, not silent skipping.

---

## Root Cause Analysis

### Bug #1: Symbol Normalization in RehydrationService

`rehydration.rs` calls `ccxt_client.fetch_open_orders()` (line ~152) and `ccxt_client.fetch_positions()` (line ~190) using the internal symbol format (`BTC_USDT`). The CCXT sidecar expects unified futures format (`BTC/USDT:USDT`).

The conversion function `to_ccxt_symbol()` exists in `exchange_api.rs:344` and is correctly used by `CcxtExchangeApi` for normal order operations, but rehydration bypasses it entirely.

**Effect:** Exchange returns empty results or errors. Rehydration logs a warning, skips the position, and leaves it PENDING in the database forever.

### Bug #2: Price Polling Gap for Live-Only Symbols

`PriceFeedService.tick()` (`price_feed.rs:125`) calls `engine.get_active_symbols()` which queries `ShadowOrderManager.open_orders_by_symbol` — a map populated only by paper orders.

Both shadow and live `TradeManagerService` instances consume the same price broadcast channel. When a symbol has only live trades (no paper trades), zero price ticks are broadcast, so `process_tick()` is never called, and `promote_pending_if_filled()` is never reached.

**Effect:** Live trade entry fills on the exchange but the backend never promotes it from Pending to Active/Filled.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Convert symbols to CCXT unified format before calling `ccxt_client.fetch_open_orders()` and `ccxt_client.fetch_positions()` in RehydrationService | Critical | Backend / Rehydration |
| FR-2 | Extend PriceFeedService symbol collection to include symbols from live TradeManagerService positions, not just ShadowEngine paper trades | Critical | Backend / Price Feed |
| FR-3 | When exchange verification fails persistently (not transient network error), mark stale pending positions as Cancelled rather than silently skipping | High | Backend / Rehydration |
| FR-4 | Add unit tests for symbol conversion in rehydration path | High | Backend / Test |
| FR-5 | Add unit test proving live-only symbols receive price ticks | High | Backend / Test |
| FR-6 | Add integration test for full pending-to-filled promotion on live-only trade | Medium | Backend / Test |

---

## Technical Implementation

### 1) Fix Symbol Normalization in Rehydration (FR-1)

**File:** `crates/router/src/services/rehydration.rs`

Import `to_ccxt_symbol` from `exchange_api.rs` and apply before CCXT client calls:

```rust
// Line ~152: fetch_open_orders
use crate::services::exchange_api::to_ccxt_symbol;

let ccxt_symbol = to_ccxt_symbol(&position.symbol);
let open_orders = ccxt_client
    .fetch_open_orders(
        &creds.exchange_name,
        &sidecar_creds,
        sandbox,
        &ccxt_symbol,  // was: &position.symbol
    )
    .await;

// Line ~190: fetch_positions
let has_position = ccxt_client
    .fetch_positions(
        &creds.exchange_name,
        &sidecar_creds,
        sandbox,
        Some(&ccxt_symbol),  // was: Some(&position.symbol)
    )
    .await;
```

### 2) Extend Price Feed to Include Live Symbols (FR-2)

**File:** `crates/router/src/services/price_feed.rs`

PriceFeedService needs access to live TradeManagerService's active symbols. Two approaches:

**Option A — Query TradeManagerService directly:**
- Add `get_active_symbols() -> Vec<String>` method to `TradeManagerService`
- Inject `Arc<TradeManagerService>` (or just the positions map) into `PriceFeedService`
- In `tick()`, merge symbols from both sources:

```rust
pub async fn tick(&self) {
    let mut symbols: HashSet<String> = {
        let engine = self.engine.read().await;
        engine.get_active_symbols().await.into_iter().collect()
    };

    // Merge live trade symbols
    if let Some(ref live_tm) = self.live_trade_manager {
        let live_symbols = live_tm.get_active_symbols().await;
        symbols.extend(live_symbols);
    }

    for symbol in symbols { /* poll prices */ }
}
```

**Option B — Shared symbol registry:**
- Create a `SymbolRegistry` that both ShadowEngine and TradeManagerService register/unregister symbols to.
- PriceFeedService queries the registry instead.
- Cleaner separation of concerns but more wiring.

**Recommended:** Option A — minimal change, direct fix, no new abstractions.

### 3) Improve Rehydration Error Handling (FR-3)

**File:** `crates/router/src/services/rehydration.rs`

Currently on verification failure, the service logs and skips. Add retry logic with a terminal state:

```rust
// After N failed verification attempts (e.g., 3), mark as Cancelled
if verification_errors >= MAX_VERIFY_RETRIES {
    log::warn!(
        "Marking stale pending position {} as Cancelled after {} failed verifications",
        position.id, MAX_VERIFY_RETRIES
    );
    // Update DB status to Cancelled
    // Update in-memory OrderGroup status
}
```

For single-run rehydration (startup), distinguish between:
- **Transient errors** (network timeout, sidecar unavailable): skip, will retry next restart
- **Definitive errors** (symbol not found, empty results with 200 OK): mark Cancelled

### 4) TradeManagerService Symbol Accessor (FR-2 support)

**File:** `crates/router/src/services/trade_manager/service.rs`

```rust
pub async fn get_active_symbols(&self) -> Vec<String> {
    let positions = self.positions.read().await;
    positions
        .values()
        .filter(|p| matches!(p.state, PositionState::Pending | PositionState::Filled | PositionState::Managing))
        .map(|p| p.symbol.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}
```

### 5) PriceFeedService Wiring Update

**File:** `crates/router/src/main.rs`

Pass the live TradeManagerService reference to PriceFeedService during construction (lines ~437):

```rust
let price_feed = PriceFeedService::new(
    engine.clone(),
    price_tx.clone(),
    ccxt_client.clone(),
    trade_manager_live.clone(),  // NEW: optional Arc<TradeManagerService>
);
```

---

## Files to Modify

| File | Change |
|------|--------|
| `crates/router/src/services/rehydration.rs` | Apply `to_ccxt_symbol()` to CCXT client calls; improve error handling |
| `crates/router/src/services/price_feed.rs` | Accept optional live TradeManagerService; merge symbol sets in `tick()` |
| `crates/router/src/services/trade_manager/service.rs` | Add `get_active_symbols()` method |
| `crates/router/src/main.rs` | Wire live TradeManagerService into PriceFeedService |
| `crates/router/src/services/exchange_api.rs` | Ensure `to_ccxt_symbol` is `pub` (already is) |

---

## Acceptance Criteria

1. Rehydration calls to CCXT sidecar use unified symbol format (`BTC/USDT:USDT`)
2. A live-only trade (no paper trade for that symbol) receives price ticks and promotes from Pending to Filled
3. Stale pending positions that fail exchange verification are marked Cancelled, not left indefinitely
4. All existing tests continue to pass (`cargo clippy --all-targets && cargo test`)
5. New tests cover: symbol conversion in rehydration, live-only symbol polling, stale position cleanup

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Circular dependency between PriceFeedService and TradeManagerService | Use `Arc<TradeManagerService>` as read-only dependency; PriceFeed only reads symbols |
| Aggressively cancelling positions that are actually valid | Only cancel after definitive verification failure (200 OK + empty results), not on transient errors |
| Increased price polling load for many live symbols | Symbols are deduplicated via HashSet; polling is already batched on 2s interval |

---

## Completion Signal

All functional requirements (FR-1 through FR-6) implemented and verified:
```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

No ghost pending positions appear after:
1. Placing a live-only trade that fills on exchange
2. Restarting the backend with a stale pending position in the database
