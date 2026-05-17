# Specification: Add Assertion Discipline to Engine and Orderbook

**Spec ID:** CLN-09-assertion-discipline
**Date:** 2026-05-15
**Status:** Draft
**Class:** Safety / Core
**Priority:** P1 — zero assertions in 489-line engine is a critical safety gap for financial software
**Depends on:** CLN-01, CLN-02, CLN-03, CLN-04 (builds on typed errors and unwrap removal)
**Series:** CLN-01 through CLN-09 (Phase 1 — Open-Source Readiness Cleanup)

---

## Problem Statement

The TigerBeetle comparison audit found:

> *"Assertion density is low. `engine.rs` has exactly 0 assertions in 489 lines. The core matching engine has no invariant checks."*

In a financial matching engine, assertion density is safety density. Every invariant that's checked at runtime is a bug caught before it becomes a production incident. TigerBeetle mandates **≥2 assertions per function** — while Testudo may not need that extreme density, the current state of **zero assertions in the engine** is unacceptable for a codebase that handles real money.

Key invariants that should be asserted:
1. **Balance non-negativity:** After every debit/credit, `balance >= 0`. A negative balance is a financial bug.
2. **Order book price ordering:** After every insert, `bids[0].price >= bids[1].price` (descending). Asks ascending.
3. **Trade ID monotonicity:** After every fill, `fill.trade_id > previous_trade_id`.
4. **Fill quantity consistency:** `fill.quantity == min(order.remaining, matching_order.remaining)`.
5. **User balance consistency:** After a fill, `total_reserved + total_free == previous_total` (conservation of funds).
6. **Order state transitions:** An order can only transition from `Open → PartiallyFilled → Filled` or `Open → Cancelled`. Filled orders cannot be cancelled.

The orderbook logic (integrated into `engine.rs` rather than a separate file) and the shadow engine `shadow/orders.rs` both need similar treatment.

Assertions are **debug-only** (`debug_assert!`) by Rust convention for performance — but for a financial engine, `assert!` (always-on) is preferred for critical invariants where the cost of missing a bug exceeds the cost of the check.

---

## User Stories

- **As an auditor**, I want to see invariant checks at every balance mutation point, so that I can verify the engine maintains financial integrity.
- **As a developer**, I want `assert!` failures to catch logic bugs in tests and staging before they reach production.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add `assert!` for balance non-negativity after every `reserve()`, `release()`, `debit()`, `credit()` | High | engine.rs, shadow/balances.rs |
| FR-2 | Add `assert!` for order book price ordering after every `insert()` and `remove()` | High | engine.rs (orderbook logic) |
| FR-3 | Add `assert!` for trade ID monotonicity after each fill | High | engine.rs |
| FR-4 | Add `debug_assert!` for fill quantity consistency (Q = min(remaining, matching.remaining)) | Medium | engine.rs |
| FR-5 | Add `assert!` for user balance conservation (reserved + free = constant across fills) | Medium | engine.rs, shadow/balances.rs |
| FR-6 | Add `assert!` for valid order state transitions in order lifecycle | Medium | shadow/orders.rs |
| FR-7 | Minimum 1 assertion per public function in `engine.rs` and orderbook code | Medium | engine.rs |
| FR-8 | All assertions pass (no existing behavior breaks) — `cargo test` green | High | All |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Add balance non-negativity assertions to engine.rs + shadow/balances.rs | Assertions fire on negative balance, tests pass |
| CP-2 | Add order book ordering assertions | B-tree ordering invariant verified, tests pass |
| CP-3 | Add trade ID monotonicity + fill consistency assertions | Fill invariants checked, tests pass |
| CP-4 | Add state transition assertions to shadow/orders.rs | Invalid transitions caught, tests pass |
| CP-5 | Full `cargo test` — all assertions hold (no panics from assertions) | No lurking invariant violations |

### Assertion Patterns

#### 1. Balance Non-Negativity

**Location:** `engine.rs` — wherever `check_and_lock_funds()` modifies balances, and `shadow/balances.rs` in `BalanceManager::reserve()` / `release()`.

```rust
// After every balance mutation:
assert!(
    balance.free >= Decimal::ZERO,
    "Balance invariant violated: free balance for user {user_id} in {asset} is negative: {free}",
    user_id = user_id,
    asset = asset,
    free = balance.free,
);
assert!(
    balance.reserved >= Decimal::ZERO,
    "Balance invariant violated: reserved balance for user {user_id} in {asset} is negative: {reserved}",
    user_id = user_id,
    asset = asset,
    reserved = balance.reserved,
);
```

#### 2. Order Book Price Ordering

**Location:** `engine.rs` — after inserting into the orderbook (bids are price-descending, asks price-ascending).

```rust
// After inserting a bid:
debug_assert!(
    orderbook.bids.windows(2).all(|w| w[0].price >= w[1].price),
    "Bid ordering invariant violated: prices must be descending"
);

// After inserting an ask:
debug_assert!(
    orderbook.asks.windows(2).all(|w| w[0].price <= w[1].price),
    "Ask ordering invariant violated: prices must be ascending"
);
```

**Note:** `BTreeMap` naturally maintains ordering, but explicit asserts catch bugs in custom comparators or manual tree manipulation.

#### 3. Trade ID Monotonicity

**Location:** `engine.rs` — in the fill/match logic.

```rust
let new_trade_id = self.next_trade_id;
self.next_trade_id += 1;

assert!(
    new_trade_id > self.last_fill_trade_id,
    "Trade ID not monotonic: new={new_trade_id}, previous={previous}",
    new_trade_id = new_trade_id,
    previous = self.last_fill_trade_id,
);
self.last_fill_trade_id = new_trade_id;
```

#### 4. Fill Quantity Consistency

```rust
// In match_asks / match_bids:
let fill_qty = std::cmp::min(order.remaining_qty, matching_order.remaining_qty);

assert!(
    fill_qty > Decimal::ZERO,
    "Fill quantity must be positive: got {fill_qty}",
    fill_qty = fill_qty,
);
assert!(
    fill_qty <= order.remaining_qty && fill_qty <= matching_order.remaining_qty,
    "Fill quantity {fill_qty} exceeds remaining qty: order={order_remaining}, match={match_remaining}",
    fill_qty = fill_qty,
    order_remaining = order.remaining_qty,
    match_remaining = matching_order.remaining_qty,
);
```

#### 5. Balance Conservation (After Fill)

```rust
// After processing a fill:
let total_after = balance.free + balance.reserved;
assert_eq!(
    total_after, total_before,
    "Balance conservation violated: total before={before}, after={after}",
    before = total_before,
    after = total_after,
);
```

#### 6. Order State Transitions

**Location:** `shadow/orders.rs` — in `ShadowOrder::transition_to()` or equivalent.

```rust
fn transition_to(&mut self, new_state: OrderState) {
    assert!(
        self.state.can_transition_to(new_state),
        "Invalid order state transition: {:?} -> {:?} for order {}",
        self.state, new_state, self.id,
    );
    self.state = new_state;
}
```

With a helper:
```rust
impl OrderState {
    fn can_transition_to(&self, next: OrderState) -> bool {
        matches!(
            (self, next),
            (Open, PartiallyFilled) |
            (Open, Cancelled) |
            (PartiallyFilled, Filled) |
            (PartiallyFilled, Cancelled)
        )
    }
}
```

### Assertion vs. debug_assert Policy

| Assertion type | Use for |
|----------------|---------|
| `assert!` | Financial invariants: balance non-negativity, conservation, state transitions. These MUST hold in production. |
| `debug_assert!` | Performance-sensitive invariants: price ordering (already enforced by BTreeMap), fill quantity consistency (self-verifying). |

### Paved Roads

- Rust standard library: `assert!`, `debug_assert!`, `assert_eq!`
- TigerBeetle engineering principles: assertion density ≥2 per function
- Constitution: *"All code must pass linting and tests before commit. No shortcuts that compromise security or reliability."*

### Files

- `testudo-exchange/crates/engine/src/engine/engine.rs` — balance, orderbook, trade ID, fill consistency assertions
- `testudo-exchange/crates/engine/src/shadow/balances.rs` — balance non-negativity assertions
- `testudo-exchange/crates/engine/src/shadow/orders.rs` — state transition assertions

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `engine.rs` has ≥10 assertions (minimum 1 per public function)
- [ ] Balance non-negativity asserted after every reserve/release/debit/credit in engine
- [ ] Order book price ordering asserted after every insert
- [ ] Trade ID monotonicity asserted after every fill
- [ ] Fill quantity consistency asserted
- [ ] Balance conservation asserted across fill operations
- [ ] Order state transitions validated in shadow engine
- [ ] `cargo test` passes — zero assertion failures
- [ ] `cargo clippy --all-targets` passes

---

## Risks

1. **Assertions uncover existing bugs.** If an assertion fires during `cargo test`, it means the invariant was already violated — the engine has a latent bug. Mitigation: fix the bug, don't weaken the assertion. This is the whole point of CLN-09.
2. **Performance impact.** `assert!` in hot paths adds a branch. Mitigation: use `debug_assert!` for ordering/consistency checks in the fill loop (the most critical path); use always-on `assert!` for post-mutation balance checks (already outside the inner loop).
3. **Over-assertion.** Asserting obvious things (e.g., `assert!(true)`) dilutes the value. Mitigation: every assertion should check a non-trivial invariant that could plausibly be violated by a future code change.

---

## Completion Signal

This spec is complete when:
1. `engine.rs` has ≥10 meaningful assertions
2. `shadow/balances.rs` and `shadow/orders.rs` have state/balance assertions
3. All assertions pass in `cargo test`
4. Code committed to master
