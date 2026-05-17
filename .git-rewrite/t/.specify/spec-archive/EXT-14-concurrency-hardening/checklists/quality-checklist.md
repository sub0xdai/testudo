# Quality Checklist: EXT-14-concurrency-hardening

> Spec: EXT-14 | Date: 2026-02-16

---

## Pre-Implementation

- [ ] Read full spec and understand all 10 FRs
- [ ] Verify `autoFilled()` bug reproduces (focus any field in TradeForm)
- [ ] Verify existing test suite baseline: `bun run test` passes
- [ ] Verify typecheck baseline: `bun run typecheck` passes

## Implementation

### FR-1: autoFilled() Fix
- [ ] Replace all 4 `autoFilled()` references with per-field `autoFilledFields().has(fieldName)`
- [ ] Verify no other references to `autoFilled` exist: `grep -rn "autoFilled()" src/`

### FR-2: Token Refresh Mutex
- [ ] Add `refreshInFlight` module-scope Promise latch
- [ ] Extract refresh logic to `doRefresh()` private function
- [ ] Verify concurrent callers share same Promise
- [ ] Add unit test: two concurrent calls → single fetch

### FR-3: Retry Depth Limit
- [ ] Add `retried` parameter to `executeTrade`, `listTrades`, `getBalances`
- [ ] Verify no recursive call without `retried = true`
- [ ] Add unit test: double-401 does not infinite loop

### FR-4: Alt+X Async Guard
- [ ] Add `altXPending` module-scope boolean
- [ ] Check flag alongside `!isVisible()` in keydown handler
- [ ] Set flag before first await, clear in `finally`

### FR-5: Scraper Telemetry Queue
- [ ] Replace fire-and-forget with chained `telemetryQueue` Promise
- [ ] Verify `.catch()` on queue prevents unhandled rejections

### FR-6: WS Reconnect Debounce
- [ ] Add `debouncedConnectWebSocket()` with 300ms trailing edge
- [ ] Replace `storage.onChanged` handler to use debounced version

### FR-7: Pure Derived Signal
- [ ] Extract `onCountChange` call to `createEffect`
- [ ] Verify `activeTrades()` is a pure computation (no side effects)

### FR-8: Dead Code Removal
- [ ] Delete `startWatching`, `stopWatching`, `observer`, `onToolDetected`
- [ ] Verify no imports reference these symbols

### FR-9: Toast Cap
- [ ] Add `activeToasts` array with MAX_TOASTS = 3
- [ ] Evict oldest on overflow
- [ ] Clean up array reference on natural timeout removal

## Post-Implementation

- [ ] `bun run test` — all tests pass
- [ ] `bun run typecheck` — zero errors
- [ ] `bun run build` — builds without warnings
- [ ] Grep checks from acceptance criteria all pass
- [ ] Manual test: Alt+X on TradingView, field focus, rapid dismiss/reopen
