# ENG-01a Learnings

## T1 (2026-04-21)
Migration placed `dignitas_pill_hidden` directly on `users` table via `ALTER TABLE`. The single migration file covers both tables + the column; no need to split.

## T2 (2026-04-21)
`DignitasWeights::without_coach()` renormalizes in-place. The degenerate-config guard (four_sum = 0) returns equal quarters to avoid division-by-zero.

## T3 (2026-04-21)
Fixtures defined purely in terms of InputContributions — no P&L fields. The `spec_weights()` helper mirrors the migration seed exactly so the ungameability assertion is a contract test against the actual deployed defaults.

## T4 (2026-04-21)
`compute_coach_severity_penalty` returns `0.0` for empty input — callers MUST check `report_severities.is_empty()` and invoke `DignitasWeights::without_coach()` before computing the composite score. A `0.0` penalty is NOT the same as a clean coach record.

## T5 (2026-04-21)
- `compute_score` formula: `100 × Σ(weight_i × adjusted_input_i)` where coach axis flips the penalty to `(1 − coach_severity_penalty)`. Clamp to `[0, 100]`.
- `take_daily_snapshot` cold-start check counts EXISTING rows before today's upsert. If count < `cold_start_min_days`, score is pinned to 50.
- Risk-per-trade consistency uses the user's most-recent total equity (sum across all exchange accounts) as the denominator. This is a balance-at-snapshot proxy, not true balance-at-entry. Acceptable for MVP; can be refined with a LATERAL join on balance_snapshots when per-trade precision matters.
- `RiskConfig` is loaded from `cache_entries` (key `risk:config:{user_id}`, JSONB). If the cache entry has expired or never been set, the orchestrator falls back to hardcoded defaults (5% daily drawdown, 2% risk/trade). No coach_reports → uses `without_coach()` renormalization.
- The `test_me_returns_user_info` auth test is a pre-existing failure (unrelated to dignitas).

## T11 (2026-04-21)
`coach_severity_penalty` is a penalty (high = bad), so the transparency page inverts it to `1 - value` for the "COACH ALIGNMENT" display row. The `inverted` flag in the `InputRow` definition handles this cleanly. The bar color, displayed percentage, and points contribution all use the inverted value consistently.

## T9b (2026-04-21)
`testudo-journal` is NOT a submodule — it's a plain directory in the parent testudo repo. Commits made while `cwd` is `testudo-journal/` go to the parent repo directly; no submodule pointer bump step is needed. The "submodule pointer bump" commits in T2–T8 refer to `testudo-exchange`, not `testudo-journal`.
`PerformanceRadar` now fetches its own data via `createResource(fetchDignitasMe)` — no props. The API already returns `contributions` on `DignitasCurrent`, so no extra endpoint is needed. Coach Alignment cold_start dim uses ECharts `indicator[].color` per-item override.

## Cold-start redesign (2026-04-25)

**Symptom that triggered the rework.** A user with one closed trade in 30 days saw `DIGNITAS 50.0 —` for weeks. The `cold_start = (count_of_dignitas_history_rows < 7)` rule treated "scheduler hasn't run for 7 days yet" as a proxy for "not enough signal" — fine on paper, bad in practice. The proxy decoupled from the underlying signal: a user trading actively but registered after a missed midnight could stay pinned at 50 longer than a user who'd never traded. Worse, the `dec!(50)` short-circuit meant the score the user saw was not the score the inputs implied — a small but real form of dishonesty.

**What we changed.**
1. `cold_start = trade_count_30d < cold_start_min_trades` (default 10). Replaces the snapshot-count gate entirely. The signal that drives the score (closed trades feeding setup_adherence, risk_consistency, journal_consistency) is now the same signal that decides whether the score is "firm".
2. Removed the `if cold_start { dec!(50) }` short-circuit in `take_daily_snapshot`. The score is always the real computed value.
3. Persisted `trade_count_30d` on `dignitas_history` so `/api/dignitas/me` returns it without a second query — the pill is on every page so this matters.
4. UI copy in `DignitasPanel` shifted from "NEUTRAL — BUILDING BASELINE" to "PRELIMINARY — N of 10 trades". A trader knows exactly how many more trades will firm the score; they can act on it.

**Why n=10.** Setup adherence and journal consistency are means over the trade set. Standard error on a fraction halves at √n. n=10 puts SE at ~±15 percentage points; below n=10 the score moves more from sample noise than from behavior change — which is the exact "score feels arbitrary" UX failure the original 50-pin was trying to avoid. Same goal, sharper instrument.

**The original Risk #4 mitigation still holds.** "100-and-only-goes-down" was the worry. With trade-count gating + the `cold_start` flag still surfacing in the UI, a user with 3 trades sees their real score *labelled preliminary*. They get the truth (real number) and the context (preliminary) at once, instead of a placeholder that was wrong twice (wrong number, no context).

**Migration.** `20260425000000_dignitas_trade_count.up.sql` adds `dignitas_history.trade_count_30d INTEGER NOT NULL DEFAULT 0` and seeds `dignitas_config.cold_start_min_trades = 10`. Existing rows backfill to 0 (effectively cold-start until next scheduler run rewrites them — which is correct: a row written under the old rule has no trade-count metadata to trust). The legacy `cold_start_min_days` config row is now dead but left in place for forward/backward compatibility; safe to clean up in a later migration.

**Files touched.** `migrations/20260425000000_*`, `services/dignitas/{config,snapshot,types}.rs`, `routes/dignitas.rs`, `testudo-journal/src/api/client.ts`, `testudo-journal/src/components/DignitasPanel.tsx`, `testudo-journal/src/lib/help-content.ts`.

**Tests.** Added `cold_start_returns_real_score_not_50` as a regression guard against the old short-circuit. Existing `cold_start_ungameability_invariant` and `compute_score_on_cold_start_matches_4_axis_composite` still pass and now exercise the live path rather than a dead branch.
