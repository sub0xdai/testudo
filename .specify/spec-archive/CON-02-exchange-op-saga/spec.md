# Specification: Exchange Operation Saga with Recoverable Intent State

**Spec ID:** CON-02-exchange-op-saga
**Date:** 2026-03-30
**Status:** Draft
**Class:** Infrastructure / Data Consistency
**Priority:** P0 — If process crashes between exchange order placement and shadow engine ID registration, the exchange has a live order that rehydration cannot reconcile. Real money at risk.
**Depends on:** None (independent of CON-01)
**Series:** CON-01 through CON-03 (Distributed data consistency hardening)

---

## Problem Statement

The trade creation flow in `trade_management.rs:806-967` follows a three-phase pattern with no durable intent record:

1. **Shadow state** (lines 806-848): `engine_handle.place_order()` + `engine_handle.configure_group()` — in-memory only.
2. **Exchange I/O** (lines 867-883): `tm.place_order()` → HTTP to CCXT sidecar or Hyperliquid SDK — external, non-transactional.
3. **ID registration** (lines 897-924): `engine_handle.register_exchange_order_id()` — in-memory only.
4. **DB persistence** (lines 975-1019): `tm.register(managed)` — PostgreSQL, but only logs a warning on failure.

If the process crashes after phase 2 but before phase 3 or 4, the exchange has a live order with real margin locked, but:
- The shadow engine has no exchange order ID mapped to the group (lost on crash, in-memory only).
- `managed_positions` has no record of the position (phase 4 never ran).
- On restart, `RehydrationService` loads from `managed_positions` — finds nothing.
- The exchange order is orphaned. It can fill, consume margin, and the system has no record of it.

The current ambiguous-error handling (lines 956-965) already acknowledges this gap by keeping the shadow order tracked and warning the user. But it doesn't persist the intent to a durable store.

The fix is a **saga pattern** using `managed_positions.exchange_op` state column. Before calling the exchange, persist the intent (`placing`). After success, update to `confirmed`. On startup, rehydration retries or reconciles any rows stuck in `placing`.

---

## User Stories

- **As a trader**, I want my trade to either fully succeed (order on exchange + tracked in system) or fully fail (no order, no ghost state), so that I never have orphaned exchange orders consuming my margin.
- **As the system operator**, I want the system to self-heal after a crash during trade placement, so that I don't need to manually reconcile exchange orders against the database.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add `exchange_op VARCHAR(16) DEFAULT NULL` column to `managed_positions` table. Valid states: `NULL` (no pending op), `placing` (intent recorded, exchange call in-flight), `confirmed` (exchange acknowledged). | High | repository |
| FR-2 | Before calling `tm.place_order()` on the exchange, INSERT the `ManagedPosition` into the database with `exchange_op = 'placing'` and the full order parameters (symbol, side, price, quantity, SL, TP). | High | trade_management |
| FR-3 | On successful exchange response, UPDATE `exchange_op = 'confirmed'` and store the returned exchange order IDs. | High | trade_management |
| FR-4 | On definitive rejection from the exchange, UPDATE `exchange_op = NULL` and `state = 'closed'` (or DELETE the row). This is the rollback path. | High | trade_management |
| FR-5 | On ambiguous error (timeout, parse failure), leave `exchange_op = 'placing'`. Warn the user. The row survives restart for reconciliation. | High | trade_management |
| FR-6 | `RehydrationService` on startup: find all rows with `exchange_op = 'placing'`, query the exchange for open orders matching the `client_order_id`, and either confirm or rollback each. | High | rehydration |
| FR-7 | `client_order_id` (already stamped as `testudo:{group_id}:entry`) must be persisted in `managed_positions` so rehydration can match exchange orders by client ID. | Medium | repository |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Add `exchange_op` and `client_order_id` columns to `managed_positions`. Write migration + unit test for state transitions. | Schema correctness |
| CP-2 | Restructure `create_trade` handler: INSERT with `placing` before exchange call, UPDATE with `confirmed` after. Test both success and rejection paths. | Saga write-ahead |
| CP-3 | Extend `RehydrationService` to reconcile `placing` rows against exchange open orders. | Crash recovery |

### State Machine

```
                    ┌─────────┐
                    │  NULL    │ ← no exchange operation
                    └────┬────┘
                         │ INSERT (before exchange call)
                         ▼
                    ┌─────────┐
           ┌───────│ placing  │───────┐
           │       └─────────┘       │
           │ exchange confirms       │ exchange rejects definitively
           ▼                         ▼
      ┌──────────┐            ┌───────────┐
      │confirmed │            │ NULL/closed│
      └──────────┘            └───────────┘
           │
           │ (normal lifecycle continues)
           ▼
      state = 'filled' → 'managing' → 'closed'
```

### Modified Trade Creation Flow

```rust
// trade_management.rs — restructured create_trade

// Phase 1: Shadow state (unchanged)
let placed_order = state.engine_handle.place_order(user_id, order).await?;
let group = state.engine_handle.get_group_by_entry_order(placed_order.id).await;

// Phase 2: Persist intent BEFORE exchange call
let client_order_id = crate::services::numeric_client_order_id(group_id, 1);
managed.exchange_op = Some("placing".to_string());
managed.client_order_id = Some(client_order_id.clone());
tm.register(managed).await?;  // Now this MUST succeed — it's the write-ahead log

// Phase 3: Exchange I/O (unchanged HTTP call)
match tm.place_order(exchange_request).await {
    Ok(result) => {
        // Phase 4a: Confirm — update DB with exchange IDs
        tm.confirm_exchange_op(group_id, &result.id, result.stop_loss_order_id, result.take_profit_order_id).await?;
        state.engine_handle.register_exchange_order_id(group_id, OrderRole::Entry, result.id).await;
    }
    Err(e) if is_definitive_rejection(&e) => {
        // Phase 4b: Rollback — mark closed in DB, cancel shadow
        tm.rollback_exchange_op(group_id).await?;
        state.engine_handle.cancel_order(user_id, placed_order.id).await?;
        return HttpResponse::BadGateway().json(...);
    }
    Err(e) => {
        // Phase 4c: Ambiguous — leave as 'placing', warn user
        tracing::warn!("Ambiguous exchange error for group {}: {}", group_id, e);
    }
}
```

### Repository Changes

```rust
// repository.rs — new methods

/// Update exchange operation state to confirmed with exchange order IDs.
pub async fn confirm_exchange_op(
    &self,
    id: Uuid,
    entry_order_id: &str,
    sl_order_id: Option<&str>,
    tp_order_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    let order_ids = serde_json::json!({
        "entry_order_id": entry_order_id,
        "stop_loss_order_id": sl_order_id,
        "take_profit_order_id": tp_order_id,
    });
    sqlx::query(
        "UPDATE managed_positions SET exchange_op = 'confirmed', \
         exchange_order_ids = $2, updated_at = now() WHERE id = $1"
    )
    .bind(id)
    .bind(order_ids)
    .execute(&self.pool)
    .await?;
    Ok(())
}

/// Rollback a failed exchange operation.
pub async fn rollback_exchange_op(&self, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE managed_positions SET exchange_op = NULL, state = 'closed', \
         updated_at = now() WHERE id = $1"
    )
    .bind(id)
    .execute(&self.pool)
    .await?;
    Ok(())
}

/// Load positions stuck in 'placing' state (for startup reconciliation).
pub async fn load_placing(&self) -> Result<Vec<ManagedPosition>, sqlx::Error> {
    // ... SELECT WHERE exchange_op = 'placing'
}
```

### Rehydration Reconciliation

```rust
// rehydration.rs — new method

/// Reconcile positions stuck in 'placing' state after a crash.
/// For each, query exchange open orders by client_order_id.
/// If found → confirm. If not found → rollback.
pub async fn reconcile_placing_positions(&self) -> Result<usize, String> {
    let placing = self.repository.load_placing().await?;
    for pos in &placing {
        let client_id = pos.client_order_id.as_deref().unwrap_or("");
        match self.query_exchange_by_client_id(pos, client_id).await {
            Some(exchange_order) => {
                self.repository.confirm_exchange_op(pos.id, &exchange_order.id, ...).await?;
                // Also register in engine for fill detection
            }
            None => {
                self.repository.rollback_exchange_op(pos.id).await?;
            }
        }
    }
    Ok(placing.len())
}
```

### Paved Roads

- **`PositionRepository::insert()` and `update_state()`** (`trade_manager/repository.rs:75-128, 151-178`): Existing CRUD patterns. New methods follow the same `sqlx::query` + `.bind()` + `.execute()` pattern.
- **`RehydrationService::rehydrate()`** (`rehydration.rs:43-66`): Already loads `managed_positions` and rebuilds engine state. We add a pre-step for `placing` rows.
- **`numeric_client_order_id()`** (`services/mod.rs`): Already generates deterministic `testudo:{group_id}:1` IDs. These are already sent to the exchange and can be used for reconciliation.
- **`is_definitive_rejection()`** (`trade_management.rs`): Already classifies exchange errors. Reused for the rollback/ambiguous branching.
- **CCXT sidecar `fetchOpenOrders`** (`POST /orders/open`): Already used by rehydration verification. Can filter by `clientOrderId`.

### Files

- `crates/router/src/services/trade_manager/repository.rs` — Add `confirm_exchange_op()`, `rollback_exchange_op()`, `load_placing()` methods; add `exchange_op` and `client_order_id` columns to `create_table()`
- `crates/router/src/services/trade_manager/types.rs` — Add `exchange_op: Option<String>` and `client_order_id: Option<String>` fields to `ManagedPosition`
- `crates/router/src/routes/trade_management.rs` — Restructure `create_trade`: INSERT with `placing` before exchange call, UPDATE on success/rejection
- `crates/router/src/services/rehydration.rs` — Add `reconcile_placing_positions()` pre-step
- `crates/sqlx_postgres/migrations/` — New migration: `ALTER TABLE managed_positions ADD COLUMN exchange_op VARCHAR(16) DEFAULT NULL, ADD COLUMN client_order_id VARCHAR(64) DEFAULT NULL`

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `managed_positions` has `exchange_op` column with valid states: `NULL`, `placing`, `confirmed`.
- [ ] Trade creation INSERT happens BEFORE the exchange HTTP call, with `exchange_op = 'placing'`.
- [ ] Successful exchange response updates row to `exchange_op = 'confirmed'` with exchange order IDs.
- [ ] Definitive exchange rejection updates row to `state = 'closed'` and `exchange_op = NULL`.
- [ ] Ambiguous exchange error leaves row as `exchange_op = 'placing'` and warns user.
- [ ] On startup, `RehydrationService` finds and reconciles all `placing` rows before starting HTTP server.
- [ ] Paper trading (unauthenticated) path is unaffected — no exchange_op for shadow-only trades.
- [ ] `cargo clippy --all-targets && cargo test` passes.

---

## Risks

1. **Rehydration query latency** — Querying the exchange for each `placing` row adds startup time. Mitigation: `placing` rows should be rare (only exist if process crashed mid-placement). Typical count is 0-1. Batch the exchange query using `fetchOpenOrders` (already paginated by symbol) rather than per-order queries.

2. **Race condition: user retries while `placing` row exists** — If the user re-submits the same trade while a `placing` row exists from a prior attempt, they could get double orders. Mitigation: Before creating a new trade, check for existing `placing` rows for the same user + symbol. Return a 409 Conflict with details.

3. **Column migration on live database** — Adding `exchange_op` column requires `ALTER TABLE` on `managed_positions`. Mitigation: `ADD COLUMN ... DEFAULT NULL` is an online DDL in PostgreSQL — no table lock, no rewrite. The existing `CREATE TABLE IF NOT EXISTS` + idempotent `ALTER TABLE ADD COLUMN IF NOT EXISTS` pattern in `repository.rs:26-71` handles this.

---

## Completion Signal

This spec is complete when:
1. Trade creation persists intent to `managed_positions` before exchange I/O
2. Rehydration reconciles `placing` rows on startup
3. All acceptance criteria met
4. `cargo clippy --all-targets && cargo test` passes
5. Code committed to master
