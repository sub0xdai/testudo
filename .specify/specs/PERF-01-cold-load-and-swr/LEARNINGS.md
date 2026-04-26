# PERF-01 Learnings

## 2026-04-26 — CP-1: Wallet defer

### vite-env.d.ts was missing
The project had no `src/vite-env.d.ts` (`/// <reference types="vite/client" />`), causing
`import.meta.env` to fail type-checking across 9 files. Adding this file fixed the bulk of the
pre-existing TS errors.

### Pre-existing type errors
`bun run typecheck` had never been run before this spec. There were ~20 accumulated type errors
in files untouched by CP-1 (unused imports, ECharts `nameLocation: 'center'` → must be `'middle'`,
`PerformanceRadar` baseRadar type conflict, `DatabaseTable` Solid.js `nativeEvent` doesn't exist —
use plain cast `(e as MouseEvent).shiftKey`). All fixed as part of establishing the typecheck gate.

### manualChunks + modulepreload interaction
`vite.config.ts` `manualChunks` entries that are statically reachable from the entry (even via
dynamic imports inside a statically-imported module) get a `<link rel="modulepreload">` in the
built HTML. `wallet.ts` is statically imported by `AuthContext.tsx` → Vite analyzes the
`import('@reown/appkit')` dynamic calls inside it and adds modulepreload for `vendor-wallet`.
Result: browser DOWNLOADS the wallet file on page load (background fetch), but `createAppKit()`
is NOT EXECUTED until `loadWallet()` is called from a user gesture. TTI benefit is real
(no wallet parse/eval on main thread at boot); bandwidth deferral is partial. For full "no-fetch
until click" behavior, `wallet.ts` would need to be removed from the static import chain entirely.

### connectWallet type change
`AuthContextValue.connectWallet` changed from `() => void` to `() => Promise<void>`.
Callers that don't await are unaffected (fire-and-forget); callers that need await can now do so.

### Main entry chunk measurement (CP-1 baseline)
Build: `index-QoKlizKF.js` — 77.75 kB raw / **26.17 kB gzip** (well under 250 KB FR-3 budget).
`vendor-wallet-*.js` — 2,152.95 kB raw / 632.21 kB gzip (separate chunk, deferred execution).
Lighthouse TTI delta: to be measured manually (T8 deferred to live session).

---

## 2026-04-26 — CP-3: useCachedResource + analytics migration

### createEffect scheduling requires deferred test assertions
Solid.js schedules `createEffect` on the microtask queue. Unit tests using `createRoot` must
`await` multiple `Promise.resolve()` flushes to observe signal updates. Stale-revalidate tests
require a deferred promise (not `mockResolvedValue`) to separate the "stale data served" moment
from the "background fetch resolved" moment — otherwise both happen within the same tick window.

### Overview.tsx retains createResource for snapshot
`Overview.tsx` still uses Solid's `createResource` for the live risk snapshot (which is
WebSocket-driven, not analytics). Only import `useCachedResource` for the analytics resources;
keep `createResource` in the import list for the snapshot.

### TradeDetail.tsx conditional tags pattern
The original `createResource(() => !props.isActive, ...)` conditional maps cleanly to
`useCachedResource(() => !props.isActive ? 'tags:all' : undefined, ...)`. When key returns
`undefined`, the cache skips all fetching. Callers that used `allTags() ?? []` are unaffected
since `CachedResource<T>()` returns `T | undefined`.

### CachedResource loading is a reactive getter, not a function
`CachedResource<T>.loading` is typed as `boolean` (a reactive getter via `Object.defineProperty`)
matching Solid's `Resource<T>.loading` pattern. Callers write `data.loading` in JSX — it works
reactively because the getter reads the underlying signal inside a tracked scope. Do NOT type it
as `() => boolean` or callers would need `data.loading()`, breaking the drop-in compatibility.
