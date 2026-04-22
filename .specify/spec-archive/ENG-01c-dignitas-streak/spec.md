# Specification: Dignitas Streak — Days Without a Concerning Flag

**Spec ID:** ENG-01c-dignitas-streak
**Date:** 2026-04-20
**Status:** Draft
**Class:** Feature / Backend + Frontend
**Priority:** P2 — retention amplifier; layers on ENG-01a + ENG-01b
**Depends on:** ENG-01a (pill + panel to hook into), ENG-01b (public profile to optionally display on), RSK-03 (live — coach severity flags already in production)
**Parent series:** ENG-01 (carved 2026-04-20 from monolithic ENG-01-dignitas-living-score)
**Siblings:** ENG-01a (score-living baseline), ENG-01b (public profile)

---

## Problem Statement

A living score (ENG-01a) and a shareable profile (ENG-01b) give Dignitas shape. A **streak** gives it weight. Duolingo's streak is not about language learning; it is identity collateral the user defends. Once a user has invested 47 clean days of trading discipline into a counter, loss would mean admitting something about themselves — and they resist it.

RSK-03 already produces weekly coach reports with severity flags (`Concerning` being the highest). This spec turns those flags into a **days-since-Concerning counter** attached to the user's Dignitas identity.

Critically:

- **No streak freezes.** The weight of the break IS the mechanic. Freezes are the first step toward Duolingo infantilization.
- **Silent reset.** No toast, no modal, no notification. Users discover the reset by looking at the pill or profile — respects the brutalist-serious tone.
- **`longest_ever` persisted** so a user who breaks a 200-day streak keeps a trophy of their best run.

---

## User Stories

- **As someone who just finished a disciplined week**, I want a streak counter that rewards days without severity-flagged behavior, so discipline accrues to a visible artifact.
- **As someone who broke my own rules last night**, I want my streak reset to feel *earned* and visible — not hidden, not forgiven — so the system maintains credibility.
- **As someone who had a 200-day run and broke it**, I want to keep a record of my longest run as a trophy of my best discipline, so one bad day doesn't erase the history of my best self.
- **As a trader building a public identity**, I want my streak (and my longest-ever) visible on my public profile if I opt in, so discipline becomes verifiable over time.

---

## Non-Goals

- **No streak freezes / protection cards.** Explicit anti-goal.
- **No streak notifications.** Silent increment, silent reset. No celebratory milestones (`7 days!`, `30 days!`) — those are the gamification tone this product rejects.
- **No competitive streak leaderboard.** Visible on own pill/panel + opt-in public profile only.
- **No retroactive streak grants** for existing users before RSK-03 shipped. Streaks start the day this spec ships.
- **No streak-based rewards, discounts, or in-product perks.** Streak is identity, not currency.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `dignitas_streak` table: `(user_id UNIQUE, days_clean, last_concerning_flag_at, longest_ever, streak_started_at)` | High | backend |
| FR-2 | Daily job (runs as part of the ENG-01a snapshot scheduler) reads RSK-03 coach reports for the current user. If any report in the last 24h carries `severity = Concerning` and its `flagged_at > last_concerning_flag_at`, reset `days_clean = 0` and update `last_concerning_flag_at`. Otherwise increment `days_clean` | High | backend |
| FR-3 | On every reset, update `longest_ever = MAX(longest_ever, days_clean_before_reset)` | High | backend |
| FR-4 | `/api/dignitas/me` (from ENG-01a) response extended with `streak: { days_clean: number; longest_ever: number } \| null`. `null` when user has no RSK-03 data yet | High | router |
| FR-5 | `DignitasPanel` (from ENG-01a) displays `STREAK 47d  LONGEST 92d` below sparkline. Falls back to `STREAK —` when streak is `null` | High | journal/frontend |
| FR-6 | Public profile (ENG-01b) gains `show_streak` visibility toggle (default off). When enabled, profile response includes `streak_days` + `longest_ever` | High | backend + journal/frontend |
| FR-7 | Streak reset is **silent** — no toast, modal, notification, email, or push. Discovered only via pill/panel/profile | High | product |
| FR-8 | No "milestone" UI (no `7 days!`, `30 days!` celebrations). The number counts, that is all | High | product |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | `dignitas_streak` migration + streak maintenance logic in `crates/router/src/services/dignitas/streak.rs`, hooked into ENG-01a's daily scheduler | Streak increments; resets on Concerning flag; longest_ever persists |
| CP-2 | `/api/dignitas/me` extended with streak; `DignitasPanel` renders streak + longest | End-to-end visible on own pill |
| CP-3 | `show_streak` toggle in `IdentitySettings` (ENG-01b); public profile endpoint + page render streak when opted-in | Streak shareable on public profile |
| CP-4 | Integration test: simulated Concerning flag on day 8 of a streak resets `days_clean = 0`, sets `longest_ever = 8`. Next day increments to 1 | Reset semantics locked in |

### Key Types

```rust
// crates/router/src/services/dignitas/streak.rs

pub struct DignitasStreak {
    pub user_id: Uuid,
    pub days_clean: u32,
    pub last_concerning_flag_at: Option<DateTime<Utc>>,
    pub longest_ever: u32,
    pub streak_started_at: Option<DateTime<Utc>>,
}
```

```typescript
// testudo-journal/src/api/client.ts — extends DignitasCurrent from ENG-01a

export interface DignitasCurrent {
  // ...existing fields from ENG-01a
  streak: {
    days_clean: number
    longest_ever: number
  } | null   // null when user has no RSK-03 coach data yet
}

// PublicProfile from ENG-01b extended:

export interface PublicProfile {
  // ...existing fields from ENG-01b
  streak_days: number | null       // null unless show_streak: true AND streak exists
  longest_ever: number | null
}
```

### Streak Maintenance Logic

Pseudocode for the daily job (runs once per user after the ENG-01a snapshot):

```
let row = dignitas_streak.find_or_default(user_id);
let latest_concerning_in_last_day = coach_reports.find_concerning(
    user_id, since=row.last_concerning_flag_at.unwrap_or(streak_started_at)
);

if latest_concerning_in_last_day.is_some() {
    row.longest_ever = max(row.longest_ever, row.days_clean);
    row.days_clean = 0;
    row.last_concerning_flag_at = Some(flag.flagged_at);
    row.streak_started_at = Some(now);
} else {
    row.days_clean += 1;
}
dignitas_streak.upsert(row);
```

Edge cases:
- User with zero coach reports ever: `days_clean = 0`, API returns `streak: null` (UI falls back to `—`)
- User's first coach report is Concerning: `days_clean` stays 0, `streak_started_at` set to now, `longest_ever = 0`
- Multiple Concerning flags in the same day: treated as one reset (idempotent via `flagged_at > last_concerning_flag_at` gate)

### Files

**New (backend):**
- `crates/router/src/services/dignitas/streak.rs`
- `crates/router/tests/dignitas_streak_test.rs`
- `crates/sqlx_postgres/migrations/NNNN_dignitas_streak.up.sql` + `.down.sql`

**Modified (backend):**
- `crates/router/src/services/dignitas/scheduler.rs` (from ENG-01a) — invoke streak maintenance after snapshot
- `crates/router/src/routes/dignitas.rs` (from ENG-01a) — extend `/me` response with streak
- `crates/router/src/routes/public_profile.rs` (from ENG-01b) — extend with streak when `show_streak: true`

**Modified (frontend):**
- `testudo-journal/src/components/DignitasPanel.tsx` (from ENG-01a) — render streak + longest
- `testudo-journal/src/pages/PublicProfile.tsx` (from ENG-01b) — render streak when opted-in
- `testudo-journal/src/components/account/IdentitySettings.tsx` (from ENG-01b) — add `show_streak` toggle
- `testudo-journal/src/api/client.ts` — extend types

### Paved Roads

- **RSK-03 `coach_reports` table + `severity` enum** — source of truth for Concerning flags; already in production
- **ENG-01a daily scheduler** — hosts the streak tick; no separate cron
- **ENG-01a `DignitasPanel`** — streak slots in under the sparkline
- **ENG-01b visibility toggle pattern** — `show_streak` follows the exact shape of `show_score`, `show_sparkline`
- **Brutalist font-mono + muted-color styling** — `STREAK 47d  LONGEST 92d` renders plain, no emoji, no animation

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `dignitas_streak` row created for a user on their first snapshot; initial state `days_clean = 0, longest_ever = 0`
- [ ] Daily tick increments `days_clean` when no new Concerning flag exists
- [ ] Concerning flag resets `days_clean` to 0 and updates `longest_ever = MAX(longest_ever, previous days_clean)`
- [ ] `longest_ever` persists across resets and is displayed in panel
- [ ] `/api/dignitas/me` returns `streak: null` for users with no coach reports, `streak: { days_clean, longest_ever }` otherwise
- [ ] `DignitasPanel` renders `STREAK 47d  LONGEST 92d` or `STREAK —` fallback
- [ ] Public profile `show_streak` toggle defaults to false; when true, public endpoint includes `streak_days` + `longest_ever`; when false, both are `null`
- [ ] Streak reset produces **no toast, modal, notification, or side effect** — purely a DB update
- [ ] Integration test: Concerning flag on day 8 → reset semantics verified (`days_clean = 0, longest_ever = 8`)
- [ ] Idempotency test: two Concerning flags in same day produce one reset, not two
- [ ] Verification: `cd testudo-exchange && cargo clippy --all-targets && cargo test`
- [ ] Verification: `cd testudo-journal && bun run build`

---

## Risks

1. **Streak break feels disproportionate.** A 200-day streak broken by one bad Wednesday could feel punitive enough to disengage the user entirely. *Mitigation:* resist adding freezes (explicit non-goal). Accept the weight as the feature. `longest_ever` gives the user a trophy of their best run.
2. **RSK-03 false positives cascade into wrongful resets.** If the LLM flags a trade Concerning incorrectly, the user's streak dies for an AI error. *Mitigation:* RSK-03's citation validator already filters low-confidence flags. Additionally, add a "dispute coach flag" control (out of scope here — tracked in RSK-03 follow-up) so users can annul a flag before the daily tick.
3. **Retroactive streak demand.** Power users with long clean histories may request credit for pre-RSK-03 discipline. *Mitigation:* explicit non-goal. Communicate clearly: streaks start at spec-ship date. Prior discipline is invisible to the system; this is honest.
4. **Cron races with coach report generation.** If the streak tick fires before the daily coach batch completes, a Concerning flag for "yesterday" might miss the window. *Mitigation:* sequence the scheduler so streak tick runs AFTER the day's coach narrator job completes (observable via a completion marker).
5. **Gamification-tone drift.** Once streak ships, there is pressure to add milestones, celebrations, "on fire" badges. *Mitigation:* non-goal FR-8 is the gate; any PR adding milestone UI gets reviewed for tone.

---

## Completion Signal

1. FR-1 through FR-8 implemented and tested
2. Streak counter correctly increments and resets against live RSK-03 coach reports over ≥ 2 weeks for ≥ 3 test users
3. `longest_ever` observed updating correctly on at least one natural reset
4. At least one user has opted-in `show_streak` on their public profile and verified it from incognito
5. Verification commands pass
6. Committed: `feat(eng-01c): Dignitas streak counter — days without a Concerning flag, silent reset, longest_ever trophy`
