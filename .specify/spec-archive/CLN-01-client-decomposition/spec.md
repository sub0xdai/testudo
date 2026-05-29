# Specification: Decompose `client.ts` + Fix FormData Retry Bug

**Spec ID:** CLN-01-client-decomposition
**Date:** 2026-05-29
**Status:** Draft
**Class:** Infrastructure / Refactor
**Priority:** P1 — `client.ts` is 1,033 lines (violates 1k-rule), and `fetchWithCredentials` has a correctness bug with FormData body retry on 401
**Depends on:** None (filesystem refactor, no backend changes)
**Series:** CLN-01 through CLN-03 (Journal Frontend Cleanup) — nuclear review findings on `testudo-journal/`

---

## Problem Statement

A thermo-nuclear code quality review of the `testudo-journal/` Solid.js frontend identified two issues in `src/api/client.ts`:

1. **File size violation (rule 1):** `client.ts` is 1,033 lines — a single barrel file containing 40+ TypeScript interfaces, 30+ async functions spanning 8 API domains (Analytics, Trades, Journal, Coach, Dignitas, Exchange, Risk, Auth). Every new endpoint defaults into this file, and the file has no internal structure beyond comment dividers (`// ─── Coach API ───`). This makes it hard to navigate, hard to review diffs for, and a magnet for further sprawl.

2. **Correctness bug (rule 4):** `fetchWithCredentials` (line 52) retries requests on 401 after refreshing the auth token. For `JSON.stringify(...)` bodies this works — the body is an immutable string. But `uploadJournalImage` passes a `FormData` object whose internal stream is consumed on the first `fetch()`. On retry, the consumed `FormData` sends an empty body, and the upload silently fails. Any future `ReadableStream`-body endpoint would have the same bug.

The decomposition also creates the structural home for the retry fix: with domain-separated modules, each can import a fixed `fetchWithCredentials` from a shared `api/core.ts` without circular dependencies.

---

## User Stories

- **As a journal developer**, I want API modules organized by domain so that I can find the right file without scrolling through 1,000+ lines.
- **As a journal developer**, I want to add a new Dignitas endpoint in `api/dignitas.ts` without touching a monolithic barrel file.
- **As a user uploading journal images**, I want uploads to survive token expiry without failing silently.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Split `src/api/client.ts` into domain modules: `core.ts`, `analytics.ts`, `trades.ts`, `journal.ts`, `coach.ts`, `dignitas.ts`, `exchange.ts`, `risk.ts`. | High | `src/api/` |
| FR-2 | Preserve backward compatibility: `src/api/client.ts` re-exports everything from the domain modules so no import paths break. | High | `src/api/client.ts` |
| FR-3 | Fix `fetchWithCredentials` to not retry requests with consumed bodies: clone the request before sending, or restrict retry to idempotent methods (GET/HEAD/OPTIONS). | High | `src/api/core.ts` |
| FR-4 | Run `bun run build` and `npx tsc --noEmit` — both must exit 0 with zero new errors. | High | `testudo-journal/` |
| FR-5 | Verify the performance budget: all entry chunks remain within 250 KB gzip. | Medium | `testudo-journal/` |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Extract `fetchWithCredentials` + `buildParams` + shared types into `src/api/core.ts`. Fix FormData retry bug. Re-export from `client.ts`. | `bun run build` passes, no import breakage |
| CP-2 | Split analytics types/functions into `src/api/analytics.ts`; re-export from `client.ts` | `bun run build` passes, analytics pages still load |
| CP-3 | Split remaining domains: trades, journal, coach, dignitas, exchange, risk | `bun run build` passes, all pages still load |
| CP-4 | Verify `client.ts` is under 200 lines (pure re-exports), remove dead `IdentitySettings` import from `Account.tsx` | `npx tsc --noEmit` exits 0; `client.ts` < 200 lines |

### FR-1: Module Split

```
src/api/
├── client.ts          # barrel re-exports + backward compatibility (< 200 lines)
├── core.ts            # fetchWithCredentials, buildParams, fetchApi, fetchCrud, fetchExchange
├── types.ts           # shared types: StatsFilter, KellyInputs, etc.
├── analytics.ts       # OverviewResponse, PerformanceStats, RiskStats, fetchOverview, fetchAnalyticsBatch, etc.
├── trades.ts          # JournalTrade, TradeDetail, fetchTrades, updateTradeNotes, etc.
├── journal.ts         # JournalEntry, JournalTag, createEntry, fetchEntries, uploadJournalImage, StorageUsage
├── coach.ts           # CoachDigest, CoachLatestResponse, fetchLatestCoachReport, etc.
├── dignitas.ts        # DignitasCurrent, DignitasHistory, fetchDignitasMe, etc.
├── exchange.ts        # ExchangeInfo, ExchangeAccount, exchangeApi object, etc.
└── risk.ts            # RiskSnapshot, fetchRiskSnapshot, etc.
```

**Paved Road:** Each domain module imports `fetchWithCredentials` from `./core`. No circular dependencies — `core.ts` is a leaf, domain modules import core, `client.ts` imports all.

### FR-2: Backward Compatibility

`client.ts` becomes a pure re-export barrel:

```typescript
// client.ts — barrel re-exports for backward compatibility
export * from './core'
export * from './types'
export * from './analytics'
export * from './trades'
export * from './journal'
export * from './coach'
export * from './dignitas'
export * from './exchange'
export * from './risk'
```

All existing `import { ... } from '../api/client'` statements continue to work without changes. New code can import from domain modules.

### FR-3: FormData Retry Fix

Two options:

**Option A — restrict retry to safe methods (preferred, simplest):**

```typescript
async function fetchWithCredentials(url: string, init?: RequestInit): Promise<Response> {
  const opts: RequestInit = { ...init, credentials: 'include' }
  let res = await fetch(url, opts)
  if (res.status === 401) {
    const refreshRes = await fetch(`${API_BASE}/api/v1/auth/refresh`, {
      method: 'POST',
      credentials: 'include',
    })
    if (!refreshRes.ok) throw new Error('Session expired')
    // Only retry safe/idempotent methods. Mutations and uploads must be
    // re-issued by the caller. On 401, they get a thrown error that the
    // UI can handle with a re-auth flow.
    const method = (init?.method ?? 'GET').toUpperCase()
    if (method === 'GET' || method === 'HEAD' || method === 'OPTIONS') {
      res = await fetch(url, opts)
    } else {
      throw new Error('Session expired — please re-authenticate')
    }
  }
  return res
}
```

**Option B — clone the request (breaks for FormData):**

The `Request` constructor accepts a `Request` object to clone, but `FormData` streams can't be cloned in all environments. Option A is safer and more explicit.

**Decision: Option A.** Safe methods get transparent retry. Mutations that hit 401 surface the error to the UI, which can trigger a re-auth flow. This is correct behavior — a mutation that fails due to auth expiry shouldn't silently retry with a potentially stale body.

### Files

- `src/api/core.ts` — new: `fetchWithCredentials`, `buildParams`, `fetchApi`, `fetchCrud`, `fetchExchange`
- `src/api/types.ts` — new: shared types (`StatsFilter`, `KellyInputs`, `SetupTagEntry`, etc.)
- `src/api/analytics.ts` — new: analytics types + functions
- `src/api/trades.ts` — new: trade types + functions
- `src/api/journal.ts` — new: journal types + functions
- `src/api/coach.ts` — new: coach types + functions
- `src/api/dignitas.ts` — new: dignitas types + functions
- `src/api/exchange.ts` — new: exchange types + functions
- `src/api/risk.ts` — new: risk snapshot types + functions
- `src/api/client.ts` — modified: becomes barrel re-exports (target: < 200 lines)
- `src/pages/Account.tsx` — modified: remove unused `IdentitySettings` import

### Dependencies Added

None. Filesystem-only refactor — no new npm packages.

---

## Acceptance Criteria

- [ ] `src/api/client.ts` is under 200 lines and contains only barrel re-exports
- [ ] 8 domain modules exist in `src/api/` with no import cycles
- [ ] `fetchWithCredentials` does not retry POST/PUT/DELETE/PATCH after token refresh
- [ ] `bun run build` exits 0
- [ ] `npx tsc --noEmit` exits 0 (existing TS6133 in `IdentitySettings.tsx` should be addressed in CLN-03)
- [ ] Performance budget check passes: `bun run build:check` — all chunks ≤ 250 KB gzip

---

## Risks

1. **Import path breakage** — moving functions between files could break `import` statements in 30+ component files. Mitigation: barrel re-export in `client.ts` means zero import changes are required. CP-1 through CP-3 each end with `bun run build` to catch breakage.
2. **Circular dependency** — if a domain module accidentally imports from another domain module via `client.ts`. Mitigation: domain modules import only from `./core` and `./types`, never from `./client` or other domain modules. This is enforceable by code review.
3. **FormData retry behavior change** — callers that previously relied on transparent retry for `POST` with `JSON.stringify` body will now get an error on 401. Mitigation: this is correct behavior. `JSON.stringify` retry was a happy accident, not an intentional design. Callers should handle the 401 error surface.

---

## Completion Signal

This spec is complete when:
1. `src/api/client.ts` < 200 lines, pure re-exports
2. 8 domain modules created and importable
3. `fetchWithCredentials` retry restricted to safe methods
4. `bun run build && bun run build:check` passes
5. Code committed to master with message `refactor: decompose client.ts into domain modules + fix FormData retry`
