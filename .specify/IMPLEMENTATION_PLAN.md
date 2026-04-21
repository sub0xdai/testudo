# Implementation Plan

> Last updated: 2026-04-21
> Current spec: ENG-01a-dignitas-score-living
> Phase: PLAN complete — awaiting BUILD kickoff (one blocker)

---

## Active Spec: ENG-01a-dignitas-score-living

Dignitas Score as a living artifact: daily snapshot persistence, top-nav pill with 7-day delta, 90-day sparkline panel, transparency page. Ungameable by design (no frequency / P&L / win-rate inputs).

### Tasks

| Task | Description | Status |
|------|-------------|--------|
| T1 | Migrations: `dignitas_history`, `dignitas_config` (seeded w/ formula weights), `users.dignitas_pill_hidden` column | pending |
| T2 | Core types: `services/dignitas/{mod,types}.rs` — `DignitasSnapshot`, `InputContributions`, `DignitasWeights` | pending |
| T3 | **RED**: `tests/dignitas_snapshot_test.rs` — 1000 undisciplined high-freq high-P&L trades must score LOWER than 100 disciplined trades (FR-9 gate). Fails until T5 lands. | pending |
| T4 | `services/dignitas/inputs.rs` — 5 pure functions mapping DB rows → `[0..1]` contributions. Unit-tested per input. See D2 for semantics. | pending |
| T5 | `services/dignitas/{snapshot,config}.rs` — orchestrator: load weights, assemble inputs, apply formula, handle cold-start, upsert into `dignitas_history`. T3 turns **GREEN**. | pending |
| T6 | `services/dignitas/schedule.rs` — daily scheduler mirroring `coach/schedule.rs`; UTC 00:30 trigger; batch 500 users + `tokio::yield_now()`; idempotency via `UNIQUE(user_id,date)`. Wire spawn in `main.rs`. | pending |
| T7 | `routes/dignitas.rs` — `GET /api/dignitas/me` (score + delta_7d + cold_start + pill_hidden), `GET /api/dignitas/history?days=90`, `PATCH /api/dignitas/preferences`. Wire under auth middleware. | pending |
| T8 | `testudo-journal/src/api/client.ts` — `DignitasCurrent`, `DignitasHistory` types + `fetchDignitasMe/History` + `patchDignitasPreference`. | pending |
| T9 | `components/DignitasPill.tsx` + mount in `Layout.tsx` top nav; score + signed delta + color (score-green/score-red/tertiary); respect `pill_hidden`; cold-start label (`50 —`). **Plus: rewrite `PerformanceRadar.tsx` to 5-axis InputContributions per D1 resolution.** Delete client-side normalization; read `InputContributions` from `/api/dignitas/me`. | pending |
| T10 | `components/DignitasPanel.tsx` + `DignitasSparkline.tsx` (EChart line) — popover: current score, 90-day sparkline, hide-pill shortcut, "View breakdown →" link to `/desk/dignitas`. **No input breakdown inside the popover** (that role moves to the Overview radar per D1 resolution B′). | pending |
| T11 | `pages/Dignitas.tsx` at `/desk/dignitas` — transparency table (input × user-value × weight × explanation) + formula; register route in `index.tsx`; add `help-content.ts` entries (`dignitas.pill`, `dignitas.transparency`, `dignitas.inputs.*`). | pending |
| T12 | Final verification: `cargo clippy --all-targets && cargo test` + `cd testudo-journal && bun run build`; manual QA notes; archive spec. | pending |

**Task ordering is dependency-correct:** T1→T2→T3→T4→T5 (Red→Green), T5→T6, T5→T7, T7→T8→(T9‖T10‖T11)→T12.

---

## Discoveries

### D1 — RESOLVED (2026-04-21): Option B′ — Replace radar with InputContributions, collapse popover breakdown

**Decision:** `PerformanceRadar` is replaced by a 5-axis radar rendering the new `InputContributions` from `dignitas_history`. Popover (T10) simplifies to score + sparkline + link (no input breakdown inside popover — eliminates DRY violation against the new radar + transparency page).

**5 axes (from D2):**
1. Drawdown Adherence
2. Risk-per-Trade Consistency
3. Setup Adherence
4. Coach Alignment (= `1 − coach_severity_penalty`; displayed as a positive axis)
5. Journal Consistency

**Zoom-in hierarchy across all Dignitas surfaces:**
- Pill (T9) — ambient status (score + 7d delta)
- Popover (T10) — quick glance (score + 90-day sparkline + "View breakdown →")
- **Overview radar (this decision)** — the daily visual of where the score comes from
- Transparency page (T11) — full formula + per-input table

**Rationale:** The current 6-axis radar is labeled "Dignitas" but measures P&L outcomes — a brand lie. Four of the six axes are mathematically correlated (WR / PF / W/L / avgR), so the shape carries little information. The new 5 axes are independent behavioural levers, each directly actionable, stable on 30-day aggregates, and decoupled from luck. Outcome metrics migrate to journal tables/charts where "what happened" belongs; the radar visualizes "who you are."

**Implementation impact:**
- `PerformanceRadar.tsx` is rewritten: new 5 axes, reads from `GET /api/dignitas/me` (which already returns `InputContributions`), all client-side normalization (PF × 20 etc.) deleted. The `PerformanceStats` / `RiskStats` props become unused.
- Overview must no longer pass outcome stats to the radar — may require small Overview layout tweak (slot is freed). Existing outcome stats remain visible in journal pages / charts; not erased from product.
- Added to T12 verification: manual QA — observe a disciplined-but-losing user scoring high, and an undisciplined-but-winning user scoring low. The RED test at T3 codifies this invariant.
- The rename from "Performance radar" to "Dignitas inputs radar" is cosmetic — file can stay `PerformanceRadar.tsx` or be renamed; decide at implementation.

**Plan update:**
- T10 scope reduced: popover no longer renders input breakdown.
- T11 unchanged (transparency page carries the full table).
- Add a micro-task to the radar rewrite: document as part of T9 or a new T9b. **Proposed: extend T9 scope to include the radar rewrite** ("DignitasPill + mount + PerformanceRadar rewrite to 5-axis InputContributions"). Rationale: T9 already touches Overview-proximate frontend surfaces; bundling keeps the Overview layout change atomic.

### D2 — Input semantics (proposed definitions, grounded in schema)

All five inputs have verified sources. Proposed `[0..1]` mappings:

| Input | Source | Proposed computation |
|------|--------|----------------------|
| `drawdown_adherence` | `journal_daily_stats.drawdown_pct` + `RiskConfig.daily_max_drawdown_percent` | fraction of days in trailing 30 where `drawdown_pct ≤ daily_max_drawdown_percent`; `1.0` if limit never breached |
| `risk_per_trade_consistency` | `journal_trades.risk_amount` + account balance at entry + `RiskConfig.account_risk_percent` | `1 − mean(|actual_pct − configured_pct| / configured_pct)` over trailing 30d, floored at 0 |
| `setup_adherence` | `journal_trades.setup_tag` | `count(setup_tag IS NOT NULL) / count(*)` over trailing 30d (RSK-02 shipped column; no whitelist, any non-null tag adherent) |
| `coach_severity_penalty` | `coach_reports.digest_json → flagged_patterns[].severity` | weighted count of `Notable` (×0.5) + `Concerning` (×1.0) patterns across last 4 weekly reports, normalized by max-expected. `0` if no reports (neutral, per spec). |
| `journal_consistency` | `journal_trades.notes` + `journal_entries` | `count(trades with non-empty notes OR linked journal_entries) / count(closed trades)` over trailing 30d |

These are defaults; all weights live in `dignitas_config` and are tunable without redeploy (FR-6).

### D3 — Scheduler trigger hour

Daily cron fires at **UTC 00:30** (30 min past midnight UTC). Chosen to:
- Not collide with coach's Sunday 18:00 UTC run.
- Give downstream processors 30 min of quiet after midnight boundary.
- Mirror `coach/schedule.rs` hourly-poll pattern (checks `already_fired_today(user, date)` each poll).

Trigger hour captured as a const in `dignitas/schedule.rs` — tunable without migration.

### D4 — Backfill policy for existing users at launch

`cold_start = true` when fewer than 7 days of input data available. When ENG-01a ships, **existing users with ≥7 days of trading activity will show non-cold-start scores from day 1** — no backfill of historical snapshots. This means the 7-day delta for day-1 is `null` (only 1 snapshot exists) — frontend renders `DIGNITAS 72.4 —` with em-dash until 7 snapshots accumulate. Documented in T5 implementation note; no user migration needed.

### D5 — Migration numbering

Next slot: **`20260421000000_dignitas_history_config.up.sql`** (paired `.down.sql`). Single migration for both tables + `users.dignitas_pill_hidden` column is acceptable since they all land in the same spec. Seed `dignitas_config` rows for formula weights in the same `.up.sql`.

### D6 — Frontend-computed radar inputs are NOT reusable

The spec's "extract pure-data portions of `PerformanceRadar` into `snapshot.rs`" guidance is misleading. The radar's current axes are exactly the inputs FR-9 forbids. T4's input computation is genuinely new backend work, grounded in the schema sources above (D2) — not a port.

---

## Status

PLAN COMPLETE — 12 atomic tasks defined, dependencies mapped. **D1 RESOLVED** (Option B′).

**Ready for BUILD** — no blockers remain.

Spec: ENG-01a-dignitas-score-living
Total Tasks: 12
Ready for BUILD mode.

Next task: T1 — Migrations (`dignitas_history`, `dignitas_config` + seed weights, `users.dignitas_pill_hidden`)
