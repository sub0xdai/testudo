# Quality Checklist — CEX-05 WebSocket Fill Streaming

**Spec ID:** CEX-05-ws-fill-streaming
**Date:** 2026-03-15

## Implementation

- [ ] WebSocket server at `/ws/orders` path
- [ ] `exchange.on("fill", ...)` wired to forward events
- [ ] Fill events include all fields: id, symbol, status, side, price, amount, filled, remaining, average, timestamp
- [ ] Store diffing detects order removals (cancellations)
- [ ] Cancellation events emitted with `status: "canceled"`
- [ ] Order matching logic correlates fills to Store orders
- [ ] Subscription message handling from Rust backend

## Testing

- [ ] Test: fill event for regular order produces correct WebSocket message
- [ ] Test: fill event for algo order produces correct WebSocket message
- [ ] Test: order removal from Store emits cancellation event
- [ ] Test: event shape matches `OrderUpdateEvent` struct
- [ ] `bun test` all pass

## Verification

- [ ] Event shape compared against `OrderUpdateEvent` in `ccxt_client.rs`
- [ ] `bun run build` succeeds
