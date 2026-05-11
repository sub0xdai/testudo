# Specification: Batched Analytics Endpoint + Journal Service Worker

**Spec ID:** PERF-02-batch-analytics-and-sw
**Date:** 2026-04-26
**Status:** Draft
**Class:** Refactor / Performance (Backend + Frontend Infra)
**Priority:** P2 — Builds on PERF-01's foundation; targets first-paint server roundtrips and returning-visit shell-load. Smaller felt impact than PERF-01 but compounds with it.
**Depends on:** PERF-01-cold-load-and-swr — the SWR cache is the substrate the batch endpoint feeds, and the SW should not precache an unbudgeted bundle.
**Series:** PERF-01 → PERF-02 (full @brotzky-style optimization sweep on the journal SPA)

---

## Problem Statement

PERF-01 collapses within-session navigation to ~instant via SWR and trims main-bundle parse time by deferring the wallet stack. Two ceilings remain that PERF-01 deliberately did not touch.

**First-paint cold Overview is still ~7 round-trips.** `Overview` mounts and fans out `fetchOverview`, `fetchEquityCurve`, `fetchDailyPnl`, `fetchSymbolBreakdown`, `fetchSetupBreakdown`, `fetchDurationProfit`, `fetchReturnDistribution`, and `fetchTimeDistribution` in parallel. Browsers cap parallelism at 6 per origin on HTTP/1.1 and round-trips serialize at ~30 ms RTT to Cloudflare + ~80 ms origin work each. Total wall-clock today on a cold cache: ~300–600 ms. Each handler hits its own database query under a fresh `analytics_pool` checkout (`crates/router/src/routes/journal.rs:1253–1416`), so origin-side fan-out is bounded by pool throughput, not by request multiplexing.

**Repeat visits still re-download the app shell.** Users who open Desk multiple times a day pay a full HTML + main-chunk + CSS + font roundtrip every visit. There is no service worker; nothing is precached; the network is the only path. Home-wifi cold paint: ~400–800 ms. On flaky connections, the app simply fails to load.

This spec adopts Phases 3 and 4 from `PERFORMANCE.md`: a single `POST /api/v1/journal/analytics/batch` endpoint that fans out server-side and returns one envelope, plus a service worker that precaches the app shell with a NetworkFirst strategy on `/api/*` and a long-lived cache for fonts. Estimated wins: −150 to −400 ms on first cold paint of Overview; ~50–150 ms shell paint on returning visits versus ~400–800 ms today; resilience to brief network blips.

The reason this is a separate spec from PERF-01 is risk surface. PERF-01 is purely frontend, a few hundred LOC, and reversible. PERF-02 touches a new public Rust handler under test (review surface, contract stability, regression risk on analytics correctness) and adds a service worker (cache-corruption class of bugs, registration lifecycle, version-update semantics). Shipping them together would conflate two distinct review burdens.

---

## User Stories

- **As a Desk user landing on Overview**, I want the seven analytics panels to populate from one round-trip rather than seven, so the cold-paint moment is a single coherent reveal instead of a staggered cascade.
- **As a daily-active user**, I want the Desk shell (HTML, JS, CSS, fonts) to come from the local cache on every revisit, so opening the app feels like opening a desktop application.
- **As a user on a flaky connection**, I want the Desk to render its shell and last-known data even when the network briefly drops, so I can read my journal without waiting for retries.
- **As a backend operator**, I want one batched analytics request to consume one connection-pool slot for one logical span, so a single user's Overview does not occupy 7 slots concurrently.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | New endpoint `POST /api/v1/journal/analytics/batch` accepts a single `StatsFilter` body and an array of section keys to compute (or implicit "all" when omitted) | High | Router |
| FR-2 | Response envelope contains a typed slot per requested section: `{ overview, equity_curve, daily_pnl, symbol_breakdown, setup_breakdown, duration_profit, return_distribution, time_distribution }`, each independently nullable so a single-section failure does not poison the entire response | High | Router |
| FR-3 | Server-side fan-out runs sections concurrently via `tokio::try_join!` (or `futures::join_all`) reusing one `analytics_pool` checkout per section; total wall-clock target ≤ 1.2× the slowest single section | High | Router |
| FR-4 | Per-section errors are captured into the envelope as `{ section: { error: "…" } }` and logged; partial failures return HTTP 200 with the envelope. Hard failures (auth, DB pool exhaustion) return 4xx/5xx as today | High | Router |
| FR-5 | Existing per-section endpoints remain operational and unchanged for backward compatibility and isolated debugging; the batch endpoint is purely additive | High | Router |
| FR-6 | Frontend `useCachedResource` (from PERF-01) gains a `useCachedBatch(sections, filter)` companion that issues one batched request, splits the response into individual cache entries keyed identically to the per-section keys used by PERF-01 | High | Cache |
| FR-7 | `Overview` page uses `useCachedBatch` instead of N individual hooks; warm sections short-circuit (do not get re-requested) when only some are stale | High | Frontend |
| FR-8 | Service worker precaches the app shell on `install`: `/desk/index.html`, the main entry chunk, the main CSS, and any critical-path font files | High | SW |
| FR-9 | Service worker uses NetworkFirst with a 3-second timeout for `/api/*` requests, falling back to cached responses with a `sw-fallback: stale` header so the cache layer can mark the data stale | High | SW |
| FR-10 | Service worker uses CacheFirst with a 30-day TTL for fonts (`*.woff2`) | Medium | SW |
| FR-11 | Service worker registration is deferred until `requestIdleCallback` (or 2 s timeout fallback) so it never competes with cold-load main-thread work | High | SW |
| FR-12 | Service worker skips caching for any request whose URL contains `?nosw=1` (escape hatch for debugging) | Low | SW |
| FR-13 | A versioned cache name (e.g. `testudo-journal-v3`) ensures stale workers from old deploys self-evict; old caches are purged on `activate` | High | SW |
| FR-14 | Service worker is opt-in by build flag for first ship: `VITE_ENABLE_SW=true`; default off until canary verification | Medium | SW |
| FR-15 | New batched endpoint ships with a Rust integration test asserting parity: batched response sections equal the per-endpoint responses for a fixed seeded fixture | High | Testing |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Rust handler `POST /api/v1/journal/analytics/batch` with concurrent fan-out + per-section error envelopes; integration test for parity vs per-section endpoints. Ship behind feature flag if needed; per-section endpoints remain. | FR-1, FR-2, FR-3, FR-4, FR-5, FR-15 |
| CP-2 | Frontend `useCachedBatch` primitive layered on PERF-01 cache; migrate `Overview` to use it; observe/measure first-paint delta. | FR-6, FR-7 |
| CP-3 | Service worker file + registration scaffolding; precache app shell; NetworkFirst for `/api/*` with 3 s timeout; CacheFirst for fonts; versioned cache eviction; behind `VITE_ENABLE_SW=true`. | FR-8, FR-9, FR-10, FR-11, FR-12, FR-13, FR-14 |
| CP-4 | SW canary: enable in production build for one week, monitor for cache-version-drift bugs, then flip default. Document rollback. | FR-14 + acceptance |

CP-1 and CP-3 are independent and can be parallelized after PERF-01 lands. CP-2 depends on CP-1. CP-4 is a rollout milestone, not new code.

### Backend: `POST /api/v1/journal/analytics/batch` (CP-1)

Handler shape (sketch, in `crates/router/src/routes/journal.rs`):

```rust
#[derive(Deserialize)]
pub struct BatchRequest {
    pub filter: StatsFilter,
    /// None = compute all sections.
    pub sections: Option<Vec<SectionKey>>,
}

#[derive(Serialize, Default)]
pub struct BatchResponse {
    pub overview: Option<SectionResult<OverviewResponse>>,
    pub equity_curve: Option<SectionResult<EquityCurveResponse>>,
    pub daily_pnl: Option<SectionResult<DailyPnlResponse>>,
    pub symbol_breakdown: Option<SectionResult<SymbolBreakdownResponse>>,
    pub setup_breakdown: Option<SectionResult<SetupBreakdownResponse>>,
    pub duration_profit: Option<SectionResult<DurationProfitResponse>>,
    pub return_distribution: Option<SectionResult<ReturnDistributionResponse>>,
    pub time_distribution: Option<SectionResult<TimeDistributionResponse>>,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum SectionResult<T> { Ok(T), Err { error: String } }

pub async fn analytics_batch(
    app_state: web::Data<AppState>,
    auth: AuthenticatedUser,
    body: web::Json<BatchRequest>,
) -> impl Responder {
    let engine = StatsEngine::new(app_state.analytics_pool.clone());
    let ts = TimeSeriesService::new(app_state.analytics_pool.clone());
    let f = body.filter.clone();
    // Concurrent fan-out: each future captures its own clone of engine/ts handles.
    // join! (not try_join!) so per-section errors don't short-circuit siblings.
    let (ov, eq, dp, sb, setb, durp, ret, td) = tokio::join!(
        run_section(|| engine.overview(&auth, &f)),
        run_section(|| ts.equity_curve(&auth, &f)),
        run_section(|| ts.daily_pnl(&auth, &f)),
        run_section(|| ts.symbol_breakdown(&auth, &f)),
        run_section(|| ts.setup_breakdown(&auth, &f)),
        run_section(|| ts.duration_profit(&auth, &f)),
        run_section(|| ts.return_distribution(&auth, &f)),
        run_section(|| ts.time_distribution(&auth, &f)),
    );
    HttpResponse::Ok().json(BatchResponse { overview: Some(ov), equity_curve: Some(eq), … })
}

async fn run_section<T, F, Fut>(f: F) -> SectionResult<T>
where F: FnOnce() -> Fut, Fut: std::future::Future<Output = anyhow::Result<T>> {
    match f().await { Ok(v) => SectionResult::Ok(v), Err(e) => SectionResult::Err { error: e.to_string() } }
}
```

Connection-pool note: each section makes its own pool checkout — not different from today's parallel HTTP requests. The win is one HTTP roundtrip + one auth check + one filter parse, not pool savings. Future optimization (out of scope here): share a single pool checkout across sections via a transaction, only worthwhile if pool contention shows up in metrics.

`SectionKey` enum + filtering when `sections: Some(...)` is provided lets the cache layer request only the stale slices. CP-2 leverages this.

### Frontend: `useCachedBatch` (CP-2)

Wraps PERF-01's cache primitive:

```ts
// src/lib/cache.ts (extend)
export function useCachedBatch(
  sections: () => SectionKey[],
  filter: () => StatsFilter,
  opts: CacheOpts = {},
) {
  // 1. compute per-section keys identical to PERF-01's `overview:hash(filter)`, `equity_curve:hash(filter)`, etc.
  // 2. on read: partition sections into FRESH (return cached, no network) and STALE_OR_MISSING (need fetch)
  // 3. issue ONE POST /analytics/batch with only stale sections
  // 4. on response: write each section back to memCache + localStorage under its individual key
  // 5. return per-section signals so UI components can subscribe selectively
}
```

Migration of `Overview.tsx`: replace 7 `useCachedResource` calls with one `useCachedBatch(['overview', 'equity_curve', …], filter)`. Each panel still reads from its individual cache key — they're populated by either path (single or batch). This means PERF-02 is purely additive: tabs that load only one section keep using `useCachedResource`; the batch hook is for fan-out scenes.

### Service Worker (CP-3)

`testudo-journal/public/sw.js` — written by hand, ~150 lines, no Workbox dependency (KISS).

```js
const CACHE = 'testudo-journal-v1' // bump on deploy
const SHELL = ['/desk/', '/desk/index.html', /* main+css filenames injected at build */]

self.addEventListener('install', (e) => {
  e.waitUntil(caches.open(CACHE).then(c => c.addAll(SHELL)).then(() => self.skipWaiting()))
})

self.addEventListener('activate', (e) => {
  e.waitUntil(
    caches.keys().then(keys => Promise.all(keys.filter(k => k !== CACHE).map(k => caches.delete(k))))
      .then(() => self.clients.claim()),
  )
})

self.addEventListener('fetch', (e) => {
  const url = new URL(e.request.url)
  if (url.searchParams.has('nosw')) return // FR-12 escape hatch
  if (url.pathname.startsWith('/api/')) return e.respondWith(networkFirstWithTimeout(e.request, 3000))
  if (/\.woff2$/.test(url.pathname)) return e.respondWith(cacheFirst(e.request, 30 * 24 * 3600 * 1000))
  if (e.request.mode === 'navigate') return e.respondWith(cacheFirst(e.request))
})
// helpers omitted for brevity
```

Build-time injection of the shell asset filenames: a tiny Vite plugin that templates `sw.js` with the emitted main entry/CSS filenames after build. The Vite plugin keeps the SW honest about hashed filenames without a runtime manifest.

Registration in `src/index.tsx`:

```ts
if (import.meta.env.VITE_ENABLE_SW === 'true' && 'serviceWorker' in navigator) {
  const register = () => navigator.serviceWorker.register('/sw.js')
  if ('requestIdleCallback' in window) requestIdleCallback(register, { timeout: 2000 })
  else setTimeout(register, 2000)
}
```

`SW.respondWith` with `sw-fallback: stale` header (FR-9) lets the cache layer in the SPA mark API responses as stale even when delivered fast — fast doesn't mean fresh. Cache stripped on activate when version bumps.

### Paved Roads

- `crates/router/src/services/journal_stats.rs` and `journal_timeseries.rs` already encapsulate every analytics computation behind a clean service struct that takes `&AuthenticatedUser` + `&StatsFilter`. The batch handler composes existing methods rather than re-implementing them — zero correctness drift risk.
- Per-section endpoints stay in place (FR-5) so debugging a single panel does not require constructing a batch envelope.
- PERF-01's `useCachedResource` is the substrate; `useCachedBatch` only adds a smarter fetcher path in front of the same cache.
- Cloudflare Pages serves `/sw.js` from the project root with no special config required; the SW scope of `/desk/` is automatic.

### Files

**New:**
- `testudo-exchange/crates/router/src/routes/journal.rs` — extend with `analytics_batch` handler, `BatchRequest`, `BatchResponse`, `SectionKey`, `SectionResult`, `run_section`
- `testudo-exchange/crates/router/tests/analytics_batch_parity.rs` — integration test asserting parity vs per-section handlers (FR-15)
- `testudo-journal/public/sw.js` — service worker source
- `testudo-journal/scripts/inject-sw-shell.ts` — Vite plugin that templates SW with built asset filenames
- `testudo-journal/src/lib/cache-batch.ts` — `useCachedBatch` primitive (or extend `cache.ts` from PERF-01 in place)

**Modified:**
- `testudo-journal/src/api/client.ts` — add `fetchAnalyticsBatch(sections, filter)` typed against `BatchResponse`
- `testudo-journal/src/components/Overview.tsx` — migrate from N hooks to one batch hook
- `testudo-journal/src/index.tsx` — registration block under `VITE_ENABLE_SW` flag
- `testudo-journal/vite.config.ts` — register the SW-shell-injection plugin
- `testudo-journal/package.json` — `VITE_ENABLE_SW` documented in `.env.example`
- `testudo-journal/CLAUDE.md` — document SW lifecycle + cache-version bump procedure

### Dependencies Added

None (Rust). Frontend: none — the SW is hand-written, no Workbox.

### Out of Scope

- Replacing per-section endpoints with batch-only. Per-section endpoints remain (FR-5).
- Background sync, push notifications, periodic background sync — not needed for cold-load wins.
- Offline mutation queue (writes while offline). Out of scope; reads-only SW.
- HTTP/2 server push or 103 Early Hints — infra-level; not in this repo's scope.
- Sharing one DB pool checkout across batched sections (transaction coalescing). Reasonable optimization, not justified until metrics show pool contention.
- Extension service worker tweaks — already a service worker (background.ts); separate concern.

---

## Acceptance Criteria

- [ ] `POST /api/v1/journal/analytics/batch` returns a populated envelope for a fixed test fixture; integration test asserts each section equals the corresponding per-section endpoint result (FR-1, FR-2, FR-15)
- [ ] Triggering a deliberate failure in one section's service (e.g. inject an error in `setup_breakdown`) returns HTTP 200 with `setup_breakdown: { error: "…" }` and other sections populated (FR-4)
- [ ] `cd testudo-exchange && cargo clippy --all-targets && cargo test` passes
- [ ] `Overview` cold paint emits exactly one `POST /analytics/batch` request in DevTools Network panel — not 7 GETs (FR-7)
- [ ] First-paint Overview wall-clock improves by ≥ 100 ms versus PERF-01 baseline (recorded in PR); record actual delta in LEARNINGS
- [ ] Partial-stale scenario: pre-warm 4 sections in the cache, navigate to Overview — exactly one batch request fires for the 3 stale sections (FR-7)
- [ ] With `VITE_ENABLE_SW=true`, a second visit to `/desk/` after a hard reload renders the shell from SW cache (verified in DevTools Application → Service Workers + Network "(ServiceWorker)" annotation) (FR-8)
- [ ] SW returns cached `/api/*` response with `sw-fallback: stale` header when origin times out > 3 s (forced via DevTools throttling) (FR-9)
- [ ] `*.woff2` cached for 30 days verified in DevTools Application → Cache Storage (FR-10)
- [ ] SW registration occurs only after page is interactive — verified by Performance recording showing no SW register entry before first paint (FR-11)
- [ ] Bumping `CACHE` version in `sw.js` evicts the old cache after activate (FR-13)
- [ ] `?nosw=1` request bypasses SW (FR-12)
- [ ] One-week canary in production with `VITE_ENABLE_SW=true` shows zero shell-staleness incidents before flipping default on (FR-14)

---

## Risks

1. **Stale-shell trap.** A bad SW deploy can pin users to a broken cached shell forever. *Mitigation:* versioned cache name + `skipWaiting` + `clients.claim` + always-honor a `?nosw=1` escape hatch + a documented manual-recovery procedure (`unregister + clear caches`) in `CLAUDE.md`. Canary phase (CP-4) catches this before it reaches all users.
2. **Batch endpoint correctness drift.** The batched handler must compute identical results to per-section handlers — otherwise cached-via-batch and cached-via-per-section produce different views of the same data. *Mitigation:* FR-15 parity test on a seeded fixture; the handler composes the existing service methods rather than reimplementing them.
3. **Connection-pool exhaustion under bursts.** A burst of 100 concurrent batch requests still issues 800 pool checkouts. *Mitigation:* per-section futures already share the same pool (existing behavior); add a Prometheus gauge for `analytics_pool` checkouts and an alert threshold. Future optimization (transaction coalescing) is documented but not scheduled.
4. **SW + SWR cache double-staleness.** SW serves stale `/api/*`, the SPA cache also marks it stale — risk of double-revalidate or stale-loop. *Mitigation:* SW adds `sw-fallback: stale` header only on fallback; SPA cache treats network-fresh-but-SW-stale identically to "stale" (revalidates once). Unit test for the loop.
5. **Cache-key skew between batch and per-section paths.** A subtle filter-hash mismatch could mean a batch-fetched `equity_curve` and a per-section-fetched `equity_curve` populate different keys. *Mitigation:* the cache key derivation is centralized in one helper consumed by both paths; explicit unit test asserting key equality across paths for identical inputs.
6. **`requestIdleCallback` not available in all targets.** Safari historically lacked it. *Mitigation:* explicit `setTimeout(register, 2000)` fallback — already in the registration sketch.
7. **Backend handler not actually faster.** If `analytics_pool` is the bottleneck rather than HTTP serialization, the batch endpoint won't move first-paint by much. *Mitigation:* PR-time benchmark records before/after on a representative filter; if delta <50 ms, document in LEARNINGS and decide whether transaction coalescing is worth the follow-up.

---

## Completion Signal

This spec is complete when:
1. CP-1 through CP-4 are merged to master.
2. All acceptance criteria are checked.
3. `cd testudo-exchange && cargo clippy --all-targets && cargo test` passes; frontend `bun run typecheck && bun run build` passes.
4. SW canary completed for ≥ 7 days in production with `VITE_ENABLE_SW=true` and zero P0/P1 incidents; default flipped to enabled.
5. LEARNINGS.md under `.specify/specs/PERF-02-batch-analytics-and-sw/` records actual measured deltas (Overview cold-paint, returning-visit shell paint) for the project record.
6. Spec archived per repo conventions; `MEMORY.md` updated to note `/api/v1/journal/analytics/batch` and the SW cache version conventions.
