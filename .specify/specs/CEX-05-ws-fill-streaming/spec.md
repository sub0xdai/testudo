# Specification: WebSocket Fill Streaming — Event-Driven OCO

**Spec ID:** CEX-05-ws-fill-streaming
**Date:** 2026-03-15
**Status:** Complete
**Class:** Core / Critical Path
**Priority:** P0 — fixes the root cause (dead watchOrders)
**Depends on:** CEX-03 (gateway), CEX-04 (handlers)
**Series:** CEX-01 through CEX-08 (safe-cex migration)

---

## Problem Statement

CCXT does not implement `watchOrders` for WOO X — the base class throws `NotSupported`. The entire WebSocket fill detection path has **never worked** for WOO X. OCO cancellation (cancel TP when SL fills, and vice versa) has never fired. This is the root cause of orphaned limit orders.

safe-cex subscribes to **both** `executionreport` (regular order fills) AND `algoexecutionreportv2` (algo/stop order fills) WebSocket topics internally. This spec wires those events to the Rust backend via WebSocket, replacing the dead `watchOrders` loop.

---

## User Stories

- **As the fill_detector**, I want real-time fill events for ALL order types (regular + algo/stop), so that OCO cancellation fires when SL or TP fills.
- **As the system**, I want cancellation events forwarded, so that the Rust backend can track order lifecycle.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Forward safe-cex `fill` events to connected Rust backend via WebSocket | High | ws-fills |
| FR-2 | Event shape matches existing `OrderUpdateEvent` struct exactly | High | ws-fills |
| FR-3 | Fill events for regular orders (limit, market) arrive at Rust backend | High | ws-fills |
| FR-4 | Fill events for algo orders (stop-market SL, conditional TP) arrive at Rust backend | High | ws-fills |
| FR-5 | Detect order cancellations via Store diffing and emit `canceled` status | High | ws-fills |
| FR-6 | WebSocket server at `/ws/orders` path (same as old sidecar) | High | ws-fills |
| FR-7 | Support subscription message from Rust backend to start streaming | Medium | ws-fills |
| FR-8 | Handle WebSocket reconnection gracefully | Medium | ws-fills |

---

## Technical Implementation

### Fill Event Forwarding

**File:** `testudo-cex/src/ws-fills.ts`

```typescript
// When Rust backend connects to /ws/orders and subscribes:
exchange.on("fill", (fill) => {
  // Find the order in the Store that just filled
  // Match by symbol + side + approximate amount
  const matchedOrder = findMatchingOrder(exchange.store.orders, fill);

  ws.send(JSON.stringify({
    event: "order_update",
    data: {
      id: matchedOrder?.id || "unknown",
      symbol: fill.symbol,
      status: "closed",
      side: fill.side,
      price: matchedOrder?.price || fill.price,
      amount: matchedOrder?.amount || fill.amount,
      filled: fill.amount,
      remaining: 0,
      average: fill.price,
      timestamp: Date.now(),
    }
  }));
});
```

### Cancellation Detection

```typescript
// Track previous order set, detect removals
let previousOrders = new Map<string, Order>();

exchange.on("update", (store) => {
  const currentOrders = new Map(store.orders.map(o => [o.id, o]));

  // Detect removed orders (cancellations)
  for (const [id, prevOrder] of previousOrders) {
    if (!currentOrders.has(id)) {
      ws.send(JSON.stringify({
        event: "order_update",
        data: {
          id,
          symbol: prevOrder.symbol,
          status: "canceled",
          side: prevOrder.side,
          price: prevOrder.price,
          amount: prevOrder.amount,
          filled: prevOrder.filled || 0,
          remaining: prevOrder.remaining || prevOrder.amount,
          average: prevOrder.price,
          timestamp: Date.now(),
        }
      }));
    }
  }

  previousOrders = currentOrders;
});
```

### WebSocket Event Shape (unchanged)

The response shape stays identical to what `fill_detector.rs` expects:

```json
{
  "event": "order_update",
  "data": {
    "id": "string",
    "symbol": "string",
    "status": "closed|canceled",
    "side": "buy|sell",
    "price": 70000,
    "amount": 0.01,
    "filled": 0.01,
    "remaining": 0,
    "average": 70050,
    "timestamp": 1710500000000
  }
}
```

This maps directly to the `OrderUpdateEvent` struct in `ccxt_client.rs`.

---

## Acceptance Criteria

- [x] Fill events for regular orders (limit, market) arrive at Rust backend
- [x] Fill events for algo orders (stop-market SL, conditional TP) arrive at Rust backend
- [x] Cancellation events arrive at Rust backend
- [x] Event shape matches existing `OrderUpdateEvent` struct exactly
- [x] `fill_detector.rs` can process events without any changes
- [x] WebSocket server at `/ws/orders` path accepts connections

---

## Risks

1. **Order matching ambiguity** — safe-cex `fill` events contain `{amount, price, side, symbol}` but no order ID. Must correlate with Store orders. Mitigation: match by symbol + side + check recently filled orders in Store.
2. **Rapid updates** — Store `update` events may fire frequently. Mitigation: only emit when order set actually changes.

---

## Completion Signal

This spec is complete when:
1. Fill streaming implemented and tested
2. Both regular and algo order fills forwarded
3. Cancellation detection works via Store diffing
4. Event shape validated against `OrderUpdateEvent` struct
5. Changes committed to master
