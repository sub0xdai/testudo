# Specification: Lock Removal + In-Flight Zombie Detection

**Spec ID:** 019e-lock-removal-zombie-detection
**Date:** 2026-03-12
**Status:** Complete
**Class:** Architecture
**Parent:** 019-actor-model-event-sourcing
**Phase:** 5 of 6
**Depends on:** 019d-actor-service-migration

---

## Overview

Two changes now that the actor is the sole engine consumer:

1. **Lock Removal**: Remove `Arc<RwLock<ShadowEngine>>` entirely. The actor owns `ShadowEngine` directly. Remove inner `RwLock` wrappers on `ShadowOrderManager`, `ShadowPositionManager`, `OrderGroupManager`. Methods take `&mut self` instead of being called through `.write().await`.

2. **In-Flight Zombie Detection**: Address the split-brain vulnerability introduced by decoupling network I/O from engine state (019c). When a caller places an order in the actor then crashes before registering the exchange ID, the actor has a ghost order. Add a `pending_placement` tracker inside the actor that sweeps stale entries on a timer.

**Origin of zombie detection (Freqtrade comparison):**
Freqtrade uses a `rely_on_exchange_state` philosophy — periodic polling reconciles local state against exchange truth. Testudo's 018-reconciliation service handles exchange-level orphans. This spec handles the *pre-exchange* gap: orders in the actor that never reached the exchange because the caller dropped.

**Current state:**
- Actor wraps `Arc<RwLock<ShadowEngine>>` (temporary bridge from 019a).
- All callers migrated to `EngineHandle` (019b, 019c, 019d).
- No detection of orders stuck in placement limbo.

**Target state:**
- Actor owns `ShadowEngine` directly — no `Arc`, no `RwLock`, no inner locks.
- In-flight placement tracker detects orders that never complete the exchange registration step.
- Stale placements are flagged for reconciliation.

---

## Functional Requirements

### Lock Removal

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Change `EngineActor` to own `ShadowEngine` directly (not `Arc<RwLock<ShadowEngine>>`). The actor constructor takes ownership. | Critical | Engine |
| FR-2 | Remove `RwLock` wrapper on `ShadowOrderManager`. Change field to `orders: ShadowOrderManager`. Methods take `&mut self`. | Critical | Engine |
| FR-3 | Remove `RwLock` wrapper on `ShadowPositionManager`. Change field to `positions: ShadowPositionManager`. Methods take `&mut self`. | Critical | Engine |
| FR-4 | Remove `RwLock` wrapper on `OrderGroupManager`. Change field to `order_groups: OrderGroupManager`. Methods take `&mut self`. | Critical | Engine |
| FR-5 | Remove `shadow_engine: Arc<RwLock<ShadowEngine>>` from `AppState` and `main.rs`. The only engine access path is `EngineHandle`. | Critical | Router |
| FR-6 | Update `EngineActor::dispatch()` — replace lock acquisition (`.read().await`, `.write().await`) with direct method calls on owned fields. | Critical | Engine |

### In-Flight Zombie Detection

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-7 | Add `pending_placements: HashMap<Uuid, Instant>` to `EngineActor`. Keyed by `group_id`, valued at insertion time. | Critical | Engine |
| FR-8 | On `PlaceOrder` command: insert `(group_id, Instant::now())` into `pending_placements`. | Critical | Engine |
| FR-9 | On `RegisterExchangeOrderId` command: remove `group_id` from `pending_placements`. | Critical | Engine |
| FR-10 | Add a `tokio::time::interval(Duration::from_secs(15))` to the actor's `run()` loop via `tokio::select!`. On each tick, sweep `pending_placements` for entries older than 30 seconds. | Critical | Engine |
| FR-11 | For each stale entry: transition the group to `AwaitingReconciliation` status, emit a `TradeEvent::PlacementTimeout { group_id }` (preparation for 019f), and log `tracing::warn!`. | High | Engine |
| FR-12 | The `ReconciliationService` (018) picks up `AwaitingReconciliation` groups on its next 30s sweep and queries the exchange for truth. If the order exists on the exchange, register the IDs. If not, cancel the group. | High | Router |
| FR-13 | Test: place order via handle, do NOT send `RegisterExchangeOrderId`, wait 35s, verify group transitions to `AwaitingReconciliation`. | Critical | Test |
| FR-14 | Test: place order, send `RegisterExchangeOrderId` within 5s, verify group stays Active (not swept). | High | Test |
| FR-15 | Test: cancel_order race — `ProcessPriceUpdate` and `CancelOrder` sent concurrently, verify cancelled order is never filled. | Critical | Test |

---

## Technical Implementation

### 1) Lock Removal (FR-1 through FR-6)

**Before (019a-019d):**
```rust
pub struct EngineActor {
    engine: Arc<RwLock<ShadowEngine>>,
    rx: mpsc::Receiver<EngineCommand>,
    fill_event_tx: mpsc::Sender<FillEvent>,
}

// In dispatch:
EngineCommand::PlaceOrder { user_id, order, reply } => {
    let engine = self.engine.read().await;
    let result = engine.orders.write().await.place_order(user_id, order);
    let _ = reply.send(result);
}
```

**After:**
```rust
pub struct EngineActor {
    engine: ShadowEngine,  // OWNED directly
    rx: mpsc::Receiver<EngineCommand>,
    fill_event_tx: mpsc::Sender<FillEvent>,
    pending_placements: HashMap<Uuid, Instant>,
}

// In dispatch — no locks at all:
EngineCommand::PlaceOrder { user_id, order, reply } => {
    let group_id = order.group_id;
    let result = self.engine.orders.place_order(user_id, order);
    if result.is_ok() {
        self.pending_placements.insert(group_id, Instant::now());
    }
    let _ = reply.send(result);
}
```

**ShadowEngine inner fields:**

Before:
```rust
pub struct ShadowEngine {
    pub orders: RwLock<ShadowOrderManager>,
    pub positions: RwLock<ShadowPositionManager>,
    pub order_groups: RwLock<OrderGroupManager>,
    pub balances: ShadowBalanceManager,  // already DashMap, no lock
}
```

After:
```rust
pub struct ShadowEngine {
    pub orders: ShadowOrderManager,
    pub positions: ShadowPositionManager,
    pub order_groups: OrderGroupManager,
    pub balances: ShadowBalanceManager,
}
```

All methods on `ShadowOrderManager`, `ShadowPositionManager`, `OrderGroupManager` change from `&self` (behind RwLock) to `&mut self` (owned by actor).

### 2) Actor Run Loop with Timer (FR-10)

```rust
impl EngineActor {
    pub async fn run(mut self) {
        let mut sweep_interval = tokio::time::interval(Duration::from_secs(15));
        tracing::info!("EngineActor started (lock-free)");

        loop {
            tokio::select! {
                cmd = self.rx.recv() => {
                    match cmd {
                        Some(cmd) => self.dispatch(cmd),
                        None => break,  // all handles dropped
                    }
                }
                _ = sweep_interval.tick() => {
                    self.sweep_stale_placements();
                }
            }
        }

        tracing::info!("EngineActor shut down");
    }

    fn sweep_stale_placements(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(30);
        let stale: Vec<Uuid> = self.pending_placements.iter()
            .filter(|(_, ts)| **ts < cutoff)
            .map(|(id, _)| *id)
            .collect();

        for group_id in stale {
            self.pending_placements.remove(&group_id);
            tracing::warn!(group_id = %group_id, "In-flight placement timeout — marking for reconciliation");
            // Transition group status
            let _ = self.engine.order_groups.update_status(group_id, OrderGroupStatus::AwaitingReconciliation);
            // Emit event (if event_tx is wired — 019f)
        }
    }
}
```

### 3) AwaitingReconciliation Status (FR-11, FR-12)

Add `AwaitingReconciliation` to `OrderGroupStatus` enum. This is a transient status that means "the actor lost track of this order's exchange state." The `ReconciliationService` treats it like an active group that needs exchange verification.

---

## Files to Modify

| File | Changes |
|------|---------|
| `crates/engine/src/shadow/mod.rs` | FR-1: Remove `Arc<RwLock<...>>` on inner fields |
| `crates/engine/src/shadow/orders.rs` | FR-2: Remove `RwLock`, methods take `&mut self` |
| `crates/engine/src/shadow/positions.rs` | FR-3: Remove `RwLock`, methods take `&mut self` |
| `crates/engine/src/shadow/order_group.rs` | FR-4: Remove `RwLock`, methods take `&mut self`. Add `AwaitingReconciliation` to `OrderGroupStatus`. |
| `crates/engine/src/shadow/actor.rs` | FR-6: Remove lock calls from dispatch. FR-7-11: Add `pending_placements`, sweep timer. |
| `crates/router/src/main.rs` | FR-5: Remove `Arc<RwLock<ShadowEngine>>`. Pass owned engine to actor. |
| `crates/router/src/types/app.rs` | FR-5: Remove `shadow_engine` field from `AppState`. |
| `crates/router/src/services/reconciliation.rs` | FR-12: Handle `AwaitingReconciliation` status in sweep. |

---

## Verification

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

- [ ] `Arc<RwLock<ShadowEngine>>` does not appear anywhere in the codebase
- [ ] `RwLock<ShadowOrderManager>` does not appear anywhere
- [ ] `RwLock<ShadowPositionManager>` does not appear anywhere
- [ ] `RwLock<OrderGroupManager>` does not appear anywhere
- [ ] Actor owns `ShadowEngine` directly
- [ ] `pending_placements` tracked on `PlaceOrder`, cleared on `RegisterExchangeOrderId`
- [ ] 15s sweep detects 30s-stale placements
- [ ] Stale placements transition to `AwaitingReconciliation`
- [ ] `ReconciliationService` picks up `AwaitingReconciliation` groups
- [ ] Cancel/fill race condition test passes (cancelled order never filled)
- [ ] All existing tests pass (zero regression)

---

## Completion Signal

All verification checkboxes green. `cargo clippy --all-targets && cargo test` passes. Zero locks remain in ShadowEngine. In-flight zombie detection operational.
