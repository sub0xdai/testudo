# Quality Checklist — EXT-21 Live Trade Execution

| Spec ID | EXT-21-live-trade-execution |
|---------|------------------------------|
| Date    | 2026-02-28                   |

## Pre-Implementation
- [ ] All root causes understood (symbol mismatch, silent shadow fallback, no error feedback)
- [ ] Backend env vars documented (CCXT_SIDECAR_URL, CCXT_SANDBOX, CCXT_ENABLED)
- [ ] CCXT sidecar running and reachable before testing

## Implementation
- [ ] FR-1: normalizeSymbol USD->USDT upgrade implemented
- [ ] FR-1: Tests added for BTCUSD, ETHUSD, SOLUSD cases
- [ ] FR-2: execution_mode field added to trade response
- [ ] FR-3: 503 error returned when sidecar unavailable
- [ ] FR-4: Startup warning logged
- [ ] FR-5: Extension surfaces backend errors via toast

## Testing
- [ ] `npx vitest run` — all extension tests pass
- [ ] `cargo test` — all backend tests pass
- [ ] Manual: BTCUSD chart -> trade -> correct symbol on exchange
- [ ] Manual: BTCUSDT chart -> trade -> correct symbol on exchange
- [ ] Manual: Sidecar down -> trade -> error shown (not silent PENDING)
- [ ] Manual: Sidecar up + real credentials -> trade appears on exchange

## Post-Implementation
- [ ] No regressions in existing functionality
- [ ] Commit message follows project conventions
