# Implementation Plan

> Last updated: 2026-04-19
> Current spec: RSK-03-ai-trade-coach
> Phase: PLANNING COMPLETE — ready for BUILD

---

## Active Spec: RSK-03-ai-trade-coach

### Gap Analysis

**Backend (`testudo-exchange/crates/`):**
- `crates/db-processor/` is a minimal tokio queue worker binary (78-line `main.rs`, 235-line `query.rs`). It does not own trade ingestion — `JournalService` (router/services/journal_service.rs), `TradeEventWriter` (router/services/trade_event_writer.rs) and `FillDetector` do. The spec's "natural home in db-processor" comes from an outdated memory. **Planning deviation (see Discoveries #1):** coach module will live at `crates/router/src/services/coach/`, alongside `journal_service.rs`, `journal_stats.rs`, `journal_timeseries.rs`, `risk_snapshot.rs`. That's where pool handles, `analytics_pool`, `AppState`, and `JwtMiddleware` already live.
- **No LLM dep exists.** `grep -r "async-openai\|anthropic\|openai" Cargo.toml` → zero hits. RSK-03 adds `async-openai = "0.27"` to `crates/router/Cargo.toml`.
- **No pg_cron / cron scheduler.** Established pattern: `tokio::time::interval` tasks spawned in `router/main.rs` with `CancellationToken` graceful-shutdown plumbing (see ShadowEngine sweep @~560, TradeEventWriter flush @267, sidecar health @276). Coach scheduler follows the same shape.
- **AppState** (`crates/router/src/types/app.rs:14-41`): 15 fields, constructed in `main.rs:380-396`. `pool`, `analytics_pool`, `token_service`, `config`, `engine_handle` all present. Adding `pub coach_service: Arc<CoachService>` is mechanical.
- **RouterConfig** (`crates/router/src/config.rs`): uses `confik` crate + `EnvSource` + explicit `std::env::var` for secrets. Add `llm_base_url`, `llm_model`, `coach_enabled_global`, `coach_min_lifetime_trades`, `coach_min_week_trades`; pull `OPENAI_API_KEY` via `std::env::var` like `JWT_ACCESS_SECRET`.
- **Latest migration**: `20260418000000_add_setup_tag_to_trades` (RSK-02). Next slot: `20260419000000_coach_schema`. Convention: `YYYYMMDDHHMMSS_description.{up,down}.sql`.
- **Journal tables already carry coach inputs**: `journal_trades` has `setup_tag` (RSK-02), `r_multiple`, `net_pnl`, `realized_pnl`, `closed_at`, `opened_at`, `symbol`, `side`, `user_id`, `id`. Baselines computable as straight SQL aggregations on `analytics_pool`.
- **RSK-01 `bucket_for()` / `extract_base_asset()`** in `services/risk_snapshot.rs:111-138` are private `fn`. Plan changes them to `pub(crate)` so `coach/patterns/correlation_stack.rs` can reuse without duplicating `BUCKET_MAP`.
- **JWT extractor**: `AuthenticatedUser { user_id: Uuid, wallet_address: String }` from `middleware/auth.rs`. Same pattern as RSK-01 / RSK-02 routes.
- **`users` table**: `AUTH-02` migration (wallet-primary). No per-user preferences column yet. RSK-03 adds `coach_enabled BOOLEAN NOT NULL DEFAULT TRUE` + `coach_banner_last_viewed_at TIMESTAMPTZ NULL` columns — cheaper than a separate `user_preferences` table for two fields.
- **Analytics endpoint pattern**: `routes/journal.rs::setup_breakdown` (RSK-02 T6) shows the `AuthenticatedUser` + `fetch_all` + `{ data: [...] }` envelope style for new coach routes.

**Frontend (`testudo-journal/src/`):**
- **CoachBanner placeholder confirmed alive** at `components/account/CoachBanner.tsx`: `export function CoachBanner() { return null }`. Already imported + mounted at `Account.tsx:239-241` inside `max-w-6xl mx-auto w-full px-8 pb-10`. RSK-01 reservation honoured; T11 replaces the `null` return with a real banner.
- **Routing**: `index.tsx` uses `@solidjs/router` with `base="/desk"` + `root={Layout}`. Routes: `/`, `/trades`, `/journal`, `/account`, `/pair`. Pattern: `<Route path="/coach" component={lazy(() => import('./pages/Coach'))} />` drops in alongside. Lazy imports already in use.
- **Nav**: `Layout.tsx:8-12` `NAV_ITEMS = [{ path, label }]` array iterated via `<For>` in both desktop + mobile nav. Add `{ path: '/coach', label: 'COACH' }`.
- **Markdown rendering**: `components/journal/MarkdownPreview.tsx` uses `marked` + `DOMPurify` (already on dep tree). Can reuse as-is, or a thin `NarrativeBlock` wrapper that pre-processes `[T-xxx]` citation tokens into `<a href="/desk/trades?trade={uuid}">` links before handing content to `marked.parse`.
- **Trade deep-link**: Currently only modal-based (`Trades.tsx` signal-toggles `TradeDetail`). Plan: add `useSearchParams()` read inside `Trades.tsx` — if `?trade={uuid}` present, pre-open modal. Avoids a new route.
- **API client split**: `fetchApi()` for `/analytics/*` with `StatsFilter`; `fetchCrud()` for everything else. Coach endpoints are user-scoped but not StatsFilter-scoped → `fetchCrud`.
- **HELP tooltip**: `lib/help-content.ts` flat keys (`risk.*`, `chart.*`, `page.*`). Add `coach.*` namespace for narrative/pattern/provider tooltips.
- **Preferences storage**: localStorage (e.g. `testudo-theme`, `testudo-extension-paired`). Coach opt-out is server-authoritative (the cron decides whether to generate a report), so it lives on the `users` row, not localStorage. Frontend fetches via `/api/v1/coach/preference` and PATCHes on toggle.
- **`createResource` + `<Show>`**: Standard data-fetch pattern in `pages/Overview.tsx`, `pages/Account.tsx`. CoachBanner + /desk/coach follow same style.

---

### Design Decisions (captured before tasking)

1. **Coach module lives in `router/src/services/coach/`, not `db-processor/src/coach/`.** The spec cites db-processor as the "natural home for scheduled background work," but db-processor is a thin queue worker (not a library) with no analytics, baselines, or stats logic. Router already owns the pool, analytics_pool, JWT middleware, existing journal/stats services, and the established tokio scheduler pattern. Co-locating avoids re-exporting half of router's private types to a new lib crate, and matches RSK-01/RSK-02 precedent (`risk_snapshot.rs`, `journal_service.rs` both live in router/services).

2. **`coach_reports` table, not event-sourced JSONB in `trade_events`.** Reports are write-once per user-per-week, read-many (archive view). A dedicated table with `UNIQUE(user_id, week_start)` gives simple idempotency on cron re-run and fast archive pagination.

3. **Two-column `users` extension instead of new `user_preferences` table.** Only two fields are needed: `coach_enabled BOOLEAN NOT NULL DEFAULT TRUE` + `coach_banner_last_viewed_at TIMESTAMPTZ NULL`. A separate table would add a join for every auth-gated read with no real benefit at this spec's scope. Future preferences can either land here column-by-column or migrate to a dedicated table when the count justifies it.

4. **Skip rules enforced at scheduler level; no row persisted on skip.** FR-5 + acceptance criterion "on weeks I didn't trade, generate no new report, coach doesn't feel like a form letter." The scheduler checks: (a) `coach_enabled=FALSE` → skip; (b) lifetime trades < `coach_min_lifetime_trades` (30) → skip; (c) this-week trades < `coach_min_week_trades` (3) → skip. Skip reason logged via `tracing::info!` with `skip_reason` field. Previous week's report stays as "latest" in the banner until a new meaningful week overwrites it.

5. **`Narrator` trait for DI + testability.** `trait Narrator { async fn narrate(&self, digest: &CoachDigest) -> Result<NarratedReport, NarratorError> }`. Prod impl: `OpenAiNarrator` (async-openai pointed at `OPENAI_BASE_URL`). Test impl: `MockNarrator { response: Result<NarratedReport, NarratorError> }` for unit tests in validator + schedule. Keeps HTTP dependency out of the pure-logic test paths.

6. **Citation validator is a hard gate.** Every `NarrativeSection.citations` entry must be in `digest.flagged_trades.*.id`. Invalid → reject the whole `NarratedReport`. Log which IDs failed. Scheduler then persists stats-only fallback (narrative_sections_json=NULL) instead of discarding the work. FR-12 is satisfied by the same path.

7. **Citation token format: `[T-{first_8_of_uuid}]`.** Short enough to read inline, long enough to be unique within a single digest's flagged_trades slice (typical size 3-15 trades). Backend includes `short_id: first 8 chars of id` on each `TradeEvidence`; frontend matches tokens against `flagged_trades.*.short_id` to resolve full UUID for deep-links. Full uuid ships on `TradeEvidence.id`.

8. **Trade deep-link via query param, not a new route.** Coach narrative's `[T-xxx]` → `<a href="/desk/trades?trade={uuid}">T-xxx</a>`. `Trades.tsx` gains a one-liner `useSearchParams()` read that pre-opens the `TradeDetail` modal if present. No new route, no URL reorganization, no breakage of bookmarks.

9. **Prompt structure: two-message cache-optimized layout.**
   - **Message 1 (system role, cached prefix ~8k tokens):** role intro, 6-pattern taxonomy with definitions, JSON output schema, citation rule ("every claim MUST cite [T-xxx]"), 2-3 few-shot examples, tone directives ("direct, non-judgmental, data-first, no moralizing").
   - **Message 2 (user role, per-request ~1-2k):** `CoachDigest` as JSON + instruction to generate a `NarratedReport` JSON response.

   `async-openai` response parsing: `ChatCompletion.choices[0].message.content` → parse as `NarratedReport`. Cache-hit metrics from provider response (`usage.prompt_cache_hit_tokens` on DeepSeek) logged for the ≥70% acceptance criterion.

10. **Stats-only fallback persists a valid row.** On narrator timeout, rate-limit, parse failure, or citation-validation failure: persist `coach_reports` row with `narrative_sections_json=NULL`, `digest_json=...`, `model_used="unavailable"`. Frontend renders "● coach unavailable this week" in the narrative slot. Acceptance criterion verified by pointing a test at a 404 base URL.

11. **No email / push / webhook.** Deliberate per spec FR-6 — the banner + `● new` indicator on Account is the only discovery surface. Lower engagement, higher signal-to-noise; no infra.

12. **`coach_banner_last_viewed_at` drives `● new`.** On `GET /api/v1/coach/latest`, compare `report.generated_at` vs `user.coach_banner_last_viewed_at`: if generated > last-viewed (or last-viewed is NULL), response has `has_new_indicator=true`. Visiting `/desk/coach` triggers `POST /api/v1/coach/mark-viewed` which sets `coach_banner_last_viewed_at = NOW()`. Separate from `banner_dismissed_at` on the report row (per-week dismiss, FR-7).

13. **RSK-01 bucket logic reused, not copied.** Mark `bucket_for` + `extract_base_asset` as `pub(crate)` in `risk_snapshot.rs`. Single source of truth for asset-family taxonomy. `correlation_stack.rs` pattern detector imports them.

14. **Scheduler timing.** Default cron: Sunday 18:00 UTC. Implementation: `tokio::time::interval(Duration::from_secs(3600))` hourly wakeup, with an `is_trigger_moment(now) -> bool` check computing "is it 18:00 UTC on a Sunday" + a `already_fired_this_week(pool)` SQL guard. Cheaper than a full cron-expr parser, and the precision (within 1h) is fine for a weekly report. `COACH_CRON` env var reserved for future upgrade to a real cron-expr if needed — MVP ignores it and logs a warning if set to anything non-default.

15. **Pattern thresholds from `coach_config` table — deferred.** Spec mentions a `coach_config` table for tunable thresholds. MVP uses hardcoded constants in each detector with `const` Decimals. Adding the table is a straight follow-up migration once a threshold actually needs tuning in production. Flagged as Discoveries #5 so it doesn't bleed into MVP scope.

16. **No manual trigger endpoint.** Spec risk #6 mentions "per-user rate limit on manual coach trigger if we add one." MVP does not add a manual trigger endpoint — the only way to generate a report is the weekly cron. Cost-cap concern moot.

17. **Chart dep: none.** Coach page renders deterministic stats in a small table + narrative as markdown. No ECharts usage. Reuses existing tokens, `HelpTip`, `PageSubHeader`, `MarkdownPreview` patterns.

---

### Parallel Track Detection

```
T1 (migration — coach_reports + users columns)
  │
  ├── T2 (types + module scaffolding)
  │     │
  │     ├── T3 (baseline + detect_all skeleton) ────┐
  │     │     │                                      │
  │     │     ├── T3a sizing_drift                   │
  │     │     ├── T3b frequency_spike                │
  │     │     ├── T3c session_anomaly                │ (T3a-T3f parallelizable)
  │     │     ├── T3d setup_fatigue                  │
  │     │     ├── T3e correlation_stack              │
  │     │     └── T3f streak_risk                    │
  │     ├── T4 (digest composer)                     │
  │     ├── T5 (narrator trait + OpenAI impl)        │
  │     └── T6 (citation validator)                  │
  │                                                  │
  │                                                  └── T7 (weekly scheduler + CoachService orchestration) ── T8 (routes + AppState + config)
  │                                                                                                              │
  └──────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
                                                                                                                 │
                                                                                                                 ↓
                                                                                            T9 (frontend API types + fetchers)
                                                                                                                 │
                                                                                                                 ↓
                                                                                            T10 (/desk/coach page + components)
                                                                                                                 │
                                                                                                                 ↓
                                                                                            T11 (CoachBanner live + nav + help)
                                                                                                                 │
                                                                                                                 ↓
                                                                                            T12 (final verification + commit)
```

Parallel opportunity after T2: T3/T4/T5/T6 are independent pure-logic modules. Sequential execution picked for single-agent BUILD mode; flagged in Discoveries #4 for fast-follow parallelization if needed.

---

## Tasks

### T1: Migration — coach_reports table + users coach preference columns — `complete`

**Scope:** CP-6 persistence layer. New `coach_reports` table; `users` gains `coach_enabled` + `coach_banner_last_viewed_at` columns.

**Files:**
- `testudo-exchange/crates/sqlx_postgres/migrations/20260419000000_coach_schema.up.sql` — NEW:
  ```sql
  ALTER TABLE users ADD COLUMN coach_enabled BOOLEAN NOT NULL DEFAULT TRUE;
  ALTER TABLE users ADD COLUMN coach_banner_last_viewed_at TIMESTAMPTZ NULL;

  CREATE TABLE coach_reports (
      id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
      week_start TIMESTAMPTZ NOT NULL,
      week_end TIMESTAMPTZ NOT NULL,
      generated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      model_used TEXT NOT NULL,
      headline TEXT NULL,
      narrative_sections_json JSONB NULL,   -- NULL when narrator failed (stats-only fallback)
      digest_json JSONB NOT NULL,           -- full CoachDigest for transparency + debugging
      cache_hit_ratio NUMERIC(4, 3) NULL,   -- 0..1 from provider response
      banner_dismissed_at TIMESTAMPTZ NULL,
      UNIQUE (user_id, week_start)
  );

  CREATE INDEX idx_coach_reports_user_generated ON coach_reports(user_id, generated_at DESC);
  ```
- `testudo-exchange/crates/sqlx_postgres/migrations/20260419000000_coach_schema.down.sql` — NEW:
  ```sql
  DROP INDEX IF EXISTS idx_coach_reports_user_generated;
  DROP TABLE IF EXISTS coach_reports;
  ALTER TABLE users DROP COLUMN coach_banner_last_viewed_at;
  ALTER TABLE users DROP COLUMN coach_enabled;
  ```
- `testudo-exchange/crates/router/src/models/user.rs` — MODIFIED: add `coach_enabled: bool` + `coach_banner_last_viewed_at: Option<DateTime<Utc>>` fields to `User`; update any `FromRow` / `SELECT *` call-sites that bind to the struct.

**Validate:** `cd testudo-exchange && cargo check --all-targets` (migrations run at router startup; verify no compile regressions from added columns).

**Acceptance:**
- Migration applies cleanly up + down on a fresh DB.
- `users.coach_enabled` defaults TRUE for all rows (existing + new).
- `coach_reports` UNIQUE(user_id, week_start) blocks duplicate insert on cron re-run.
- `User` model struct matches new columns; `cargo check` passes.

---

### T2: Coach module scaffolding + types — `complete`

**Scope:** CP-1/CP-2/CP-3 type surface. All structs/enums defined, no logic yet. Establishes the wire contract for T3-T6 to fill in.

**Files:**
- `testudo-exchange/crates/router/src/services/coach/mod.rs` — NEW:
  ```rust
  pub mod types;
  pub mod digest;
  pub mod patterns;
  pub mod narrator;
  pub mod validator;
  pub mod schedule;
  pub mod service;

  pub use service::CoachService;
  pub use types::*;
  ```
- `testudo-exchange/crates/router/src/services/coach/types.rs` — NEW:
  - `CoachDigest { user_id, week_start, week_end, baseline: UserBaseline, week_stats: WeekStats, flagged_patterns: Vec<FlaggedPattern>, flagged_trades: Vec<TradeEvidence> }`
  - `UserBaseline { avg_trades_per_day: Decimal, avg_position_size_usd: Decimal, typical_session_hours_utc: Vec<u8>, win_rate: Decimal, avg_r_multiple: Decimal, setup_baselines: HashMap<String, SetupBaseline> }`
  - `SetupBaseline { trade_count: i64, avg_r_multiple: Decimal, win_rate: Decimal }`
  - `WeekStats { trade_count: i64, win_rate: Decimal, total_pnl: Decimal, total_r: Decimal, trades_by_hour_utc: [i64; 24], by_setup: HashMap<String, SetupBaseline> }`
  - `TradeEvidence { id: Uuid, short_id: String, symbol: String, side: String, opened_at: DateTime<Utc>, closed_at: DateTime<Utc>, pnl: Decimal, r_multiple: Option<Decimal>, setup_tag: Option<String>, position_size_usd: Decimal }`
  - `FlaggedPattern { pattern: PatternKind, severity: Severity, evidence: Vec<Uuid>, metrics: serde_json::Value }`
  - `enum PatternKind { SizingDrift, FrequencySpike, SessionAnomaly, SetupFatigue, CorrelationStack, StreakRisk }` (Serialize with `#[serde(rename_all = "snake_case")]`)
  - `enum Severity { Info, Notable, Concerning }`
  - `NarratedReport { headline: String, sections: Vec<NarrativeSection>, model_used: String, cache_hit_ratio: Option<Decimal>, generated_at: DateTime<Utc> }`
  - `NarrativeSection { pattern: PatternKind, body: String, citations: Vec<Uuid> }`
  - `StoredCoachReport { id: Uuid, user_id: Uuid, week_start: DateTime<Utc>, week_end: DateTime<Utc>, generated_at: DateTime<Utc>, model_used: String, headline: Option<String>, narrative_sections: Option<Vec<NarrativeSection>>, digest: CoachDigest, cache_hit_ratio: Option<Decimal>, banner_dismissed_at: Option<DateTime<Utc>> }`
  - `CoachConfig { min_lifetime_trades: i64, min_week_trades: i64, enabled_global: bool }`
  - `NarratorError` enum: `Timeout`, `RateLimit`, `Parse(String)`, `Provider(String)`.
  - `ValidationError` enum: `UnknownCitation { section_index: usize, trade_id: Uuid }`, `UnknownToken { section_index: Option<usize>, token: String }`.
- `testudo-exchange/crates/router/src/services/coach/service.rs` — NEW (stub):
  - `pub struct CoachService { pool: PgPool, analytics_pool: PgPool, narrator: Arc<dyn Narrator + Send + Sync>, config: CoachConfig }`
  - Stub public methods: `latest_for`, `archive_for`, `set_preference`, `get_preference`, `mark_viewed`, `dismiss_banner`, `generate_for`. All return unimplemented `todo!()` for now — filled in T7.
- `testudo-exchange/crates/router/src/services/mod.rs` — MODIFIED: `pub mod coach;`
- `testudo-exchange/crates/router/src/services/coach/digest.rs` — NEW stub.
- `testudo-exchange/crates/router/src/services/coach/patterns/mod.rs` — NEW stub listing all 6 detectors as modules.
- `testudo-exchange/crates/router/src/services/coach/narrator.rs` — NEW stub trait + OpenAi impl skeleton.
- `testudo-exchange/crates/router/src/services/coach/validator.rs` — NEW stub.
- `testudo-exchange/crates/router/src/services/coach/schedule.rs` — NEW stub.

**Validate:** `cargo check --all-targets` — whole tree compiles with stubs.

**Acceptance:**
- All types defined, derive `Serialize`, `Deserialize`, `Debug`, `Clone` where appropriate.
- `cargo check` passes.
- No runtime behavior yet.

---

### T3: Baseline computation + detector orchestrator skeleton — `complete`

**Scope:** CP-1 foundation. Baseline + week-stats + week-trades SQL aggregations on `analytics_pool`. Empty `detect_all` orchestrator + empty `patterns/mod.rs` exports. Bump RSK-01 bucket helpers to `pub(crate)`. No detectors yet — each lands in its own task (T3a-T3f) so atomic-task discipline holds and a single broken detector doesn't retry-thrash the whole bundle.

**Files:**
- `testudo-exchange/crates/router/src/services/coach/digest.rs` — MODIFIED:
  - `pub async fn compute_user_baseline(analytics_pool: &PgPool, user_id: Uuid, as_of: DateTime<Utc>) -> Result<UserBaseline>`:
    - SQL aggregates on `journal_trades WHERE user_id = $1 AND closed_at BETWEEN $2 - INTERVAL '30 days' AND $2`.
    - `typical_session_hours_utc`: top-4 hours by trade count.
    - `setup_baselines`: grouped by `LOWER(COALESCE(setup_tag, '(untagged)'))`.
  - `pub async fn compute_week_stats(analytics_pool, user_id, week_start, week_end) -> Result<WeekStats>`.
  - `pub async fn fetch_week_trades(analytics_pool, user_id, week_start, week_end) -> Result<Vec<TradeEvidence>>`.
- `testudo-exchange/crates/router/src/services/coach/patterns/mod.rs` — NEW:
  - `pub fn detect_all(baseline: &UserBaseline, trades: &[TradeEvidence], stats: &WeekStats) -> Vec<FlaggedPattern>` returning `Vec::new()` for now; detector calls wired as each T3x lands.
  - Empty `pub use` block — populated by T3a-T3f.
- `testudo-exchange/crates/router/src/services/risk_snapshot.rs` — MODIFIED:
  - `fn bucket_for` → `pub(crate) fn bucket_for`
  - `fn extract_base_asset` → `pub(crate) fn extract_base_asset`

**Validate:** `cargo clippy --all-targets && cargo test coach::digest` — baseline + week_stats return stable shapes on seeded fixtures.

**Acceptance:**
- Baseline SQL returns stable `UserBaseline` on a seeded fixture (deterministic, `rust_decimal` throughout, no `f64`).
- `compute_week_stats` + `fetch_week_trades` round-trip a fixture week correctly.
- `detect_all` compiles, returns empty `Vec`, callable from T4's digest composer.
- `bucket_for` / `extract_base_asset` are `pub(crate)`; `risk_snapshot.rs` internal callers still compile.

---

### T3a: Pattern detector — `sizing_drift` — `complete`

**Scope:** CP-1 detector #1. Pure-logic function + 2 unit tests. Wire into `detect_all`.

**Files:**
- `testudo-exchange/crates/router/src/services/coach/patterns/sizing_drift.rs` — NEW:
  - `pub fn detect_sizing_drift(baseline: &UserBaseline, trades: &[TradeEvidence], _stats: &WeekStats) -> Option<FlaggedPattern>`.
  - Rule: last 3 post-loss trades' avg position size > 1.5 × `baseline.avg_position_size_usd` → `FlaggedPattern { pattern: PatternKind::SizingDrift, severity: Notable|Concerning based on multiplier, evidence: [trade_ids], metrics: { size_multiplier } }`.
  - All math in `rust_decimal::Decimal`.
- `testudo-exchange/crates/router/src/services/coach/patterns/mod.rs` — MODIFIED:
  - `pub use sizing_drift::detect_sizing_drift;`
  - Call from `detect_all`, push result if `Some`.

**Validate:** `cargo clippy --all-targets && cargo test coach::patterns::sizing_drift`.

**Acceptance:**
- Trigger fixture (3 post-loss trades sized 2× baseline) fires → returns `Some(FlaggedPattern)` with evidence = 3 trade IDs.
- Non-trigger fixture (post-loss sizing within baseline) returns `None`.

---

### T3b: Pattern detector — `frequency_spike` — `complete`

**Scope:** CP-1 detector #2. Pure-logic + 2 unit tests. Wire into `detect_all`.

**Files:**
- `testudo-exchange/crates/router/src/services/coach/patterns/frequency_spike.rs` — NEW:
  - `pub fn detect_frequency_spike(baseline: &UserBaseline, trades: &[TradeEvidence], _stats: &WeekStats) -> Option<FlaggedPattern>`.
  - Rule: any 6h rolling window this week has trade count > p90 of rolling 6h windows over `baseline` 30-day period.
  - `baseline` grows a field `p90_trades_per_6h: Decimal` (add to `UserBaseline` in T3's baseline SQL — revisit if missing).
- `testudo-exchange/crates/router/src/services/coach/patterns/mod.rs` — MODIFIED: add `pub use` + `detect_all` wire.

**Validate:** `cargo clippy --all-targets && cargo test coach::patterns::frequency_spike`.

**Acceptance:**
- Trigger fixture (6 trades in one afternoon vs baseline p90=3) fires → `Some(FlaggedPattern)` with evidence = trades in the spike window.
- Non-trigger (evenly-spaced week) → `None`.

---

### T3c: Pattern detector — `session_anomaly` — `complete`

**Scope:** CP-1 detector #3. Pure-logic + 2 unit tests. Wire into `detect_all`.

**Files:**
- `testudo-exchange/crates/router/src/services/coach/patterns/session_anomaly.rs` — NEW:
  - `pub fn detect_session_anomaly(baseline: &UserBaseline, trades: &[TradeEvidence], _stats: &WeekStats) -> Option<FlaggedPattern>`.
  - Rule: ≥ 2 trades this week in UTC hours NOT in `baseline.typical_session_hours_utc` (top-4).
- `testudo-exchange/crates/router/src/services/coach/patterns/mod.rs` — MODIFIED: `pub use` + wire.

**Validate:** `cargo clippy --all-targets && cargo test coach::patterns::session_anomaly`.

**Acceptance:**
- Trigger (2+ trades at 03:00 UTC vs baseline=[13,14,15,16]) → `Some`.
- Non-trigger (all trades in baseline hours) → `None`.

---

### T3d: Pattern detector — `setup_fatigue` — `complete`

**Scope:** CP-1 detector #4. Pure-logic + 2 unit tests. Wire into `detect_all`.

**Files:**
- `testudo-exchange/crates/router/src/services/coach/patterns/setup_fatigue.rs` — NEW:
  - `pub fn detect_setup_fatigue(baseline: &UserBaseline, trades: &[TradeEvidence], _stats: &WeekStats) -> Option<FlaggedPattern>`.
  - Rule: for any setup in `baseline.setup_baselines` with ≥ 5 baseline trades, compare its trailing-10 avg R (across baseline+week) to its all-time baseline avg R. If trailing-10 < 0.5 × baseline avg R → flag.
  - Uses RSK-02 `setup_tag` already on `TradeEvidence`.
- `testudo-exchange/crates/router/src/services/coach/patterns/mod.rs` — MODIFIED: `pub use` + wire.

**Validate:** `cargo clippy --all-targets && cargo test coach::patterns::setup_fatigue`.

**Acceptance:**
- Trigger (setup "breakout" baseline R=1.2, last 10 avg R=0.4) → `Some` with evidence = trailing-10 trade IDs for that setup.
- Non-trigger (setup still performing at baseline) → `None`.
- Setup with < 5 baseline trades never triggers (insufficient data).

---

### T3e: Pattern detector — `correlation_stack` — `complete`

**Scope:** CP-1 detector #5. Pure-logic + 2 unit tests. Wire into `detect_all`. Reuses RSK-01 `bucket_for`/`extract_base_asset` bumped to `pub(crate)` in T3.

**Files:**
- `testudo-exchange/crates/router/src/services/coach/patterns/correlation_stack.rs` — NEW:
  - `pub fn detect_correlation_stack(_baseline: &UserBaseline, trades: &[TradeEvidence], _stats: &WeekStats) -> Option<FlaggedPattern>`.
  - Rule: group week trades by `(bucket_for(extract_base_asset(symbol)), side)`. If any group has ≥ 3 trades whose open→close windows overlap concurrently for > 4h → flag.
  - Imports `crate::services::risk_snapshot::{bucket_for, extract_base_asset}`.
- `testudo-exchange/crates/router/src/services/coach/patterns/mod.rs` — MODIFIED: `pub use` + wire.

**Validate:** `cargo clippy --all-targets && cargo test coach::patterns::correlation_stack`.

**Acceptance:**
- Trigger (3 concurrent longs in ETH + ARB + OP = `L1` bucket, overlap 6h) → `Some` with evidence = 3 trade IDs.
- Non-trigger (concurrent positions in different buckets, or sequential not concurrent) → `None`.
- `pub(crate)` import of `bucket_for` compiles cleanly.

---

### T3f: Pattern detector — `streak_risk` — `pending`

**Scope:** CP-1 detector #6. Pure-logic + 2 unit tests. Wire into `detect_all`.

**Files:**
- `testudo-exchange/crates/router/src/services/coach/patterns/streak_risk.rs` — NEW:
  - `pub fn detect_streak_risk(_baseline: &UserBaseline, trades: &[TradeEvidence], _stats: &WeekStats) -> Option<FlaggedPattern>`.
  - Rule: sort week trades chronologically. Flag if (a) 3+ consecutive losses, OR (b) 5+ consecutive wins with position size monotonically non-decreasing.
- `testudo-exchange/crates/router/src/services/coach/patterns/mod.rs` — MODIFIED: `pub use` + wire. This is the last detector — `detect_all` now calls all six.

**Validate:** `cargo clippy --all-targets && cargo test coach::patterns`.

**Acceptance:**
- Trigger-loss (3 consecutive losses) → `Some` severity Notable.
- Trigger-win (5 wins, sizes 1,1.2,1.5,1.8,2.0) → `Some` severity Concerning.
- Non-trigger (mixed W/L pattern) → `None`.
- Full `cargo test coach::patterns` passes — all 6 detectors × 2 tests + baseline tests = 14+ green.

---

### T4: CoachDigest composer — `pending`

**Scope:** CP-2. `build_digest(pool, analytics_pool, user_id, week_start, week_end, config) -> Result<Option<CoachDigest>>`. Returns `Ok(None)` if skip rules fire. Otherwise orchestrates baseline → week_stats → week_trades → detect_all → filters `flagged_trades` to only those referenced by a flag.

**Files:**
- `testudo-exchange/crates/router/src/services/coach/digest.rs` — MODIFIED:
  - `pub async fn build_digest(pool: &PgPool, analytics_pool: &PgPool, user_id: Uuid, week_start, week_end, config: &CoachConfig) -> Result<Option<(CoachDigest, /* skip_reason */ ())>, BuildDigestError>`:
    1. Read `users.coach_enabled` — if FALSE, return `Ok(None)` with `skip_reason="opt_out"` logged.
    2. Lifetime trade count check (`SELECT COUNT(*) FROM journal_trades WHERE user_id = $1`) — if < `config.min_lifetime_trades`, `Ok(None)` with `skip_reason="lifetime_below_threshold"`.
    3. Week trade count — if < `config.min_week_trades`, `Ok(None)` with `skip_reason="week_below_threshold"`.
    4. Build baseline + week_stats + week_trades in parallel (`tokio::join!`).
    5. `detect_all(...)` → flagged_patterns. If empty → `Ok(None)` with `skip_reason="no_flags"`.
    6. `flagged_trades` = `week_trades` filtered to IDs referenced by any flag's `evidence`.
    7. Return `Ok(Some(CoachDigest { ... }))`.
- `testudo-exchange/crates/router/tests/coach_digest_snapshot_test.rs` — NEW:
  - Golden-snapshot test: seed a fixture week → assert `CoachDigest` JSON matches committed snapshot.

**Validate:** `cargo test coach::digest` — all tests pass; snapshot committed.

**Acceptance:**
- Skip rules return `Ok(None)`, never errors.
- Non-skipped digest has `flagged_patterns.len() > 0 ==> flagged_trades.len() > 0`.
- `flagged_trades` only contains trades referenced by ≥1 flag's `evidence`.

---

### T5: Narrator trait + OpenAI-compatible implementation — `pending`

**Scope:** CP-3 half-one. `Narrator` trait + `OpenAiNarrator` impl using `async-openai` against `OPENAI_BASE_URL`. Mock impl for tests.

**Files:**
- `testudo-exchange/crates/router/Cargo.toml` — MODIFIED: add `async-openai = "0.27"` (confirm latest at build time).
- `testudo-exchange/crates/router/src/services/coach/narrator.rs` — MODIFIED:
  - `#[async_trait] pub trait Narrator: Send + Sync { async fn narrate(&self, digest: &CoachDigest) -> Result<NarratedReport, NarratorError>; }`.
  - `pub struct OpenAiNarrator { client: async_openai::Client<OpenAIConfig>, model: String }`.
  - `impl Narrator for OpenAiNarrator`:
    - System message from `const SYSTEM_PROMPT: &str = include_str!("prompts/system.md")`.
    - User message: `serde_json::to_string(digest)?`.
    - Chat completion call; parse `choices[0].message.content` as `NarratedReport`.
    - Extract `usage.prompt_cache_hit_tokens` / `usage.prompt_tokens` for `cache_hit_ratio`.
    - Map errors to `NarratorError`.
  - `pub struct MockNarrator { pub response: std::sync::Mutex<Option<Result<NarratedReport, NarratorError>>> }`.
  - `impl Narrator for MockNarrator`.
- `testudo-exchange/crates/router/src/services/coach/prompts/system.md` — NEW: static system-role prompt (~8k tokens: role, taxonomy, JSON schema, citation rule, few-shot, tone).
- `testudo-exchange/crates/router/tests/coach_narrator_test.rs` — NEW:
  - MockNarrator returns configured response.
  - Parse-error mapping on malformed content.

**Validate:** `cargo clippy --all-targets && cargo test coach::narrator`.

**Acceptance:**
- Trait compiles object-safe (`Arc<dyn Narrator + Send + Sync>` used in CoachService).
- OpenAiNarrator constructs from `(base_url, api_key, model)`.
- MockNarrator drives success + all failure modes.
- `prompts/system.md` ≥ 4 kB (real content, not stub).

---

### T6: Citation validator — `pending`

**Scope:** CP-3 half-two. Hard grounding gate.

**Files:**
- `testudo-exchange/crates/router/src/services/coach/validator.rs` — MODIFIED:
  - `pub fn validate(report: &NarratedReport, digest: &CoachDigest) -> Result<(), ValidationError>`:
    - `HashSet<Uuid>` from `digest.flagged_trades.iter().map(|t| t.id)`.
    - `HashSet<&str>` from `digest.flagged_trades.iter().map(|t| t.short_id.as_str())`.
    - Each section's `citations` uuid must be in the set.
    - Regex `\[T-([0-9a-f]{8})\]` applied to every section body + headline; each capture must match a `short_id`.
- `testudo-exchange/crates/router/tests/coach_validator_test.rs` — NEW:
  - Positive: valid report passes.
  - Negative 1: uuid not in digest → `UnknownCitation`.
  - Negative 2: unknown token in body → `UnknownToken`.
  - Negative 3: token in headline only → `UnknownToken`.

**Validate:** `cargo test coach::validator`.

**Acceptance:** All four tests green; regex compiled once via `once_cell::sync::Lazy` or `lazy_static`.

---

### T7: Weekly scheduler + CoachService orchestration — `pending`

**Scope:** CP-6. `CoachService::generate_for` = digest → narrate → validate → persist. `schedule.rs` spawns tokio task firing once per Sunday 18:00 UTC.

**Files:**
- `testudo-exchange/crates/router/src/services/coach/service.rs` — MODIFIED:
  - `generate_for(user_id, week_start, week_end) -> Result<Option<StoredCoachReport>>`:
    1. `let digest = build_digest(...)?`; `None` → log + `Ok(None)`.
    2. Call `self.narrator.narrate(&digest).await`. On `Err`: log warn, set `narrated = None`.
    3. On `Ok(report)`: call `validate(&report, &digest)`. On `Err`: log warn + rejection details, set `narrated = None`.
    4. `persist(...)`: `INSERT INTO coach_reports (...) VALUES (...) ON CONFLICT (user_id, week_start) DO NOTHING RETURNING *`.
    5. Return `Ok(Some(stored))`.
  - `latest_for(user_id) -> Result<Option<(StoredCoachReport, bool /* has_new */)>>`: SELECT ordered by `generated_at DESC LIMIT 1`; `has_new` = `generated_at > COALESCE(coach_banner_last_viewed_at, '-infinity')`.
  - `archive_for(user_id, limit, offset)`: paginated SELECT.
  - `set_preference(user_id, enabled)`: UPDATE users.
  - `get_preference(user_id)`: SELECT coach_enabled.
  - `mark_viewed(user_id)`: UPDATE users SET coach_banner_last_viewed_at = NOW().
  - `dismiss_banner(user_id, report_id)`: UPDATE coach_reports SET banner_dismissed_at = NOW() WHERE id = $1 AND user_id = $2.
- `testudo-exchange/crates/router/src/services/coach/schedule.rs` — MODIFIED:
  - `pub fn spawn_weekly_task(coach_service: Arc<CoachService>, pool: PgPool, cancel: CancellationToken)`:
    - `tokio::spawn` with `loop { tokio::select! { _ = cancel.cancelled() => break, _ = tokio::time::sleep(Duration::from_secs(3600)) => { if is_trigger_moment(Utc::now()) && !already_fired_this_week(&pool, week_start).await.unwrap_or(true) { run_batch(&coach_service, &pool, week_start, week_end).await; } } } }`.
    - `is_trigger_moment(now)`: `now.weekday() == Weekday::Sun && now.hour() == 18`.
    - `already_fired_this_week(pool, week_start)`: `SELECT EXISTS (SELECT 1 FROM coach_reports WHERE week_start = $1)`.
    - `run_batch`: `SELECT id FROM users WHERE coach_enabled = TRUE` → bounded-concurrency (`buffer_unordered(10)`) `generate_for`.
- `testudo-exchange/crates/router/tests/coach_service_test.rs` — NEW:
  - Happy path with MockNarrator(Ok) → persists narrative row.
  - MockNarrator(Err) → persists stats-only row (`narrative_sections_json IS NULL`).
  - Validation-fail → persists stats-only row.
  - Idempotent: double `generate_for` → single row.

**Validate:** `cargo clippy --all-targets && cargo test coach::service`.

**Acceptance:** All four tests green; scheduler compiles with cancellation plumbing.

---

### T8: Routes + AppState wiring + config/env — `pending`

**Scope:** CP-4/CP-7/CP-8 backend surface.

**Files:**
- `testudo-exchange/crates/router/src/config.rs` — MODIFIED:
  - Add: `llm_base_url: String`, `llm_model: String`, `coach_enabled_global: bool` (default `true`), `coach_min_lifetime_trades: i64` (default 30), `coach_min_week_trades: i64` (default 3).
- `testudo-exchange/crates/router/src/types/app.rs` — MODIFIED:
  - Add `pub coach_service: Arc<CoachService>`.
- `testudo-exchange/crates/router/src/main.rs` — MODIFIED:
  - Pull `OPENAI_API_KEY` via `std::env::var`.
  - Build `OpenAiNarrator::new(base_url, api_key, model)`.
  - `let coach_service = Arc::new(CoachService::new(pool.clone(), analytics_pool.clone(), narrator, coach_config));`.
  - Add to AppState literal.
  - Spawn scheduler: `coach::schedule::spawn_weekly_task(app_state.coach_service.clone(), pool.clone(), shutdown_token.clone());`.
  - Register routes under `/api/v1/coach` nested scope wrapped with `JwtMiddleware`.
- `testudo-exchange/crates/router/src/routes/coach.rs` — NEW:
  - `GET /latest` → `{ data: Option<StoredCoachReport>, has_new_indicator: bool }`.
  - `GET /archive?limit=20&offset=0` → `{ data: Vec<StoredCoachReport> }`.
  - `GET /preference` → `{ coach_enabled: bool }`.
  - `PATCH /preference` body `{ enabled: bool }` → 204.
  - `POST /mark-viewed` → 204.
  - `PATCH /{report_id}/dismiss-banner` → 204.
- `testudo-exchange/crates/router/src/routes/mod.rs` — MODIFIED: `pub mod coach;`.

**Validate:** `cargo clippy --all-targets && cargo test` — full suite green.

**Acceptance:**
- All six endpoints return correct status codes + bodies.
- Unauthenticated → 401.
- Missing `OPENAI_API_KEY` → startup panic with descriptive message.

---

### T9: Frontend API client types + fetchers — `pending`

**Scope:** CP-4/CP-7 wire layer.

**Files:**
- `testudo-journal/src/api/client.ts` — MODIFIED:
  - Add interfaces: `UserBaseline`, `SetupBaseline`, `WeekStats`, `FlaggedPattern`, `TradeEvidence`, `NarrativeSection`, `CoachDigest`, `StoredCoachReport`, `CoachLatestResponse { data: StoredCoachReport | null; has_new_indicator: boolean }`, `CoachPreferenceResponse { coach_enabled: boolean }`.
  - Fetchers:
    - `fetchLatestCoachReport(): Promise<CoachLatestResponse>` — `fetchCrud('coach/latest')`.
    - `fetchCoachArchive(limit = 20, offset = 0): Promise<{ data: StoredCoachReport[] }>`.
    - `fetchCoachPreference(): Promise<CoachPreferenceResponse>`.
    - `setCoachPreference(enabled: boolean): Promise<void>` — PATCH.
    - `markCoachViewed(): Promise<void>` — POST.
    - `dismissCoachBanner(reportId: string): Promise<void>` — PATCH.
  - Decimal fields typed as `string`, uuid as `string`, dates as ISO `string`.

**Validate:** `cd testudo-journal && bun run build`.

**Acceptance:** `bun run build` exit 0; all six fetchers exported.

---

### T10: /desk/coach page + CoachReport + CoachArchive + NarrativeBlock — `pending`

**Scope:** CP-4/CP-5 read surface.

**Files:**
- `testudo-journal/src/pages/Coach.tsx` — NEW:
  - `createResource(fetchLatestCoachReport)` + `createResource(() => offset(), fetchCoachArchive)`.
  - `createResource(fetchCoachPreference)`.
  - `onMount` (if latest exists): `markCoachViewed()` fire-and-forget.
  - Pre-threshold UI: "N/30 trades to unlock the coach" (requires lifetime-trade count — piggyback on existing Overview stats or add tiny backend helper `GET /api/v1/coach/progress` returning `{ lifetime_trades, required }`; **decision at T10 build time**).
  - Opt-out toggle → `setCoachPreference`.
  - Privacy disclosure inline section stating LLM provider (sourced from `latest.data.model_used` or a config-echo).
  - Renders `<CoachReport report={latest()} />` + `<CoachArchive items={archive()?.data ?? []} />`.
- `testudo-journal/src/components/coach/CoachReport.tsx` — NEW:
  - Top: deterministic stats block (week dates, trade count, win rate, total PnL, flagged patterns list as badges).
  - Middle: `<NarrativeBlock sections={report.narrative_sections} flagged={report.digest.flagged_trades} />` OR "● coach unavailable this week" fallback.
  - Bottom: metadata (model_used, generated_at, cache_hit_ratio).
- `testudo-journal/src/components/coach/NarrativeBlock.tsx` — NEW:
  - Pre-process: regex `/\[T-([0-9a-f]{8})\]/g` → `<a href="/desk/trades?trade={uuid}">T-xxxxxxxx</a>` using `flagged.find(t => t.short_id === match[1])`.
  - Hand resulting markdown string to existing `MarkdownPreview`.
- `testudo-journal/src/components/coach/CoachArchive.tsx` — NEW:
  - Paginated list: `week_start` date + headline + pattern badges.
  - MVP: expand inline on click (no modal).
- `testudo-journal/src/pages/Trades.tsx` — MODIFIED:
  - `useSearchParams()`: on mount, if `trade` present, pre-open `TradeDetail` modal.
- `testudo-journal/src/index.tsx` — MODIFIED:
  - Add `<Route path="/coach" component={lazy(() => import('./pages/Coach'))} />`.

**Validate:** `bun run build`; manual smoke across all four states (no report / last-week-only / narrative / stats-only fallback).

**Acceptance:**
- All four states render without errors.
- `[T-xxx]` links navigate to `/desk/trades?trade={uuid}` and open modal.
- Opt-out round-trips.
- Privacy disclosure names provider.

---

### T11: CoachBanner live + nav entry + HELP entries — `pending`

**Scope:** CP-7/CP-8 discovery.

**Files:**
- `testudo-journal/src/components/account/CoachBanner.tsx` — MODIFIED:
  - Replace `return null` with `createResource(fetchLatestCoachReport)`.
  - `null` data OR `banner_dismissed_at` set → `return null`.
  - Else render border-bounded row: left (● new indicator pulsing green when `has_new_indicator`, headline, "view coach report →" link), right (dismiss button).
  - Dismiss → `dismissCoachBanner(report.id)` + `mutate` resource to hide.
  - Body click → `useNavigate()('/coach')`.
- `testudo-journal/src/components/Layout.tsx` — MODIFIED: `NAV_ITEMS` gets `{ path: '/coach', label: 'COACH' }`.
- `testudo-journal/src/lib/help-content.ts` — MODIFIED:
  - `page.coach`, `coach.narrative`, `coach.citations`, `coach.provider`, `coach.patterns.sizing_drift`, `coach.patterns.frequency_spike`, `coach.patterns.session_anomaly`, `coach.patterns.setup_fatigue`, `coach.patterns.correlation_stack`, `coach.patterns.streak_risk`.

**Validate:** `bun run build`; manual: banner appears on Account when a report exists; dismiss + nav + HELP entries all surface.

**Acceptance:**
- Banner renders only when non-dismissed report exists.
- `● new` green-pulse → green-static after /desk/coach visit (next `has_new_indicator` refresh).
- Body-click navigates; dismiss button does not navigate.
- Nav "COACH" link present on desktop + mobile.

---

### T12: Final verification + commit — `pending`

**Scope:** Completion Protocol.

**Verifications:**
- `cd testudo-exchange && cargo clippy --all-targets && cargo test` — all green, 0 new warnings beyond the pre-existing 3 (actor.rs:1842, cex_client.rs:653, evaluator.rs:188).
- `cd testudo-journal && bun run build` — exit 0.
- Migration up+down clean on a fresh DB.
- Integration grep: `coach_reports`, `CoachService`, `CoachDigest`, `NarratedReport` wired consistently across router; `CoachReport`, `fetchLatestCoachReport`, `CoachBanner` wired consistently across journal.

**Manual QA:**
- Seed a test user with 30+ lifetime trades + 4 trades this week engineered to trigger ≥2 patterns. Invoke `generate_for` manually (direct DB + restart, or tests harness). Confirm `/desk/coach` narrative with citations; banner `● new`; dismiss flow; opt-out flow; stats-only fallback via unreachable base URL.

**Deferred to live session:**
- Production prompt-cache hit rate measurement (≥ 70% criterion).
- First-20-reports human review.

**Commit plan:**
- T1: `feat(rsk-03): migration — coach_reports + users coach prefs`
- T2: `feat(rsk-03): coach module scaffolding + types`
- T3: `feat(rsk-03): baseline + 6 pattern detectors`
- T4: `feat(rsk-03): CoachDigest composer + skip rules`
- T5: `feat(rsk-03): Narrator trait + OpenAI-compatible impl`
- T6: `feat(rsk-03): citation validator`
- T7: `feat(rsk-03): weekly scheduler + CoachService orchestration`
- T8: `feat(rsk-03): coach HTTP routes + AppState + config`
- T9: `feat(rsk-03): journal api client — coach types + fetchers`
- T10: `feat(rsk-03): /desk/coach page + CoachReport + NarrativeBlock`
- T11: `feat(rsk-03): CoachBanner live + nav + HELP`
- T12: umbrella: `feat(rsk-03): weekly AI trade coach — pattern detection + narrated report (in-app only)`

**Archive:** Move `.specify/specs/RSK-03-ai-trade-coach/` → `.specify/spec-archive/` after T12.

---

## Discoveries

### 2026-04-19 — RSK-03 planning

1. **Coach module in router/services/, not db-processor/.** db-processor is a 78-line queue worker with no library surface, no analytics helpers, no baseline code — the spec's citation of it as "natural home per memory" is inaccurate. Router/services/ is where every mature async service lives (`journal_service`, `journal_stats`, `journal_timeseries`, `risk_snapshot`, `fill_detector`, `rehydration`, `reconciliation`, `trade_event_writer`, `import_worker`, `ws_subscription_manager`), and it already owns `pool`, `analytics_pool`, `JwtMiddleware`, and the existing `tokio::time::interval` scheduler pattern. Placing coach there avoids a new library-crate boundary.

2. **Users table extension preferred over user_preferences table.** Only two per-user fields needed (`coach_enabled` + `coach_banner_last_viewed_at`). A separate table would add a JOIN per auth-gated read with no benefit at MVP scope.

3. **`users` model struct must be updated alongside the T1 migration.** Any `SELECT * FROM users` or `FromRow` usage in `PostgresUserRepository` must add both new columns, else compile breaks.

4. **Parallel opportunity after T2: T3/T4/T5/T6.** Four pure-logic modules depend only on types. Single-agent BUILD stays sequential; fast-follow could dispatch 4 agents into worktrees.

5. **`coach_config` thresholds table deferred.** MVP uses hardcoded `const Decimal` thresholds in detectors. Follow-up migration adds the table only when tuning is actually needed.

6. **`include_str!("prompts/system.md")` for cached prefix.** Byte-stable string compiled into binary = reliable prompt-cache hits, no runtime file read, no deploy-time coupling.

7. **Reuse RSK-01 bucketing via `pub(crate)`.** `bucket_for` + `extract_base_asset` in `risk_snapshot.rs` are single source of truth for asset-family taxonomy. Mark `pub(crate)`, import from `correlation_stack.rs`.

8. **Trade deep-link via `?trade={uuid}` on Trades page, not a new route.** One-liner `useSearchParams()` on mount pre-opens the modal. Preserves existing Trades UX; avoids new route boilerplate.

9. **`async-openai` is the first LLM dep in repo.** Added to `crates/router/Cargo.toml`. Pointed at `OPENAI_BASE_URL` for DeepSeek/GLM/OpenRouter compatibility.

10. **Scheduler: hourly tick + weekly SQL gate, not a cron-expr lib.** MVP: `is_trigger_moment(now)` (Sun 18:00 UTC) + `already_fired_this_week(pool, week_start)` idempotent guard. 20 lines, zero new deps, restart-safe. Future upgrade to cron-expr possible; `COACH_CRON` env reserved as placeholder.

11. **MockNarrator DI enables pure unit tests for CoachService.** `Arc<dyn Narrator + Send + Sync>` lets happy path, narrator-failure, and validation-failure all test without HTTP mocking.

12. **Citation token `[T-{first_8_hex}]`.** Frontend regex `/\[T-([0-9a-f]{8})\]/g`; backend includes `short_id` on each `TradeEvidence`. Validator enforces both forms (uuid in `citations`, token in `body`/`headline`).

13. **`digest_json` persisted alongside narrative.** Transparency + debugging — user can audit why a flag fired; enables future re-narration over older digests if a prompt changes.

14. **Stats-only fallback sentinel: `model_used = "unavailable"`.** Distinct from the real provider name so the frontend can render "coach unavailable this week" without guessing.

15. **No Overview changes.** Coach discovery surface = Account banner + nav + /desk/coach. Overview stays pure data-dashboard.

---

## Status

PLANNING COMPLETE

Spec: RSK-03-ai-trade-coach
Total Tasks: 18 (T1, T2, T3, T3a–T3f, T4–T12)
Ready for BUILD mode.

Note: T3 was split 2026-04-19 into T3 (baseline + orchestrator skeleton) + T3a-T3f (one detector each) to honour atomic-task discipline. The original T3 bundled 7 concerns (baseline + 6 detectors + 12 unit tests) which would cause retry-thrash if any single detector failed validation. Each T3x is now independently completable + committable. Recommended `--max-iterations 22` for build (18 tasks × 1.2 retry budget + 2 slack).

Next task: T3f — Pattern detector — streak_risk
