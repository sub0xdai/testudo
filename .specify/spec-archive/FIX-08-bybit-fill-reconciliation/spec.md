# Specification: Bybit Fill-Price Reconciliation

**Spec ID:** FIX-08-bybit-fill-reconciliation
**Date:** 2026-04-22 (Updated 2026-04-23)
**Status:** Draft
**Class:** Fix / Backend
**Priority:** P0 — journal P&L is currently fabricated for every CEX-routed live trade that exits via stop or take-profit. Additionally, many trades fail to register at all if price data is missing from the WebSocket event.
**Depends on:** None (code-level fix)
**Siblings:** FIX-02 (same problem class for Hyperliquid; this spec is the Bybit analogue)

---

## Problem Statement

Two distinct but related issues have been identified with CEX-routed (Bybit, WOO, etc.) trade closure:

1.  **Incorrect Prices (Original Bug):** On Bybit, SL/TP fill events often carry a `triggerPrice` or `markPrice` snapshot in the `price` field while leaving `average` null. `fill_detector.rs:261` captures this bogus price, leading to mathematical nonsense in the journal.
2.  **Missing Trades (Expanded Bug):** On 2026-04-23, users reported trades failing to register in the journal entirely. Analysis found that if *both* `average` and `price` are missing from the WS event, `fill_detector.rs` skips emitting the `TradeClosed` event. Furthermore, the 30s polling `ReconciliationService` detects closed orders but **does not emit journal events**, causing trades missed by WS to be lost forever.

Root cause in `crates/router/src/services/fill_detector.rs`:
```rust
if let (Some(ref group), Some(ref side), Some(price)) =
    (&action.group_snapshot, &action.close_event_side, action.exit_price)
{
    self.emit_trade_closed(group, price, side);
}
```
If `exit_price` is `None`, no journal entry is created.

---

## User Stories

- **As a trader whose TP or SL just filled**, I want my trade to appear in the journal immediately, even if the exact fill price takes a few seconds to reconcile.
- **As a trader whose WS connection dropped during a fill**, I want the background reconciliation service to find the closed trade and add it to my journal automatically.

---

## Non-Goals

- **No retroactive balance-delta estimation.** Reconciliation pulls the real fill from the exchange REST API.
- **No change to the live placement / decision loop.**

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | After the FillDetector writes a TradeClosed event, async-spawn a reconciliation task that queries `POST /orders/fetch` on the CCXT sidecar | High | router |
| FR-2 | On successful fetch, update `journal_trades.exit_price` and recompute all derived P&L fields | High | router |
| FR-3 | CCXT sidecar exposes `POST /order/fetch` returning `avgFillPrice` + `fills[]` | High | ccxt-sidecar |
| FR-4 | Reconciliation has bounded retry (3 attempts, exponential backoff 1s/4s/16s) | Medium | router |
| FR-5 | Rebuild journal_daily_stats for any corrected dates | Medium | router |
| FR-6 | Offline backfill command `cargo run --bin reconcile-fills` | Medium | router |
| FR-7 | Add `journal_trades.needs_reconciliation BOOLEAN NOT NULL DEFAULT FALSE` | High | backend |
| FR-8 | Exclude `needs_reconciliation = true` rows from stats aggregations | Medium | router |
| **FR-9** | **Force Journaling:** `FillDetector` must emit `TradeClosed` even if `exit_price` is missing, using `0` as a placeholder and setting `needs_reconciliation = true` | High | router |
| **FR-10** | **Polling Journaling:** `ReconciliationService` (30s poll) must emit `TradeClosed` events when it detects a position has closed on the exchange | High | router |

---

## Technical Implementation

### Vertical Checkpoints

| CP | Scope | Validates |
|----|-------|-----------|
| CP-1 | Migration: `journal_trades.needs_reconciliation`. Update `exit_price` to be nullable (or keep NOT NULL but allow 0 placeholder). Stats queries exclude these rows. | System prepared for "dirty" rows |
| CP-2 | CCXT sidecar `POST /order/fetch`. | Data source available |
| CP-3 | `fill_detector.rs` update: emit `TradeClosed` always. `reconciliation.rs` update: emit `TradeClosed` on sweep detection. | Trades no longer "go missing" |
| CP-4 | Post-write reconciliation task + offline backfill binary. | Accuracy restored |

### Database Changes
```sql
-- Migration: Add flag and relax price constraint
ALTER TABLE journal_trades ADD COLUMN needs_reconciliation BOOLEAN NOT NULL DEFAULT FALSE;
-- Ensure existing rows are marked false, new rows can be true.
```

### FillDetector Logic Change
```rust
// crates/router/src/services/fill_detector.rs

// Old:
// if let (..., Some(price)) = (...) { self.emit_trade_closed(group, price, side); }

// New:
let needs_recon = action.exit_price.is_none() || is_bybit_and_potentially_bogus;
self.emit_trade_closed(group, action.exit_price.unwrap_or(Decimal::ZERO), side, needs_recon);
```

### ReconciliationService Logic Change
```rust
// crates/router/src/services/reconciliation.rs

// In sweep logic:
if action.new_status.is_terminal() {
    // Emit TradeClosed event if not already journaled
    self.emit_trade_closed_from_recon(group, ...);
}
```

---

## Acceptance Criteria

- [x] (Identified) Bug root cause confirmed: price-less events skip journaling.
- [ ] Migration adds `needs_reconciliation` and allows placeholder `exit_price`.
- [ ] Bybit SL/TP fill with NO price in WS event still produces a journal row (marked for reconciliation).
- [ ] Trade closed while server is offline (detected by 30s poll on restart) produces a journal row.
- [ ] Flagged rows excluded from all journal_stats aggregations.
- [ ] Manual QA: kill WS connection, fill trade on Bybit, verify trade appears in desk after 30s poll.
