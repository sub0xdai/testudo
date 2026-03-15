# Quality Checklist — CEX-03 ExchangeGateway

**Spec ID:** CEX-03-exchange-gateway
**Date:** 2026-03-15

## Implementation

- [x] `ExchangeGateway` class created in `testudo-cex/src/gateway.ts`
- [x] Cache key derived from `hash(exchange_id + api_key + sandbox)`
- [x] `getOrCreate()` returns cached instance on duplicate calls
- [x] `createExchange()` called with correct config shape
- [x] `exchange.on("fill", onFill)` wired during creation
- [x] `exchange.on("error", ...)` and `exchange.on("log", ...)` wired
- [x] `exchange.start()` called to establish WebSocket connections
- [x] Failed `start()` throws without caching instance
- [x] `dispose()` and `disposeAll()` clean up correctly

## Testing

- [x] Test: create returns new instance
- [x] Test: second getOrCreate returns cached instance
- [x] Test: dispose removes instance
- [x] Test: start failure does not cache instance
- [x] `bun test` all pass (10/10)

## Verification

- [x] `bun run build` succeeds (143 modules, 1.1 MB)
- [x] No lint errors
