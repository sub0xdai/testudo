# Optimization Target: api-dedup

## Goal
Extract the duplicated HTTP request boilerplate in `background.ts` into a single typed helper. Currently 15+ API functions each repeat: getSettings → getTokens → auth check → fetch → response.ok → 401 retry → JSON parse → Zod validate → error normalize → catch. This pattern spans ~500 lines of near-identical code.

"Better" means: fewer lines, single retry/validation path, zero behavior change. Every existing caller must produce identical responses for identical inputs.

## Target Files
- `testudo-extension/src/background.ts`

## Constraints
- Public behavior of every API function must remain identical (same return shapes, same error messages, same retry semantics)
- Do NOT modify test files (`background.test.ts`, `utils.test.ts`, `types.test.ts`)
- Do NOT modify `schemas.ts` or `types.ts`
- Do NOT add new dependencies
- Do NOT change the message handler dispatch (that's a separate optimization target)
- Preserve the two auth patterns:
  - **Hard auth**: `if (!tokens || tokens.expires_in <= 0)` → early return error (used by executeTrade, listTrades, cancelTrade, cleanupTrades, getLiveBalance, fetchExchangePositions, closeExchangePosition)
  - **Soft auth**: `if (tokens && tokens.expires_in > 0) headers["Authorization"] = ...` — proceeds without auth if no token (used by listExchanges, listExchangeAccounts, addExchangeAccount, deleteExchangeAccount, testExchangeConnection)
- Preserve per-function quirks:
  - `listTrades`: uses `normalizeTradeListResponse()` before validation
  - `cancelTrade`, `cleanupTrades`, `executeTrade`: uses `normalizeBackendAck()` after validation
  - `getLiveBalance`: uses `AbortSignal.timeout(10000)` and maps balance fields
  - `fetchExchangePositions`: uses `AbortSignal.timeout(15000)`
  - `closeExchangePosition`: uses both `ensureActiveExchange()` pre-check AND body payload
  - `login`, `register`: no 401 retry (not authenticated yet)

## Strategy Hints
- Start with a generic `apiCall()` helper that handles: settings lookup, token retrieval, auth header, fetch, 401 retry + recursive call, JSON parse, error extraction
- Use generics + Zod schema parameter for type-safe validation: `apiCall<T>(endpoint, schema, options)`
- Options bag for: method, body, requireAuth (hard/soft/none), signal timeout, normalizer callback
- Each existing function becomes a thin wrapper: endpoint + schema + options → apiCall
- Consider extracting in phases: first the helper, then migrate 2-3 simple functions, then the rest
- Keep `normalizeBackendAck()` and `normalizeTradeListResponse()` as-is — they're response normalizers, not request boilerplate

## Verification
```bash
cd testudo-extension && bun run build && bun run test
```

## Metric
- METRIC_DIRECTION=MINIMIZE
- Benchmark: .specify/optimize/api-dedup/benchmark.sh
- BENCHMARK_RUNS=1
