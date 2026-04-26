# Specification: Journal Cold-Load Diet + SWR Cache

**Spec ID:** PERF-01-cold-load-and-swr
**Date:** 2026-04-25
**Status:** Draft
**Class:** Refactor / Frontend Performance
**Priority:** P1 — Two compounding wins (cold-load TTI and within-session nav latency) gate the perceived quality of the Desk UI; both blocked on a single eager wallet import and the absence of any client cache.
**Depends on:** None
**Series:** PERF-01 (single spec; Phases 3–4 from PERFORMANCE.md alpha — batch endpoint, service worker — explicitly out of scope here, may become PERF-02/03)

---

## Problem Statement

The Desk SPA (`testudo-journal`) has two concrete, measurable performance ceilings, both fixable purely on the frontend without backend or infra changes.

**Cold load is fat.** `src/index.tsx:7` imports `./config/wallet`, which top-level-evaluates `createAppKit()` from `@reown/appkit` plus `EthersAdapter`, `SolanaAdapter`, `ethers`, `bs58`, and the `mainnet/arbitrum/base/polygon/solana` network configs. This block runs on every cold visit to `/desk/`, including users who never connect a wallet in the session. Estimated cost: 250–400 KB gzipped of main-bundle JavaScript and a parse/eval pause that delays Time-To-Interactive by 0.5–1 s on desktop and 2–4 s on mid-tier mobile. The wallet code is genuinely needed only when the user clicks "Connect" on the Account page or wallet button — i.e. interaction-bound, not boot-bound.

**Within-session navigation re-fetches everything.** The API client (`src/api/client.ts`) has no caching layer. Navigating Overview → Trades → Journal → back to Overview re-runs all 7 analytics fan-out calls every time. Each round-trip is 40–200 ms; the total feels laggy on every nav. Standard SPA practice is stale-while-revalidate: render last-known data immediately, revalidate in the background, patch reactively. We have none of that.

This spec adopts two compounding micro-optimizations from the @brotzky performance playbook (`PERFORMANCE.md` §1, §3): defer the heavy interaction-bound dependency off the cold path, and add a stale-while-revalidate cache hydrated from `localStorage`. Together these turn the Desk from "decent SPA" into "feels instant" without touching the Rust backend, the extension, or any infra.

---

## User Stories

- **As a returning Desk user**, I want navigating between Overview / Trades / Journal / Coach to feel instant, so the app stops feeling like it's reloading itself every time I click a tab.
- **As a first-time visitor on mobile**, I want the Desk to be interactive within ~2 seconds, not 5+ seconds, so the experience matches the visual quality.
- **As a logged-in user who hasn't connected a wallet yet**, I don't want to pay for the WalletConnect / ethers / solana bundle on every page visit, since I may never use it in this session.
- **As a developer**, I want a single, ergonomic `useResource(key, fetcher)` primitive so adding new cached endpoints is one line, not a re-invention of caching per-component.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `@reown/appkit`, `@reown/appkit-adapter-ethers`, `@reown/appkit-adapter-solana`, `ethers`, `bs58` MUST NOT appear in the main entry chunk emitted by `vite build` | High | Bundle |
| FR-2 | Wallet stack initializes only on first call to `connectWallet()` (or equivalent), triggered by user gesture (button click) — never at module-evaluation time | High | Wallet |
| FR-3 | `vite build` enforces a budget: main entry chunk ≤ 250 KB gzipped; build fails if exceeded | High | CI |
| FR-4 | A `useCachedResource<T>(key, fetcher, opts)` Solid primitive provides: in-memory cache, optional `localStorage` hydration, configurable stale TTL, render-stale + background revalidate | High | Cache |
| FR-5 | All 7 analytics endpoints in `src/api/client.ts` (`fetchOverview`, `fetchEquityCurve`, `fetchDailyPnl`, `fetchSymbolBreakdown`, `fetchSetupBreakdown`, `fetchDurationProfit`, `fetchReturnDistribution`, `fetchTimeDistribution`) are accessible through `useCachedResource` with a 30-second stale TTL | High | Cache |
| FR-6 | `fetchFilterOptions`, `fetchTags`, `fetchUserSetupTags` use a 5-minute stale TTL | Medium | Cache |
| FR-7 | Cache invalidation API: mutations (e.g. `addNote`, `addTag`, `deleteTag`) invalidate affected keys so the next read refetches; no manual reload required | High | Cache |
| FR-8 | `localStorage` hydration is namespaced by user identity (handle or wallet), so logging out / switching identity does not show another user's stale data | High | Cache |
| FR-9 | Hover/touchstart on primary nav links (`/trades`, `/journal`, `/coach`, `/dignitas`) prefetches both the route chunk (`import()`) and that route's primary data | Medium | Prefetch |
| FR-10 | App boot inserts `<link rel="preconnect">` to the API host and the WS host before the main bundle parses | Medium | Prefetch |
| FR-11 | Cache layer surfaces a stale-indicator boolean per resource so UI can optionally show a subtle "revalidating…" cue (no UI work required in this spec; signal must exist) | Low | Cache |
| FR-12 | Public profile route (`/desk/d/:handle`) bypasses cache hydration from `localStorage` (privacy: don't leak one user's cached aggregates into another user's session) | High | Cache |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Wallet defer: replace eager `createAppKit()` with lazy `connectWallet()` factory; wire the Connect button to call it; remove the side-effect import from `index.tsx`. Ship + measure. | FR-1, FR-2 — bundle inspection + manual click flow + Lighthouse cold-load delta |
| CP-2 | Bundle-budget guardrail: add `rollup-plugin-visualizer` (dev-only) and a CI check that asserts main chunk gz size ≤ 250 KB. | FR-3 — `bun run build` fails if budget exceeded |
| CP-3 | `useCachedResource` primitive + integrate the 7 analytics endpoints. Memory cache only; render-stale + revalidate; mutation invalidation API. | FR-4, FR-5, FR-7, FR-11 — unit tests for cache hit/miss/stale/invalidate |
| CP-4 | `localStorage` hydration tier with identity-namespaced keys + 5-min tier for filter-options/tags. | FR-6, FR-8, FR-12 — tests for cross-identity isolation + TTL expiry |
| CP-5 | Hover/touchstart prefetch on nav + `<link rel="preconnect">` injection. | FR-9, FR-10 — DevTools Network panel shows prefetch fires before click |

Each checkpoint is independently committable. CP-1 and CP-2 are pure deletes/diet (low risk). CP-3 is the largest behavioural change. CP-4 adds storage I/O. CP-5 is polish.

### Wallet Deferral (CP-1)

Current shape (`src/config/wallet.ts:11`):

```ts
export const appKit = createAppKit({ adapters: [ethersAdapter, solanaAdapter], ... })
```

This runs at module load. Replace with a lazy singleton:

```ts
// src/config/wallet.ts
let appKitPromise: Promise<AppKit> | null = null

export async function connectWallet(): Promise<AppKit> {
  if (appKitPromise) return appKitPromise
  appKitPromise = (async () => {
    const [{ createAppKit }, { EthersAdapter }, { SolanaAdapter }, networks] = await Promise.all([
      import('@reown/appkit'),
      import('@reown/appkit-adapter-ethers'),
      import('@reown/appkit-adapter-solana'),
      import('@reown/appkit/networks'),
    ])
    return createAppKit({
      adapters: [new EthersAdapter(), new SolanaAdapter()],
      networks: [networks.mainnet, networks.arbitrum, networks.base, networks.polygon, networks.solana],
      projectId: import.meta.env.VITE_WALLETCONNECT_PROJECT_ID || '',
      metadata: { name: 'Testudo', description: '…', url: window.location.origin, icons: ['/testudo-icon.png'] },
      themeMode: 'dark',
    })
  })()
  return appKitPromise
}
```

Remove `import './config/wallet'` from `src/index.tsx`. Audit `AuthContext` and any wallet-button components for top-level imports of `appKit` — they must call `connectWallet()` from a click handler instead. Manual `vite.config.ts` chunk `vendor-wallet` already isolates these dependencies; with the dynamic import they auto-split into a chunk loaded on demand.

### `useCachedResource` Primitive (CP-3, CP-4)

Solid-native, ~80 lines. Wraps `createResource` so existing call sites change minimally.

```ts
// src/lib/cache.ts
type CacheEntry<T> = { data: T; updatedAt: number }
const memCache = new Map<string, CacheEntry<unknown>>()

export interface CacheOpts {
  staleMs?: number          // default 30_000
  persist?: boolean         // default false (memory only)
  identity?: string | null  // namespace for localStorage; null disables persist
}

export function useCachedResource<T>(
  key: () => string | undefined,
  fetcher: (k: string) => Promise<T>,
  opts: CacheOpts = {},
) {
  // 1. on first read with key K: check memCache; if miss, check localStorage (if persist+identity); else fetch
  // 2. if hit but age > staleMs: return data immediately, kick off background refetch, write back to caches
  // 3. expose isStale() signal for FR-11
}

export function invalidate(keyPrefix: string) { /* drop matching memCache + localStorage entries */ }
```

`localStorage` keys: `testudo:cache:{identity}:{key}`. The `identity` slot defends FR-8 / FR-12. On logout, `AuthContext` calls `clearCacheForIdentity(prevIdentity)`.

### Analytics Migration (CP-3)

Each call site in `src/components/Overview.tsx`, `src/pages/Trades.tsx`, etc. currently uses `createResource(filters, fetchOverview)`. Replace with:

```ts
const overview = useCachedResource(
  () => `overview:${stableHash(filters())}`,
  () => fetchOverview(filters()),
  { staleMs: 30_000, persist: true, identity: auth.identity() },
)
```

`stableHash` is a tiny deterministic JSON canonicalizer (5 lines) so equivalent filter objects produce identical keys.

### Mutation Invalidation (CP-3, FR-7)

After `updateNotes`, `addTag`, `deleteTag`, `createEntry`, `updateEntry` — call `invalidate('trades:')` / `invalidate('entries:')` / `invalidate('overview:')` as appropriate. Concrete map produced during CP-3 by reading `client.ts`.

### Prefetch (CP-5)

Tiny `<NavLink>` wrapper that on `mouseenter`/`touchstart`:
1. Calls the lazy-route's `import()` (idempotent — Solid caches the module).
2. Calls a route-registered prefetch hook (e.g. Trades pre-fires `fetchTrades(defaultParams)` into the cache).

`<link rel="preconnect" href={API_BASE} />` and `<link rel="preconnect" href={WS_BASE} />` injected from `Layout` head on mount.

### Paved Roads

- `vite.config.ts` already declares `vendor-wallet` and `vendor-echarts` manual chunks — dynamic-import on `@reown/appkit` will populate `vendor-wallet` lazily without further config.
- ECharts is already correctly tree-shaken via `src/lib/echarts-setup.ts` (`echarts/core` + explicit components). Type-only `import type { EChartsOption } from 'echarts'` in chart components is compile-time only and does not affect runtime bundle. **No work needed there** — earlier survey confirmed this; #5 from the PERFORMANCE.md analysis is already paved.
- `marked` + `dompurify` are used only inside `Coach`/`Journal` pages, which are already lazy routes — they are not in the cold path.
- `fetchWithCredentials` (`client.ts:52`) keeps its 401-refresh-retry semantics — the cache wraps around it, doesn't replace it.

### Files

**New:**
- `testudo-journal/src/lib/cache.ts` — `useCachedResource`, `invalidate`, `clearCacheForIdentity`, `stableHash`
- `testudo-journal/src/lib/cache.test.ts` — unit tests (hit, miss, stale revalidation, identity isolation, TTL expiry, invalidation)
- `testudo-journal/src/components/NavLink.tsx` — prefetch-on-hover wrapper
- `testudo-journal/scripts/check-bundle-budget.ts` — CI script enforcing FR-3

**Modified:**
- `testudo-journal/src/config/wallet.ts` — lazy `connectWallet()` factory; remove top-level `createAppKit()`
- `testudo-journal/src/index.tsx` — remove `import './config/wallet'`; add `<link rel="preconnect">` injection
- `testudo-journal/src/context/AuthContext.tsx` — call `connectWallet()` on user gesture only; call `clearCacheForIdentity()` on logout
- `testudo-journal/src/components/Overview.tsx`, `src/pages/Trades.tsx`, `src/pages/Journal.tsx`, `src/pages/Coach.tsx`, `src/pages/Dignitas.tsx`, `src/pages/Account.tsx` — migrate `createResource` → `useCachedResource` for analytics + mutation-invalidation calls
- `testudo-journal/src/components/Layout.tsx` — preconnect + integrate `NavLink`
- `testudo-journal/vite.config.ts` — confirm `vendor-wallet` chunk still triggers via dynamic imports; add bundle-visualizer plugin (dev-only)
- `testudo-journal/package.json` — add `rollup-plugin-visualizer` dev dep, add `build:check` script that runs build + budget check
- `testudo-journal/CLAUDE.md` — note the cache primitive and the wallet deferral pattern

### Dependencies Added

- `rollup-plugin-visualizer` (devDependency) — bundle inspection for budget enforcement

### Out of Scope

- Service Worker (Phase 4 of PERFORMANCE.md). Defer to PERF-02.
- Backend `/analytics/batch` endpoint (Phase 3). Defer to PERF-02.
- Extension WS prefetch (PERFORMANCE.md §6 / item #8). Separate spec; lives in extension repo.
- Public-profile SEO/CLS work — already partially covered by `_headers` + recent radar fix.
- Switching from `marked` to a smaller markdown renderer — already off the cold path; not worth the risk.

---

## Acceptance Criteria

- [ ] `bun run build` output: main entry chunk reports ≤ 250 KB gzipped (FR-1, FR-3)
- [ ] `vendor-wallet` chunk only fetched after the user clicks "Connect Wallet"; verified in DevTools Network panel (FR-2)
- [ ] Lighthouse cold-load on `/desk/` (mobile preset, throttled): TTI improves by ≥ 30% versus pre-spec baseline; baseline recorded in CP-1 PR description (FR-1, FR-2)
- [ ] Cache unit tests cover: cold miss, warm hit, stale-revalidate, identity isolation, TTL expiry, manual invalidation (FR-4, FR-5, FR-6, FR-7, FR-8)
- [ ] Cross-identity test: log in as user A, navigate Desk, log out, log in as user B — user B sees no user A data flash (FR-8, FR-12)
- [ ] Manual test: Overview → Trades → Overview within 30 s shows cached Overview rendered in <50 ms (Performance panel timing, FR-5)
- [ ] Mutation invalidation test: add a note via `updateNotes`, navigate away and back — note is present (FR-7)
- [ ] Hover any nav link in DevTools — see route-chunk + data fetch fire before click (FR-9)
- [ ] View page source — `<link rel="preconnect">` for API + WS hosts present (FR-10)
- [ ] `cd testudo-journal && bun run typecheck && bun run build` passes (no `bun run build` in extension per repo memory)
- [ ] Spec-defined budget script fails the build when budget is exceeded (verified by temporarily lowering budget in a test PR)

---

## Risks

1. **Cache staleness bugs.** Mutation paths missed during CP-3 cause "I added a tag but it doesn't show" reports. *Mitigation:* exhaustively map every mutation in `client.ts` to its invalidation key in CP-3 commit; document the map in `src/lib/cache.ts` header; add an integration test that asserts each mutation invalidates the expected key.
2. **Identity-leak from cache.** Caching aggregates per-user is sensitive — a wrong namespace key could leak one user's PnL summary to another. *Mitigation:* FR-8 + FR-12, plus `clearCacheForIdentity()` on logout, plus unit test asserting a stored entry under identity-A is unreadable when identity is B.
3. **Wallet deferral breaks existing sign-in flows.** `AuthContext` currently assumes `appKit` exists synchronously. *Mitigation:* CP-1 audit phase explicitly enumerates every consumer of `appKit`; convert each to await `connectWallet()`; manually exercise wallet connect + sign + disconnect on Arbitrum + Solana before merging CP-1.
4. **`localStorage` quota or cross-tab race.** Multi-MB cache in `localStorage` can throw `QuotaExceededError`. *Mitigation:* cap each persisted entry at 64 KB serialized; on quota error, evict oldest by `updatedAt` and retry once; on second failure, fall back to memory-only and `console.warn`.
5. **Lighthouse improvement under-delivers.** TTI gain may be smaller than estimated if Reown is partially deferred internally. *Mitigation:* CP-1 records before/after numbers in the PR. If gain <20%, open a follow-up to investigate the next biggest top-of-bundle culprit (use the visualizer added in CP-2). Do not block the spec on hitting an exact number — the deferral is correct regardless.
6. **Prefetch wastes bandwidth.** Aggressive hover-prefetch on metered connections is rude. *Mitigation:* skip prefetch when `navigator.connection?.saveData === true` or `navigator.connection?.effectiveType === '2g' | 'slow-2g'`.

---

## Completion Signal

This spec is complete when:
1. CP-1 through CP-5 are merged to master, each as its own commit.
2. All acceptance criteria are checked.
3. `cd testudo-journal && bun run typecheck && bun run build` passes; bundle-budget script passes.
4. PERFORMANCE.md is updated (or a `LEARNINGS.md` is added under `.specify/specs/PERF-01-cold-load-and-swr/`) with the actual measured before/after Lighthouse numbers.
5. Spec is archived per repo conventions; `MEMORY.md` is updated with the cache primitive's location.
