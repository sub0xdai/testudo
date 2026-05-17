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
