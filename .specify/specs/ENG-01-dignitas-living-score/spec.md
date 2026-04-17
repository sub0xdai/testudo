# Specification: Dignitas as Living Score — Identity Collateral for Discipline

**Spec ID:** ENG-01-dignitas-living-score
**Date:** 2026-04-17
**Status:** Draft
**Class:** Feature / Frontend + Backend + Engagement Mechanic
**Priority:** P1 — retention primitive; converts the existing static Dignitas score into identity collateral the user defends. Score-living portion ships standalone; streak portion layers on RSK-03.
**Depends on:** None hard for score-living (CP-1 through CP-5). Streak mechanic (CP-6, CP-7) depends on RSK-03's `Concerning` severity flag being in production.
**Series:** ENG-01 through ENG-03 (ENG-02 on-chain discipline attestations, ENG-03 journal streaks — both future)

---

## Problem Statement

The Dignitas Score (`70.2` on the Overview radar in `components/Overview.tsx`) is currently a **static telemetry number** — computed on-demand from performance + risk inputs, displayed once, never referenced again until the user opens Overview. It has no history, no delta, no shareability, no weight. It is a statistic, not an artifact.

The insight this spec acts on: **retention lives on invisible trust barriers — streaks, scores, and attestations users have invested belief in and refuse to lose.** Duolingo's streak is not about language learning; it is identity collateral the user defends. GitHub contribution graphs became career capital. Strava KOMs became athletic identity. None of these work because of points; they work because the user has *committed identity* to the system and loss would mean admitting something about themselves.

Testudo's core ideology is **trading discipline**. That ideology is the only honest thing to tokenize — not frequency (perverse), not P&L (perverse), not streak-of-app-opens (cheap, gameable). A Dignitas score that lives daily, shows its trajectory, breaks visibly when discipline fails, and can be shared as a public credential converts the abstract "am I getting better?" question into a concrete artifact the user builds over months and refuses to lose over one tilted afternoon.

This spec ships the **score-living + public profile** baseline that works standalone, then layers the **discipline streak** (days without a `Concerning` flag from the RSK-03 coach) on top once RSK-03 is in production. Critically: **the score is tuned so that trading more, trading less, or trading bigger cannot directly improve it** — only adherence to disciplined risk behavior can. Gaming the score is impossible by design.

---

## User Stories

- **As a Testudo user**, I want my Dignitas score visible in the top nav on every page with a delta indicator, so it becomes part of my daily situational awareness rather than a buried stat.
- **As a consistent trader**, I want to see my score's trajectory over time (sparkline, history chart), so I can correlate my behavior with its direction.
- **As someone who just finished a disciplined week**, I want a streak counter that rewards days without severity-flagged behavior, so discipline accrues to a visible artifact.
- **As someone who broke my own rules last night**, I want my streak reset to feel *earned* and visible — not hidden, not forgiven — so the system maintains credibility.
- **As a trader applying to a prop firm / pitching my edge / building a public identity**, I want an opt-in public profile page (`testudo.app/d/<handle>`) showing my score + streak + select stats, so my discipline becomes portable credential.
- **As a privacy-first user**, I want the public profile to be strictly opt-in and fully revocable, so default installation reveals nothing.
- **As a user who doesn't care about the score**, I want to hide the top-nav pill, so the feature is invisible if I don't want it.

---

## Non-Goals (Explicit Anti-Scope)

- **Not a leaderboard.** No ranking of users against each other. Competitive social mechanics corrupt the discipline framing.
- **Not points redeemable for anything.** Score is not currency; attaching rewards to score destroys its credibility. Attestations (ENG-02) handle that layer separately.
- **Not a frequency-tied score.** Trading more does not raise the score. Trading less does not raise it. Only adherence raises it.
- **Not P&L-tied.** Winning trades do not directly raise the score. A lucky gambler should not out-score a disciplined trader.
- **No badges, confetti, level-up animations, or celebratory modals.** Brutalist-serious tone is load-bearing for the trust barrier to work. Gamification UI reads as insincere on a financial product.
- **No streak protection / freeze cards.** The weight of the break IS the mechanic. "Freezes" are the first step toward the Duolingo infantilization this is explicitly avoiding.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Dignitas score computed daily via scheduled job, persisted to a time-series table (`dignitas_history`) keyed by (user_id, date) | High | db-processor |
| FR-2 | Score inputs are **discipline-adherence inputs only** — max drawdown within plan, risk-per-trade consistency, setup adherence rate (from RSK-02), coach-severity-flag frequency (from RSK-03 if available), journal consistency. **Explicitly excludes trade frequency, raw P&L, and win rate as direct inputs** | High | db-processor |
| FR-3 | `DignitasPill` in top nav on every page: shows current score + delta since 7-day-ago baseline (`DIGNITAS 70.2 ▲0.4`); color subtle (score-green on positive delta, score-red on negative, tertiary on no change); hidden via user preference | High | journal/frontend |
| FR-4 | Clicking the pill opens a panel (popover) with: current score, 90-day sparkline, streak counter (once RSK-03 ships), breakdown of input contributions, "Share profile" action (if public profile enabled), "Hide pill" preference shortcut | High | journal/frontend |
| FR-5 | User handle system: each user may claim one globally-unique handle (3–24 chars, `[a-z0-9_-]`, reserved word list enforced); handle is separate from any exchange account or display name; unclaimed by default | High | backend + journal/frontend |
| FR-6 | Public profile route `/d/:handle` (on the journal app — lives at `testudo.app/desk/d/<handle>` initially; vanity-URL `testudo.app/<handle>` is a web submodule concern deferred to ENG-02) | High | journal/frontend |
| FR-7 | Public profile renders: handle, current score, streak counter (if RSK-03 ships), 90-day score sparkline, join date, optional bio (≤ 140 chars). Absolutely no P&L, no balance, no position data, no trade history — discipline only | High | journal/frontend |
| FR-8 | Public profile visibility is **opt-in per element**: toggle score visible, toggle streak visible, toggle sparkline visible. Default all-off (profile claimed but empty is allowed for handle reservation) | High | backend + journal/frontend |
| FR-9 | Handle reservation: first-come-first-served; reserved-word list (`admin`, `testudo`, `api`, etc.); profanity filter; owner can release handle but handles do not transfer between users | High | backend |
| FR-10 | **Streak counter** (requires RSK-03): days since the user's most recent `Concerning` severity flag from the coach. Resets to 0 on a new `Concerning` flag. Displayed on pill panel and public profile (if opted-in) | High | db-processor + journal/frontend |
| FR-11 | Streak reset is **silent in the product** — no modal, no toast, no notification. User discovers the reset by looking at the pill or profile. Respects the brutalist-serious tone | High | journal/frontend |
| FR-12 | Score computation is transparent: `/desk/dignitas` page (or help modal) shows the user exactly which inputs contributed what to their current score, with explanatory text for each | Medium | journal/frontend |
| FR-13 | Public profile page is **static and SEO-friendly** — plain HTML structure, server-side-rendered or pre-rendered where possible, meta-tags for social sharing (OpenGraph: handle, score, streak as OG image) | Medium | journal/frontend + router |
| FR-14 | Anti-abuse: public profile access rate-limited per IP; handle claims rate-limited per user (1 handle change per 30 days) | Medium | router |
| FR-15 | User preference to hide the entire Dignitas pill from the top nav (for users who don't want the mechanic); default on | Medium | journal/frontend |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates | Depends on |
|------------|-------|-----------|------------|
| CP-1 | `dignitas_history` migration + daily snapshot cron in `crates/db-processor/src/dignitas/snapshot.rs` using **existing PerformanceRadar inputs**, filtered to discipline-only subset | Score persists as time-series; 7-day delta computable | None |
| CP-2 | `user_handles` migration + handle claim / release endpoints (`POST /api/dignitas/handle`, `DELETE /api/dignitas/handle`); reserved-word list + profanity filter | Handles are unique, claimable, releasable | None |
| CP-3 | `DignitasPill.tsx` in `Layout.tsx` fetches current score + delta; pill click opens `DignitasPanel.tsx` with sparkline | Top-nav visibility works end-to-end | CP-1 |
| CP-4 | `IdentitySettings.tsx` on Account page: handle claim UI, visibility toggles (score/streak/sparkline), hide-pill preference | User can manage their public presence | CP-2 |
| CP-5 | Public profile route `/desk/d/:handle` + `GET /api/public/profile/:handle` endpoint (respects visibility flags, returns 404 if handle unclaimed) | Public shareable page works; nothing leaks without opt-in | CP-2, CP-3 |
| CP-6 | Streak counter logic in `crates/db-processor/src/dignitas/streak.rs` — reads RSK-03 coach reports, counts days since last `Concerning` severity flag | Streak mechanic works **once RSK-03 is live** | RSK-03 in production |
| CP-7 | Streak display integrated into `DignitasPanel` + public profile; graceful fallback when RSK-03 has no data yet (`—` display) | UI correctly shows/hides streak based on coach-data availability | CP-6 |

**Sequencing note:** CP-1 through CP-5 ship the score-living + public profile standalone without needing RSK-03. This alone is meaningful retention value. CP-6 and CP-7 layer on when RSK-03 is in production.

### Key Types

```rust
// testudo-exchange/crates/db-processor/src/dignitas/types.rs

pub struct DignitasSnapshot {
    pub user_id: Uuid,
    pub date: NaiveDate,
    pub score: Decimal,              // 0.0 .. 100.0
    pub input_contributions: InputContributions,
}

pub struct InputContributions {
    // All discipline-only; explicitly no frequency, no raw P&L, no win rate
    pub drawdown_adherence: Decimal,      // 0..1, are you within planned max DD?
    pub risk_per_trade_consistency: Decimal, // 0..1, variance of risk-per-trade
    pub setup_adherence: Decimal,          // 0..1, % of trades with a setup tag (RSK-02)
    pub coach_severity_penalty: Decimal,   // 0..1, inverse of Concerning+Notable flag frequency
    pub journal_consistency: Decimal,      // 0..1, % of trades with a journal entry
}

pub struct DignitasStreak {
    pub user_id: Uuid,
    pub days_clean: u32,
    pub last_concerning_flag_at: Option<DateTime<Utc>>,
    pub longest_ever: u32,
}

pub struct UserHandle {
    pub user_id: Uuid,
    pub handle: String,            // normalized lowercase
    pub claimed_at: DateTime<Utc>,
    pub profile_visibility: ProfileVisibility,
    pub bio: Option<String>,       // ≤ 140 chars, optional
}

pub struct ProfileVisibility {
    pub show_score: bool,          // default false
    pub show_streak: bool,         // default false
    pub show_sparkline: bool,      // default false
}
```

```typescript
// testudo-journal/src/api/client.ts additions
export interface DignitasCurrent {
  score: string                    // decimal as string
  delta_7d: string                 // signed decimal
  streak_days: number | null       // null until RSK-03 has data
  handle: string | null
  pill_hidden: boolean
}

export interface DignitasHistory {
  snapshots: { date: string; score: string }[]  // 90 days
}

export interface PublicProfile {
  handle: string
  bio: string | null
  score: string | null             // null if not opted-in to show
  streak_days: number | null
  sparkline: { date: string; score: string }[] | null
  member_since: string
}
```

### Score Formula (starting point — tunable)

```
score = 100 * (
    0.25 * drawdown_adherence
  + 0.20 * risk_per_trade_consistency
  + 0.20 * setup_adherence
  + 0.20 * (1 - coach_severity_penalty)     // inverted: fewer flags = higher score
  + 0.15 * journal_consistency
)
```

All weights in a `dignitas_config` table so they can be tuned without a redeploy. None of the inputs reference trade count, raw P&L, or win rate — making the score ungameable by trading more or gambling on outcomes. The only path to a higher score is **behaving more disciplined**.

### Architecture Overview

```
┌──────────────────────────────────────────────────────────────────┐
│  testudo-exchange/crates/db-processor/src/dignitas/              │
│                                                                  │
│   snapshot.rs              ← daily cron, writes dignitas_history │
│   streak.rs                ← reads RSK-03 coach reports,         │
│                              maintains days-since-concerning      │
│   config.rs                ← reads tunable weights from DB       │
│   types.rs                                                       │
└──────────────────────────────────────────────────────────────────┘
              │
              ▼
   dignitas_history (PG, user_id, date, score, inputs_json)
   dignitas_streak  (PG, user_id, days_clean, last_flag_at)
   user_handles     (PG, user_id unique, handle unique, visibility)
              │
              ▼
┌──────────────────────────────────────────────────────────────────┐
│  testudo-exchange/crates/router/src/routes/dignitas.rs           │
│    GET  /api/dignitas/me          (auth: current user snapshot)  │
│    GET  /api/dignitas/history     (auth: 90-day series)          │
│    POST /api/dignitas/handle      (auth: claim handle)           │
│    DELETE /api/dignitas/handle    (auth: release handle)         │
│    PATCH /api/dignitas/visibility (auth: update ProfileVisibility)│
│    PATCH /api/dignitas/preferences (auth: hide pill etc.)        │
│    GET  /api/public/profile/:handle  (no-auth, respects visibility)│
└──────────────────────────────────────────────────────────────────┘
              │
              ▼
┌──────────────────────────────────────────────────────────────────┐
│  testudo-journal                                                 │
│    components/DignitasPill.tsx         ← top nav, all pages      │
│    components/DignitasPanel.tsx        ← popover on pill click   │
│    components/account/IdentitySettings.tsx ← on Account page     │
│    pages/PublicProfile.tsx             ← /desk/d/:handle         │
│    pages/Dignitas.tsx                  ← /desk/dignitas          │
│                                          (transparency / inputs) │
└──────────────────────────────────────────────────────────────────┘
```

### Paved Roads

- **`PerformanceRadar`** already computes the input metrics used here — extract the pure-data portions of its computation into `crates/db-processor/src/dignitas/snapshot.rs` and reuse.
- **`crates/db-processor` scheduled jobs** — same pattern as RSK-03's weekly cron, daily cadence.
- **`PageSubHeader`, `HELP`, signal colors, font-mono** — aesthetic tokens preserved.
- **`echarts`** — already imported in journal; use `LineChart` with minimal styling for the 90-day sparkline (no axes, no legend, brutalist-minimal).
- **RSK-01 pulse strip** — pattern for a slim, always-present layout element informs the `DignitasPill` styling.
- **Existing auth middleware** — public profile endpoint is the only unauth route; all others use existing SIWE session.

### Files

**New (backend):**
- `testudo-exchange/crates/db-processor/src/dignitas/mod.rs`
- `testudo-exchange/crates/db-processor/src/dignitas/snapshot.rs`
- `testudo-exchange/crates/db-processor/src/dignitas/streak.rs`
- `testudo-exchange/crates/db-processor/src/dignitas/config.rs`
- `testudo-exchange/crates/db-processor/src/dignitas/types.rs`
- `testudo-exchange/crates/db-processor/tests/dignitas_snapshot_test.rs`
- `testudo-exchange/crates/db-processor/tests/dignitas_streak_test.rs`
- `testudo-exchange/crates/router/src/routes/dignitas.rs`
- `testudo-exchange/crates/router/src/routes/public_profile.rs`
- `testudo-exchange/crates/sqlx_postgres/migrations/NNNN_dignitas_history.sql`
- `testudo-exchange/crates/sqlx_postgres/migrations/NNNN_dignitas_streak.sql`
- `testudo-exchange/crates/sqlx_postgres/migrations/NNNN_user_handles.sql`
- `testudo-exchange/crates/sqlx_postgres/migrations/NNNN_dignitas_config.sql`

**New (frontend):**
- `testudo-journal/src/components/DignitasPill.tsx`
- `testudo-journal/src/components/DignitasPanel.tsx`
- `testudo-journal/src/components/DignitasSparkline.tsx`
- `testudo-journal/src/components/account/IdentitySettings.tsx`
- `testudo-journal/src/pages/PublicProfile.tsx`
- `testudo-journal/src/pages/Dignitas.tsx`
- `testudo-journal/src/config/dignitas-reserved-handles.ts`

**Modified:**
- `testudo-journal/src/components/Layout.tsx` — mount `DignitasPill` in the top nav
- `testudo-journal/src/pages/Account.tsx` — include `IdentitySettings` section
- `testudo-journal/src/index.tsx` — register `/d/:handle` and `/dignitas` routes
- `testudo-journal/src/api/client.ts` — add Dignitas API surface
- `testudo-journal/src/lib/help-content.ts` — explanations for score inputs
- `testudo-exchange/crates/router/src/routes/mod.rs` — wire routes

### Dependencies Added

None (echarts, rust_decimal, chrono, async-openai all already available).

---

## Acceptance Criteria

### Score-living (CP-1 through CP-5)

- [ ] Daily cron writes `dignitas_history` for every active user; `/api/dignitas/me` returns current score + 7-day delta
- [ ] `DignitasPill` appears in top nav on every page with correct score + signed delta
- [ ] Clicking pill opens panel with 90-day sparkline, input-contribution breakdown
- [ ] User can hide the pill via preference; change persists; pill disappears immediately
- [ ] Handle claim flow: claim unique handle succeeds; claim taken handle returns 409; claim reserved word returns 400
- [ ] Visibility toggles: default claimed profile with all visibility off returns 200 with null score/streak/sparkline (confirms opt-in, not opt-out)
- [ ] Public profile `/desk/d/:handle` returns 404 for unclaimed handles
- [ ] Public profile respects each visibility toggle independently
- [ ] Score formula cannot be gamed: test fixture of 1000 high-frequency high-P&L undisciplined trades produces a lower score than 100 disciplined trades (captured in `dignitas_snapshot_test.rs`)
- [ ] Handle change rate-limit: second change within 30 days returns 429

### Streak (CP-6, CP-7 — deferred until RSK-03 in production)

- [ ] Streak counter reads RSK-03 coach reports; `days_clean` increments daily when no Concerning flag; resets to 0 the day after a Concerning flag
- [ ] `longest_ever` persisted and displayed in panel
- [ ] Streak break is silent — no toast, no modal; user discovers via pill/panel
- [ ] `DignitasPanel` shows `Streak —` until first coach report exists for the user

### Verification

- [ ] Backend: `cd testudo-exchange && cargo clippy --all-targets && cargo test`
- [ ] Frontend: `cd testudo-journal && bun run build`
- [ ] Manual QA: one user with claimed handle, score visible in profile, verified via incognito browser against `/desk/d/:handle`
- [ ] Overview page visually unchanged (PerformanceRadar still renders the score as it does today — pill is additive, not a replacement)

---

## Risks

1. **Score formula feels arbitrary to users.** If users don't understand how the score moves, trust erodes and they ignore the pill. *Mitigation:* FR-12's transparency page (`/desk/dignitas`) breaks down each input with its contribution and explanatory text. Show the formula weights. Score being legible is more important than any particular weight being correct.
2. **Streak mechanic feels disproportionate on reset.** A 200-day streak broken by one bad Wednesday could feel punitive enough to make users disengage entirely. *Mitigation:* resist the urge to add "freeze cards" (those destroy the mechanic per the non-goals). Accept the weight as the feature. Maintain `longest_ever` so the user keeps a trophy of their best run even after a reset.
3. **Handle squatting.** First-come-first-served means early users grab `0xwhale`, `cz`, etc. *Mitigation:* reserved-word list covers obvious impersonation (`testudo`, `admin`, major figure names configurable), profanity filter, 30-day rate limit on changes. Accept some squatting as the cost of frictionless onboarding; handle review process is a Phase 2 concern if abuse emerges.
4. **Public profile as gameable social pressure.** If score becomes a brag, users may stop trading to preserve it rather than trade *better*. *Mitigation:* score formula explicitly rewards journal consistency and setup adherence — sitting on your hands produces a *mediocre* score, not a high one. Active-but-disciplined trading is the only path to a genuinely high score.
5. **Overview's PerformanceRadar and the pill disagree.** If they use different snapshots, trust breaks. *Mitigation:* both read from the same source (`dignitas_history`, with the radar showing latest and the pill showing latest + delta). A single source of truth, computed once per day.
6. **Public profile SEO exposes user handles and scores.** Some users may not want their profile indexed even if claimed. *Mitigation:* `robots: noindex` default; opt-in to public indexing via a separate toggle in `IdentitySettings`.
7. **Daily cron job load.** Every user's score recomputed daily is O(N) in users. *Mitigation:* job batches users in chunks of 500 with `yield` between batches; parallelize via `tokio::join!` where safe; measure and move to incremental computation if aggregate time exceeds 10 minutes.
8. **Streak mechanic is hollow without RSK-03.** If ENG-01 ships before RSK-03 is in production, the pill has a score but no streak, which reduces the retention punch. *Mitigation:* CP-6 and CP-7 are explicitly sequenced after RSK-03. The score-living + public profile baseline (CP-1 through CP-5) delivers standalone value; streak is additive. Do not ship CP-6/CP-7 until RSK-03 has ≥ 2 weeks of production coach data.
9. **Gamification tone risk.** Any candy-colored UX leaking into the pill breaks the brutalist-serious pact the rest of the app maintains. *Mitigation:* explicit design constraint in the Non-Goals; any PR touching Dignitas UI is reviewed for tone. No emoji in the pill. No confetti anywhere.

---

## Completion Signal

This spec is complete when:

**Score-living baseline (CP-1 through CP-5) — can ship before RSK-03:**
1. FR-1 through FR-9 implemented and tested
2. Daily cron producing reliable snapshots for ≥ 7 consecutive days with zero failures
3. At least one user has claimed a handle and opted-in to a public profile visible at `/desk/d/<handle>`
4. Score formula gaming test passes (anti-frequency, anti-P&L-correlation test fixtures)
5. Verification commands pass: `cargo clippy --all-targets && cargo test`; `bun run build`
6. Committed: `feat(eng-01): Dignitas living score + public profile (score only, streak pending RSK-03)`

**Streak layer (CP-6, CP-7) — after RSK-03 is live:**
7. FR-10, FR-11 implemented and tested
8. Streak counter correctly increments and resets against live RSK-03 coach reports for ≥ 3 test users over 2 weeks
9. Public profile shows streak when opted-in
10. Committed: `feat(eng-01): Dignitas streak counter wired to coach severity flags`

**Spec-level:**
11. `/desk/dignitas` transparency page documents every input and weight
12. Overview's `PerformanceRadar` continues to render identically — no regression
13. ENG-02 (on-chain discipline attestations) scoped as fast-follow with milestone definitions (e.g. "30-day clean streak", "90-day clean streak", "100 trades with setup tag") informed by ENG-01 production data
