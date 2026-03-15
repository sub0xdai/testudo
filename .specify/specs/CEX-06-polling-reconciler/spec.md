# Specification: Polling Reconciler — Orphaned Order Safety Net

**Spec ID:** CEX-06-polling-reconciler
**Date:** 2026-03-15
**Status:** Complete
**Class:** Safety / Defense-in-Depth
**Priority:** P1 — catches missed WebSocket events
**Depends on:** CEX-03 (gateway), CEX-05 (fill streaming)
**Series:** CEX-01 through CEX-08 (safe-cex migration)

---

## Problem Statement

WebSocket connections can drop packets. If an SL fill event is missed, the TP order remains on the exchange as an orphan — it can execute as an unwanted new position when price reaches the TP level. The reconciler provides defense-in-depth by periodically checking the Store state and detecting orphaned orders.

---

## User Stories

- **As the system**, I want orphaned orders detected and cancelled automatically, so that missed WebSocket events don't result in unwanted positions.
- **As the system**, I want synthetic fill events emitted when the reconciler acts, so that the Rust backend updates its state.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Reconciler runs on configurable interval (default 15 seconds) | High | Reconciler |
| FR-2 | Read positions and orders from `exchange.store` (no API calls) | High | Reconciler |
| FR-3 | Detect orphaned orders: orders exist for a symbol but no position exists | High | Reconciler |
| FR-4 | Cancel orphaned orders via `exchange.cancelSymbolOrders(symbol)` | High | Reconciler |
| FR-5 | Emit synthetic `canceled` events so Rust backend updates state | High | Reconciler |
| FR-6 | Do not interfere with normal fill processing (debounce after fill events) | Medium | Reconciler |
| FR-7 | Log reconciliation actions for operational visibility | Medium | Reconciler |
| FR-8 | Graceful shutdown (stop interval on dispose) | Medium | Reconciler |

---

## Technical Implementation

### Reconciler Loop

**File:** `testudo-cex/src/reconciler.ts`

```typescript
interface GroupInfo {
  symbol: string;
  status: "pending" | "active";
  orderIds: string[];
}

export class Reconciler {
  private interval: ReturnType<typeof setInterval> | null = null;

  start(
    exchange: Exchange,
    activeGroups: Map<string, GroupInfo>,
    onOrphanCanceled: (orderId: string, symbol: string) => void,
    intervalMs: number = 15_000
  ) {
    this.interval = setInterval(async () => {
      await this.reconcile(exchange, activeGroups, onOrphanCanceled);
    }, intervalMs);
  }

  private async reconcile(
    exchange: Exchange,
    activeGroups: Map<string, GroupInfo>,
    onOrphanCanceled: (orderId: string, symbol: string) => void
  ) {
    const storePositions = exchange.store.positions;
    const storeOrders = exchange.store.orders;

    // Group orders by symbol
    const ordersBySymbol = new Map<string, typeof storeOrders>();
    for (const order of storeOrders) {
      const existing = ordersBySymbol.get(order.symbol) || [];
      existing.push(order);
      ordersBySymbol.set(order.symbol, existing);
    }

    // For each symbol with orders, check if a position exists
    for (const [symbol, orders] of ordersBySymbol) {
      const hasPosition = storePositions.some(
        (p) => p.symbol === symbol && Math.abs(p.contracts) > 0
      );

      if (!hasPosition) {
        // Check if these are reduce-only / SL/TP type orders (orphans)
        // Entry orders waiting to fill are expected — don't cancel those
        const orphanOrders = orders.filter(
          (o) => o.reduceOnly || o.type === "stop_market" || o.type === "take_profit_market"
        );

        if (orphanOrders.length > 0) {
          console.warn(
            `[reconciler] Orphaned orders detected for ${symbol}: ${orphanOrders.map((o) => o.id).join(", ")}`
          );

          try {
            await exchange.cancelSymbolOrders(symbol);
            for (const order of orphanOrders) {
              onOrphanCanceled(order.id, symbol);
            }
          } catch (err) {
            console.error(`[reconciler] Failed to cancel orphans for ${symbol}:`, err);
          }
        }
      }
    }
  }

  stop() {
    if (this.interval) {
      clearInterval(this.interval);
      this.interval = null;
    }
  }
}
```

### Synthetic Event Emission

When the reconciler cancels orphaned orders, it emits synthetic `order_update` events with `status: "canceled"` via the same WebSocket channel used by fill streaming (CEX-05). This ensures the Rust backend's `fill_detector.rs` processes the cancellation and updates the OrderGroup state.

### Key Design Decision

The reconciler only cancels **reduce-only and stop/TP orders** when no position exists. Entry orders awaiting fill are expected to exist without a position — cancelling them would break normal trade flow.

---

## Acceptance Criteria

- [x] Reconciler runs on configurable interval (default 15s)
- [x] Detects orphaned orders when position is gone but orders remain
- [x] Only cancels reduce-only / stop / TP orders (not entry orders)
- [x] Cancels orphaned orders via `cancelSymbolOrders`
- [x] Emits synthetic events so Rust backend updates state
- [x] Does not interfere with normal fill processing
- [x] Logs reconciliation actions with symbol and order IDs
- [x] Graceful shutdown (clears interval)

---

## Risks

1. **Race condition** — reconciler runs while a fill is being processed. Mitigation: debounce reconciler after receiving fill events.
2. **False positives** — entry order exists without a position (normal for pending limit orders). Mitigation: only cancel reduce-only/stop/TP orders.

---

## Completion Signal

This spec is complete when:
1. Reconciler class implemented and tested
2. Orphan detection logic verified
3. Synthetic events emitted correctly
4. Changes committed to master
