# Specification: Co-locate Journal Writes in TradeEventWriter Transaction

**Spec ID:** CON-01-journal-cowrite
**Date:** 2026-03-30
**Status:** Draft
**Class:** Infrastructure / Data Consistency
**Priority:** P0 — Fire-and-forget `tokio::spawn` in fill_detector.rs:582 loses journal writes on crash or task cancellation. Financial records become permanently incomplete.
**Depends on:** None (first in series)
**Series:** CON-01 through CON-03 (Distributed data consistency hardening)

---

## Problem Statement

When a stop-loss or take-profit fills, `FillDetectorService::fire_journal_write()` (`router/src/services/fill_detector.rs:514-587`) constructs a `TradeCloseEvent` and spawns it as a detached Tokio task:

```rust
tokio::spawn(async move {
    if let Err(e) = journal.record_trade_close(event).await {
        tracing::warn!("Journal write failed: {}", e);
    }
});
```

This is a classic fire-and-forget dual-write. The engine state has already been updated (group marked `StoppedOut` or `TookProfit` via `engine_handle.on_stop_loss_filled()` at line 234), and the `trade_events` table will record the fill via `TradeEventWriter`. But the `journal_trades` INSERT runs in an untracked, un-awaited, un-retried spawned task. If the process shuts down, the runtime drops pending futures, or the DB is temporarily unreachable — the journal write is lost permanently. The `JournalService::record_trade_close()` is idempotent by `trade_group_id`, but idempotency only helps if the write is ever retried. Currently it is not.

The fix is to co-locate the journal write inside `TradeEventWriter::flush_transaction()`, which already atomically writes `trade_events` + updates `managed_positions` state in a single PostgreSQL transaction. Since `TradeEventWriter` already processes `StopLossFilled` and `TakeProfitFilled` events and has the `PgPool`, extending it to also INSERT into `journal_trades` within the same transaction eliminates the consistency gap with zero new infrastructure.

---

## User Stories

- **As a trader**, I want every closed trade to appear in my journal, so that my P&L history is accurate and I can trust my trading statistics.
- **As the system operator**, I want trade close records to be atomic with the event log, so that I never have to manually reconcile missing journal entries after a crash or restart.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `TradeEventWriter::flush_transaction()` inserts a `journal_trades` row when processing `StopLossFilled` or `TakeProfitFilled` events, within the same DB transaction as the event insert and `managed_positions` update. | High | trade_event_writer |
| FR-2 | The `TradeEvent` payload for fill events must include all fields needed to construct a `TradeCloseEvent` (user_id, symbol, side, entry_price, exit_price, quantity, leverage, fees, stop_price, target_price, opened_at, group_id, exchange_order_ids). | High | engine / trade_event |
| FR-3 | Journal insert is idempotent: if `trade_group_id` already exists in `journal_trades`, skip the insert (matching existing `JournalService` behavior). | High | trade_event_writer |
| FR-4 | `DerivedFields` computation (P&L, R-multiple, duration) is extracted into a pure function callable from both `JournalService` and `TradeEventWriter` (already exists as `compute_derived_fields()`). | Medium | journal_service |
| FR-5 | Remove `fire_journal_write()` from `FillDetectorService` and the `tokio::spawn` call sites at lines 336 and 372. | High | fill_detector |
| FR-6 | Daily stats upsert (`upsert_daily_stats`) is called within the same transaction, or enqueued to `queue_database` for deferred processing if including it would make the transaction too complex. | Medium | trade_event_writer |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Enrich `TradeEvent` payload with journal fields for fill events. Write a unit test asserting the payload contains all required fields. | Payload completeness |
| CP-2 | Add journal INSERT inside `flush_transaction()` for SL/TP fill events. Test with existing integration test infrastructure. | Atomic co-write |
| CP-3 | Remove `fire_journal_write()` and `tokio::spawn` from `FillDetectorService`. Verify builds clean. | Dead code removal |

### Enriched TradeEvent Payload

The `TradeEvent` struct (`engine/src/shadow/trade_event.rs`) already carries a `payload: serde_json::Value`. For `StopLossFilled` and `TakeProfitFilled` events, the payload must be enriched at the emission site (inside `EngineActor` or `FillDetectorService`) to include:

```rust
// Payload shape for fill events (embedded in TradeEvent.payload)
{
    "entry_price": "50000.00",
    "exit_price": "49000.00",
    "quantity": "0.1",
    "side": "LONG",          // derived from close_side
    "leverage": 1,
    "fees": "0.00",
    "stop_price": "49000.00",
    "target_price": "52000.00",
    "opened_at": "2026-03-30T10:00:00Z",
    "exchange_order_ids": ["order-1", "order-2"],
    "exchange": "cex"
}
```

### Journal Co-Write in flush_transaction

```rust
// Inside TradeEventWriter::flush_transaction(), after the event INSERT loop:
for event in batch {
    match event.event_type {
        TradeEventType::StopLossFilled | TradeEventType::TakeProfitFilled => {
            if let Some(group_id) = event.group_id {
                // Idempotency check
                let exists: Option<(i64,)> = sqlx::query_as(
                    "SELECT 1 FROM journal_trades WHERE trade_group_id = $1"
                )
                .bind(group_id)
                .fetch_optional(&mut *tx)
                .await?;

                if exists.is_none() {
                    // Extract fields from enriched payload
                    let p = &event.payload;
                    // ... parse fields, compute derived ...
                    sqlx::query(
                        "INSERT INTO journal_trades (...) VALUES (...)"
                    )
                    // ... bind all fields ...
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }
        // existing EntryFilled / managed_positions update logic unchanged
        _ => {}
    }
}
```

### Method Mapping

| Current (fire-and-forget) | New (co-write) |
|---------------------------|----------------|
| `FillDetectorService::fire_journal_write()` constructs `TradeCloseEvent`, spawns task | Removed entirely |
| `JournalService::record_trade_close()` called in detached `tokio::spawn` | Journal INSERT inlined in `TradeEventWriter::flush_transaction()` |
| `compute_derived_fields()` in `journal_service.rs` | Shared — called from both `JournalService` (for imports) and `TradeEventWriter` (for live fills) |
| Daily stats upsert: fire-and-forget after journal insert | Enqueued to `queue_database` after transaction commits (guaranteed by pg_queue at-least-once) |

### Paved Roads

- **TradeEventWriter batched transaction pattern** (`trade_event_writer.rs:112-158`): Already does `pool.begin()` → bulk inserts → derived updates → `tx.commit()`. We extend this, not replace it.
- **`compute_derived_fields()`** (`journal_service.rs:48-83`): Pure function, already unit-tested, takes `&TradeCloseEvent` and returns `DerivedFields`.
- **`JournalService::record_trade_close()` idempotency** (`journal_service.rs:101-120`): Check by `trade_group_id` before insert. We replicate this inside the transaction.
- **pg_queue for deferred work** (`pg_queue/src/queue.rs:58-70`): `push()` enqueues JSONB payloads with LISTEN/NOTIFY triggers. Use for daily stats if needed.

### Files

- `crates/router/src/services/trade_event_writer.rs` — Add journal INSERT to `flush_transaction()` for fill events
- `crates/router/src/services/fill_detector.rs` — Remove `fire_journal_write()` method and `tokio::spawn` call sites (lines 336, 372, 514-587)
- `crates/router/src/services/journal_service.rs` — Extract `compute_derived_fields()` to be importable (already public), no structural changes
- `crates/engine/src/shadow/trade_event.rs` — Document enriched payload contract for fill events (no struct changes, payload is `serde_json::Value`)

### Dependencies Added

None. All required crates (`sqlx`, `serde_json`, `rust_decimal`, `chrono`) are already in `router/Cargo.toml`.

---

## Acceptance Criteria

- [ ] `StopLossFilled` and `TakeProfitFilled` events produce a `journal_trades` row in the same DB transaction as the `trade_events` insert and `managed_positions` state update.
- [ ] Journal insert is idempotent: duplicate `trade_group_id` does not error or create duplicate rows.
- [ ] `fire_journal_write()` and both `tokio::spawn` call sites are removed from `fill_detector.rs`.
- [ ] `JournalService::record_trade_close()` remains functional for the import pipeline (`import_worker.rs`).
- [ ] `compute_derived_fields()` is called from `TradeEventWriter` with the same inputs and produces identical P&L, R-multiple, and duration values as the existing path.
- [ ] If the flush transaction fails and retries, the journal write is retried atomically with the rest of the batch.
- [ ] Daily stats upsert either runs in-transaction or is enqueued to `queue_database` with at-least-once delivery.
- [ ] `cargo clippy --all-targets && cargo test` passes with zero warnings.

---

## Risks

1. **Payload bloat** — Enriching the `TradeEvent` JSONB payload with journal fields increases the average event size. Mitigation: Only fill events (2 of 13 event types) carry the enriched payload. The additional ~200 bytes per fill event is negligible against the 50-event batch size.

2. **Transaction duration increase** — Adding a journal INSERT + idempotency check to the flush transaction increases its duration. Mitigation: The idempotency check is a single-row index lookup on `trade_group_id` (already indexed). The journal INSERT is a single row. Combined overhead is <1ms, well within the 100ms flush interval.

3. **Draft notes merge** — `JournalService::record_trade_close()` currently merges draft notes from `journal_trade_drafts` after the initial insert (lines 168-191). This secondary step can remain fire-and-forget or be moved to a separate queue job. Mitigation: Draft notes are a UX convenience, not a financial consistency requirement. Keep the merge as a post-commit step via `queue_database`.

---

## Completion Signal

This spec is complete when:
1. Journal writes for trade closes happen atomically within the `TradeEventWriter` flush transaction
2. All `tokio::spawn` journal writes are removed from `FillDetectorService`
3. All acceptance criteria met
4. `cargo clippy --all-targets && cargo test` passes
5. Code committed to master
