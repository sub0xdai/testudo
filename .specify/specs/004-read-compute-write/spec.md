# Feature: Read-Compute-Write Lock Optimization

> Spec ID: 004-read-compute-write
> Created: 2026-01-20
> Status: Ready
> Priority: P1 (Performance)

---

## Overview

The `process_price_update` function in Shadow Engine acquires 4 sequential write locks (orders, balances, positions, order_groups) and holds them for the entire processing loop. This creates a "stop-the-world" event for every price tick, blocking concurrent read operations. This spec refactors to a Read-Compute-Write pattern that minimizes write-lock duration.

---

## User Stories

- [x] As a trader, I want responsive order book updates during high volatility so that I can react to market conditions.
- [x] As a system operator, I want consistent <16ms frame times so that the UI doesn't lag during price spikes.

---

## Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | Refactor `process_price_update` to use Read-Compute-Write pattern | High |
| FR-2 | Read phase: Acquire read locks, collect triggered orders | High |
| FR-3 | Compute phase: Calculate fills in memory without locks | High |
| FR-4 | Write phase: Acquire write locks only for affected data, apply changes | High |
| FR-5 | Write locks must be held for minimum duration (no loop iterations) | High |

---

## Acceptance Criteria

- [ ] `process_price_update` acquires read locks first, releases before write phase
- [ ] Write locks are acquired only once per price update (not per order)
- [ ] All existing tests pass
- [ ] No race conditions introduced (orders not double-filled)
- [ ] `cargo clippy --all-targets` passes
- [ ] `cargo test` passes

---

## Technical Notes

### Files to Modify

- `testudo-exchange/crates/engine/src/shadow/mod.rs` - Lines 234-338

### Current Pattern (Anti-pattern)

```rust
// Current: Holds all locks across entire loop
let mut orders = self.orders.write().await;      // Lock 1
let mut balances = self.balances.write().await;  // Lock 2
let mut positions = self.positions.write().await; // Lock 3
let mut order_groups = self.order_groups.write().await; // Lock 4

for order in triggered_orders {
    // Process order while holding ALL locks
    // Other tasks blocked for entire duration
}
```

### Target Pattern (Read-Compute-Write)

```rust
// Phase 1: READ - Identify work
let triggered_orders = {
    let orders = self.orders.read().await;
    orders.get_triggered_orders(symbol, bid, ask)
    // Read lock released here
};

// Phase 2: COMPUTE - Calculate fills in memory
let fills: Vec<FillOperation> = triggered_orders
    .iter()
    .map(|o| compute_fill(o, bid, ask))
    .collect();

// Phase 3: WRITE - Apply changes atomically
{
    let mut orders = self.orders.write().await;
    let mut balances = self.balances.write().await;
    let mut positions = self.positions.write().await;
    let mut order_groups = self.order_groups.write().await;

    for fill in fills {
        apply_fill(&mut orders, &mut balances, &mut positions, &mut order_groups, fill);
    }
    // All locks released together after batch apply
}
```

### Dependencies

- None - internal refactor

### Assumptions

- Order state doesn't change between read and write phases (acceptable race condition window)
- Batch apply is atomic enough for our use case

---

## Completion Signal

### Implementation Checklist
- [ ] All functional requirements implemented
- [ ] All acceptance criteria verified
- [ ] Code follows project constitution standards
- [ ] No new linting warnings introduced

### Testing Requirements
- [ ] `cd testudo-exchange && cargo clippy --all-targets && cargo test` passes
- [ ] Existing `process_price_update` tests pass
- [ ] Manual load test: rapid price updates don't cause lock contention

### Quality Verification
- [ ] No deadlocks under concurrent price updates
- [ ] Fills are correctly calculated and applied

### Iteration Protocol
If any check fails:
1. Identify the issue from error output
2. Fix the code
3. Commit the fix
4. Re-run verification
5. Repeat until ALL checks pass

### Done Signal
When ALL above criteria are satisfied, output:
```
<promise>DONE</promise>
```

---

## Clarifications Needed

None - pattern is well-defined.

---

*Template version: 1.0*
