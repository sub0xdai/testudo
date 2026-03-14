# Specification: Actor Route Migration

**Spec ID:** 019c-actor-route-migration
**Date:** 2026-03-12
**Status:** Complete
**Class:** Architecture
**Parent:** 019-actor-model-event-sourcing
**Phase:** 3 of 6
**Depends on:** 019b-actor-adapter-migration

---

## Overview

Migrate all HTTP route handlers from `Arc<RwLock<ShadowEngine>>` to `EngineHandle`. The critical change in this phase is restructuring `create_trade()` to decouple network I/O from engine state — the primary performance win of the actor model.

**Current state:**
- `trade_management.rs` acquires write locks during 1-3 exchange round-trips (100-500ms each), blocking `list_trades()`, `process_price_update()`, and GC.
- `order.rs` and `paper_balance.rs` access the engine through `state.shadow_engine.read().await`.

**Target state:**
- All route handlers use `state.engine_handle.method().await`.
- `create_trade()` follows the decoupled I/O pattern: place in actor (microseconds) → exchange I/O (no state held) → register IDs in actor (microseconds).
- `list_trades()` responds immediately even during concurrent `create_trade()`.

---

## Constraint: Behavioral Equivalence

The decoupled I/O pattern changes the *timing* of lock acquisition but not the *semantics*. All responses must be identical. The only observable difference is reduced latency for concurrent read operations.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Migrate `create_trade` in `trade_management.rs` to use `EngineHandle`. Restructure to: (1) `engine_handle.place_order().await` — returns immediately, (2) exchange I/O via CCXT — no engine state held, (3) `engine_handle.register_exchange_ids().await` — returns immediately. | Critical | Router |
| FR-2 | Migrate `list_trades`, `get_trade`, `cancel_trade` in `trade_management.rs` to use `EngineHandle`. | Critical | Router |
| FR-3 | Migrate `update_stop_loss`, `update_take_profit`, `update_entry_price`, `enable_break_even` in `trade_management.rs` to use `EngineHandle`. | Critical | Router |
| FR-4 | Migrate `execute_order`, `get_open_orders`, `cancel_all_orders` in `order.rs` to use `EngineHandle`. | High | Router |
| FR-5 | Migrate paper balance routes (`get_balances`, `reset_user`) in `paper_balance.rs` to use `EngineHandle`. | High | Router |
| FR-6 | Test: `create_trade` through actor does NOT hold engine state during simulated exchange I/O delay. Verify `list_trades` returns immediately even when `create_trade` is mid-flight. | Critical | Test |
| FR-7 | Test: concurrent `list_trades` and `create_trade` produce consistent state (no stale reads). | High | Test |

---

## Technical Implementation

### 1) Decoupled create_trade (FR-1)

This is the most important change. Current flow holds locks during network I/O:

```
acquire engine read lock
  acquire order_groups write lock
    place shadow order
    place entry on exchange (100-500ms)    <-- LOCK HELD
    place SL on exchange (100-500ms)       <-- LOCK HELD
    place TP on exchange (100-500ms)       <-- LOCK HELD
    register exchange order IDs
  release order_groups write lock
release engine read lock
```

New flow — engine state never held during I/O:

```
engine_handle.place_order(order).await          // actor: microseconds
engine_handle.create_order_group(group).await   // actor: microseconds

place entry on exchange (network I/O)           // NO ENGINE STATE
place SL on exchange (network I/O)              // NO ENGINE STATE
place TP on exchange (network I/O)              // NO ENGINE STATE

engine_handle.register_exchange_ids(ids).await  // actor: microseconds
engine_handle.update_group_status(Active).await // actor: microseconds
```

**Error handling for the decoupled flow:**
- If entry placement fails: `engine_handle.cancel_order(entry_id).await` + `engine_handle.update_group_status(Cancelled).await`
- If SL placement fails: cancel the entry on exchange, then `engine_handle.cancel_order(entry_id).await` + rollback group (per AUD-01 FR-2)
- If TP placement fails: proceed without TP, log warning (per AUD-01 FR-3)

### 2) Simple Route Migrations (FR-2, FR-3, FR-4, FR-5)

Pattern for simple read operations:

Before:
```rust
let engine = state.shadow_engine.read().await;
let groups = engine.order_groups.read().await.list_groups(user_id);
```

After:
```rust
let groups = state.engine_handle.list_trade_groups(user_id).await;
```

Pattern for mutations:

Before:
```rust
let engine = state.shadow_engine.read().await;
let mut groups = engine.order_groups.write().await;
groups.update_status(group_id, new_status)?;
```

After:
```rust
state.engine_handle.update_group_status(group_id, new_status).await?;
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `crates/router/src/routes/trade_management.rs` | FR-1, FR-2, FR-3: Replace all lock-based access with `EngineHandle`. Restructure `create_trade()` for decoupled I/O. |
| `crates/router/src/routes/order.rs` | FR-4: Replace lock-based access with `EngineHandle`. |
| `crates/router/src/routes/paper_balance.rs` | FR-5: Replace lock-based access with `EngineHandle`. |

---

## Verification

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

- [ ] `create_trade()` does NOT hold engine state during exchange I/O
- [ ] `list_trades()` responds immediately during concurrent `create_trade()`
- [ ] All trade management routes use `EngineHandle`
- [ ] All order routes use `EngineHandle`
- [ ] All paper balance routes use `EngineHandle`
- [ ] Error handling preserved (SL failure → rollback, TP failure → proceed)
- [ ] No `shadow_engine.read().await` or `shadow_engine.write().await` in any route file
- [ ] All existing tests pass (zero regression)

---

## Completion Signal

All verification checkboxes green. `cargo clippy --all-targets && cargo test` passes. No new warnings.
