# Quality Checklist — CEX-06 Polling Reconciler

**Spec ID:** CEX-06-polling-reconciler
**Date:** 2026-03-15

## Implementation

- [ ] `Reconciler` class created in `testudo-cex/src/reconciler.ts`
- [ ] Interval runs every 15s by default (configurable)
- [ ] Reads positions and orders from `exchange.store`
- [ ] Detects orphaned reduce-only/stop/TP orders when no position exists
- [ ] Does NOT cancel entry orders waiting to fill
- [ ] Calls `exchange.cancelSymbolOrders()` to cancel orphans
- [ ] Emits synthetic `canceled` events via callback
- [ ] Logs reconciliation actions with symbol and order IDs
- [ ] `stop()` clears interval cleanly

## Testing

- [ ] Test: detects orphaned stop order when position absent
- [ ] Test: does NOT cancel entry order when position absent
- [ ] Test: emits synthetic cancellation events
- [ ] Test: stops cleanly
- [ ] `bun test` all pass

## Verification

- [ ] `bun run build` succeeds
