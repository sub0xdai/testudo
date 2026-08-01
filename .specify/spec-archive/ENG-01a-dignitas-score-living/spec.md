# Specification: Dignitas as Living Score — Baseline

**Spec ID:** ENG-01a-dignitas-score-living
**Date:** 2026-04-20
**Status:** Draft
**Class:** Feature / Backend + Frontend
**Priority:** P1 — retention primitive, ships the score-living baseline standalone
**Depends on:** None
**Parent series:** ENG-01 (carved 2026-04-20 from monolithic ENG-01-dignitas-living-score)
**Siblings:** ENG-01b (public profile), ENG-01c (streak counter)

---

## Problem Statement

The Dignitas Score (`70.2` on the Overview radar in `components/Overview.tsx`) is currently a **static telemetry number** — computed on-demand, displayed once, never referenced again until the user opens Overview. It has no history, no delta, no weight. It is a statistic, not an artifact.

Retention lives on invisible trust barriers — streaks, scores, and attestations users have invested belief in and refuse to lose. A Dignitas score that lives daily, shows its trajectory, and breaks visibly when discipline fails converts the abstract "am I getting better?" question into a concrete artifact the user builds over months.

This spec ships the **score-living baseline**: daily snapshot persistence, a top-nav pill with delta, a panel with 90-day sparkline, and a transparency page. No public profile (ENG-01b). No streak (ENG-01c).

Critically: the score is tuned so that **trading more, trading less, or trading bigger cannot directly improve it** — only adherence to disciplined risk behavior can. Gaming the score is impossible by design.

---

## User Stories

- **As a Testudo user**, I want my Dignitas score visible in the top nav on every page with a delta indicator, so it becomes part of my daily situational awareness.
- **As a consistent trader**, I want to see my score's trajectory over time (sparkline, history), so I can correlate my behavior with its direction.
- **As a user who doesn't care about the score**, I want to hide the top-nav pill, so the feature is invisible if I don't want it.
- **As a user who distrusts arbitrary numbers**, I want a transparency page that shows me exactly which inputs contributed what to my current score.

---

## Non-Goals (Explicit Anti-Scope)

- **No public profile / handle / shareable URL.** That is ENG-01b.
- **No streak counter.** That is ENG-01c.
- **Not a leaderboard.** No ranking users against each other.
- **Not points redeemable for anything.**
- **Not frequency- or P&L-tied.** Trading more does not raise the score; winning does not raise it.
- **No badges, confetti, level-up animations, or celebratory modals.** Brutalist-serious tone is load-bearing.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Dignitas score computed daily via scheduled job, persisted to `dignitas_history` keyed by (user_id, date) | High | backend |
| FR-2 | Score inputs are **discipline-adherence only**: drawdown adherence, risk-per-trade consistency, setup adherence (RSK-02), coach severity penalty (RSK-03 if available), journal consistency. **Explicitly excludes trade frequency, raw P&L, win rate** | High | backend |
| FR-3 | `DignitasPill` in top nav on every page: shows current score + delta since 7-day-ago baseline (`DIGNITAS 70.2 ▲0.4`); subtle color (score-green / score-red / tertiary); hidden via user preference | High | journal/frontend |
| FR-4 | Clicking pill opens `DignitasPanel`: current score, 90-day sparkline, breakdown of input contributions, "Hide pill" preference shortcut | High | journal/frontend |
| FR-5 | Cold-start: while the user has fewer than 10 closed trades in the trailing 30d, the score is computed from real inputs but flagged `cold_start: true`. UI renders "PRELIMINARY — N of 10 trades" instead of a 50 placeholder. (Revised 2026-04-25 — see LEARNINGS § "Cold-start redesign".) | High | backend |
| FR-6 | Score weights live in `dignitas_config` table so they can be tuned without a redeploy | Medium | backend |
| FR-7 | `/desk/dignitas` transparency page: shows every input, its current value for the user, its weight, and plain-English explanation. Shows the formula | Medium | journal/frontend |
| FR-8 | User preference to hide the entire Dignitas pill from top nav; default visible | Medium | journal/frontend |
| FR-9 | Score formula is ungameable — trading frequency, raw P&L, and win rate are never inputs. Verified by test fixture | High | backend/tests |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Migrations (`dignitas_history`, `dignitas_config`, `users.dignitas_pill_hidden`), daily snapshot job in `crates/router/src/services/dignitas/snapshot.rs` reusing `PerformanceRadar` input subset | Score persists; 7-day delta computable |
| CP-2 | `GET /api/dignitas/me`, `GET /api/dignitas/history`, `PATCH /api/dignitas/preferences` routes + auth middleware | API returns current + delta + 90-day series |
| CP-3 | `DignitasPill.tsx` in `Layout.tsx` top nav, `DignitasPanel.tsx` popover, `DignitasSparkline.tsx` (echarts minimal) | Top-nav visibility end-to-end |
| CP-4 | `/desk/dignitas` transparency page showing inputs + weights + values | Score legibility |
| CP-5 | Ungameability test: fixture of 1000 high-frequency high-P&L undisciplined trades scores lower than 100 disciplined trades | Anti-gaming locked in |

### Module Placement (deviation from original monolithic spec)

Per the RSK-03 precedent, dignitas modules live at `crates/router/src/services/dignitas/` — not `crates/db-processor/`. `db-processor` is deprecated for new scheduled work; router hosts its own cron via `tokio::spawn` + scheduler pattern already established by RSK-03's weekly narrator.

### Key Types

```rust
// crates/router/src/services/dignitas/types.rs

pub struct DignitasSnapshot {
    pub user_id: Uuid,
    pub date: NaiveDate,
    pub score: Decimal,              // 0.0 .. 100.0
    pub input_contributions: InputContributions,
}

pub struct InputContributions {
    pub drawdown_adherence: Decimal,         // 0..1
    pub risk_per_trade_consistency: Decimal, // 0..1
    pub setup_adherence: Decimal,            // 0..1 (RSK-02)
    pub coach_severity_penalty: Decimal,     // 0..1 (RSK-03, 0 if no data)
    pub journal_consistency: Decimal,        // 0..1
}
```

```typescript
// testudo-journal/src/api/client.ts additions

export interface DignitasCurrent {
  score: string               // decimal as string
  delta_7d: string            // signed decimal
  cold_start: boolean         // true during neutral-50 window
  pill_hidden: boolean
}

export interface DignitasHistory {
  snapshots: { date: string; score: string }[]  // 90 days
}
```

### Score Formula (starting point — tunable via `dignitas_config`)

```
score = 100 * (
    0.25 * drawdown_adherence
  + 0.20 * risk_per_trade_consistency
  + 0.20 * setup_adherence
  + 0.20 * (1 - coach_severity_penalty)
  + 0.15 * journal_consistency
)
```

When RSK-03 has no data for a user, `coach_severity_penalty = 0` (neutral — no reward, no punishment). Weights renormalize only when explicitly tuned in `dignitas_config`.

### Files

**New (backend):**
- `crates/router/src/services/dignitas/mod.rs`
- `crates/router/src/services/dignitas/snapshot.rs`
- `crates/router/src/services/dignitas/config.rs`
- `crates/router/src/services/dignitas/types.rs`
- `crates/router/src/services/dignitas/scheduler.rs`
- `crates/router/src/routes/dignitas.rs`
- `crates/router/tests/dignitas_snapshot_test.rs`
- `crates/sqlx_postgres/migrations/NNNN_dignitas_history.up.sql` + `.down.sql`
- `crates/sqlx_postgres/migrations/NNNN_dignitas_config.up.sql` + `.down.sql`

**New (frontend):**
- `testudo-journal/src/components/DignitasPill.tsx`
- `testudo-journal/src/components/DignitasPanel.tsx`
- `testudo-journal/src/components/DignitasSparkline.tsx`
- `testudo-journal/src/pages/Dignitas.tsx`

**Modified:**
- `testudo-journal/src/components/Layout.tsx` — mount `DignitasPill`
- `testudo-journal/src/api/client.ts` — add Dignitas surface
- `testudo-journal/src/lib/help-content.ts` — explanations for score inputs
- `testudo-journal/src/index.tsx` — register `/dignitas` route
- `crates/router/src/routes/mod.rs` — wire routes
- `crates/router/src/main.rs` — spawn daily snapshot scheduler

### Paved Roads

- **`PerformanceRadar` input computation** — extract pure-data portions into `snapshot.rs` and reuse.
- **RSK-03 weekly scheduler pattern** — daily cadence mirrors the pattern in `crates/router/src/services/coach/scheduler.rs`.
- **`PageSubHeader`, `HELP`, signal colors, font-mono** — aesthetic tokens preserved.
- **`echarts`** — already imported in journal; `LineChart` with minimal styling for sparkline.
- **Overview's `PerformanceRadar`** — continues to render the score identically; pill is additive, not a replacement. Both read from `dignitas_history`.

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Daily cron writes `dignitas_history` for every active user
- [ ] `/api/dignitas/me` returns current score + 7-day delta + `cold_start` flag
- [ ] `DignitasPill` appears in top nav on every page with correct score + signed delta
- [ ] Clicking pill opens panel with 90-day sparkline + input contribution breakdown
- [ ] User can hide the pill; pill disappears immediately; preference persists
- [ ] `/desk/dignitas` transparency page renders every input with its value, weight, and explanation
- [ ] Score formula ungameable: test fixture of 1000 high-frequency high-P&L undisciplined trades produces lower score than 100 disciplined trades (`dignitas_snapshot_test.rs`)
- [ ] Cold-start: user with <10 closed trades in trailing 30d sees `cold_start: true` with the real computed score (revised 2026-04-25; replaces the original "score = 50" rule)
- [ ] Overview's `PerformanceRadar` reads from `dignitas_history` and continues rendering unchanged
- [ ] Verification: `cd testudo-exchange && cargo clippy --all-targets && cargo test`
- [ ] Verification: `cd testudo-journal && bun run build`

---

## Risks

1. **Score formula feels arbitrary.** *Mitigation:* transparency page (FR-7) breaks down every input with explanatory text and the formula. Legibility over perfect weights.
2. **PerformanceRadar and pill disagree.** *Mitigation:* both read from `dignitas_history`. Radar shows latest; pill shows latest + delta. Single source of truth.
3. **Daily cron load O(N) users.** *Mitigation:* batch in chunks of 500 with `tokio::yield_now()` between batches; measure and move to incremental if >10 min total.
4. **Cold-start neutral chosen wrong.** 50 is a judgment call; could feel arbitrary. *Mitigation:* `cold_start` flag exposed in API so the frontend can label the pill (`DIGNITAS 50 —`) with a tooltip explaining the neutral window.
5. **Score with zero RSK-03 data is already 80% of the weight.** A user with no coach reports effectively gets a `coach_severity_penalty = 0` freebie. *Mitigation:* accepted — until RSK-03 has user data, the coach dimension is silently neutral. ENG-01c adds the streak dimension that rewards sustained clean coach history.

---

## Completion Signal

1. FR-1 through FR-9 implemented and tested
2. Daily cron producing snapshots for ≥ 7 consecutive days with zero failures in staging
3. Ungameability test passes
4. Verification commands pass
5. Committed: `feat(eng-01a): Dignitas score as living artifact — pill, history, transparency`
