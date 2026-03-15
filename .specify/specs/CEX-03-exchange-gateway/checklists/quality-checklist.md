# Quality Checklist — CEX-03 ExchangeGateway

**Spec ID:** CEX-03-exchange-gateway
**Date:** 2026-03-15

## Implementation

- [ ] `ExchangeGateway` class created in `testudo-cex/src/gateway.ts`
- [ ] Cache key derived from `hash(exchange_id + api_key + sandbox)`
- [ ] `getOrCreate()` returns cached instance on duplicate calls
- [ ] `createExchange()` called with correct config shape
- [ ] `exchange.on("fill", onFill)` wired during creation
- [ ] `exchange.on("error", ...)` and `exchange.on("log", ...)` wired
- [ ] `exchange.start()` called to establish WebSocket connections
- [ ] Failed `start()` throws without caching instance
- [ ] `dispose()` and `disposeAll()` clean up correctly

## Testing

- [ ] Test: create returns new instance
- [ ] Test: second getOrCreate returns cached instance
- [ ] Test: dispose removes instance
- [ ] Test: start failure does not cache instance
- [ ] `bun test` all pass

## Verification

- [ ] `bun run build` succeeds
- [ ] No lint errors
