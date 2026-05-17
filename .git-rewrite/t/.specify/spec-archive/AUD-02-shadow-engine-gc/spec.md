# Specification: Shadow Engine Garbage Collection

**Spec ID:** AUD-02-shadow-engine-gc
**Date:** 2026-03-07
**Status:** Complete
**Class:** Audit
**Phase:** 2 (Reliability)
**Audit Refs:** ML-1, ML-2, ML-3, ML-4, ML-8

---

## Overview

Add garbage collection to the Shadow Engine's in-memory collections that currently grow unboundedly. Every order, position, and order group placed since process start persists in memory forever, even after reaching terminal states (filled, cancelled, closed, stopped out).

**Current state:**
- `ShadowOrderManager.orders` HashMap: every order ever placed stays forever (~400B each)
- `ShadowPositionManager.positions` HashMap: every position ever opened stays forever
- `OrderGroupManager`: 5 HashMaps (`groups`, `groups_by_user`, `groups_by_entry_order`, `groups_by_linked_order`, `groups_by_exchange_order`) — terminal groups stay in all 5 forever (~500B+ each)
- `TradeManagerService.positions` and `last_amend`: closed positions and debounce timestamps never removed
- `OrderBook.user_orders`: stale order IDs from filled orders never cleaned (acknowledged in code comment)

**Target state:**
- Terminal entries are retained for a configurable TTL (default: 1 hour) then evicted
- A periodic GC task prunes all collections on a fixed interval
- Memory usage is bounded proportional to active state, not historical state

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add `prune_terminal(cutoff: Instant)` to `ShadowOrderManager` — remove orders with terminal status older than cutoff from `orders` and `orders_by_user` | Critical | Engine / Orders |
| FR-2 | Add `prune_terminal(cutoff: Instant)` to `ShadowPositionManager` — remove closed positions older than cutoff from `positions` and `positions_by_user` | Critical | Engine / Positions |
| FR-3 | Add `prune_terminal(cutoff: Instant)` to `OrderGroupManager` — remove terminal groups from all 5 maps | Critical | Engine / OrderGroups |
| FR-4 | Add `completed_at: Option<Instant>` field to `ShadowOrder`, `ShadowPosition`, and `OrderGroup` — set when transitioning to terminal state | High | Engine |
| FR-5 | Add periodic GC task in `main.rs` that calls all three `prune_terminal()` methods every 5 minutes with a 1-hour cutoff | High | Router / Main |
| FR-6 | Add `prune_closed()` to `TradeManagerService` — remove closed positions from `positions` map and stale entries from `last_amend` | High | Router / Trade Manager |
| FR-7 | Fix `OrderBook.user_orders` cleanup — remove filled order IDs using `other_user_id` from Fill struct | Medium | Engine / Orderbook |
| FR-8 | Add tests verifying terminal entries are pruned after cutoff | High | Test |
| FR-9 | Add test verifying active entries are NOT pruned | High | Test |

---

## Technical Implementation

### 1) Terminal Timestamp (FR-4)

Add `completed_at` to each struct, set on status transition:

```rust
// In ShadowOrder
pub completed_at: Option<Instant>,

// In cancel_order / apply_fills
stored_order.status = ShadowOrderStatus::Cancelled;
stored_order.completed_at = Some(Instant::now());
```

### 2) Prune Methods (FR-1, FR-2, FR-3)

```rust
// ShadowOrderManager
pub fn prune_terminal(&mut self, cutoff: Instant) -> usize {
    let to_remove: Vec<Uuid> = self.orders.iter()
        .filter(|(_, o)| !o.status.is_open() && o.completed_at.map_or(false, |t| t < cutoff))
        .map(|(id, _)| *id)
        .collect();

    for id in &to_remove {
        self.orders.remove(id);
    }

    // Clean orders_by_user
    for ids in self.orders_by_user.values_mut() {
        ids.retain(|id| !to_remove.contains(id));
    }

    to_remove.len()
}
```

Same pattern for `ShadowPositionManager` and `OrderGroupManager` (the latter must clean all 5 maps).

### 3) GC Background Task (FR-5)

```rust
// In main.rs, after service initialization
let engine_gc = engine.clone();
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 min
    loop {
        interval.tick().await;
        let cutoff = Instant::now() - Duration::from_secs(3600); // 1 hour
        let engine = engine_gc.read().await;
        let mut orders = engine.orders.write().await;
        let mut positions = engine.positions.write().await;
        let mut groups = engine.order_groups.write().await;
        let pruned = orders.prune_terminal(cutoff)
            + positions.prune_terminal(cutoff)
            + groups.prune_terminal(cutoff);
        if pruned > 0 {
            log::info!("GC: pruned {} terminal entries", pruned);
        }
    }
});
```

### 4) OrderBook user_orders Fix (FR-7)

In `match_asks` and `match_bids`, after removing from `order_locations`, also remove from `user_orders`:

```rust
for fill in &fills {
    self.order_locations.remove(&fill.order_id);
    // Fill contains other_user_id — use it to clean user_orders
    if let Some(ids) = self.user_orders.get_mut(&fill.other_user_id) {
        ids.remove(&fill.order_id);
    }
}
```

---

## Verification

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

- [ ] Terminal orders pruned after 1-hour cutoff
- [ ] Terminal positions pruned after 1-hour cutoff
- [ ] Terminal order groups pruned from all 5 maps
- [ ] Active/pending entries not affected by GC
- [ ] TradeManagerService closed positions cleaned
- [ ] OrderBook user_orders cleaned on fills
- [ ] GC task logs pruned count
- [ ] All existing tests still pass
