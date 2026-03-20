# Quality Checklist — CEX-04 HTTP Handlers

**Spec ID:** CEX-04-http-handlers
**Date:** 2026-03-15

## Implementation

- [ ] `POST /balance` reads from `exchange.store.balance`
- [ ] `POST /order` calls `exchange.placeOrder()` with full param support
- [ ] `POST /order/cancel` calls `exchange.cancelOrders()`
- [ ] `POST /orders/cancel-all` calls `exchange.cancelSymbolOrders()`
- [ ] `POST /position` reads from `exchange.store.positions`
- [ ] `POST /leverage` calls `exchange.setLeverage()`
- [ ] `POST /orders/open` reads from `exchange.store.orders`
- [ ] `POST /order/edit` calls `exchange.updateOrder()`
- [ ] `GET /health` checks `store.loaded` flags
- [ ] Bracket order returns entry + SL + TP order IDs
- [ ] All numerics stringified before response
- [ ] Request envelope parsed correctly
- [ ] Error responses map to correct HTTP status codes (401, 402, 404, 429, 502)
- [ ] `clientOrderId` passthrough works
- [ ] `reduceOnly` flag works

## Testing

- [ ] Test: balance endpoint returns stringified values
- [ ] Test: order placement with bracket params
- [ ] Test: order cancellation
- [ ] Test: error mapping for each error type
- [ ] `bun test` all pass

## Verification

- [ ] Response shapes compared against `SidecarOrderResponse` struct in Rust backend
- [ ] `bun run build` succeeds
