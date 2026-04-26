# Implementation Plan — PERF-02 Batched Analytics Endpoint + Journal Service Worker

**Spec:** `.specify/specs/PERF-02-batch-analytics-and-sw/spec.md`
**Depends on:** PERF-01-cold-load-and-swr (merged; cache primitive at
`testudo-journal/src/lib/cache.ts`).
**Strategy:** Four vertical checkpoints. CP-1 (backend handler + parity
test) and CP-3 (service worker + Vite injection) are independent and can
be parallelized after PERF-01. CP-2 (frontend `useCachedBatch` +
Overview migration) depends on CP-1's wire shape. CP-4 is a one-line
default-flip after a one-week SW canary; no new code.

---

## Discoveries

### Backend topology (router crate)

- **All 8 analytics handlers** (`crates/router/src/routes/journal.rs:1253–1433`)
  are GET, accept `web::Query<StatsFilter>`, use `app_state.analytics_pool`,
  and require the `AuthenticatedUser` extractor. Each is a thin adapter
  that calls one `StatsEngine` or `TimeSeriesService` method and wraps
  the result in `OverviewResponse` or `DataWrapper<Vec<…>>`. Adapter
  logic per handler is non-trivial — the response structs (`DailyPnlResponse`,
  `SymbolBreakdownResponse`, `SetupBreakdownResponse`, `DurationProfitResponse`,
  `ReturnBucketResponse`, `TimeSlotResponse`) rename / map fields from the
  service-tier types (e.g. `net_pnl → pnl`, `duration_minutes → duration_secs`,
  `bucket_label → bucket`, `day_count → count`). **The batch handler must
  reuse the SAME conversion logic** to satisfy parity (FR-15) — extract
  a pure helper per response type so drift becomes structurally hard.

- **Service signatures** (all `&self`, all `Result<T, sqlx::Error>`,
  all `(user_id: Uuid, filter: &StatsFilter)`):
  - `StatsEngine::account_overview / performance_stats / risk_stats`
    (`services/journal_stats.rs:229+, 245+, 322+`).
  - `TimeSeriesService::equity_curve / daily_pnl / symbol_breakdown /
    setup_breakdown / duration_profit / return_distribution /
    time_distribution` (`services/journal_timeseries.rs:257+`).
  - `Result<T, sqlx::Error>` — **not `anyhow::Result`** as the spec
    sketch implies. The `run_section` helper must operate on
    `Result<T, sqlx::Error>` and stringify via `e.to_string()` in the
    `Err` arm.

- **No batch types exist anywhere.** Grep for `BatchRequest`,
  `BatchResponse`, `SectionKey` returns zero hits. Land them in
  `routes/journal.rs` alongside the existing handlers (no
  `routes/mod.rs` re-export needed — routes are wired by direct path
  reference in `main.rs`).

- **Route registration** is in `crates/router/src/main.rs:1093–1124`
  under `web::scope("/journal").wrap(JwtMiddleware)`. The new POST
  batch route slots in immediately after `/analytics/time-distribution`
  (line 1103), before `/trades` (line 1104). No CORS / middleware
  changes needed — the `/journal` scope already covers it.

- **No `crates/router/tests/` directory exists.** Per AGENTS.md the
  router crate is binary-only (no `src/lib.rs`), so a top-level
  `tests/analytics_batch_parity.rs` (as the spec proposes) **cannot
  compile** — it would have nothing to `use`. The parity test must
  live inline in `routes/journal.rs` as `#[cfg(test)] mod batch_tests`,
  gated `#[tokio::test] #[ignore]` with `DATABASE_URL` env. The spec's
  proposed filename in its `## Files` section is a planning oversight —
  corrected here.

- **`StatsFilter` derives `Deserialize`** (proven by
  `web::Query<StatsFilter>` working today). It will deserialize equally
  cleanly from `web::Json<BatchRequest>` where
  `BatchRequest.filter: StatsFilter`. No additional derive work needed.

### Frontend cold-paint reality (Overview)

- **The spec's "Overview fans out 8 fetches" is incorrect for the real
  code.** Direct survey of every analytics fetch site under
  `src/components/` and `src/pages/` shows exactly **5** analytics
  requests fire on Overview cold-paint:

  | # | Component | Fetcher | Filter shape |
  |---|-----------|---------|--------------|
  | 1 | `Overview.tsx:42` | `fetchOverview` | global `filters()` |
  | 2 | `Overview.tsx:48` | `fetchEquityCurve` | global `filters()` |
  | 3 | `charts/PnlCalendar.tsx:58` | `fetchDailyPnl` | **monthly** `monthFilter()` |
  | 4 | `charts/PnlTreemap.tsx:16` (default ChartSelector left) | `fetchSymbolBreakdown` | global `filters()` |
  | 5 | `charts/DailyPnl.tsx:16` (default ChartSelector right) | `fetchDailyPnl` | global `filters()` |

  The other 4 analytics endpoints (setup_breakdown, duration_profit,
  return_distribution, time_distribution) are mounted lazily inside
  ChartSelector branches and only fetch when the user picks a
  different chart. `PerformanceRadar` (Overview sidebar, line 287)
  calls `fetchDignitasMe`, which is **not** an analytics endpoint.

  Implications:
  - The cold-paint win is **5 → 1 batch + 1 monthly GET = 5 → 2**.
    Smaller than spec's "7 → 1" framing, but still meaningful.
  - The acceptance criterion "exactly one POST /analytics/batch on
    cold paint" must be relaxed to "exactly one POST /analytics/batch
    + at most one /analytics/daily-pnl GET (PnlCalendar's monthly
    filter)".

- **PnlCalendar uses a different filter shape than Overview.** Lines
  56–60 of `charts/PnlCalendar.tsx` derive `monthFilter()` with explicit
  `start_date` / `end_date` for the visible calendar month. The spec's
  `BatchRequest { filter: StatsFilter, sections: Vec<SectionKey> }`
  schema — verbatim — only carries one filter. **Architectural call:**
  PnlCalendar stays on its own `fetchDailyPnl` GET; the batch endpoint
  remains single-filter. Per-section filter overrides are out of scope
  (YAGNI: one edge case does not justify schema complexity, and the
  parity-test surface doubles).

- **Two `fetchDailyPnl` call sites coexist by design.** PnlCalendar
  (monthly filter) and the DailyPnl chart (global filter) produce
  different cache keys — they are not deduplicating misses. CP-2's
  `useCachedBatch` integrates the global-filter call site only.

- **The 7 `createResource` chart panels behind ChartSelector**
  (`DrawdownChart`, `SymbolBreakdown`, `MarketReturn`,
  `ExpectancyBySymbol`, `HoldingPeriodAnalysis`, `SymbolDonut`,
  `SetupBreakdown`) were never migrated by PERF-01 and are out of
  PERF-02's scope. They mount only when the user picks them; not on
  cold paint. Future cleanup, not this spec.

### Cache primitive (`src/lib/cache.ts`)

- **`_memCache` is intentionally non-public** (line 20: "Exported for
  testing only — do not read/write externally in production code"). The
  batch hook needs to write to N keys — add a narrow `prime(key, data)`
  export rather than widening `_memCache`'s contract.

- **Existing public surface** consumed by per-section call sites:
  `useCachedResource`, `invalidate`, `clearCacheForIdentity`,
  `stableHash`, `prefetch` (route-prefetch helper from PERF-01 CP-5).
  CP-2 adds `prime` and `cacheKeyForSection`.

- **Cache key derivation is currently inlined at every call site**
  (`'overview:' + stableHash(filters())`,
  `'symbol-breakdown:' + stableHash(filters())`, etc.). Spec risk #5
  (cache-key skew between batch and per-section paths) is real because
  of this scatter. CP-2 introduces a single exported helper
  `cacheKeyForSection(SectionKey, StatsFilter): string` consumed by
  both call sites; existing per-section call sites are migrated to use
  it as part of CP-2 (one-line change per site).

### Service worker

- **No service worker exists today** — confirmed via grep for
  `serviceWorker.register`, absence of `public/sw.js`, no SW plugin in
  `vite.config.ts`. Greenfield.

- **Vite 5.4.x has a stable `writeBundle` plugin hook.** A small
  inline plugin (~30 LOC) reads the emitted bundle, locates the entry
  chunk filename + main CSS asset, templates a `sw.js` source from
  `public/sw.template.js`, and writes the result to `dist/sw.js`. No
  Workbox dep — matches spec's KISS framing.

- **`?nosw=1` escape hatch and `VITE_ENABLE_SW` flag default off in
  CP-3.** CP-3's commit must NOT change the default; CP-4's one-line
  flip happens after the canary week.

- **Registration site:** `src/index.tsx:25–43` already sets up
  preconnect before `render()`. SW registration sits after preconnect,
  inside an `import.meta.env.VITE_ENABLE_SW === 'true'` guard, with
  `requestIdleCallback` (Chromium) / `setTimeout(2000)` fallback
  (Safari/Firefox).

- **`VITE_API_URL` and `VITE_WS_URL` already exist** (`src/index.tsx:38–39`).
  `VITE_ENABLE_SW` is the only new flag.

### Test infra (frontend + backend)

- **vitest 3.2.4 + jsdom** already configured — `src/lib/cache.test.ts`
  is the existing pattern for colocated `.test.ts`. SW logic in CP-3 is
  hard to unit-test (depends on `caches`, `fetch`, MV3 lifecycle); a
  thin extracted-helper layer can be unit-tested for the URL-routing
  decision without a real ServiceWorker context. The SW shell itself is
  verified manually in CP-3's acceptance walkthrough + observed during
  CP-4 canary.

- **Backend integration test pattern (per AGENTS.md):**
  `#[tokio::test] #[ignore]` + `DATABASE_URL` env. `cargo test` excludes
  ignored tests by default, so CI stays green; `cargo test -- --ignored`
  runs the parity test against a live Postgres locally and in the soak
  job. No `sqlx::test` — workspace `sqlx` lacks the `macros` feature.

---

## Tasks

### CP-1 — Backend batch endpoint + parity test (FR-1, FR-2, FR-3, FR-4, FR-5, FR-15)

- [ ] **T1** — Extract pure response-conversion helpers in
  `crates/router/src/routes/journal.rs`. New `pub(super)` (or
  `fn`-local) helpers, one per non-trivial response struct:
  - `to_daily_pnl_response(raw: Vec<DailyPnlPoint>) -> Vec<DailyPnlResponse>`
  - `to_symbol_breakdown_response(raw: Vec<SymbolBreakdown>) -> Vec<SymbolBreakdownResponse>`
  - `to_setup_breakdown_response(raw: Vec<SetupBreakdown>) -> Vec<SetupBreakdownResponse>`
  - `to_duration_profit_response(raw: Vec<DurationProfitPoint>) -> Vec<DurationProfitResponse>`
  - `to_return_distribution_response(raw: Vec<ReturnBucket>) -> Vec<ReturnBucketResponse>`
  - `to_time_distribution_response(raw: Vec<TimeDistribution>) -> Vec<TimeSlotResponse>`
  - `to_overview_response(account, performance, risk) -> OverviewResponse`
  Migrate the existing per-section handlers to call these helpers
  (the existing inline `.map().collect()` blocks become single-line
  `let data = to_*_response(raw)`). Per-section endpoint responses
  remain byte-for-byte identical — verified by visual inspection of
  the helper call sites.
  *Complexity: medium — 7 handlers touched, transformation is mechanical.*

- [ ] **T2** — Add new types to `routes/journal.rs`:
  ```rust
  #[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
  #[serde(rename_all = "snake_case")]
  pub enum SectionKey {
      Overview, EquityCurve, DailyPnl, SymbolBreakdown,
      SetupBreakdown, DurationProfit, ReturnDistribution, TimeDistribution,
  }

  #[derive(Deserialize)]
  pub struct BatchRequest {
      pub filter: StatsFilter,
      /// `None` (or omitted) = compute all sections.
      #[serde(default)]
      pub sections: Option<Vec<SectionKey>>,
  }

  #[derive(Serialize)]
  #[serde(untagged)]
  pub enum SectionResult<T: Serialize> {
      Ok(T),
      Err { error: String },
  }

  #[derive(Serialize, Default)]
  pub struct BatchResponse {
      #[serde(skip_serializing_if = "Option::is_none")]
      pub overview: Option<SectionResult<OverviewResponse>>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub equity_curve: Option<SectionResult<DataWrapper<Vec<EquityCurvePoint>>>>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub daily_pnl: Option<SectionResult<DataWrapper<Vec<DailyPnlResponse>>>>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub symbol_breakdown: Option<SectionResult<DataWrapper<Vec<SymbolBreakdownResponse>>>>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub setup_breakdown: Option<SectionResult<DataWrapper<Vec<SetupBreakdownResponse>>>>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub duration_profit: Option<SectionResult<DataWrapper<Vec<DurationProfitResponse>>>>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub return_distribution: Option<SectionResult<DataWrapper<Vec<ReturnBucketResponse>>>>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub time_distribution: Option<SectionResult<DataWrapper<Vec<TimeSlotResponse>>>>,
  }
  ```
  Note: `DataWrapper` envelopes preserved per-section so the wire shape
  matches what the per-section endpoints already return. Sections not
  requested are `None` and stripped from JSON via `skip_serializing_if`.
  *Complexity: simple — pure type definitions.*

- [ ] **T3** — Implement `analytics_batch` handler in `routes/journal.rs`:
  ```rust
  pub async fn analytics_batch(
      app_state: web::Data<AppState>,
      user: AuthenticatedUser,
      body: web::Json<BatchRequest>,
  ) -> Result<HttpResponse> {
      let engine = StatsEngine::new(app_state.analytics_pool.clone());
      let ts = TimeSeriesService::new(app_state.analytics_pool.clone());
      let filter = body.filter.clone();
      let user_id = user.user_id;
      let want = |k: SectionKey| body.sections.as_ref().map_or(true, |s| s.contains(&k));

      let (ov, eq, dp, sb, setb, durp, ret, td) = tokio::join!(
          run_section_when(want(SectionKey::Overview),         compute_overview(&engine, user_id, &filter)),
          run_section_when(want(SectionKey::EquityCurve),      compute_equity_curve(&ts, user_id, &filter)),
          run_section_when(want(SectionKey::DailyPnl),         compute_daily_pnl(&ts, user_id, &filter)),
          run_section_when(want(SectionKey::SymbolBreakdown),  compute_symbol_breakdown(&ts, user_id, &filter)),
          run_section_when(want(SectionKey::SetupBreakdown),   compute_setup_breakdown(&ts, user_id, &filter)),
          run_section_when(want(SectionKey::DurationProfit),   compute_duration_profit(&ts, user_id, &filter)),
          run_section_when(want(SectionKey::ReturnDistribution), compute_return_distribution(&ts, user_id, &filter)),
          run_section_when(want(SectionKey::TimeDistribution), compute_time_distribution(&ts, user_id, &filter)),
      );

      Ok(HttpResponse::Ok().json(BatchResponse {
          overview: ov, equity_curve: eq, daily_pnl: dp,
          symbol_breakdown: sb, setup_breakdown: setb,
          duration_profit: durp, return_distribution: ret, time_distribution: td,
      }))
  }

  async fn run_section_when<T, Fut>(wanted: bool, fut: Fut) -> Option<SectionResult<T>>
  where T: Serialize, Fut: Future<Output = Result<T, sqlx::Error>>,
  {
      if !wanted { return None }
      Some(match fut.await {
          Ok(v) => SectionResult::Ok(v),
          Err(e) => {
              tracing::error!("analytics_batch section error: {e}");
              SectionResult::Err { error: e.to_string() }
          }
      })
  }
  ```
  Where `compute_overview` / `compute_*` are thin async wrappers that
  call the service method + the corresponding `to_*_response` helper
  from T1 — the batch path and per-section path produce identical
  envelopes by construction. `tokio::join!` (not `try_join!`) so a
  per-section failure does not short-circuit siblings (FR-4).
  *Complexity: medium.*

- [ ] **T4** — Wire route in `crates/router/src/main.rs:1093–1124`
  under the `/journal` scope, immediately after the existing
  `/analytics/time-distribution` line:
  ```rust
  .route("/analytics/batch", web::post().to(journal::analytics_batch))
  ```
  *Complexity: trivial.*

- [ ] **T5** — Inline parity test module
  `#[cfg(test)] mod batch_tests` at the bottom of `routes/journal.rs`:
  - `#[tokio::test] #[ignore]` gated, `DATABASE_URL` from env.
  - Seed fixture: insert 3–5 closed `journal_trades` rows for a synthetic
    `user_id` covering ≥ 2 distinct symbols, ≥ 1 setup tag, ≥ 1 winning
    + 1 losing trade so every section has data.
  - Build a `StatsFilter` with no constraints (whole window).
  - For each section, call the section-specific `compute_*` helper
    directly (no Actix HTTP layer needed — the helpers are the shared
    code path) and capture the result.
  - Call the same `compute_*` helpers via the same `tokio::join!`
    pattern used by `analytics_batch` for `sections: None`.
  - Assert each section's `serde_json::Value` is structurally equal
    via `assert_eq!(serde_json::to_value(&per_section)?,
    serde_json::to_value(&batch_envelope.overview)?)` etc.
  - Cleanup: delete seeded rows in FK-respecting order (use `let _ =
    ...` for idempotent cleanups per AGENTS.md).
  - Add a partial-failure test: stub one section to return
    `Err(sqlx::Error::RowNotFound)` (via a wrapped helper that injects
    the error), assert envelope returns 200 with that section's
    `SectionResult::Err { error }` populated and other sections `Ok` —
    FR-4 guarantee.
  *Complexity: medium.*

- [ ] **T6** — Verification:
  `cd testudo-exchange && cargo clippy --all-targets && cargo test`
  (excludes ignored tests, must stay green). Then
  `cargo test -- --ignored` against a developer-local Postgres for the
  parity test. Record both runs in the commit message.
  *Complexity: simple — verification step.*

### CP-2 — Frontend `useCachedBatch` + Overview migration (FR-6, FR-7)

- [ ] **T7** — Extend `testudo-journal/src/lib/cache.ts`:
  - Add `export type SectionKey = 'overview' | 'equity_curve' | 'daily_pnl'
    | 'symbol_breakdown' | 'setup_breakdown' | 'duration_profit'
    | 'return_distribution' | 'time_distribution'` matching the Rust
    enum's `serde(rename_all = "snake_case")` output.
  - Add `export function cacheKeyForSection(key: SectionKey, filter:
    StatsFilter): string` returning the spec-aligned key shape:
    `'overview:' + stableHash(filter)` for `overview`,
    `'equity-curve:' + stableHash(filter)` for `equity_curve`, etc.
    (matches existing per-section call-site keys).
  - Add `export function prime<T>(key: string, data: T, opts?:
    { identity?: string | null; persist?: boolean }): void` — writes
    `{ data, updatedAt: Date.now() }` to `_memCache`, and to
    `localStorage` when `opts.persist && opts.identity`. No-op if key
    already has a fresher entry. The narrow primitive the batch hook
    needs; do not export `_memCache`.
  *Complexity: simple — additive API extensions.*

- [ ] **T8** — Migrate existing per-section call sites to use
  `cacheKeyForSection`. One-line change per site (replace inline
  `'<name>:' + stableHash(filters())` with
  `cacheKeyForSection('<key>', filters())`):
  - `src/components/Overview.tsx:41` (overview), `:47` (equity_curve)
  - `src/components/charts/PnlTreemap.tsx:15`
  - `src/components/charts/PnlCalendar.tsx:57` (uses `monthFilter()`
    — keep distinct)
  - `src/components/charts/DailyPnl.tsx:15`
  - `src/components/charts/DurationScatter.tsx:15`
  - `src/components/charts/ReturnHistogram.tsx:15`
  - `src/components/charts/TimeHeatmap.tsx:18`
  - `src/pages/Coach.tsx:26`
  No behavior change — purely centralizes key derivation. Defends spec
  risk #5 (key skew). *Complexity: simple — repetitive.*

- [ ] **T9** — Add to `testudo-journal/src/api/client.ts`:
  ```ts
  export type BatchSection = 'overview' | 'equity_curve' | 'daily_pnl'
    | 'symbol_breakdown' | 'setup_breakdown' | 'duration_profit'
    | 'return_distribution' | 'time_distribution'

  export interface BatchAnalyticsResponse {
    overview?:           OverviewResponse | { error: string }
    equity_curve?:       { data: EquityPoint[] } | { error: string }
    daily_pnl?:          { data: DailyPnlPoint[] } | { error: string }
    symbol_breakdown?:   { data: SymbolBreakdownItem[] } | { error: string }
    setup_breakdown?:    { data: SetupBreakdownItem[] } | { error: string }
    duration_profit?:    { data: DurationProfitPoint[] } | { error: string }
    return_distribution?:{ data: ReturnBucket[] } | { error: string }
    time_distribution?:  { data: TimeSlot[] } | { error: string }
  }

  export async function fetchAnalyticsBatch(
    sections: BatchSection[] | undefined,
    filter: StatsFilter,
  ): Promise<BatchAnalyticsResponse> {
    return postJson<BatchAnalyticsResponse>(
      '/api/v1/journal/analytics/batch',
      { filter, sections },
    )
  }
  ```
  Use the existing JSON-POST helper used by other `client.ts`
  mutations (confirm exact name during implementation; falls under
  `fetchApi` per the survey at `client.ts:151–156`).
  *Complexity: simple.*

- [ ] **T10** — New file `testudo-journal/src/lib/cache-batch.ts` (or
  inlined at the bottom of `cache.ts` if total LOC stays under ~300):
  ```ts
  export interface BatchOpts {
    staleMs?: number
    persist?: boolean
    identity?: string | null
  }

  export function useCachedBatch(
    sections: () => SectionKey[],
    filter: () => StatsFilter,
    opts?: BatchOpts,
  ): {
    sections: Record<SectionKey, CachedResource<unknown>>
    anyLoading: () => boolean
    refetch: () => void
  }
  ```
  Implementation flow on each reactive read:
  1. Compute current per-section keys via
     `cacheKeyForSection(s, filter())`.
  2. Partition `sections()` into FRESH (memCache hit, age < staleMs)
     vs STALE_OR_MISSING. Fresh sections short-circuit — no network.
  3. If STALE_OR_MISSING is empty, return — FR-7's "warm sections
     short-circuit" criterion.
  4. Otherwise issue exactly one
     `fetchAnalyticsBatch(STALE_OR_MISSING, filter())`.
  5. On response: for each section in the response, call
     `prime(cacheKeyForSection(section, filter()), payload, opts)`.
     Sections with `{ error: ... }` are NOT primed — fall back to the
     stale entry if any (consistent with `useCachedResource`'s
     "render-stale-on-error" semantics).
  6. Return per-section reactive `CachedResource<T>` accessors that
     read from `_memCache` (build them on top of `useCachedResource`
     with a no-op fetcher once `prime` populates the entry, OR — the
     simpler approach — back them with private signals updated as
     entries arrive). Pick the simpler approach during build.
  *Complexity: medium — partition/fan-out/prime is the heart of the
  feature.*

- [ ] **T11** — Unit tests in `src/lib/cache-batch.test.ts`
  (vitest + jsdom, colocated like `cache.test.ts`):
  - All-cold: no entries primed → exactly one batched fetch fires
    for all requested sections.
  - All-warm: prime every requested section within `staleMs` → zero
    fetches.
  - Mixed: prime 4 of 7 sections → exactly one batched fetch fires
    for the 3 stale sections (verifies FR-7).
  - Per-section error: mock fetch to return
    `{ overview: { error: '…' }, equity_curve: { data: […] } }` →
    `equity_curve` cached, `overview` NOT cached, batch resolves
    without throwing.
  - Cross-path key parity: assert
    `cacheKeyForSection('overview', filter) ===
    'overview:' + stableHash(filter)` for a fixed filter (defends
    spec risk #5).
  - `prime` then `useCachedResource` reads the primed entry without
    triggering its own fetcher (verifies FR-6).
  *Complexity: medium.*

- [ ] **T12** — Migrate `src/components/Overview.tsx`:
  - Replace the two `useCachedResource` calls (`overview`,
    `equity_curve`) with one
    `useCachedBatch(() => ['overview', 'equity_curve',
    'symbol_breakdown', 'daily_pnl'], filters,
    { staleMs: 30_000, persist: true,
      identity: auth.user()?.id ?? null })`.
    Including `symbol_breakdown` + `daily_pnl` covers ChartSelector's
    two default-chart panels (`PnlTreemap` reads `symbol-breakdown`
    cache; `DailyPnl` chart reads `daily_pnl` cache) — they get cache
    HITS from the primed batch and skip their own fetches.
  - Render data from `batch.sections.overview()` and
    `batch.sections.equity_curve()` (instead of `stats()` /
    `equity()`); update `loading` / `error` reads to match the new
    accessors.
  - **Carve-out:** PnlCalendar (`fetchDailyPnl` with `monthFilter()`)
    keeps its own `useCachedResource` call — its filter shape differs
    from `filters()`, so its cache key differs, and the batch
    endpoint's single-filter shape can't include it. Documented in
    Discoveries; this is the one expected residual GET on cold paint.
  *Complexity: medium — Overview reactive flow needs careful
  re-stitching, data shapes unchanged.*

- [ ] **T13** — Verify
  `cd testudo-journal && bun run typecheck && bun run build && bun
  run build:check` passes. Main entry chunk gzipped budget (PERF-01's
  250 KB) holds. *Complexity: simple.*

- [ ] **T14** — Manual browser verification:
  - DevTools Network on cold Overview: exactly one
    `POST /analytics/batch` + at most one
    `GET /analytics/daily-pnl` (PnlCalendar carve-out). Adjusted
    acceptance: spec's "1 vs 7" framing → "1 batch + 1 calendar GET".
  - Pre-warm 3 sections via `useCachedResource` (e.g. visit charts
    that consume them, return to Overview within `staleMs`): the
    batch request fires only for the still-stale sections.
  - First-paint Overview wall-clock improvement vs PERF-01 baseline ≥
    100 ms — record before/after numbers in the commit message and in
    the spec's LEARNINGS.md.
  - Inject a 500-error in one section's service handler (temporary):
    confirm batch returns 200 with that section's `{ error: '…' }`
    and other panels render normally.
  *Complexity: medium — purely manual, hard gate before merge.*

### CP-3 — Service worker + Vite injection plugin (FR-8 through FR-14)

- [ ] **T15** — New file `testudo-journal/public/sw.template.js` —
  hand-written ~150 LOC, no dependencies. Implements:
  - `const CACHE = '__CACHE_NAME__'` — placeholder replaced by the
    Vite plugin at build time. Set per-deploy via injected version
    (`testudo-journal-v1`, bumped per FR-13).
  - `const SHELL = "[__SHELL__]"` — placeholder replaced with the
    JSON list of built asset filenames (`index.html`, the entry
    chunk, main CSS) by the Vite plugin.
  - `install` listener:
    `caches.open(CACHE).then(c => c.addAll(SHELL)).then(() => self.skipWaiting())`.
  - `activate` listener: enumerate `caches.keys()`, delete every key
    !== `CACHE`, then `self.clients.claim()` (FR-13).
  - `fetch` listener:
    - If `url.searchParams.has('nosw')` → return (no `respondWith`,
      browser handles natively, FR-12).
    - If `url.pathname.startsWith('/api/')` →
      `respondWith(networkFirstWithTimeout(req, 3000))`.
      `networkFirstWithTimeout`: race `fetch(req)` vs
      `setTimeout(3000)`; on win-by-network, clone response, write to
      cache, return; on timeout or fetch throw, return cached response
      with a `sw-fallback: stale` header injected via
      `new Response(body, { headers: new Headers([...orig.headers,
      ['sw-fallback', 'stale']]) })` (FR-9).
    - If `/\.woff2$/.test(url.pathname)` →
      `respondWith(cacheFirst(req, 30 * 24 * 3600 * 1000))` —
      cache-first with 30-day TTL via stored `cached-at` header
      (FR-10).
    - If `req.mode === 'navigate'` →
      `respondWith(cacheFirst(req))` — shell from cache, never
      re-validate during page load (FR-8).
    - Otherwise: pass through.
  - Keep file under 200 LOC. KISS, no Workbox.
  *Complexity: medium.*

- [ ] **T16** — New Vite plugin in
  `testudo-journal/scripts/inject-sw-shell.ts` (~30 LOC, plain TS —
  Vite supports TS plugins natively):
  ```ts
  import type { Plugin } from 'vite'
  import { readFileSync, writeFileSync } from 'node:fs'
  import { resolve } from 'node:path'

  export function injectSwShell(opts: { version: string }): Plugin {
    return {
      name: 'testudo-inject-sw-shell',
      apply: 'build',
      writeBundle(outOpts, bundle) {
        const entry = Object.values(bundle).find(
          c => c.type === 'chunk' && c.isEntry,
        ) as any
        const css = Object.values(bundle).find(
          c => c.type === 'asset' && /\.css$/.test(c.fileName),
        ) as any
        const shell = JSON.stringify([
          '/', '/index.html',
          '/' + entry.fileName,
          ...(css ? ['/' + css.fileName] : []),
        ])

        const tmpl = readFileSync(
          resolve(process.cwd(), 'public/sw.template.js'), 'utf8')
        const out = tmpl
          .replace('__CACHE_NAME__', `testudo-journal-${opts.version}`)
          .replace('"[__SHELL__]"', shell)

        const outDir = outOpts.dir ?? resolve(process.cwd(), 'dist')
        writeFileSync(resolve(outDir, 'sw.js'), out)
      },
    }
  }
  ```
  Wire into `vite.config.ts` plugins array:
  `injectSwShell({ version: process.env.VITE_SW_VERSION ?? 'v1' })`.
  Source `VITE_SW_VERSION` from build env to bump per deploy.
  *Complexity: simple.*

- [ ] **T17** — Documentation:
  - Create `testudo-journal/.env.example` with `VITE_API_URL`,
    `VITE_WS_URL`, `VITE_WALLETCONNECT_PROJECT_ID`,
    `VITE_ENABLE_SW=false`, `VITE_SW_VERSION=v1`.
  - Update `testudo-journal/CLAUDE.md`: SW lifecycle (install →
    skipWaiting → activate → claim → fetch); cache-version bump
    procedure; manual user recovery (`unregister + clear caches`);
    `?nosw=1` debugging flag.
  *Complexity: simple — docs only.*

- [ ] **T18** — Wire SW registration in `src/index.tsx` after the
  preconnect block (lines 25–43):
  ```ts
  if (import.meta.env.VITE_ENABLE_SW === 'true' && 'serviceWorker' in navigator) {
    const register = () => {
      navigator.serviceWorker.register('/sw.js')
        .catch(err => console.warn('[sw] register failed', err))
    }
    if ('requestIdleCallback' in window) {
      (window as any).requestIdleCallback(register, { timeout: 2000 })
    } else {
      setTimeout(register, 2000)
    }
  }
  ```
  Default `VITE_ENABLE_SW=false` — no behavioral change in production
  on this commit (FR-14). *Complexity: simple.*

- [ ] **T19** — Extracted URL-routing helper for testability:
  `src/lib/sw-route.ts` exporting a pure function `classifyRequest(url:
  string, mode: RequestMode): 'bypass' | 'api' | 'font' | 'navigate' |
  'passthrough'`. The SW inlines a copy of this logic (KISS — service
  workers don't import npm modules cleanly without a bundler). Colocated
  unit test in `src/lib/sw-route.test.ts` exercises each branch
  (bypass when `?nosw=1`, api for `/api/*`, font for `.woff2`, navigate
  for `mode === 'navigate'`, passthrough otherwise).
  *Complexity: simple.*

- [ ] **T20** — Build + manual verification:
  - `cd testudo-journal && VITE_ENABLE_SW=false bun run build`
    produces `dist/sw.js` with placeholders replaced; main entry chunk
    still under 250 KB gzip (`bun run build:check` passes).
  - Set `VITE_ENABLE_SW=true` in `.env`, rebuild, `bun run preview`,
    hard-reload twice. Second visit shows "(ServiceWorker)" in DevTools
    Network for shell requests (FR-8). Cache Storage panel shows
    `testudo-journal-v1` populated with `index.html`, the entry chunk,
    the CSS file (FR-8). `*.woff2` requests on subsequent loads served
    from Cache Storage (FR-10).
  - DevTools Network throttling: simulate `/api/*` taking > 3 s.
    Confirm fallback to cached response with `sw-fallback: stale`
    response header (FR-9).
  - Hit `/desk/?nosw=1`: confirm no SW interception (FR-12).
  - Bump `VITE_SW_VERSION=v2`, rebuild, hard-reload: confirm
    `testudo-journal-v1` cache deleted on `activate`, `v2` populated
    (FR-13).
  - Performance recording on cold load: SW register entry occurs after
    `domcontentloaded` + idle gap, not before first paint (FR-11).
  *Complexity: medium — extensive manual verification, no fast
  iteration loop.*

### CP-4 — Canary + default flip (FR-14, completion-signal items)

- [ ] **T21** — Deploy CP-3 to production with `VITE_ENABLE_SW=true`
  set ONLY for a beta cohort (route via Cloudflare env or a
  `?canary=1` opt-in URL). Document the canary plan in
  `.specify/specs/PERF-02-batch-analytics-and-sw/CANARY.md` (route,
  rollback procedure, monitoring SLOs). *Complexity: simple — docs +
  Cloudflare config.*

- [ ] **T22** — Soak for ≥ 7 days. Monitor:
  - Browser console errors (any SW-related crash).
  - User-reported "stuck on old shell" issues (Discord / support).
  - SW cache-version bump applied during soak — verify no users
    pinned to `testudo-journal-v1` after one full deploy cycle.
  *Complexity: trivial — observation only.*

- [ ] **T23** — Flip default. One-line change in CI/build env or
  `.env.production`: `VITE_ENABLE_SW=true`. Commit:
  `chore(PERF-02): default-enable journal service worker after canary
  (CP-4)`. *Complexity: trivial.*

- [ ] **T24** — Write
  `.specify/specs/PERF-02-batch-analytics-and-sw/LEARNINGS.md` with:
  actual measured first-paint deltas (CP-2 numbers from T14),
  shell-paint deltas (CP-3 numbers from T20 + canary), gotchas (spec's
  "8 fetches on Overview" vs reality's 5; PnlCalendar filter-shape
  collision; Vite plugin `writeBundle` vs `closeBundle` timing),
  deferred follow-ups (transaction-coalescing for backend pool; batch
  priming for the 7 unmigrated chart panels; per-section filter
  overrides if PnlCalendar case multiplies). Update root `MEMORY.md`
  with one-liner: batch endpoint at `POST /api/v1/journal/analytics/batch`,
  SW cache-version convention `testudo-journal-v{N}` bumped per deploy.
  *Complexity: trivial.*

- [ ] **T25** — Archive spec per repo convention:
  `.specify/specs/PERF-02-batch-analytics-and-sw/` →
  `.specify/spec-archive/PERF-02-batch-analytics-and-sw/`. Final
  `IMPLEMENTATION_PLAN.md` status flip to "COMPLETE — archived".
  *Complexity: trivial.*

---

## Commit strategy

- **T1 + T2 + T3 + T4 + T5 + T6 bundled** as
  `feat(PERF-02): batched analytics endpoint + parity test (CP-1)`.
  T1 (helper extraction) is a precondition for T3 (the batch handler
  reuses the helpers); T5 (parity test) verifies T1+T3 together —
  splitting them leaves master in an inconsistent state. T6 is
  verification, not new code; goes in the commit message body.
- **T7 + T8 + T9 + T10 + T11 + T12 + T13 + T14 bundled** as
  `feat(PERF-02): useCachedBatch + Overview migration (CP-2)`.
  T7 (cache extensions) is unused until T10/T12 land; T11 (tests) is
  TDD for T10. T8 (key-helper migration) ships in the same commit as
  T7 to avoid leaving inline keys vs centralized keys in a conflicting
  state.
- **T15 + T16 + T17 + T18 + T19 + T20 bundled** as
  `feat(PERF-02): journal service worker (CP-3, default off)`.
  All six describe one feature behind one flag; default stays `false`
  per FR-14.
- **T21 + T22 are not commits** — monitoring/operations.
- **T23** is its own commit:
  `chore(PERF-02): default-enable journal service worker after canary
  (CP-4)`.
- **T24 + T25** bundled as `docs(PERF-02): LEARNINGS + spec archive`.

Per AGENTS.md: NO `Co-Authored-By: Claude` trailers in this repo.

---

## Risks (from spec, with concrete mitigations)

1. **Stale-shell trap.** Mitigated by versioned cache name (T16 +
   `VITE_SW_VERSION`), `skipWaiting`/`clients.claim` (T15), `?nosw=1`
   escape hatch (T15), CP-4 one-week canary (T22), and manual-recovery
   procedure documented in CLAUDE.md (T17).
2. **Batch correctness drift.** Mitigated structurally — T1 extracts
   the conversion helpers; both per-section and batch handlers reuse
   them. Drift is hard to introduce. T5 parity test is the regression
   net.
3. **Pool exhaustion under bursts.** Out of scope for code; documented
   as a future follow-up (transaction-coalescing) only if metrics
   show analytics_pool contention. Out-of-spec per "Out of Scope"
   section.
4. **SW + SWR cache double-staleness.** Mitigated by T15's
   `sw-fallback: stale` header — the SPA cache layer treats it
   identically to its own "stale" signal. Manual loop-avoidance test
   covered in T14 + T20.
5. **Cache-key skew between batch and per-section paths.** Mitigated
   by centralizing key derivation in `cacheKeyForSection` (T7) and
   migrating all existing call sites to use it (T8). Unit-tested in
   T11 ("cross-path key parity" case).
6. **`requestIdleCallback` not available in Safari/Firefox.**
   Mitigated by `setTimeout(register, 2000)` fallback (T18).
7. **Backend handler not actually faster.** Mitigated by recording
   real measured deltas in T14 (frontend Overview cold-paint) and T6
   (backend isolated-handler timing). If delta < 50 ms, document in
   LEARNINGS and decline to flip CP-4 default — fall back to keeping
   the batch endpoint additive without forcing the frontend migration.

### Plan-specific risks (added beyond spec)

8. **Spec's "Overview fans out 8 fetches" framing is incorrect.**
   Real cold-paint fan-out is 5 (see Discoveries). Mitigated by
   adjusting the acceptance criterion in T14 to "exactly one POST + at
   most one PnlCalendar GET". The win is real, just smaller than
   spec's framing implies.
9. **PnlCalendar's monthly-filter shape collides with the spec's
   single-filter `BatchRequest`.** Mitigated by leaving PnlCalendar
   on its own GET (carve-out documented in T12). Per-section filter
   overrides explicitly out of scope.
10. **Vite plugin `writeBundle` timing.** The hook fires after all
    assets are emitted but before dev-server / preview hooks. If
    `outOpts.dir` is undefined in some Vite configurations, the
    plugin falls back to `resolve(process.cwd(), 'dist')` (T16).
    Verified in T20.
11. **The 7 plain-`createResource` chart panels (DrawdownChart,
    SymbolBreakdown variants) are NOT migrated.** Living outside cold
    paint, scope-bounded out of PERF-02. Documented in Discoveries as
    a future cleanup — they don't benefit from batch priming until a
    future migration.
12. **Per-section error variant tagging.** The spec sketch uses
    `#[serde(untagged)]` on `SectionResult`. Untagged enums match by
    field shape; if the success type happens to have an `error: String`
    field, deserialization is ambiguous. None of the existing response
    types contain a top-level `error` field, so the conflict does not
    arise — but documented here so a future contributor adding such a
    field doesn't break parsing.

---

## Blockers

None. PERF-01's cache primitive (`src/lib/cache.ts`) is the substrate
for CP-2; backend service methods are clean enough that extracting
conversion helpers (T1) is a mechanical refactor; SW is greenfield
with no infra constraints (Cloudflare Pages serves `/sw.js` from
project root by default with `/desk/` scope automatic).

---

## PLANNING COMPLETE

Spec: PERF-02-batch-analytics-and-sw
Total Tasks: 25 (T1–T25)
Ready for BUILD mode.

Next task: T1 — extract pure response-conversion helpers in
`crates/router/src/routes/journal.rs` so per-section and batch
handlers share identical adapter logic.
