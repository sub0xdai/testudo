# AGENT-03-journal-memory — Implementation Plan

## Current State Summary

The journal analytics pipeline is mature and battle-tested: `StatsEngine` (in `services/journal_stats.rs`) computes account overview, performance stats, and risk stats via SQL-side aggregation — streaks, drawdowns, day/week/month extremes are all computed server-side. `TimeSeriesService` (in `services/journal_timeseries.rs`) produces equity curves, daily P&L, symbol breakdowns, setup breakdowns (by `setup_tag`), duration-profit scatter data, return distributions, and time distributions. Together these power 11 chart types behind the human-facing `/journal/analytics/*` endpoints.

The coach pipeline (`services/coach/`) has six deterministic pattern detectors (sizing drift, frequency spike, session anomaly, setup fatigue, correlation stacking, streak risk) that flag behavioral patterns weekly, stored in `coach_reports`. The `CoachDigest` type already carries `TradeEvidence` entries with `short_id` (first 8 hex chars of UUID) — the citation token convention `[T-xxxxxxxx]` the spec requires. The `CoachService` exposes `latest_for()` and `archive_for()` for reading stored reports.

What's missing is the agent-facing query surface. All the underlying computation exists — `StatsEngine::account_overview()`, `StatsEngine::performance_stats()`, `StatsEngine::risk_stats()`, `TimeSeriesService::setup_breakdown()`, `TimeSeriesService::equity_curve()`, and `CoachService::latest_for()` provide every data point the agent endpoints need. The gaps are:

1. **Filter gaps**: `StatsFilter` only supports `exchange`, `symbol`, `date_from`, `date_to`, and `tags`. The spec requires `source` (to isolate agent trades), `setup_tag` (to filter by strategy), and `side` (LONG/SHORT) filtering. These columns exist in `journal_trades` but aren't filtered on in any analytics SQL query.

2. **No composition layer**: No service combines StatsEngine + TimeSeriesService into a single `AgentSummary` response. No formatter turns that summary into LLM-optimized markdown.

3. **No on-demand insights**: The coach pipeline generates weekly digests on a cron. The spec needs ad-hoc insight extraction from the latest stored digest, adapted for agent consumption.

4. **No comparison logic**: Running StatsEngine twice and computing deltas between two time periods doesn't exist.

5. **No route handlers**: Three endpoints need handlers, route registration, and JWT middleware wiring.

No new database migrations are needed — all columns exist. No new crates or dependencies. Pure Rust composition over existing machinery.

---

## Checkpoints

### CP-1: Models + StatsFilter extension + SQL filter support ✅
- **Touches**: `crates/router/src/models/agent_journal.rs` (NEW), `crates/router/src/models/mod.rs`, `crates/router/src/services/journal_stats.rs`, `crates/router/src/services/journal_timeseries.rs`
- **Tasks**:
  1. Create `models/agent_journal.rs` with all wire types from the spec: `AgentSummaryQuery`, `SummaryFormat`, `AgentSummary`, `OverallStats`, `SetupBreakdown`, `TradeCitation`, `AgentInsight`, `PatternKind`, `Severity`, `CompareRequest`, `TimeframeRange`, `ComparisonResult`, `MetricDelta`, `DeltaDirection`, `PaginatedInsights`.
  2. Register `pub mod agent_journal;` in `models/mod.rs`.
  3. Extend `StatsFilter` with three new fields: `source: Option<String>`, `setup_tag: Option<String>`, `side: Option<String>`.
  4. Update all SQL queries in `StatsEngine` (`aggregate_trades`, `fetch_streaks_sql`, `fetch_drawdown_sql`, `fetch_day_extremes`, `fetch_rolling_extremes`) to filter by `source`, `setup_tag`, and `side` when non-None.
  5. Update all SQL queries in `TimeSeriesService` (`equity_curve` → `fetch_daily_aggregates`, `symbol_breakdown`, `setup_breakdown`, `duration_profit`, `time_distribution`) to filter by the new fields.
  6. Update the `fetch_daily_aggregates` helper (used by equity_curve, daily_pnl, return_distribution) to accept the new filters.
- **Verification**: `cargo test -p router -- stats` passes all existing StatsEngine tests. `cargo clippy --all-targets && cargo test` in testudo-exchange passes.
- **Commit message**: `feat: extend StatsFilter with source, setup_tag, and side filtering`

Completed 2026-05-21 by /skill:vox build.

### CP-2: Agent journal service + JSON summary endpoint ✅
- **Touches**: `crates/router/src/services/agent_journal.rs` (NEW), `crates/router/src/services/mod.rs`, `crates/router/src/routes/agent_journal.rs` (NEW), `crates/router/src/routes/mod.rs`, `crates/router/src/main.rs`
- **Tasks**:
  1. Create `services/agent_journal.rs` with `AgentJournalService` struct holding `PgPool` + `analytics_pool`. Implement `build_summary(user_id, query) -> AgentSummary` that:
     - Translates `AgentSummaryQuery` → `StatsFilter` (timeframe → date_from/date_to, plus symbol, side, setup_tag, exchange, source)
     - Calls `StatsEngine::account_overview()`, `performance_stats()`, `risk_stats()` in parallel (tokio::join!)
     - Calls `TimeSeriesService::setup_breakdown()`, `TimeSeriesService::equity_curve()`
     - Fetches top-10 trades by `r_multiple DESC NULLS LAST` (or `net_pnl DESC` when r_multiple is null) as `TradeCitation` entries with short_id
     - Assembles `AgentSummary { timeframe, overall, by_setup, top_trades, equity }`
  2. Register `pub mod agent_journal;` in `services/mod.rs`.
  3. Create `routes/agent_journal.rs` with:
     - `get_summary` handler: parse `AgentSummaryQuery` from query params, call `agent_journal_service.build_summary()`, return JSON
     - Error handling: missing auth → 401, invalid date range → 400, empty result → 200 with zeroed stats
  4. Register `pub mod agent_journal;` in `routes/mod.rs`.
  5. Wire in `main.rs`:
     - Instantiate `AgentJournalService::new(pg_pool.clone(), analytics_pool.clone())` — store as `Arc<AgentJournalService>` or as a field in `AppState`
     - Add `web::scope("/journal/agent")` under the `/api/v1` scope with `GET /summary` → `agent_journal::get_summary`, wrapped with `JwtMiddleware`
  6. Unit tests: valid query → 200 with non-zero stats, missing timeframe → defaults to "90d", empty user (no trades) → 200 with zeroed data, unauthenticated → 401.
- **Verification**: `cargo test -p router -- agent_journal` passes. Manual: `GET /api/v1/journal/agent/summary?format=json&timeframe=90d` returns structured JSON with overall stats, by_setup array, top_trades array, and equity curve.
- **Commit message**: `feat: GET /journal/agent/summary?format=json — agent performance summary`

Completed 2026-05-21 by /skill:vox build.

### CP-3: LLM markdown formatter + format=llm ✅
- **Touches**: `crates/router/src/services/agent_journal_formatter.rs` (NEW), `crates/router/src/services/mod.rs`, `crates/router/src/routes/agent_journal.rs`, `crates/router/src/services/agent_journal.rs`
- **Tasks**:
  1. Create `services/agent_journal_formatter.rs` with a pure function `format_summary_llm(summary: &AgentSummary) -> String` that produces markdown matching the spec's template:
     - `## Journal Summary: {symbols} ({timeframe})`
     - `### Overall Performance` table with trade_count, win_rate, avg_r, total_pnl, max_drawdown, profit_factor, sharpe_ratio
     - `### By Setup Tag` markdown table with columns: Setup | Trades | Win Rate | Avg R | P&L
     - `### Top Performers` unordered list: `- [T-{short_id}] {symbol} {side} — {setup}, {r}R, opened {date}`
     - `### Actionable Insights` section with auto-generated observations: best/worst setup, stop distance heuristic (if available), session timing (if available in time_distribution)
  2. Register `pub mod agent_journal_formatter;` in `services/mod.rs`.
  3. Add a `format_llm` path in `routes/agent_journal.rs`: when `format=llm`, call `format_summary_llm(&summary)`, set `Content-Type: text/markdown`, return the string body.
  4. Add `build_summary_with_insights()` to `AgentJournalService` that also fetches time_distribution data for session timing heuristics and computes "tight stops correlate with losses" (trades where SL distance < 1.5% of entry and result is loss).
  5. Unit test: `format_summary_llm` with sample data produces markdown containing `[T-xxxxxxxx]` citation tokens, `### By Setup Tag`, `### Top Performers`.
- **Verification**: `cargo test -p router -- agent_journal_formatter` passes. Manual: `GET /api/v1/journal/agent/summary?format=llm` returns `Content-Type: text/markdown` with valid markdown.
- **Commit message**: `feat: LLM markdown formatter with [T-xxxxxxxx] citation tokens`

Completed 2026-05-21 by /skill:vox build.

### CP-4: Insights endpoint from coach patterns ✅
- **Touches**: `crates/router/src/services/agent_journal.rs`, `crates/router/src/routes/agent_journal.rs`
- **Tasks**:
  1. Add `build_insights(user_id) -> Vec<AgentInsight>` to `AgentJournalService`:
     - Call `CoachService::latest_for(user_id)` to get the latest `StoredCoachReport`
     - Iterate `digest.flagged_patterns`, map each `FlaggedPattern` → `AgentInsight`:
       - `pattern`: map `PatternKind` to spec enum (SizingDrift, FrequencySpike, etc.)
       - `severity`: map `Severity` (Info → Info, Notable → Notable, Concerning → Concerning)
       - `headline`: human-readable one-liner per pattern (e.g., "Position sizes are 2.1× your 30-day average")
       - `detail`: expanded description using `metrics` JSON blob
       - `recommendation`: actionable guidance (e.g., "Reduce position size to baseline levels")
       - `evidence_count`: `evidence.len()` as i64
     - Also compute ad-hoc insights: low win-rate setups (from week_stats.by_setup where win_rate < 40%), stop distance analysis (when available)
     - Support limit query param (default 10)
  2. Add `get_insights` handler in `routes/agent_journal.rs`:
     - Call `agent_journal_service.build_insights(user_id)`
     - Return `PaginatedInsights { insights, total }`
  3. Wire `GET /journal/agent/insights` in main.rs (under same `web::scope("/journal/agent")`).
  4. Unit tests: insights from stored coach report with sizing_drift pattern → correct headline/severity/recommendation, empty user (no coach reports) → 200 with empty list, unauthenticated → 401.
- **Verification**: `cargo test -p router -- agent_journal` passes insights tests. Manual: `GET /api/v1/journal/agent/insights` returns pattern list with headline/detail/recommendation for each.
- **Commit message**: `feat: GET /journal/agent/insights — pattern detection from coach pipeline`

Completed 2026-05-21 by /skill:vox build.

### CP-5: Compare endpoint ✅
- **Touches**: `crates/router/src/services/agent_journal.rs`, `crates/router/src/routes/agent_journal.rs`
- **Tasks**:
  1. Add `build_comparison(user_id, request: CompareRequest) -> ComparisonResult` to `AgentJournalService`:
     - Translate `period_a` and `period_b` `TimeframeRange` → two `StatsFilter` instances (date_from/date_to swapped)
     - Apply shared `filters` if provided (symbol, setup_tag, exchange, source)
     - Run `StatsEngine::performance_stats()` and `StatsEngine::risk_stats()` for both periods in parallel (two `tokio::join!` calls)
     - Compute `MetricDelta` for each metric: trade_count, win_rate, avg_r, total_pnl, max_drawdown, profit_factor, sharpe_ratio
     - `delta_pct = (value_b - value_a) / value_a * 100` (handle division by zero: if value_a == 0 and value_b > 0, delta_pct = 100; if both zero, delta_pct = 0)
     - `direction`: delta_pct > 5% → Improved, delta_pct < -5% → Declined, otherwise → Neutral (invert for drawdown: negative delta = improved)
     - Run `TimeSeriesService::setup_breakdown()` for both periods, compute per-setup deltas (only for setups present in either period)
     - Assemble `ComparisonResult { period_a: PeriodInfo, period_b: PeriodInfo, deltas, by_setup_deltas }`
  2. Add `post_compare` handler in `routes/agent_journal.rs`:
     - Validate `CompareRequest` (from/to required, date ranges must be valid)
     - Call `agent_journal_service.build_comparison(user_id, request)`
     - Return 200 with `ComparisonResult`, 400 on invalid date ranges
  3. Wire `POST /journal/agent/compare` in main.rs.
  4. Unit tests: two periods with identical data → all deltas neutral, period B worse → Declined on loss metrics, period B better → Improved, missing from → 400, invalid date order → 400, unauthenticated → 401.
- **Verification**: `cargo test -p router -- agent_journal` passes comparison tests. `cargo clippy --all-targets && cargo test` in testudo-exchange passes.
- **Commit message**: `feat: POST /journal/agent/compare — period-over-period performance comparison`

Completed 2026-05-21 by /skill:vox build.

---

## Risks & Open Questions

1. **Coach insights freshness** — The coach pipeline runs weekly (Sun 18:00 UTC). Between runs, `GET /journal/agent/insights` returns the latest stored report's patterns, which may be up to 7 days stale. Mitigation: document this. A future enhancement could compute patterns on-demand by calling `build_digest()` with a custom window, but that's expensive (30-day baseline + 6 detectors + LLM narration). NOT in scope for this spec.

2. **StatsFilter extension is a breaking change to the function signature** — `StatsFilter` is used by `JournalService::list_trades()` and all analytics endpoints. Adding three `Option<String>` fields is backwards-compatible (serde skips None by default), but the SQL query changes in `aggregate_trades` and the timeseries methods will add 3 new bind parameters. All existing callers pass a `StatsFilter` that will default to `None` for the new fields, so behavior is preserved. Unit tests confirm.

3. **AppState field for AgentJournalService** — Currently `AppState` has 19 fields. Adding `agent_journal_service: Arc<AgentJournalService>` brings it to 20. This is the simplest approach — no new `web::Data<>` injectors needed, matching the pattern of `coach_service`. Alternative: inject as separate `web::Data` like the auth dependencies. Choosing AppState field for consistency with existing analytics services.

4. **"Actionable insights" in LLM markdown are heuristic, not LLM-generated** — The spec shows an insights section in the markdown output. These are auto-generated from stats (best/worst setup, stop distance heuristic) — NOT generated by an LLM call. The coach pipeline's LLM narration is separate and surfaced via `/journal/agent/insights`. This distinction matters: the summary endpoint is fast and deterministic; the insights endpoint may return stale weekly data.

5. **Top trades query** — The spec shows top-5 trades by R-multiple in the LLM format. The query orders by `r_multiple DESC NULLS LAST` to prioritize R-multiple but falls back to `net_pnl DESC` for trades without R data. Trades with NULL `r_multiple` still appear in the list, just sorted after those with R values.

6. **TigerBeetle alignment** — StatsEngine already follows TigerBeetle philosophy (SQL-side aggregation, no `Vec` allocations on hot path, Decimal for money, typed errors). The new service layer will be thin composition with no dynamic allocation on the hot path. Assertion density: every public function will assert user_id is non-nil and dates are valid before querying.

7. **No new dependencies** — All computation uses existing `rust_decimal`, `chrono`, `sqlx`, `serde`, `uuid`, `actix-web`. No changes to Cargo.toml.

---

Plan ready: 5 checkpoints, ~10–14 hours total. Run `/skill:vox build AGENT-03-journal-memory` to start CP-1.
