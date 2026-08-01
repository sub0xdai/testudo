# Specification: Actor Service Migration

**Spec ID:** 019d-actor-service-migration
**Date:** 2026-03-12
**Status:** Complete
**Class:** Architecture
**Parent:** 019-actor-model-event-sourcing
**Phase:** 4 of 6
**Depends on:** 019c-actor-route-migration

---

## Overview

Migrate all background services from `Arc<RwLock<ShadowEngine>>` to `EngineHandle`. This phase includes the `ProcessPriceUpdate` fire-and-forget optimization — the PriceFeedService pushes prices without awaiting a response, and fills are emitted as downstream events rather than returned via oneshot.

**Current state:**
- `PriceFeedService` acquires read lock, calls `process_price_update` with 3-phase RCW pattern, handles fills inline.
- `FillDetectorService` accesses `Arc<RwLock<OrderGroupManager>>` directly.
- `RehydrationService` acquires write lock to bulk-load order groups at startup.
- `ReconciliationService` acquires read lock to sweep active groups.
- GC task acquires write lock to prune terminal entries.

**Target state:**
- All services use `EngineHandle`.
- `PriceFeedService` sends fire-and-forget price updates (no oneshot response).
- Fills triggered by price updates are emitted as events to a separate channel (preparation for 019f event log).
- `Arc<RwLock<ShadowEngine>>` has ZERO remaining consumers (all migrated).

---

## Constraint: Fire-and-Forget Price Updates

`ProcessPriceUpdate` is the highest-frequency command. The PriceFeedService currently polls every 2s, but this could increase. Generating and returning oneshot channels per tick adds overhead for a caller that doesn't use the response. The caller pushes prices; the system reacts to fills downstream.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add a `ProcessPriceUpdateFireAndForget` command variant to `EngineCommand` that carries no `reply` channel. The actor processes price updates and emits any fills to an internal `fill_event_tx: mpsc::Sender<FillEvent>` channel instead of returning them. | Critical | Engine |
| FR-2 | Add `EngineHandle::push_price(symbol, bid, ask, high, low)` method that sends `ProcessPriceUpdateFireAndForget` — awaits only for channel backpressure, not for processing. | Critical | Engine |
| FR-3 | Migrate `PriceFeedService` to use `engine_handle.push_price()`. Remove direct lock access. | Critical | Router |
| FR-4 | Migrate `FillDetectorService` to use `EngineHandle`. Replace `Arc<RwLock<OrderGroupManager>>` with handle calls. | Critical | Router |
| FR-5 | Migrate `RehydrationService` to use `EngineHandle`. Use `LoadOrderGroups` command for bulk loading. Rehydration must complete before HTTP server starts (blocking startup). | High | Router |
| FR-6 | Migrate `ReconciliationService` to use `EngineHandle`. Replace `Arc<RwLock<OrderGroupManager>>` read access with handle calls. | High | Router |
| FR-7 | Migrate GC task to use `EngineHandle`. Use `PruneTerminal` command. | High | Router |
| FR-8 | Verify: no remaining references to `shadow_engine.read().await` or `shadow_engine.write().await` across the entire router crate. | Critical | Router |
| FR-9 | Test: price update via fire-and-forget triggers fill event on the fill channel when a limit order matches. | Critical | Test |
| FR-10 | Test: rehydration via `EngineHandle` populates actor state before HTTP server starts. | High | Test |

---

## Technical Implementation

### 1) Fire-and-Forget Price Update (FR-1, FR-2)

Add to `EngineCommand`:
```rust
// Fire-and-forget variant — no reply channel
ProcessPriceUpdateFireAndForget {
    symbol: String,
    bid: Decimal,
    ask: Decimal,
    high: Decimal,
    low: Decimal,
},
```

Add to `EngineActor`:
```rust
pub struct EngineActor {
    engine: Arc<RwLock<ShadowEngine>>,
    rx: mpsc::Receiver<EngineCommand>,
    fill_event_tx: mpsc::Sender<FillEvent>,  // NEW — downstream fill events
}
```

In the actor's dispatch:
```rust
EngineCommand::ProcessPriceUpdateFireAndForget { symbol, bid, ask, high, low } => {
    let result = /* call engine price update */;
    // Emit fills to downstream channel (FillDetector subscribes to this)
    for fill in result.filled {
        let _ = self.fill_event_tx.try_send(FillEvent::from(fill));
    }
    // No reply — fire-and-forget
}
```

Add to `EngineHandle`:
```rust
pub async fn push_price(&self, symbol: String, bid: Decimal, ask: Decimal,
                         high: Decimal, low: Decimal) -> Result<(), EngineError> {
    self.tx.send(EngineCommand::ProcessPriceUpdateFireAndForget {
        symbol, bid, ask, high, low
    }).await.map_err(|_| EngineError::ActorShutdown)
}
```

### 2) FillDetector Rewiring (FR-4)

Currently FillDetector receives fill events from WsSubscriptionManager via an mpsc channel. With fire-and-forget price updates, it ALSO receives fill events from the actor's `fill_event_tx`. These are different channels serving different fill sources:

- **WsSubscriptionManager channel**: fills detected via WebSocket (exchange → sidecar → WS manager)
- **Actor fill_event_tx**: fills triggered by price updates (shadow engine matching)

FillDetector should listen to both. Use `tokio::select!` to receive from either channel.

### 3) Rehydration Blocking (FR-5)

Rehydration must complete before the HTTP server accepts connections. Use the `EngineHandle` with a guaranteed response:

```rust
// In main.rs, before HttpServer::new():
let groups = load_groups_from_db(&pool).await?;
engine_handle.load_order_groups(groups).await;
// Now the actor has state — safe to start HTTP server
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `crates/engine/src/shadow/actor.rs` | FR-1: Add `ProcessPriceUpdateFireAndForget` variant, `fill_event_tx` field |
| `crates/router/src/main.rs` | FR-5: Rehydration via handle before HTTP start. Create fill_event channel. |
| `crates/router/src/services/price_feed.rs` | FR-3: Use `engine_handle.push_price()` |
| `crates/router/src/services/fill_detector.rs` | FR-4: Use `EngineHandle`, listen to actor fill channel |
| `crates/router/src/services/rehydration.rs` | FR-5: Use `EngineHandle`, `LoadOrderGroups` command |
| `crates/router/src/services/reconciliation.rs` | FR-6: Use `EngineHandle` |
| `crates/router/src/services/trade_manager/service.rs` | FR-7: GC task uses `EngineHandle` |

---

## Verification

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

- [ ] `PriceFeedService` uses fire-and-forget `push_price()` — no oneshot
- [ ] `FillDetectorService` receives fills from actor channel
- [ ] `RehydrationService` uses `EngineHandle`, blocks startup until complete
- [ ] `ReconciliationService` uses `EngineHandle`
- [ ] GC task uses `EngineHandle`
- [ ] `grep -r "shadow_engine.read()" crates/router/` returns zero results
- [ ] `grep -r "shadow_engine.write()" crates/router/` returns zero results
- [ ] Fire-and-forget price update triggers fill event test passes
- [ ] All existing tests pass (zero regression)

---

## Completion Signal

All verification checkboxes green. `cargo clippy --all-targets && cargo test` passes. No references to `shadow_engine.read()` or `shadow_engine.write()` remain in the router crate.
