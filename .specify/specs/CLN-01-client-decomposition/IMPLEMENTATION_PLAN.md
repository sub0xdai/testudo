# CLN-01-client-decomposition — Implementation Plan

## Current State Summary

`testudo-journal/src/api/client.ts` is 1,036 lines — a monolithic barrel file containing:
- `fetchWithCredentials` (lines 54-66) — fetch wrapper with token refresh retry; **has a FormData consumption bug** on retry
- `buildParams` (lines 68-74) — query string builder for `StatsFilter`
- 3 internal helpers: `fetchApi<T>` (line 154, analytics), `fetchCrud<T>` (line 339, trades/journal), `fetchExchange<T>` (line 619, exchange endpoints)
- 40+ exported interfaces and 30+ async functions spanning 8 domains

61 files import from `client.ts` — all must continue working via barrel re-export.

Pre-existing `npx tsc --noEmit` errors: 9 unused-variable errors in `IdentitySettings.tsx` (CLN-03 scope), 1 dead import in `Account.tsx` (CP-4 scope here).

## Checkpoints

### CP-1: Extract core.ts + fix FormData retry bug ✅
- Completed 2026-05-29 by /skill:vox build. Build passes, 56/56 tests pass, all chunks within 250KB budget.
- **Touches**: `src/api/core.ts` (new), `src/api/client.ts` (modified)
- **Tasks**:
  1. Create `src/api/core.ts` with: `API_BASE`, `fetchWithCredentials` (with retry restricted to GET/HEAD/OPTIONS), `buildParams`, `fetchApi<T>`, `fetchCrud<T>`, `fetchExchange<T>`
  2. In `client.ts`: delete original `fetchWithCredentials`, `buildParams`, `fetchApi`, `fetchCrud`, `fetchExchange`
  3. In `client.ts`: add `export * from './core'` as first re-export
- **Verification**: `cd testudo-journal && bun run build` produces no build errors
- **Verification**: `cd testudo-journal && bun test` — existing tests pass (token refresh test in `lib/cache-batch.test.ts`)
- **Commit message**: `fix: extract core API layer, restrict 401 retry to safe methods`

### CP-2: Extract analytics.ts + shared types.ts
- **Touches**: `src/api/types.ts` (new), `src/api/analytics.ts` (new), `src/api/client.ts` (modified)
- **Tasks**:
  1. Create `src/api/types.ts` with shared interfaces: `StatsFilter`, `AccountStats`, `PerformanceStats`, `RiskStats`, `SetupTagEntry`, `KellyInputs`, `FilterOptions`
  2. Create `src/api/analytics.ts` with: `OverviewResponse`, `EquityPoint`, `DailyPnlPoint`, `SymbolBreakdownItem`, `SetupBreakdownItem`, `DurationProfitPoint`, `ReturnBucket`, `TimeSlot`, `SymbolCount`, `BatchSection`, `BatchAnalyticsResponse`, and all `fetch*` functions
  3. Shift domain-specific types into their respective modules (e.g., `JournalTrade`, `JournalTag`, `JournalEntry` → `journal.ts` in CP-3)
  4. Delete extracted code from `client.ts`, add `export * from './types'` and `export * from './analytics'`
- **Verification**: `cd testudo-journal && bun run build` — all 26 analytics-consuming components still work
- **Commit message**: `refactor: extract analytics and shared types into domain modules`

### CP-3: Extract trades.ts, journal.ts, coach.ts, dignitas.ts, exchange.ts, risk.ts
- **Touches**: 6 new files in `src/api/`, `client.ts` (modified)
- **Tasks**:
  1. Create `src/api/trades.ts` — `JournalTrade`, `TradeDetail`, `TradeWithTags`, `TradesResponse`, `TradeListParams`, all trade CRUD functions, active positions, draft notes, journal sync trigger
  2. Create `src/api/journal.ts` — `JournalEntry`, `JournalTag`, `uploadJournalImage`, `UploadError`, `StorageUsage`, all entry/tag CRUD functions
  3. Create `src/api/coach.ts` — `CoachPatternKind`, `CoachSeverity`, `CoachDigest`, `CoachLatestResponse`, `CoachArchiveResponse`, all coach API functions
  4. Create `src/api/dignitas.ts` — `DignitasCurrent`, `DignitasHistory`, `IdentityPreferences`, `PublicProfile`, all dignitas + identity API functions, `pairExtension`, `checkPairStatus`
  5. Create `src/api/exchange.ts` — `ExchangeInfo`, `ExchangeAccount`, all account types, `TestConnectionResult`, `ExchangeBalanceResponse`, `exchangeApi` object with all methods, agent wallet types
  6. Create `src/api/risk.ts` — `RiskSnapshot`, `PositionEntry`, `VenuePositions`, `VenueMargin`, `CorrelationBucket`, `fetchRiskSnapshot`
  7. Delete all extracted code from `client.ts`, add 6 `export * from './...'` lines
- **Verification**: `cd testudo-journal && bun run build` — zero build errors across all 61 consumers
- **Commit message**: `refactor: extract trades, journal, coach, dignitas, exchange, risk into domain modules`

### CP-4: Final cleanup — client.ts < 200 lines, fix Account.tsx dead import
- **Touches**: `src/api/client.ts` (modified), `src/pages/Account.tsx` (modified)
- **Tasks**:
  1. Verify `client.ts` is pure re-exports (8 `export *` lines + anchor header + `API_BASE` export)
  2. Remove dead `import { IdentitySettings }` from `Account.tsx` (line 19), fixing TS6133 error
  3. Run `npx tsc --noEmit` — verify only 9 TS6133 errors remain (all in `IdentitySettings.tsx`, CLN-03 scope)
- **Verification**: `cd testudo-journal && bun run build && bun run build:check` passes
- **Verification**: `wc -l src/api/client.ts` prints `< 200`
- **Verification**: `npx tsc --noEmit 2>&1 | grep -c TS6133` prints `9` (only IdentitySettings.tsx)
- **Commit message**: `refactor: client.ts barrel re-exports under 200 lines, fix Account.tsx dead import`

---

## Risks

1. **Import resolution order** — `export *` from sibling modules in `client.ts` works with esbuild/Vite but might shadow names. Mitigation: no two domain modules export the same named member. Verify by checking `bun run build` after each CP.
2. **Circular dependency** — domain modules must never import from `../api/client` (only `./core` and `./types`). Mitigation: code review after CP-3.
3. **Test breakage** — `lib/cache-batch.test.ts` imports from `client.ts`. Mitigation: barrel re-export means zero test changes needed.
