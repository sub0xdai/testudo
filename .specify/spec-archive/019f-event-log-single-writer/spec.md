# Specification: Event Log + Single-Writer Persistence

**Spec ID:** 019f-event-log-single-writer
**Date:** 2026-03-12
**Status:** Complete
**Class:** Architecture
**Parent:** 019-actor-model-event-sourcing
**Phase:** 6 of 6
**Depends on:** 019e-lock-removal-zombie-detection

---

## Overview

Add financial-grade audit logging via an append-only `trade_events` PostgreSQL table, and consolidate all trade-related database writes through a single `TradeEventWriter` that writes both the event log AND mutable state tables in a single transaction. This eliminates the dual-write divergence risk where in-memory state, mutable DB tables, and the event log could get out of sync.

**Origin (Freqtrade comparison):**
Freqtrade uses mutable SQLAlchemy — an `UPDATE trades SET is_open = false` destroys previous state. When a bug causes incorrect closure (like Testudo's ghost position #3315-3318), the prior state is gone. An append-only event log preserves every transition. Debugging becomes `SELECT * FROM trade_events WHERE group_id = $1 ORDER BY seq`.

**Critical design decision — Single-Writer pattern:**
The `TradeEventWriter` is the SOLE writer to PostgreSQL for trade-related state. The actor never writes to the database directly. For each batch of events:

```
BEGIN;
  INSERT INTO trade_events (...) VALUES ...;     -- append-only log
  UPDATE order_groups SET status = $1 WHERE ...;  -- mutable fast-read table
  UPDATE managed_positions SET ... WHERE ...;     -- if applicable
COMMIT;
```

This guarantees the mutable rehydration tables never diverge from the audit log.

**Current state:**
- Write-behind persistence: in-memory state changes first, callers persist to `order_groups` / `managed_positions` afterward, outside locks. Crash between mutation and persist loses state.
- No audit trail for state transitions.

**Target state:**
- Every state transition logged as an immutable event.
- Mutable tables and event log updated atomically in one transaction.
- Debugging any issue = `SELECT * FROM trade_events WHERE group_id = $1 ORDER BY seq`.
- `GET /api/v1/trades/{id}/events` endpoint for the extension to query event history.

---

## Constraint: Non-Blocking Actor

The actor must NEVER block on database writes. Events are emitted via `try_send()` (non-blocking) to the `TradeEventWriter`. If the event channel is full, the actor drops the event and increments a metric. The `TradeEventWriter` batches and flushes asynchronously.

At-most-once delivery from actor to writer is acceptable for the audit log — the mutable state (owned by the actor) is always authoritative. The event log is supplementary evidence, not the source of truth for state. Lost events are detectable by sequence gaps.

---

## Functional Requirements

### Event Schema

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Create `trade_events` table: `(seq BIGSERIAL PRIMARY KEY, event_type TEXT NOT NULL, group_id UUID, user_id UUID NOT NULL, symbol TEXT, payload JSONB NOT NULL DEFAULT '{}', created_at TIMESTAMPTZ NOT NULL DEFAULT now())`. Index on `(group_id, seq)` and `(user_id, created_at)`. | Critical | Database |
| FR-2 | Define `TradeEvent` enum with variants: `TradeCreated`, `EntryPlaced`, `EntryFilled { fill_price }`, `StopLossPlaced`, `StopLossFilled`, `TakeProfitPlaced`, `TakeProfitFilled`, `OrderCancelled { reason }`, `GroupStatusChanged { from, to }`, `BreakEvenTriggered`, `StopLossAmended { old_price, new_price }`, `ReconciliationAction { action }`, `PlacementTimeout { group_id }`. Each variant carries `group_id`, `user_id`, `symbol`, and variant-specific payload. | Critical | Engine |

### Actor Event Emission

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-3 | Add `event_tx: mpsc::Sender<TradeEvent>` to `EngineActor`. Channel capacity: 1024. | Critical | Engine |
| FR-4 | After each state-mutating command dispatch, emit the appropriate `TradeEvent` via `self.event_tx.try_send()`. On `TrySendError::Full`, increment a dropped-events counter and log `tracing::warn!`. | Critical | Engine |
| FR-5 | Wire the event channel: create in `main.rs`, pass sender to actor, pass receiver to `TradeEventWriter`. | Critical | Router |

### Single-Writer TradeEventWriter

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-6 | Implement `TradeEventWriter` as a background Tokio task. It owns `mpsc::Receiver<TradeEvent>` and a `PgPool`. | Critical | Router |
| FR-7 | Batching: flush every 100ms or 50 events, whichever comes first. Use `tokio::select!` on the channel and a `tokio::time::interval`. | Critical | Router |
| FR-8 | Each flush executes a SINGLE Postgres transaction containing: (1) bulk `INSERT INTO trade_events` for all events in the batch, (2) `UPDATE order_groups` for any `GroupStatusChanged` events, (3) `UPDATE managed_positions` for any fill events that affect positions. | Critical | Router |
| FR-9 | On transaction failure: log error, retain batch, retry with exponential backoff (100ms, 200ms, 400ms). After 3 failures: log `tracing::error!`, emit `pg_notify('system.alerts', ...)`, discard batch and continue. | High | Router |
| FR-10 | Remove existing write-behind persistence calls from callers. The `TradeEventWriter` is now the ONLY code path that writes to `order_groups` and `managed_positions`. | Critical | Router |

### API Endpoint

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-11 | Add `GET /api/v1/trades/{id}/events` endpoint. Returns the event history for a trade group, ordered by `seq`. Authenticated, user-scoped (only the owning user can see events). | Medium | Router |
| FR-12 | Response format: `{ "events": [{ "seq": 1, "event_type": "TradeCreated", "payload": {...}, "created_at": "..." }, ...] }` | Medium | Router |

### Configuration

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-13 | Live trades MUST have events logged. Shadow/paper trades SHOULD have events logged but MAY be gated by a config flag `SHADOW_EVENT_LOGGING=true/false` if write volume is a concern. Default: `true`. | Medium | Router |

### Testing

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-14 | Test: full trade lifecycle (create → entry fill → SL fill) produces expected event sequence in `trade_events` table. | Critical | Test |
| FR-15 | Test: `GroupStatusChanged` event in batch triggers corresponding `UPDATE order_groups` in the same transaction. | Critical | Test |
| FR-16 | Test: `TradeEventWriter` flush failure retries 3 times, then discards and continues processing. | High | Test |
| FR-17 | Test: `GET /api/v1/trades/{id}/events` returns events for the authenticated user, 403 for other users. | High | Test |

---

## Technical Implementation

### 1) trade_events Table (FR-1)

**New migration:** `crates/sqlx_postgres/migrations/YYYYMMDD_trade_events.up.sql`

```sql
CREATE TABLE trade_events (
    seq         BIGSERIAL PRIMARY KEY,
    event_type  TEXT NOT NULL,
    group_id    UUID,
    user_id     UUID NOT NULL,
    symbol      TEXT,
    payload     JSONB NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_trade_events_group ON trade_events (group_id, seq);
CREATE INDEX idx_trade_events_user  ON trade_events (user_id, created_at);
```

### 2) TradeEvent Enum (FR-2)

**New file:** `crates/engine/src/shadow/trade_event.rs`

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct TradeEvent {
    pub event_type: TradeEventType,
    pub group_id: Option<Uuid>,
    pub user_id: Uuid,
    pub symbol: Option<String>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum TradeEventType {
    TradeCreated,
    EntryPlaced,
    EntryFilled,
    StopLossPlaced,
    StopLossFilled,
    TakeProfitPlaced,
    TakeProfitFilled,
    OrderCancelled,
    GroupStatusChanged,
    BreakEvenTriggered,
    StopLossAmended,
    ReconciliationAction,
    PlacementTimeout,
}
```

### 3) TradeEventWriter — Single-Transaction Flush (FR-6, FR-7, FR-8)

**New file:** `crates/router/src/services/trade_event_writer.rs`

```rust
pub struct TradeEventWriter {
    rx: mpsc::Receiver<TradeEvent>,
    pool: PgPool,
}

impl TradeEventWriter {
    pub async fn run(mut self) {
        let mut batch: Vec<TradeEvent> = Vec::with_capacity(50);
        let mut flush_interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                Some(event) = self.rx.recv() => {
                    batch.push(event);
                    if batch.len() >= 50 {
                        self.flush(&mut batch).await;
                    }
                }
                _ = flush_interval.tick() => {
                    if !batch.is_empty() {
                        self.flush(&mut batch).await;
                    }
                }
            }
        }
    }

    async fn flush(&self, batch: &mut Vec<TradeEvent>) {
        let mut retries = 0;
        loop {
            match self.flush_transaction(batch).await {
                Ok(_) => {
                    batch.clear();
                    return;
                }
                Err(e) => {
                    retries += 1;
                    if retries > 3 {
                        tracing::error!(
                            error = %e,
                            batch_size = batch.len(),
                            "TradeEventWriter: flush failed after 3 retries, discarding batch"
                        );
                        // Alert via pg_notify
                        let _ = sqlx::query("SELECT pg_notify('system.alerts', $1)")
                            .bind(format!("event_writer_flush_failed: {}", e))
                            .execute(&self.pool).await;
                        batch.clear();
                        return;
                    }
                    let delay = Duration::from_millis(100 * (1 << retries));
                    tracing::warn!(error = %e, retry = retries, "TradeEventWriter: flush failed, retrying");
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    async fn flush_transaction(&self, batch: &[TradeEvent]) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        // 1. Bulk insert events
        for event in batch {
            sqlx::query(
                "INSERT INTO trade_events (event_type, group_id, user_id, symbol, payload)
                 VALUES ($1, $2, $3, $4, $5)"
            )
            .bind(event.event_type.as_str())
            .bind(event.group_id)
            .bind(event.user_id)
            .bind(&event.symbol)
            .bind(&event.payload)
            .execute(&mut *tx).await?;
        }

        // 2. Apply mutable state updates derived from events
        for event in batch {
            match &event.event_type {
                TradeEventType::GroupStatusChanged => {
                    // Extract from/to from payload, update order_groups table
                    if let (Some(group_id), Some(to_status)) = (event.group_id, event.payload.get("to")) {
                        sqlx::query(
                            "UPDATE order_groups SET status = $1, updated_at = now() WHERE id = $2"
                        )
                        .bind(to_status.as_str())
                        .bind(group_id)
                        .execute(&mut *tx).await?;
                    }
                }
                TradeEventType::EntryFilled | TradeEventType::StopLossFilled | TradeEventType::TakeProfitFilled => {
                    // Update managed_positions if this is a live trade
                    if let Some(group_id) = event.group_id {
                        sqlx::query(
                            "UPDATE managed_positions SET updated_at = now() WHERE group_id = $1"
                        )
                        .bind(group_id)
                        .execute(&mut *tx).await?;
                    }
                }
                _ => {} // Other events are log-only
            }
        }

        tx.commit().await?;
        Ok(())
    }
}
```

### 4) Remove Existing Write-Behind Calls (FR-10)

Search the codebase for all direct writes to `order_groups` and `managed_positions` tables. These exist in:
- `trade_management.rs` — `persist_closed()` calls
- `fill_detector.rs` — terminal state persistence
- `rehydration.rs` — position insertion

All of these write paths must be removed. The ONLY writer is now `TradeEventWriter`. The actor emits the event, the writer persists it.

**Discovery step:** `grep -r "managed_positions" crates/router/src/` and `grep -r "order_groups" crates/router/src/ | grep -i "insert\|update\|delete"` to find all direct DB writes.

---

## Files to Create

| File | Contents |
|------|----------|
| `crates/engine/src/shadow/trade_event.rs` | `TradeEvent`, `TradeEventType` |
| `crates/router/src/services/trade_event_writer.rs` | `TradeEventWriter` |
| `crates/router/src/routes/trade_events.rs` | `GET /api/v1/trades/{id}/events` |
| `crates/sqlx_postgres/migrations/YYYYMMDD_trade_events.up.sql` | Table + indexes |

## Files to Modify

| File | Changes |
|------|---------|
| `crates/engine/src/shadow/mod.rs` | Add `pub mod trade_event;` |
| `crates/engine/src/shadow/actor.rs` | FR-3, FR-4: Add `event_tx`, emit events after mutations |
| `crates/router/src/main.rs` | FR-5: Create event channel, spawn `TradeEventWriter` |
| `crates/router/src/services/mod.rs` | Add `pub mod trade_event_writer;` |
| `crates/router/src/routes/mod.rs` | Add `pub mod trade_events;` |
| `crates/router/src/routes/trade_management.rs` | FR-10: Remove direct DB writes for trade state |
| `crates/router/src/services/fill_detector.rs` | FR-10: Remove direct DB writes for terminal state |

---

## Verification

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

- [ ] `trade_events` table created with both indexes
- [ ] `TradeEvent` enum covers all state transitions
- [ ] Actor emits events via `try_send()` after each mutation
- [ ] `TradeEventWriter` batches and flushes every 100ms or 50 events
- [ ] Each flush writes events + mutable state updates in ONE transaction
- [ ] No direct writes to `order_groups` or `managed_positions` outside `TradeEventWriter`
- [ ] Flush failure retries 3x with backoff, then discards and alerts
- [ ] `GET /api/v1/trades/{id}/events` returns correct events, user-scoped
- [ ] Full lifecycle test: trade_events table contains expected sequence
- [ ] All existing tests pass (zero regression)

---

## Completion Signal

All verification checkboxes green. `cargo clippy --all-targets && cargo test` passes. Event audit trail queryable. Single-writer persistence guarantees mutable state and event log never diverge.
