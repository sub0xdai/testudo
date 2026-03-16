# Quality Checklist — HL-04 WS Fills

**Spec ID:** HL-04-ws-fills
**Date:** 2026-03-16

## Implementation

- [ ] HyperliquidFillSubscriber struct
- [ ] WsProvider connection to Hyperliquid WebSocket
- [ ] subscribe_order_updates subscription handler
- [ ] OrderUpdate to OrderUpdateEvent translation layer
- [ ] mpsc channel forwarding to fill detector
- [ ] Exponential backoff reconnection strategy
- [ ] Graceful shutdown with cancellation token

## Verification

- [ ] `cargo clippy --all-targets` passes with zero warnings
- [ ] `cargo test` passes with zero failures
