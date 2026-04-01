# Specification: HL REST Fill Poller to Journal Pipeline

**Spec ID:** REL-02-hl-journal-pipeline
**Date:** 2026-03-31
**Status:** Draft
**Class:** Core / Data Pipeline
**Priority:** P0 — No HL trade data reaching journal, charts, or overview since Mar 27
**Depends on:** None (first in series)
**Series:** REL-02 through CON-01a (HL data pipeline fix + CON-01 regression cleanup)

---

## Problem Statement

Hyperliquid trade data stopped flowing into the journal, charts, and overview pages on Mar 27. Root cause: the FillDetector's OrderGroup matching gate requires an exact exchange order ID (OID) match between what was registered at order placement and what arrives in the WS/REST fill event. For HL trigger orders (SL/TP), the OID returned at placement time (`WaitingForTrigger` → CLOID fallback → `"cloid:..."` string) never matches the numeric OID assigned when the trigger fires and fills on-exchange.

This affects **6 downstream systems**: journal_trades writes, journal_daily_stats upserts, trade_events audit log, OrderGroup status transitions, OCO sibling cancellation, and extension UI notifications. The FillDetector silently drops unmatched fills with a debug log (`"unknown exchange order ID, ignoring"`).

The proven fix already exists in the codebase: `import_worker::process_hl_fill()` writes HL closing fills to journal without requiring OrderGroup matching. It derives entry price from `closedPnl`, deduplicates via `exchange_fill_id` (HL's `tid`), and calls `JournalService::record_trade_close()` which handles daily stats and draft notes merge. This spec lifts that logic into the existing REST poll loop in `ws_fills.rs`, running every 30s for near-real-time journal updates.

Neither the official Python SDK nor the community Rust SDK solve trigger order → fill OID correlation. Both ecosystems match position closes by `closedPnl != "0"`, not by order ID. This spec aligns testudo with that pattern.

---

## User Stories

- **As a trader**, I want my HL trades to appear in the journal within 30s of closing, so that I can review my performance without running manual imports.
- **As a trader**, I want the Overview charts (equity curve, daily P&L, win rate) to reflect HL trades, so that my analytics are accurate.
- **As a trader**, I want correct trade duration (not "0s") on HL journal entries, so that I can analyze hold times.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | REST fill poller writes closing fills (`closedPnl != "0"`) directly to `journal_trades` via `JournalService::record_trade_close()`, bypassing FillDetector | High | ws_fills.rs |
| FR-2 | Deduplication via `exchange_fill_id` (HL `tid`) prevents double-writes across poll cycles and between the poller and import worker | High | JournalService |
| FR-3 | Entry price derived from `closedPnl`: LONG → `exit - (pnl/qty)`, SHORT → `exit + (pnl/qty)` | High | ws_fills.rs |
| FR-4 | Trade duration computed from open fill timestamp tracking (`open_times` HashMap maintained across polls, seeded from 24h startup reconciliation) | High | ws_fills.rs |
| FR-5 | `exchange` field set to `"hyperliquid"` (not `"cex"`) | High | ws_fills.rs |
| FR-6 | `journal_daily_stats` upserted on each journal write (via existing `JournalService` path) | High | JournalService |
| FR-7 | Emit `pg_notify` on `order.{user_id}` channel after journal write for extension UI update | Medium | ws_fills.rs |
| FR-8 | Original WS → FillDetector path remains for entry fills (OID matching works for entries) and for CCXT/WOO exchanges | High | fill_detector.rs |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Extract `process_hl_fill()` into shared fn callable from both import_worker and ws_fills | Unit tests pass, import worker still works |
| CP-2 | Add `JournalService` + `PgPool` to `HyperliquidFillSubscriber`, wire in `reconcile_since()` to write closing fills to journal | Journal entries appear for HL closing fills within 30s |
| CP-3 | Add `open_times` tracking across poll cycles with 24h startup seed | Duration shows correct hold time, not "0s" |
| CP-4 | Add `pg_notify` emission after journal write | Extension receives real-time notification of closed trades |

### Architecture Change

Current flow (broken for HL triggers):
```
HL WS/REST → OrderUpdateEvent → FillDetector → get_group_by_exchange_order(oid)
                                                  → None → DROPPED
```

New flow (HL closing fills):
```
HL REST poll (30s) → user_fills_by_time(5min lookback)
  → filter closedPnl != "0"
  → dedup by tid against seen_tids + DB unique index
  → build TradeCloseEvent (entry price from closedPnl, duration from open_times)
  → JournalService::record_trade_close()
      → INSERT journal_trades
      → upsert_daily_stats()
      → merge draft notes (JNL-20)
  → pg_notify("order.{user_id}", close event)
```

Entry fills and CCXT/WOO fills continue through FillDetector unchanged.

### Shared Fill Processing

Extract from `import_worker.rs:276-363` into a shared module:

```rust
// crates/router/src/services/hl_fill_journal.rs (new file)

use hyperliquid_sdk_rs::types::info_types::UserFillByTime;
use crate::services::journal_service::{TradeCloseEvent, JournalService};

/// Build a TradeCloseEvent from an HL closing fill.
/// Reused by both import_worker (batch) and ws_fills (live poll).
pub fn build_trade_close_event(
    fill: &UserFillByTime,
    user_id: Uuid,
    open_time_ms: Option<u64>,
    source: &str,
) -> Option<TradeCloseEvent> {
    // Filter non-closing fills
    if fill.closed_pnl == "0" || fill.closed_pnl == "0.0" {
        return None;
    }
    if fill.coin.starts_with('@') {
        return None; // Skip spot
    }

    let exit_price = Decimal::from_str(&fill.px).ok()?;
    let quantity = Decimal::from_str(&fill.sz).ok()?;
    let closed_pnl = Decimal::from_str(&fill.closed_pnl).ok()?;
    let fee = Decimal::from_str(&fill.fee).ok()?;

    if quantity == Decimal::ZERO { return None; }

    let side = if fill.dir.contains("Long") { "LONG" }
               else if fill.dir.contains("Short") { "SHORT" }
               else { match fill.side.as_str() {
                   "B" => "SHORT",
                   "A" => "LONG",
                   _ => return None,
               }};

    let entry_price = match side {
        "LONG" => exit_price - (closed_pnl / quantity),
        "SHORT" => exit_price + (closed_pnl / quantity),
        _ => exit_price,
    };

    let closed_at = timestamp_to_datetime(fill.time);
    let opened_at = open_time_ms.map(timestamp_to_datetime).unwrap_or(closed_at);
    let symbol = format!("{}_USDT", fill.coin);

    Some(TradeCloseEvent {
        user_id,
        exchange: "hyperliquid".to_string(),
        symbol,
        side: side.to_string(),
        entry_price,
        exit_price,
        quantity,
        leverage: 1,
        fees: fee,
        stop_price: None,
        target_price: None,
        risk_amount: None,
        opened_at,
        closed_at,
        trade_group_id: None,
        exchange_order_ids: vec![fill.oid.to_string()],
        source: Some(source.to_string()),
        exchange_fill_id: Some(fill.tid as i64),
    })
}
```

### Changes to HyperliquidFillSubscriber

Add to `HyperliquidFillSubscriber`:

```rust
pub struct HyperliquidFillSubscriber {
    // ... existing fields ...
    journal: Option<JournalService>,          // NEW: direct journal writer
    user_id: Option<Uuid>,                    // NEW: testudo user_id for journal writes
    notify_pool: Option<PgPool>,              // NEW: for pg_notify after journal write
    open_times: HashMap<String, u64>,         // NEW: coin → most recent open fill timestamp
    seen_tids: HashSet<i64>,                  // NEW: dedup by HL tid (separate from seen_oids)
}
```

Modify `reconcile_since()`:
- Keep existing behavior: send OrderUpdateEvents for WS-matched fills (entry fills still go to FillDetector)
- Add: for fills with `closedPnl != "0"`, build TradeCloseEvent and write directly to journal
- Track `open_times` from `dir.starts_with("Open")` fills across all poll cycles
- Seed `open_times` from the 24h startup reconciliation

### Changes to import_worker.rs

Replace inline `process_hl_fill()` body with call to shared `build_trade_close_event()`:

```rust
async fn process_hl_fill(&self, fill: &UserFillByTime, user_id: Uuid, open_time_ms: Option<u64>) -> Result<bool, ImportError> {
    let event = match hl_fill_journal::build_trade_close_event(fill, user_id, open_time_ms, "import_hl") {
        Some(e) => e,
        None => return Ok(false),
    };
    match self.journal.record_trade_close(event).await {
        Ok(_) => Ok(true),
        Err(e) => { /* existing dedup handling */ }
    }
}
```

### Paved Roads

- `import_worker.rs:276-363` — Proven HL fill → journal write logic (reuse via extraction)
- `JournalService::record_trade_close()` — Idempotent journal writer with daily stats + draft notes
- `idx_unique_import_fill` unique index on `(user_id, exchange, exchange_fill_id)` — DB-level dedup
- `ws_fills.rs::reconcile_since()` — Existing 30s REST poll infrastructure
- `main.rs:517` — Existing `pg_notify` pattern for management events

### Files

- `crates/router/src/services/hl_fill_journal.rs` — NEW: shared `build_trade_close_event()` + `timestamp_to_datetime()`
- `crates/router/src/services/hyperliquid/ws_fills.rs` — Add journal write path in `reconcile_since()`, add `open_times`/`seen_tids` state, add `JournalService` dependency
- `crates/router/src/services/import_worker.rs` — Delegate to shared `build_trade_close_event()`
- `crates/router/src/services/mod.rs` — Export new module
- `crates/router/src/main.rs` — Wire `JournalService` + `user_id` + `PgPool` into WsSubscriptionManager → HyperliquidFillSubscriber

### Dependencies Added

None — all dependencies already in the workspace.

---

## Acceptance Criteria

- [ ] HL closing fills appear in `journal_trades` within 30s of fill on exchange
- [ ] `journal_daily_stats` updated on each HL journal write (daily P&L chart works)
- [ ] Trade duration shows actual hold time (not "0s") for positions held > 1 poll cycle
- [ ] `exchange` column shows `"hyperliquid"` (not `"cex"`)
- [ ] Entry price correctly derived from closedPnl (matches HL's reported P&L)
- [ ] No duplicate journal entries across poll cycles (dedup by tid)
- [ ] No duplicate journal entries between poller and import worker (same dedup key)
- [ ] Import worker still functions correctly for batch backfill
- [ ] CCXT/WOO fill detection path unchanged (FillDetector OID matching)
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Two journal write paths** — Both CON-01 (TradeEventWriter) and this direct path write to `journal_trades`. Mitigation: different dedup keys (`trade_group_id` vs `exchange_fill_id`), both idempotent. HL fills will only arrive via the new path since FillDetector drops them anyway.

2. **open_times tracking gaps** — If the opening fill happened before the 24h startup window, duration falls back to 0s. Mitigation: acceptable edge case; import worker can backfill correct duration. Positions open > 24h before service restart are rare.

3. **JournalService in subscriber increases coupling** — The subscriber gains a DB dependency. Mitigation: optional (`Option<JournalService>`). When None, falls back to current behavior (OrderUpdateEvent only). Clean degradation.

4. **pg_notify volume** — One notify per closing fill per 30s poll. Mitigation: closing fills are infrequent (a few per day at most). Negligible load.

---

## Completion Signal

This spec is complete when:
1. HL trades appear in the journal page within 30s of close
2. Overview charts (equity curve, daily P&L) reflect HL trade data
3. All acceptance criteria pass
4. Code committed to master
