# Feature: Atomic Cascade Operations for SL/TP

> Spec ID: 005-atomic-cascades
> Created: 2026-01-20
> Status: Complete
> Priority: P1 (Data Integrity)

---

## Overview

When creating linked orders (Entry + Stop Loss + Take Profit), `orders.add_order()` may succeed but `order_groups.register_linked_order()` may fail, leaving orphan orders without their protective stops. This spec implements atomic transaction semantics for cascade operations.

---

## User Stories

- [x] As a trader, I want my SL/TP orders to always be created together with my entry so that I'm never exposed without protection.
- [x] As a system operator, I want no orphan orders in the system so that position tracking is accurate.

---

## Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | Create `TransactionContext` struct for pending changes | High |
| FR-2 | Add orders to transaction context, not directly to managers | High |
| FR-3 | Commit only if ALL cascade operations succeed | High |
| FR-4 | Rollback on any failure (no partial state) | High |
| FR-5 | Apply transaction atomically to all managers | High |

---

## Acceptance Criteria

- [ ] `TransactionContext` struct exists with `add_order()`, `register_group()`, `commit()` methods
- [ ] Entry + SL + TP are created atomically (all or none)
- [ ] Failed group registration rolls back the entry order
- [ ] `cargo clippy --all-targets` passes
- [ ] `cargo test` passes

---

## Technical Notes

### Files to Modify

- `testudo-exchange/crates/engine/src/shadow/mod.rs` - Add TransactionContext
- `testudo-exchange/crates/engine/src/shadow/orders.rs` - Support transactional add
- `testudo-exchange/crates/engine/src/shadow/order_group.rs` - Support transactional register

### Proposed Implementation

```rust
pub struct TransactionContext {
    pending_orders: Vec<ShadowOrder>,
    pending_groups: Vec<OrderGroupRegistration>,
    pending_positions: Vec<PositionUpdate>,
}

impl TransactionContext {
    pub fn new() -> Self { ... }

    pub fn add_order(&mut self, order: ShadowOrder) {
        self.pending_orders.push(order);
    }

    pub fn register_group(&mut self, group: OrderGroupRegistration) {
        self.pending_groups.push(group);
    }

    pub fn commit(
        self,
        orders: &mut ShadowOrderManager,
        groups: &mut OrderGroupManager,
        positions: &mut ShadowPositionManager,
    ) -> Result<(), TransactionError> {
        // Validate all operations can succeed
        // Apply all changes
        // Return error if any fail (no partial state)
    }
}
```

### Usage Pattern

```rust
// Instead of:
orders.add_order(entry)?;
order_groups.register_linked_order(entry_id, sl_id, tp_id)?; // Can fail!

// Do:
let mut tx = TransactionContext::new();
tx.add_order(entry);
tx.add_order(sl);
tx.add_order(tp);
tx.register_group(OrderGroupRegistration { entry_id, sl_id, tp_id });
tx.commit(&mut orders, &mut groups, &mut positions)?; // All or nothing
```

### Dependencies

- None - internal refactor

### Assumptions

- Validation can be done before commit
- Order IDs are deterministic (can be pre-generated)

---

## Completion Signal

### Implementation Checklist
- [ ] All functional requirements implemented
- [ ] All acceptance criteria verified
- [ ] Code follows project constitution standards
- [ ] No new linting warnings introduced

### Testing Requirements
- [ ] `cd testudo-exchange && cargo clippy --all-targets && cargo test` passes
- [ ] Unit test: successful cascade creates all orders
- [ ] Unit test: failed cascade creates no orders

### Quality Verification
- [ ] No orphan orders after failed operations
- [ ] Group membership is always consistent

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

None.

---

*Template version: 1.0*
