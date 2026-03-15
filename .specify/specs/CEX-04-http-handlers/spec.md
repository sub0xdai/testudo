# Specification: HTTP Handlers — Same Contract, New Engine

**Spec ID:** CEX-04-http-handlers
**Date:** 2026-03-15
**Status:** Draft
**Class:** Core / Migration
**Priority:** P1 — Rust backend depends on these endpoints
**Depends on:** CEX-02 (scaffold), CEX-03 (gateway)
**Series:** CEX-01 through CEX-08 (safe-cex migration)

---

## Problem Statement

The Rust backend's `CcxtClient` communicates with the sidecar via HTTP. The new sidecar must expose the **exact same endpoint contract** so that the Rust backend requires minimal changes. The key difference: reads (balance, positions, orders) come from safe-cex's in-memory Store (no HTTP round-trip to exchange), while writes use safe-cex methods.

---

## User Stories

- **As the Rust backend**, I want the same HTTP endpoints with the same request/response shapes, so that migration requires only renaming the client.
- **As the system**, I want balance/position reads from the Store (not API calls), so that responses are faster and don't consume rate limits.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `POST /balance` reads from `exchange.store.balance` | High | Handlers |
| FR-2 | `POST /order` calls `exchange.placeOrder(opts)` with bracket support | High | Handlers |
| FR-3 | `POST /order/cancel` calls `exchange.cancelOrders([order])` | High | Handlers |
| FR-4 | `POST /orders/cancel-all` calls `exchange.cancelSymbolOrders(symbol)` | High | Handlers |
| FR-5 | `POST /position` reads from `exchange.store.positions` | High | Handlers |
| FR-6 | `POST /leverage` calls `exchange.setLeverage(symbol, leverage)` | High | Handlers |
| FR-7 | `POST /orders/open` reads from `exchange.store.orders` | High | Handlers |
| FR-8 | `POST /order/edit` calls `exchange.updateOrder({order, update})` | High | Handlers |
| FR-9 | `GET /health` checks `store.loaded.balance && store.loaded.orders` | High | Handlers |
| FR-10 | All numeric fields returned as strings (preserve precision) | High | Handlers |
| FR-11 | Request envelope format unchanged: `{exchange_id, credentials, sandbox, params}` | High | Handlers |
| FR-12 | Bracket order via `placeOrder({stopLoss, takeProfit})` returns entry + SL + TP IDs | High | Handlers |
| FR-13 | Error responses map to existing `CcxtClientError` enum (401, 402, 404, 429, 502) | High | Handlers |
| FR-14 | `clientOrderId` passthrough support | Medium | Handlers |
| FR-15 | `reduceOnly` flag support | Medium | Handlers |
| FR-16 | Leverage configuration with graceful fallback | Medium | Handlers |

---

## Technical Implementation

### Request Envelope (unchanged)

```json
{
  "exchange_id": "woo",
  "credentials": { "apiKey": "...", "secret": "...", "password": "..." },
  "sandbox": false,
  "params": { ... }
}
```

### Endpoint Mapping

**File:** `testudo-cex/src/handlers.ts`

| Endpoint | safe-cex Method | Notes |
|----------|----------------|-------|
| `POST /balance` | `exchange.store.balance` | Read from Store (no HTTP call) |
| `POST /order` | `exchange.placeOrder(opts)` | Returns `string[]` (order IDs) |
| `POST /order/cancel` | `exchange.cancelOrders([order])` | Find order in Store by ID |
| `POST /orders/cancel-all` | `exchange.cancelSymbolOrders(symbol)` | Cancel all for symbol |
| `POST /position` | `exchange.store.positions` | Read from Store |
| `POST /leverage` | `exchange.setLeverage(symbol, leverage)` | Direct call |
| `POST /orders/open` | `exchange.store.orders` | Read from Store |
| `POST /order/edit` | `exchange.updateOrder({order, update})` | Cancel+replace internally |
| `GET /health` | Check `store.loaded` flags | `balance && orders` |

### Bracket Order Placement (FR-12)

```typescript
// safe-cex handles this natively per exchange
const orderIds: string[] = await exchange.placeOrder({
  symbol: "BTCUSDT",
  type: "limit",
  side: "buy",
  amount: 0.01,
  price: 70000,
  stopLoss: 69000,      // auto-creates SL order (algo on WOO X)
  takeProfit: 72000,     // auto-creates TP order (algo on WOO X)
});
// Returns: ["entry-id", "sl-id", "tp-id"]
```

### Response Mapping

Response maps to existing `SidecarOrderResponse` with `stop_loss_order_id` and `take_profit_order_id`:

```typescript
// Map safe-cex order IDs to response shape
const response = {
  id: orderIds[0],
  stop_loss_order_id: orderIds[1] || null,
  take_profit_order_id: orderIds[2] || null,
  // ... other fields from Store
};
```

### Numeric Stringification (FR-10)

All numeric fields (total, free, used, price, amount, filled, remaining) must be converted to strings before JSON serialization to preserve decimal precision.

```typescript
function stringify(obj: Record<string, any>): Record<string, string> {
  return Object.fromEntries(
    Object.entries(obj).map(([k, v]) => [k, String(v)])
  );
}
```

### Error Mapping (FR-13)

```typescript
function mapError(err: any): { status: number; message: string } {
  const msg = String(err?.message || err);
  if (msg.includes("401") || msg.includes("auth")) return { status: 401, message: msg };
  if (msg.includes("insufficient") || msg.includes("margin")) return { status: 402, message: msg };
  if (msg.includes("not found")) return { status: 404, message: msg };
  if (msg.includes("rate") || msg.includes("429")) return { status: 429, message: msg };
  return { status: 502, message: msg };
}
```

---

## Acceptance Criteria

- [ ] All 10 HTTP endpoints respond with same shapes as old sidecar
- [ ] Bracket order placement returns entry + SL + TP order IDs
- [ ] Balance/position reads come from Store (no redundant HTTP calls to exchange)
- [ ] All numeric fields are strings in responses
- [ ] Error mapping matches existing `CcxtClientError` enum (401, 402, 404, 429, 502)
- [ ] `clientOrderId` and `reduceOnly` passed through to safe-cex
- [ ] Request envelope format unchanged

---

## Completion Signal

This spec is complete when:
1. All endpoints implemented and tested
2. Response shapes match old sidecar (verified by comparing against `ccxt_client.rs` expectations)
3. Error mapping covers all known error types
4. Changes committed to master
