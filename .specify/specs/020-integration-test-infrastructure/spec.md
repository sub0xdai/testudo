# Specification: Integration Test Infrastructure

**Spec ID:** 020-integration-test-infrastructure
**Date:** 2026-03-13
**Status:** Complete
**Class:** Test Infrastructure
**Origin:** Gap analysis — 699 unit tests, zero cross-service integration tests
**Depends on:** 018-order-reconciliation, 019d-actor-service-migration

---

## Overview

Testudo's trade lifecycle spans multiple services: FillDetectorService processes exchange events via mpsc channels, ReconciliationService polls for orphaned orders, and the cancel_trade route orchestrates multi-step cleanup. Each service has unit tests, but no test exercises the boundaries between them.

**Current state:**
- FillDetector has 9 unit tests calling `handle_order_update()` directly — bypassing the `run()` loop and its `tokio::select!` over two channels.
- ReconciliationService has **zero tests** — its decision matrix (lines 207-331 of `reconciliation.rs`) is tightly coupled to `CcxtClient` and `ExchangeAccountRepository`, both requiring network/DB.
- The existing `MockExchangeApi` in `fill_detector.rs` tracks cancel calls but does not simulate exchange state (open orders, positions). It cannot answer "does this order still exist?" — which ReconciliationService needs.

**Target state:**
- A `StatefulMockExchangeApi` that maintains realistic exchange state (open orders, positions) with inspection methods for assertions.
- Reconciliation decision logic extracted into a pure function testable without network/DB dependencies.
- Shared test helpers for common setup patterns (actor spawn, group creation, exchange ID registration).

---

## Constraint: No Production Code Changes (Except Reconciliation Extraction)

The only production code change is extracting the reconciliation decision matrix into a testable function. All other additions are `#[cfg(test)]` only.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Create `StatefulMockExchangeApi` implementing `ExchangeApi` trait with in-memory exchange state: open orders (`HashMap<String, OrderInfo>`), positions (`HashMap<String, PositionInfo>`), and audit logs for cancel/place/cancel_all calls. | Critical | Test |
| FR-2 | `place_order` stores the order in `open_orders` and returns a predictable exchange order ID (monotonic counter). `cancel_order` removes from `open_orders` and logs the ID; returns `OrderNotFound` if not present (idempotent). `cancel_all_orders` removes all orders for the given symbol and logs. | Critical | Test |
| FR-3 | Add inspection methods: `cancelled_ids() -> Vec<String>`, `has_open_order(id) -> bool`, `open_order_count() -> usize`, `placed_ids() -> Vec<String>`, `cancel_all_symbols() -> Vec<String>`. | High | Test |
| FR-4 | Add state injection methods: `inject_position(symbol, side, quantity, entry_price)` to simulate an existing exchange position, `remove_position(symbol)` to simulate position closure. | High | Test |
| FR-5 | Extract reconciliation decision logic from `ReconciliationService::reconcile_account()` (lines 178-349) into a `pub(crate)` function: `fn determine_reconcile_actions(groups: &[OrderGroup], open_order_ids: &HashSet<String>, symbols_with_position: &HashSet<String>, open_orders: &[OpenOrderInfo]) -> Vec<ReconcileAction>`. Make `ReconcileAction` `pub(crate)`. | Critical | Reconciliation |
| FR-6 | The existing `reconcile_account()` must call the extracted function internally — zero behavior change. | Critical | Reconciliation |
| FR-7 | Create shared test helpers: `setup_test_actor() -> (EngineHandle, Receiver<FillEvent>, Receiver<TradeEvent>)`, `create_active_group(handle, user_id, symbol, entry_id, sl_id, tp_id) -> Uuid`, `create_pending_group(handle, user_id, symbol, entry_id, sl_id, tp_id) -> Uuid`. | High | Test |
| FR-8 | Register the integration test module in `services/mod.rs`: `#[cfg(test)] mod integration_tests;`. | Critical | Test |

---

## Technical Implementation

### 1) StatefulMockExchangeApi (FR-1, FR-2, FR-3, FR-4)

**File:** `crates/router/src/services/integration_tests.rs` (new, `#[cfg(test)]` only)

```rust
struct StatefulMockExchangeApi {
    open_orders: tokio::sync::Mutex<HashMap<String, PlacedOrder>>,
    positions: tokio::sync::Mutex<HashMap<String, PositionInfo>>,
    cancel_log: tokio::sync::Mutex<Vec<String>>,
    cancel_all_log: tokio::sync::Mutex<Vec<String>>,
    place_log: tokio::sync::Mutex<Vec<PlaceOrderRequest>>,
    next_id: AtomicUsize,
}

struct PlacedOrder {
    id: String,
    symbol: String,
    side: OrderSide,
    order_type: ApiOrderType,
    quantity: Decimal,
    price: Option<Decimal>,
}
```

Key behaviors:
- `cancel_order`: removes from `open_orders`, appends to `cancel_log`. If not found, returns `ExchangeApiError::OrderNotFound`.
- `cancel_all_orders`: removes all matching symbol from `open_orders`, appends symbol to `cancel_all_log`.
- `get_position`: returns from `positions` map, `None` if absent.
- All operations are `async` behind `tokio::sync::Mutex` (no blocking in async context).

### 2) Reconciliation Extraction (FR-5, FR-6)

**File:** `crates/router/src/services/reconciliation.rs`

Extract the for-loop body (lines 181-349) into:

```rust
pub(crate) fn determine_reconcile_actions(
    groups: &[OrderGroup],
    open_order_ids: &HashSet<String>,
    symbols_with_position: &HashSet<String>,
    open_orders: &[OpenOrderInfo],
) -> Vec<ReconcileAction> { ... }
```

The async EngineHandle re-query (lines 182-189) cannot be in the pure function. Instead, `reconcile_account` will:
1. Fetch exchange state (open_order_ids, symbols_with_position, open_orders)
2. Re-query each group via handle (for freshness)
3. Pass the re-queried groups + exchange state to `determine_reconcile_actions()`
4. Execute the returned actions

`ReconcileAction` becomes `pub(crate)` so integration tests can inspect it.

### 3) Shared Test Helpers (FR-7)

**File:** `crates/router/src/services/integration_tests.rs`

```rust
async fn setup_test_actor() -> (EngineHandle, mpsc::Receiver<FillEvent>, mpsc::Receiver<TradeEvent>) {
    let engine = ShadowEngine::new();
    EngineActor::spawn(engine)
}

async fn create_active_group(
    handle: &EngineHandle,
    user_id: Uuid,
    symbol: &str,
    entry_id: &str,
    sl_id: &str,
    tp_id: &str,
) -> Uuid {
    // Uses the same pattern as fill_detector::tests::setup_with_group
    // but via ShadowEngine pre-population before actor spawn
    // Returns group_id
}
```

These follow the existing pattern from `fill_detector.rs:577-610` (the `setup_with_group` helper) but are reusable across integration tests.

### 4) Module Registration (FR-8)

**File:** `crates/router/src/services/mod.rs`

Add at the end:
```rust
#[cfg(test)]
mod integration_tests;
```

---

## Verification

```bash
# Reconciliation extraction doesn't break existing behavior
cd testudo-exchange && cargo test -- reconciliation
cd testudo-exchange && cargo clippy --all-targets

# Integration test module compiles
cd testudo-exchange && cargo test integration_tests -- --nocapture

# Full suite still passes
cd testudo-exchange && cargo test
```

---

## Dependencies

- **018-order-reconciliation** (Complete): ReconciliationService exists with decision matrix
- **019d-actor-service-migration** (Complete): EngineHandle pattern used for all state access

## Blocked by

None.

## Blocks

- 021-lifecycle-integration-tests
