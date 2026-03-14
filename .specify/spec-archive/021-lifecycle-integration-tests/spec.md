# Specification: Lifecycle Integration Tests

**Spec ID:** 021-lifecycle-integration-tests
**Date:** 2026-03-13
**Status:** Complete
**Class:** Test
**Origin:** Gap analysis — zero cross-service integration tests for trade lifecycle
**Depends on:** 020-integration-test-infrastructure

---

## Overview

The Testudo trade lifecycle spans three service boundaries: FillDetectorService (event-driven OCO), ReconciliationService (polling safety net), and cancel_trade route (user-initiated cleanup). Each has unit tests in isolation, but no test verifies they work correctly when wired together through actual channels and concurrent operations.

**Current state:**
- FillDetector unit tests call `handle_order_update()` directly, bypassing the `run()` loop's `tokio::select!` over dual channels (order_rx + fill_rx).
- No test exercises the entry-cancelled → SL/TP cleanup path through channels.
- No test exercises shadow engine fills triggering exchange cancellation via the fill_rx channel.
- ReconciliationService decision matrix is untested (zero test coverage).
- No test verifies idempotent convergence under concurrent fill + cancel operations.

**Target state:**
- 7 integration tests covering every critical seam in the trade lifecycle.
- FillDetector tested through its actual `run()` loop with mpsc channel delivery.
- Reconciliation decisions tested via the extracted `determine_reconcile_actions()` function from spec 020.
- Concurrent operation safety verified under race conditions.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | **SL fill via channel**: Send SL fill event through `order_tx` mpsc channel to FillDetector `run()` loop. Assert TP is cancelled on `StatefulMockExchangeApi`, group status is `StoppedOut`. | Critical | FillDetector |
| FR-2 | **Entry cancelled via channel**: Send entry cancelled event through `order_tx`. Assert SL and TP are both cancelled on mock, group status is `Cancelled`. | Critical | FillDetector |
| FR-3 | **Shadow fill cancels exchange order**: Push price update via EngineHandle that fills SL in shadow engine. FillDetector receives `FillEvent` on `fill_rx` channel with `exchange_cancels` populated. Assert TP exchange order cancelled on mock. | Critical | FillDetector + Engine |
| FR-4 | **Reconciliation: active group, no position, orphaned TP**: Build OrderGroup in Active status. Exchange state: no position for symbol, TP order still in open_orders, SL not in open_orders. Assert `determine_reconcile_actions()` returns action with `new_status = StoppedOut` and `orders_to_cancel = [tp_id]`. | Critical | Reconciliation |
| FR-5 | **Reconciliation: pending entry gone**: Build OrderGroup in Pending status. Exchange state: entry not in open_orders, SL and TP in open_orders. Assert action returns `new_status = Cancelled` and `orders_to_cancel = [sl_id, tp_id]`. | Critical | Reconciliation |
| FR-6 | **Reconciliation: zombie recovery**: Build OrderGroup in AwaitingReconciliation status. Exchange state: open order with matching `clientOrderId = testudo:{group_id}:entry`. Assert action returns `new_status = Pending` and `orders_to_cancel = []`. | High | Reconciliation |
| FR-7 | **Concurrent fill and cancel converges**: Spawn FillDetector. Concurrently: (a) send SL fill event via channel, (b) cancel TP directly via `StatefulMockExchangeApi`. Both paths should converge on a terminal group state without panics. Idempotent `OrderNotFound` from double-cancel is expected. | High | FillDetector |

---

## Technical Implementation

### Test 1: `test_fill_detector_sl_fill_via_channel` (FR-1)

**File:** `crates/router/src/services/integration_tests.rs`

```rust
#[tokio::test]
async fn test_fill_detector_sl_fill_via_channel() {
    // 1. Spawn actor with pre-populated Active group (entry/SL/TP exchange IDs registered)
    // 2. Create StatefulMockExchangeApi with SL + TP in open_orders
    // 3. Create FillDetectorService with handle + mock
    // 4. Create mpsc channels: (order_tx, order_rx), use fill_rx from actor
    // 5. Spawn detector.run(order_rx, fill_rx) in background task
    // 6. Send OrderUpdateEvent { id: "exch-sl-1", status: "closed" } via order_tx
    // 7. Yield to runtime (tokio::task::yield_now or short sleep)
    // 8. Assert: mock.cancelled_ids() contains "exch-tp-1"
    // 9. Assert: handle.get_trade_group(group_id).status == StoppedOut
    // 10. Assert: mock.has_open_order("exch-tp-1") == false
}
```

Difference from existing `test_sl_fill_cancels_tp`: this test exercises the full `run()` loop with actual channel delivery and `tokio::select!`, not a direct method call.

### Test 2: `test_fill_detector_entry_cancelled_via_channel` (FR-2)

```rust
#[tokio::test]
async fn test_fill_detector_entry_cancelled_via_channel() {
    // 1. Spawn actor with Pending group (entry/SL/TP registered)
    // 2. Mock has entry, SL, TP in open_orders
    // 3. Spawn detector.run()
    // 4. Send OrderUpdateEvent { id: "exch-entry-1", status: "cancelled" }
    // 5. Assert: mock.cancelled_ids() contains both "exch-sl-1" and "exch-tp-1"
    // 6. Assert: group status == Cancelled
}
```

Tests the `handle_cancelled_event` path — currently untested in the existing suite.

### Test 3: `test_fill_detector_shadow_fill_cancels_exchange` (FR-3)

```rust
#[tokio::test]
async fn test_fill_detector_shadow_fill_cancels_exchange() {
    // 1. Spawn actor, place order with SL/TP, register exchange IDs
    // 2. Mark entry as filled via handle.on_entry_filled()
    // 3. Mock has SL + TP in open_orders
    // 4. Spawn detector.run() — listening on both order_rx AND fill_rx
    // 5. Push price update via handle that triggers SL fill in shadow engine
    //    (price drops below SL level)
    // 6. FillEvent arrives on fill_rx with exchange_cancels for TP
    // 7. Assert: mock.cancelled_ids() contains TP exchange order ID
}
```

This is the most architecturally significant test — it exercises the dual-channel design from 019d where shadow engine fills trigger exchange cleanup via fire-and-forget events.

### Test 4: `test_reconciliation_orphaned_tp_after_sl_fill` (FR-4)

```rust
#[tokio::test]
async fn test_reconciliation_orphaned_tp_after_sl_fill() {
    // 1. Build OrderGroup: Active, exchange_sl_order_id = "sl-1", exchange_tp_order_id = "tp-1"
    // 2. Exchange state: open_order_ids = {"tp-1"}, symbols_with_position = {} (no position)
    // 3. Call determine_reconcile_actions(&[group], &open_ids, &symbols_with_pos, &open_orders)
    // 4. Assert: one action with new_status = StoppedOut, orders_to_cancel = ["tp-1"]
}
```

Tests the reconciliation decision matrix without network/DB dependencies.

### Test 5: `test_reconciliation_pending_entry_gone` (FR-5)

```rust
#[tokio::test]
async fn test_reconciliation_pending_entry_gone() {
    // 1. Build OrderGroup: Pending, exchange_order_id (entry) = "entry-1",
    //    exchange_sl_order_id = "sl-1", exchange_tp_order_id = "tp-1"
    // 2. Exchange state: open_order_ids = {"sl-1", "tp-1"} (entry NOT present)
    // 3. Call determine_reconcile_actions()
    // 4. Assert: new_status = Cancelled, orders_to_cancel = ["sl-1", "tp-1"]
}
```

### Test 6: `test_reconciliation_zombie_recovery` (FR-6)

```rust
#[tokio::test]
async fn test_reconciliation_zombie_recovery() {
    // 1. Build OrderGroup: AwaitingReconciliation, id = known_uuid
    // 2. Open orders include one with client_order_id = "testudo:{known_uuid}:entry"
    // 3. Call determine_reconcile_actions()
    // 4. Assert: new_status = Pending, orders_to_cancel = [] (recovered, not cancelled)
}
```

### Test 7: `test_concurrent_fill_and_cancel_converges` (FR-7)

```rust
#[tokio::test]
async fn test_concurrent_fill_and_cancel_converges() {
    // 1. Active group with SL/TP registered
    // 2. Spawn detector.run()
    // 3. tokio::join!:
    //    a) Send SL fill event via order_tx
    //    b) mock.cancel_order("exch-tp-1") directly (simulating user cancel)
    // 4. Wait for convergence
    // 5. Assert: group is in terminal state (StoppedOut or Cancelled)
    // 6. Assert: no panics occurred
    // 7. Assert: double-cancel on TP resulted in at most one OrderNotFound (idempotent)
}
```

---

## Verification

```bash
# Run only integration tests
cd testudo-exchange && cargo test integration_tests -- --nocapture

# Run specific test
cd testudo-exchange && cargo test test_fill_detector_sl_fill_via_channel -- --nocapture

# Verify no regressions
cd testudo-exchange && cargo clippy --all-targets
cd testudo-exchange && cargo test
```

---

## Dependencies

- **020-integration-test-infrastructure** (Draft): StatefulMockExchangeApi, test helpers, reconciliation extraction

## Blocked by

- 020-integration-test-infrastructure

## Blocks

None.
