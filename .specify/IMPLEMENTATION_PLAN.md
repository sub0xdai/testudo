# Implementation Plan — PERF-01 Journal Cold-Load Diet + SWR Cache

**Status: COMPLETE — archived to `.specify/spec-archive/PERF-01-cold-load-and-swr/`**
**Completed:** 2026-04-26

**Spec:** `.specify/spec-archive/PERF-01-cold-load-and-swr/spec.md`
**Depends on:** None (frontend-only, single-package: `testudo-journal/`)
**Strategy:** Five vertical checkpoints, each independently committable
(CP-1..CP-5). CP-1 is the highest-leverage, lowest-risk delete (defer
the wallet bundle off the cold path). CP-2 locks that win in with a
budget guardrail. CP-3 introduces the cache primitive — the largest
behavioural change and the only one that can plausibly leak bugs into
existing analytics flows. CP-4 layers persistence + identity-namespacing
on top. CP-5 is polish (prefetch + preconnect). Land them in that order.

---

## Discoveries

- **Wallet eager-import is multi-site.** `index.tsx:7` is the obvious
  cold-path import, but the real refactor surface is
  `src/context/AuthContext.tsx` — AuthProvider calls `appKit.subscribeProviders`
  (line 43) and `appKit.subscribeAccount` (line 244) **synchronously at
  provider construction** (i.e. on every Desk page load). It also calls
  `appKit.disconnect()` from `checkSession()` on lines 67 and 71 even
  for users who arrive logged-out. Any "lazy `connectWallet()`" plan
  that only edits `index.tsx` is incomplete. Full call-site map:

  | File | Line(s) | Usage | Migration |
  |------|---------|-------|-----------|
  | `index.tsx` | 7 | Side-effect import | **Delete** |
  | `context/AuthContext.tsx` | 43 (`subscribeProviders`) | Sync at init | **Move** behind first-gesture init |
  | `context/AuthContext.tsx` | 67, 71, 140, 215, 282 (`disconnect`) | Cleanup paths | **Guard** — skip when not yet loaded |
  | `context/AuthContext.tsx` | 105 (`getChainId`) | Inside `runSiwe` | Already gated by `userInitiatedConnect` → safe (wallet must be loaded by then) |
  | `context/AuthContext.tsx` | 244 (`subscribeAccount`) | Sync at init | **Move** behind first-gesture init |
  | `context/AuthContext.tsx` | 259 (`getCaipNetwork`) | Inside subscribeAccount | Co-moves with 244 |
  | `context/AuthContext.tsx` | 276 (`open`) | `connectWallet()` action | Already user-gesture; await load |
  | `components/Layout.tsx` | 4 | Bare import, no usage | **Delete** (dead import) |
  | `components/account/WalletConnectFlow.tsx` | 79 (`getAddress`), 273 (`open`), 291 (`disconnect`) | All inside the Account → Add Exchange flow | User-gesture-bound; await load |

  Net move: introduce `loadWallet(): Promise<AppKit>` (lazy singleton),
  introduce `attachWalletListeners(appKit)` (one-shot, idempotent) called
  from inside `connectWallet()` after the kit resolves. Every other
  consumer awaits `loadWallet()` from a click handler. `checkSession()`
  never calls `disconnect()` unconditionally — it only calls it if a kit
  has already been loaded this session.

- **`vendor-wallet` chunk in `vite.config.ts:21` is incomplete.** It lists
  `@reown/appkit`, `@reown/appkit-adapter-ethers`, `ethers` — but not
  `@reown/appkit-adapter-solana` (in `package.json:14`) and not the
  `@reown/appkit/networks` import. Once dynamic-import lands those will
  auto-split into a chunk anyway, but for predictable chunk-naming we
  add them to the manualChunks list explicitly (CP-1). Note: spec says
  `bs58` but the real Solana base58 dep is `@scure/base` (used in
  `AuthContext.tsx:2`) — `bs58` is in package.json but not actually
  imported. Either way, both are off the cold path once wallet is lazy.

- **77 `createResource` sites repo-wide.** Spec scope is the 7 analytics
  endpoints + filter-options + tags + setup-tags. Concrete migration
  targets:

  | File:line | Fetcher | TTL tier |
  |-----------|---------|----------|
  | `components/Overview.tsx:39` | `fetchOverview` | 30s |
  | `components/Overview.tsx:41` | `fetchEquityCurve` | 30s |
  | `components/charts/DailyPnl.tsx:11` | `fetchDailyPnl` | 30s |
  | `components/charts/PnlCalendar.tsx:53` | `fetchDailyPnl` (month) | 30s |
  | `components/charts/PnlTreemap.tsx:11` | `fetchSymbolBreakdown` | 30s |
  | `components/charts/DurationScatter.tsx:11` | `fetchDurationProfit` | 30s |
  | `components/charts/ReturnHistogram.tsx:11` | `fetchReturnDistribution` | 30s |
  | `components/charts/TimeHeatmap.tsx:14` | `fetchTimeDistribution` | 30s |
  | `pages/Coach.tsx:22` | `fetchOverview` (no filters) | 30s |
  | `components/PageSubHeader.tsx:55` | `fetchFilterOptions` | 5min |
  | `components/journal/JournalTimeline.tsx:104` | `fetchTags` | 5min |
  | `components/journal/EntryEditor.tsx:69` | `fetchTags` | 5min |
  | `components/trades/TradeDetail.tsx:54` | `fetchTags` | 5min |

  Plus `fetchSetupBreakdown` and `fetchUserSetupTags` exist in
  `client.ts` but no current callers; the migration prep on `client.ts`
  exposes them through the cache layer regardless (FR-5, FR-6) so future
  callers default into the cache.

- **No `typecheck` script today.** `package.json:6-10` has only
  `dev`, `build`, `preview`. Spec acceptance criteria explicitly call
  `bun run typecheck && bun run build`. CP-1 adds
  `"typecheck": "tsc --noEmit"` (zero new dep — `typescript` already
  in devDeps).

- **No `bs58` runtime use.** `bs58` is in `package.json:16` but not
  actually imported. `@scure/base`'s `base58` is what `AuthContext`
  uses (`AuthContext.tsx:2`). Out-of-scope for this spec — flag for a
  future cleanup, do not delete inside CP-1 to avoid scope-creep.

- **PublicProfile is a Layout-bypass route.** `components/Layout.tsx:357-358`
  detects `/d/:handle` and renders children without the auth shell. So
  `PublicProfile.tsx:12` (`createResource(handle, fetchPublicProfile)`)
  runs without `useAuth`. Identity-namespacing for cache (FR-12)
  therefore needs an explicit "no identity → no persist" guard rather
  than relying on `auth.user()` being absent in a Layout-bypass route.
  Concretely: `useCachedResource(..., { identity: auth.user()?.id ?? null,
  persist: !isPublicProfileRoute })` and the cache layer treats
  `identity: null` as "memory only". Cleaner alternative: have
  `PublicProfile.tsx` keep using bare `createResource` (no cache layer
  at all) — it's a single endpoint per handle, no compound benefit.
  **Resolution:** PublicProfile stays on plain `createResource`, no
  cache wrap. FR-12 is satisfied structurally (the route never opts in).

- **The 30s and 5min TTLs in the spec are read-modes, not stale-times.**
  Re-read: "render-stale + revalidate; 30-second stale TTL". So when
  age ≥ 30s the cache returns the stale entry **and** triggers a
  background refetch. There is no "expire and refuse to serve" mode —
  expiry just means "definitely refetch in the background". This matters
  because it means `localStorage` entries never need explicit eviction
  by TTL (they only need eviction by quota or by identity-change). TTL
  is purely a freshness flag for the revalidator.

- **Mutation → invalidation map.** Reading `client.ts` cover-to-cover:

  | Mutation (file:line) | Invalidates (key prefix) |
  |----------------------|--------------------------|
  | `updateTradeNotes` (`client.ts:314`) | `trades:`, `trade-detail:` |
  | `addTradeTags` (`client.ts:321`) | `trades:`, `trade-detail:`, `tags:` (count rises) |
  | `removeTradeTag` (`client.ts:328`) | `trades:`, `trade-detail:` |
  | `createTag` (`client.ts:380`) | `tags:` |
  | `updateTag` (`client.ts:387`) | `tags:`, `trades:` (display name) |
  | `deleteTag` (`client.ts:394`) | `tags:`, `trades:` |
  | `createEntry` (`client.ts:350`) | `entries:`, `journal-timeline:` |
  | `updateEntry` (`client.ts:363`) | `entries:` |
  | `deleteEntry` (`client.ts:374`) | `entries:` |
  | `saveDraftNotes` (`client.ts:475`) | `draft:{groupId}` |
  | `claimHandle` / `releaseHandle` / `patchVisibility` / `updateBio` | `identity:`, `public-profile:` |
  | `setCoachPreference` / `markCoachViewed` / `dismissCoachBanner` | `coach-latest:`, `coach-archive:` |
  | `patchDignitasPreference` | `dignitas-me:` |

  Since CP-3's scope is the 7 analytics + filter-options + tags +
  setup-tags, the live invalidation work in CP-3 is narrow:
  `addTag/createTag/updateTag/deleteTag` → `invalidate('tags:')`. The
  larger map above is documented for future-spec scope; no need to wire
  every entry now — but the comment-block in `lib/cache.ts` lists the
  full map so future migrations don't re-derive it.

- **`identity` for namespacing = `auth.user()?.id`.** Wallet address is
  unstable (user can switch wallets and re-claim a UUID); the backend
  user UUID is the durable per-account identity. Use that, not the
  wallet address, as the `localStorage` key namespace.

- **No service worker exists today.** Service worker is explicitly out
  of scope (Phase 4 → PERF-02). Just confirming `localStorage` is the
  only persistence layer this spec touches.

- **Existing test infra.** `vitest@3.2.4` + `jsdom@29.0.1` already in
  `devDependencies`. `src/api/client.test.ts` is the existing pattern
  for test files — colocated `.test.ts` next to source. `lib/cache.test.ts`
  follows that convention.

- **`saveData` / `effectiveType` API surface.** `navigator.connection` is
  Chromium-only; Safari/Firefox return `undefined`. The data-saver guard
  in CP-5 is best-effort: `const conn = (navigator as any).connection;
  const slow = conn?.saveData === true || /^(2g|slow-2g)$/i.test(conn?.effectiveType ?? ''); if (slow) return;`
  Safari/Firefox always prefetch — acceptable per spec ("rude to metered
  connections" not "must skip on metered"; Chromium gives us the signal).

---

## Tasks

### CP-1 — Wallet defer (FR-1, FR-2)

- [x] **T1** — `package.json`: add `"typecheck": "tsc --noEmit"` script.
  No new dep (typescript already devDep). One-line diff.
  *Complexity: trivial.*

- [x] **T2** — Rewrite `src/config/wallet.ts`:
  - Remove top-level `createAppKit({...})` call.
  - Export `loadWallet(): Promise<AppKit>` lazy-singleton:
    ```ts
    let walletPromise: Promise<AppKit> | null = null
    export function loadWallet(): Promise<AppKit> {
      if (walletPromise) return walletPromise
      walletPromise = (async () => {
        const [{ createAppKit }, { EthersAdapter }, { SolanaAdapter }, networks] =
          await Promise.all([
            import('@reown/appkit'),
            import('@reown/appkit-adapter-ethers'),
            import('@reown/appkit-adapter-solana'),
            import('@reown/appkit/networks'),
          ])
        return createAppKit({
          adapters: [new EthersAdapter(), new SolanaAdapter()],
          networks: [networks.mainnet, networks.arbitrum, networks.base,
                     networks.polygon, networks.solana],
          projectId: import.meta.env.VITE_WALLETCONNECT_PROJECT_ID || '',
          metadata: {
            name: 'Testudo',
            description: 'Automated risk management for crypto trading',
            url: window.location.origin,
            icons: ['/testudo-icon.png'],
          },
          themeMode: 'dark',
        })
      })()
      return walletPromise
    }
    export function isWalletLoaded(): boolean { return walletPromise !== null }
    ```
  - Remove the `appKit` named export entirely (force compile errors at
    every consumer site so the migration is exhaustive — see T3..T5).
  *Complexity: medium.*

- [x] **T3** — Refactor `src/context/AuthContext.tsx`:
  - Replace `import { appKit } from '../config/wallet'` with
    `import { loadWallet, isWalletLoaded } from '../config/wallet'`.
  - Move `subscribeProviders` + `subscribeAccount` setup into a new
    helper `attachWalletListeners(kit: AppKit)` that returns
    `() => unsubscribe()` — called once after the first `loadWallet()`
    resolves; idempotency guarded by a `let listenersAttached = false`
    flag.
  - In `checkSession()`: replace bare `appKit.disconnect()` with
    `if (isWalletLoaded()) (await loadWallet()).disconnect()`. On a cold,
    logged-out visit `isWalletLoaded()` is `false` → no wallet bundle
    loaded.
  - In `connectWallet()`: become `async () => { setSiweError(null);
    userInitiatedConnect = true; const kit = await loadWallet();
    if (!listenersAttached) { attachWalletListeners(kit);
    listenersAttached = true; } kit.open(); }`.
  - In `runSiwe`/`runSiws` `appKit.disconnect()` calls (lines 140, 215):
    replace with `(await loadWallet()).disconnect()` — at this point the
    kit is guaranteed loaded since the SIWE/SIWS path is only reached
    via the subscribeAccount listener which only attaches after
    loadWallet resolves.
  - Same for `logout()` and the chainId fetch on line 105.
  - Keep `onCleanup` calling the unsub returned by `attachWalletListeners`,
    but only if listeners were attached.
  *Complexity: medium — multi-call-site refactor with subtle async ordering.*

- [x] **T4** — Refactor `src/components/Layout.tsx`:
  - Delete the unused `import { appKit } from '../config/wallet'` on
    line 4. (Audited — no usages in the file.)
  *Complexity: trivial.*

- [x] **T5** — Refactor `src/components/account/WalletConnectFlow.tsx`:
  - Replace `import { appKit } from '../../config/wallet'` with
    `import { loadWallet } from '../../config/wallet'`.
  - Convert the synchronous `appKit.getAddress()` (line 79) into an
    async helper or a memo that waits for the kit:
    `const [walletAddress] = createResource(async () => { const k = await loadWallet(); return k.getAddress() })`.
    The component is already inside the Account → Add Exchange flow —
    user has reached this view by clicking through, so loading wallet
    here is acceptable and on-demand.
  - Replace `appKit.open()` (line 273) with
    `async () => { (await loadWallet()).open() }`.
  - Replace `appKit.disconnect()` (line 291) similarly.
  *Complexity: simple.*

- [x] **T6** — Update `vite.config.ts` `manualChunks.vendor-wallet`:
  add `@reown/appkit-adapter-solana` and `@reown/appkit/networks` to
  the chunk list so the dynamically-imported pieces stay grouped under
  one chunk name. (Without this, Rollup will create one auto-named
  chunk per dynamic import — still correct, just less inspectable.)
  *Complexity: trivial.*

- [x] **T7** — Delete `import './config/wallet'` from
  `src/index.tsx:7`. *Complexity: trivial.*

- [x] **T8** — `cd testudo-journal && bun run typecheck && bun run build`
  → record main entry chunk gzipped size + `vendor-wallet` chunk
  presence/size from the build output. Manually click "Connect" in a
  browser, verify in DevTools Network tab that `vendor-wallet*.js` only
  fires after the click. Record the cold-load Lighthouse TTI (mobile
  preset) before and after as a baseline for the CP-1 commit message
  (per acceptance criteria). *Complexity: simple — verification step.*

### CP-2 — Bundle-budget guardrail (FR-3)

- [x] **T9** — Add `rollup-plugin-visualizer` as devDependency. Wire
  into `vite.config.ts` plugins array, gated by `process.env.ANALYZE === '1'`
  so the default `bun run build` doesn't open the visualizer browser
  tab in CI. *Complexity: trivial.*

- [x] **T10** — Create `scripts/check-bundle-budget.ts`:
  - Read `dist/.vite/manifest.json` (or fallback to globbing
    `dist/assets/index-*.js` — manifest is more robust).
  - For each entry chunk, compute gzipped size via Node's `zlib.gzipSync`
    on `readFileSync` of the chunk file.
  - Threshold: `MAIN_ENTRY_GZ_BUDGET = 250 * 1024` (FR-3).
  - If any entry exceeds budget, print a table (chunk → size → over-by)
    and `process.exit(1)`. Otherwise print "OK" with sizes.
  - Use plain Node (no extra deps): `zlib`, `fs`, `path`, `process`.
  *Complexity: simple.*

- [x] **T11** — Add `package.json` script `"build:check": "vite build && tsx scripts/check-bundle-budget.ts"`.
  Add `tsx` to devDependencies (the script is a one-off, ESM/TS-friendly
  runner; alternative is to write the script as plain `.mjs` and skip
  the dep — **prefer plain `.mjs`** to keep dep count low. Update the
  script filename + path accordingly: `scripts/check-bundle-budget.mjs`).
  *Complexity: trivial.*

- [x] **T12** — Document the budget contract in
  `testudo-journal/CLAUDE.md`: "Main entry chunk gzipped budget: 250KB
  (FR-3 PERF-01). Run `bun run build:check` after any cold-path-touching
  PR." *Complexity: trivial.*

- [x] **T13** — Verify FR-3 by temporarily lowering
  `MAIN_ENTRY_GZ_BUDGET` to a number below current size, confirming
  `bun run build:check` exits non-zero, then revert.
  *Complexity: trivial.*

### CP-3 — `useCachedResource` primitive + analytics integration (FR-4, FR-5, FR-7, FR-11)

- [x] **T14** — New file `src/lib/cache.ts`. Header comment lists the
  full mutation→invalidation map from Discoveries (so future migrations
  don't re-derive). Public API:
  ```ts
  export interface CacheOpts {
    staleMs?: number          // default 30_000
    persist?: boolean         // default false
    identity?: string | null  // null disables persist regardless
  }
  export interface CachedResource<T> {
    (): T | undefined
    loading: () => boolean
    error: () => unknown
    isStale: () => boolean
    refetch: () => void
  }
  export function useCachedResource<T>(
    key: () => string | undefined,
    fetcher: (k: string) => Promise<T>,
    opts?: CacheOpts,
  ): CachedResource<T>

  export function invalidate(keyPrefix: string): void
  export function clearCacheForIdentity(identity: string): void
  export function stableHash(obj: unknown): string  // tiny canonical JSON
  ```
  Implementation notes:
  - In-memory store: `const memCache = new Map<string, { data: unknown,
    updatedAt: number }>()`.
  - Reactive shape: wraps Solid's `createResource` so consumer sites
    keep familiar ergonomics. Returns the `accessor` (which is also the
    callable resource) augmented with `.isStale()` and `.refetch()`.
  - Read flow: (1) compute current key from `key()`; (2) if memCache
    hit and age < staleMs → return data, no fetch; (3) if memCache hit
    and age ≥ staleMs → return data, kick off background fetch, write
    back; (4) if no memCache hit → return `loading: true`, fetch.
    Persist tier deferred to CP-4.
  - `invalidate(prefix)`: drops every memCache entry whose key starts
    with `prefix`. Subsequent reads on those keys → cold miss → fetch.
  - `stableHash(obj)`: 5–10 line canonical JSON — sort object keys
    recursively, omit undefined values, JSON.stringify. Used to derive
    keys from `StatsFilter` etc. (e.g.
    `key: () => 'overview:' + stableHash(filters())`).
  *Complexity: medium.*

- [x] **T15** — `src/lib/cache.test.ts` (vitest, jsdom env via existing
  `vitest.config` — pattern from `src/api/client.test.ts`). Cases:
  - cold miss: first call triggers fetcher, second within staleMs does
    not.
  - stale revalidate: vi.useFakeTimers → advance past staleMs → next
    read returns last data immediately AND triggers fetcher; new data
    written back.
  - mutation invalidation: write entry, call `invalidate(prefix)`, next
    read triggers fetcher.
  - `stableHash` determinism: `stableHash({a:1,b:2}) === stableHash({b:2,a:1})`.
  - `isStale()` flips correctly across the staleMs boundary.
  *Complexity: medium.*

- [x] **T16** — Migrate `components/Overview.tsx`:
  - `createResource(filters, fetchOverview)` →
    `useCachedResource(() => 'overview:' + stableHash(filters()),
    () => fetchOverview(filters()), { staleMs: 30_000 })`. Persist
    deferred to CP-4 — wire `persist: true` flag now (no-op until CP-4
    lands the persistence tier; lets us avoid re-touching call sites).
  - Same for `fetchEquityCurve` line 41.
  *Complexity: simple.*

- [x] **T17** — Migrate the 6 chart components (`components/charts/DailyPnl.tsx`,
  `PnlCalendar.tsx`, `PnlTreemap.tsx`, `DurationScatter.tsx`,
  `ReturnHistogram.tsx`, `TimeHeatmap.tsx`) to `useCachedResource` with
  the appropriate fetcher and `staleMs: 30_000`. Same key shape:
  `'<endpoint>:' + stableHash(filters())`. *Complexity: simple but wide.*

- [x] **T18** — Migrate `pages/Coach.tsx:22` (`fetchOverview({})` no-filter
  call) → `useCachedResource(() => 'overview:{}',
  () => fetchOverview({}), { staleMs: 30_000 })`. *Complexity: trivial.*

- [x] **T19** — Migrate `components/PageSubHeader.tsx:55`
  (`fetchFilterOptions(exchange)`) → `useCachedResource(
  () => 'filter-options:' + (exchange() ?? ''),
  () => fetchFilterOptions(exchange() || undefined),
  { staleMs: 5 * 60_000 })`. *Complexity: trivial.*

- [x] **T20** — Migrate the 3 `fetchTags` sites
  (`components/journal/JournalTimeline.tsx:104`,
  `components/journal/EntryEditor.tsx:69`,
  `components/trades/TradeDetail.tsx:54`) → `useCachedResource(
  () => 'tags:all', fetchTags, { staleMs: 5 * 60_000 })`. With a single
  cache key shared across the 3 components, 2nd-render and 3rd-render
  share the same cache entry — a multiplicative win. *Complexity: simple.*

- [x] **T21** — Wire mutation invalidations (FR-7). In `client.ts`,
  after each mutation (`createTag`, `updateTag`, `deleteTag`,
  `addTradeTags`, `removeTradeTag`), import + call
  `invalidate('tags:')`. **Caveat:** mutating the cache layer from
  inside the API client creates a tight coupling. **Cleaner pattern:**
  mutations stay pure in `client.ts`; each caller invalidates after
  await. **Resolution:** wrap the callers, not the API client. Concrete
  sites: `JournalTimeline.tsx` (tag CRUD UI), `TradeDetail.tsx`
  (`addTradeTags`, `removeTradeTag`). One-line `invalidate('tags:')`
  after each `await`. *Complexity: simple.*

- [x] **T22** — Expose `isStale` indicator (FR-11). No UI work —
  just verify `cache.ts` test confirms `accessor.isStale()` returns
  `true` once age ≥ staleMs. *Complexity: trivial — covered in T15.*

### CP-4 — `localStorage` hydration + identity namespacing (FR-6, FR-8, FR-12)

- [x] **T23** — Extend `src/lib/cache.ts`:
  - Persist tier: when `opts.persist === true && opts.identity !== null`,
    on cache write also `localStorage.setItem('testudo:cache:' + identity
    + ':' + key, JSON.stringify({ data, updatedAt }))`.
  - On cold read: if memCache miss, attempt `localStorage.getItem(...)`
    under the current identity; if hit, hydrate into memCache as a stale
    entry (older than staleMs guaranteed) → triggers immediate
    background revalidate, but UI gets instant render.
  - Quota guard: cap each entry serialized at 64 KB. On
    `QuotaExceededError`, evict the oldest persisted entry under the
    same identity (sorted by `updatedAt`) and retry once. On second
    failure, fall back to memory-only and `console.warn(' [cache]
    localStorage quota exceeded; persistence disabled this session')`.
  - Identity namespacing: persist key always includes identity slot. If
    `opts.identity === null`, persist becomes a no-op for that read.
  - `clearCacheForIdentity(identity)`: iterates `localStorage` keys
    matching `'testudo:cache:' + identity + ':'` prefix, removes each;
    plus drops matching `memCache` entries.
  *Complexity: medium.*

- [x] **T24** — Wire identity into call sites. Pass
  `identity: useAuth().user()?.id ?? null` and `persist: true` to:
  Overview (T16 sites), the 6 charts (T17), Coach (T18), PageSubHeader
  filter-options (T19), JournalTimeline / EntryEditor / TradeDetail tags
  (T20).
  - Public profile (`pages/PublicProfile.tsx:12`): keep on bare
    `createResource` — no cache wrap (FR-12 satisfied structurally,
    documented in CP-1's Discoveries).
  *Complexity: simple — additive opts on existing migrated sites.*

- [x] **T25** — Logout invalidation. In `AuthContext.tsx::logout`:
  capture `previousIdentity = user()?.id` BEFORE `setUser(null)`; after
  setUser, call `clearCacheForIdentity(previousIdentity)`. Also call it
  on the wallet-switch path inside `subscribeAccount` callback (line
  244-256) where `current.wallet_address.toLowerCase() !==
  state.address.toLowerCase()` — that's an effective identity change.
  *Complexity: simple.*

- [x] **T26** — Extend `src/lib/cache.test.ts`:
  - Identity isolation: write entry under `identity: 'A'`, switch read
    to `identity: 'B'`, confirm B reads cold-miss (and does NOT see A's
    data anywhere — neither in memCache nor in `localStorage` lookup).
  - `clearCacheForIdentity('A')`: remove all A's persisted entries,
    confirm B's entries untouched.
  - Quota error: stub `localStorage.setItem` to throw
    `QuotaExceededError` once, confirm graceful degradation (memory-only
    + warn).
  - TTL hydration: write to `localStorage` directly with old
    `updatedAt`, confirm the hydrated read returns data immediately AND
    flips `isStale()` to true AND triggers a background fetch.
  *Complexity: medium.*

### CP-5 — Hover/touchstart prefetch + preconnect (FR-9, FR-10)

- [x] **T27** — `src/components/NavLink.tsx`: thin wrapper around
  `@solidjs/router`'s `<A>` that adds `onMouseEnter` and `onTouchStart`
  listeners. Both call:
  1. The route's lazy module loader (idempotent — Solid's `lazy()`
     internally caches).
  2. A route-registered prefetch hook (optional — if the route is
     registered in a small `route-prefetch` map under
     `src/lib/route-prefetch.ts`, run its data prefetcher).
  Skip prefetch when
  `(navigator as any).connection?.saveData === true ||
  /^(2g|slow-2g)$/i.test((navigator as any).connection?.effectiveType ?? '')`.
  *Complexity: simple.*

- [x] **T28** — `src/lib/route-prefetch.ts`: map route paths to
  prefetcher closures. Register:
  - `/` (Overview) → fire `useCachedResource`-equivalent reads via the
    same key + fetcher used by Overview, so the cache is populated
    before navigation.
  - `/trades` → `fetchTrades({ page: 1, limit: 20 })` + prime tags
    (`tags:all`).
  - `/coach` → `fetchLatestCoachReport()`.
  - `/dignitas` → `fetchDignitasMe()`.
  - `/journal` → `fetchEntries({})` and `tags:all`.
  Each prefetcher writes via `useCachedResource`'s underlying API (or a
  helper `prefetch(key, fetcher)` exported from `lib/cache.ts`) so the
  pre-fired data lands in the same cache slot the destination component
  reads from. *Complexity: medium.*

- [x] **T29** — Replace `<A>` with `<NavLink>` in
  `src/components/Layout.tsx` for the 4 NAV_ITEMS entries (lines 421-432
  desktop and 455-468 mobile). DignitasPill / WalletChip / ExtensionChip
  unchanged. *Complexity: trivial.*

- [x] **T30** — Inject `<link rel="preconnect">` for API + WS hosts
  in `src/index.tsx` (or in a small `setupPreconnect()` helper called
  before `render()`):
  ```ts
  function preconnect(href: string) {
    if (!href) return
    const link = document.createElement('link')
    link.rel = 'preconnect'
    link.href = href
    link.crossOrigin = 'anonymous'
    document.head.appendChild(link)
  }
  preconnect(import.meta.env.VITE_API_URL ?? '')
  preconnect(import.meta.env.VITE_WS_URL ?? '')
  ```
  Inserted before `render()` so the browser fires the preconnect during
  main-bundle parse. *Complexity: simple.*

### CP-6 — Verification

- [x] **T31** — `cd testudo-journal && bun run typecheck && bun run build`
  passes; `bun run build:check` (T11) passes. *Complexity: simple.*

- [x] **T32** — Manual browser verification of each acceptance criterion:
  - DevTools Network: `vendor-wallet*.js` only fetched on Connect click
    (FR-2).
  - Lighthouse mobile cold-load TTI delta vs baseline recorded in CP-1
    (FR-1, FR-2; ≥ 30% target, but ship regardless per spec risk #5).
  - Cross-identity test: log in as A → log out → log in as B; confirm
    no flash of A's analytics (FR-8, FR-12).
  - 30s cache test: Overview → Trades → Overview within 30s; Performance
    panel shows Overview rendered in <50ms (FR-5).
  - Mutation test: add a tag, navigate away/back, tag persists (FR-7).
  - Hover-prefetch: hover any nav link, see route-chunk + data fetch
    fire before click (FR-9).
  - View source: `<link rel="preconnect">` for API + WS hosts present
    (FR-10).
  *Complexity: medium — purely manual.*

- [x] **T33** — Create `.specify/specs/PERF-01-cold-load-and-swr/LEARNINGS.md`
  with: before/after Lighthouse numbers, gotchas (AuthContext sync→async
  refactor, stableHash determinism, navigator.connection cross-browser
  surface), and any deferred-to-PERF-02 follow-ups (service worker,
  batch endpoint). Update root `MEMORY.md` with a 1-liner pointing at
  `src/lib/cache.ts` as the cache primitive home.
  *Complexity: trivial.*

- [x] **T34** — Final commit per spec template:
  `refactor(PERF-01): journal cold-load diet + SWR cache`. Verify with
  `git log` that CP-1..CP-5 each have their own commit (per "Completion
  Signal" #1 — each as its own commit). *Complexity: trivial.*

---

## Commit strategy

- **T1 + T2 + T3 + T4 + T5 + T6 + T7 bundled** as `refactor(PERF-01): defer wallet bundle off cold path (CP-1)`.
  All 7 are inseparable — removing the `appKit` named export (T2) breaks
  every consumer compile until T3+T5 land. T4 / T6 / T7 are trivial deletes.
  T1 (typecheck script) is bundled because the verification command in
  the commit message needs it. T8 is verification, not a code change —
  goes in the commit message body.
- **T9 + T10 + T11 + T12 + T13 bundled** as `chore(PERF-01): bundle-budget guardrail (CP-2)`.
  All five describe a single guardrail; T13 is a verify-then-revert step
  that contributes a sentence to the commit message.
- **T14 + T15 + T16 + T17 + T18 + T19 + T20 + T21 + T22 bundled** as
  `refactor(PERF-01): SWR cache + analytics migration (CP-3)`. Splitting
  T14 from its consumers leaves the primitive unused on master; splitting
  the consumers from T14 leaves them broken. T15 (tests) ships in the
  same commit as T14 (TDD).
- **T23 + T24 + T25 + T26 bundled** as `feat(PERF-01): localStorage cache tier + identity namespacing (CP-4)`.
- **T27 + T28 + T29 + T30 bundled** as `feat(PERF-01): nav prefetch + preconnect (CP-5)`.
- **T31 + T32 + T33 + T34** — verification, LEARNINGS, archive. T34 is
  the final spec-archive commit if any cleanup remains.

---

## Risks (from spec, with concrete mitigations)

1. **Cache staleness bugs.** Mitigated by the mutation→invalidation
   table in Discoveries + comment-block at top of `lib/cache.ts` + T21
   wiring.
2. **Identity-leak from cache.** Mitigated by FR-8/FR-12 + T26
   isolation test + T25 logout invalidation.
3. **Wallet defer breaks SIWE flow.** Mitigated by T3's
   `attachWalletListeners` indirection + T8 manual smoke test on real
   Arbitrum + Solana wallets before merging CP-1. **This is the highest-
   risk change in the plan — T8 is a hard gate, not a checkbox.**
4. **`localStorage` quota.** Mitigated by T23's 64KB-per-entry cap +
   oldest-eviction retry + memory-fallback warning.
5. **Lighthouse improvement under-delivers.** Mitigated by recording
   real numbers in CP-1 commit and CP-2 visualizer enabling root-cause
   analysis if gain < 20% (per spec risk #5, do not block on the exact
   number).
6. **Prefetch wastes bandwidth on metered.** Mitigated by `saveData /
   effectiveType` guard in T27 (Chromium only — Safari/Firefox always
   prefetch, accepted per Discoveries).

---

## Blockers

None. All infrastructure in place: vitest + jsdom already configured,
`vendor-wallet` chunk already exists in vite.config.ts (just incomplete),
`@reown/appkit` supports dynamic imports natively, no backend or infra
work required.

---

## PLANNING COMPLETE

Spec: PERF-01-cold-load-and-swr
Total Tasks: 34 (T1–T34)
Ready for BUILD mode.

Next task: T1 — add `"typecheck": "tsc --noEmit"` script to
`testudo-journal/package.json`.
