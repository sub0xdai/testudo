# Specification: Performance Block on Public Profile

**Spec ID:** ENG-01d-performance-profile
**Date:** 2026-04-22
**Status:** PARKED — design only, do not implement until REF-01 or equivalent distribution hypothesis is committed
**Class:** Feature / Frontend + Backend
**Priority:** P3 — no user value without a referral/distribution mechanic in place
**Depends on:** ENG-01b (public profile scaffold), and any of {REF-01 referral pipeline, ENG-01c streak} to be load-bearing
**Series:** ENG-01 (Dignitas public-identity enhancements)
**Siblings:** ENG-01a (score), ENG-01b (public profile), ENG-01c (streak)

---

## Problem Statement

ENG-01b shipped a public profile page showing Dignitas score + sparkline. In-session review (2026-04-22) surfaced that this surface is commercially inert: a visitor clicking a shared profile link does not care whether the profile owner is disciplined — they care whether the profile owner makes money.

Dignitas is a **discipline** signal by explicit design (ENG-01a). That design stays intact — it is not the problem. The problem is that a shareable profile built on discipline alone fails the visitor's first question ("can this person actually trade?"), which kills any downstream conversion action the profile is meant to carry (signup, referral credit, prop-firm lead, etc.).

This spec adds an **opt-in performance block** alongside the existing opt-in Dignitas block. Dignitas remains the "how" (process integrity); performance metrics become the "what" (outcomes). Neither alone is sufficient; together they form a credible story.

**Parked because:** without a referral pipeline (REF-01) or distribution hypothesis driving traffic to the profile, this block has no conversion target and is engineering for a phantom user. Revive when distribution arrives.

---

## User Stories

- **As a trader sharing my profile**, I want to display performance outcomes alongside discipline, so visitors can evaluate whether my process translates into results.
- **As a visitor landing on a shared profile**, I want to see if this person actually trades profitably, so I can decide whether their tool/process is worth investigating.
- **As a privacy-conscious user**, I want performance metrics shown as normalized ratios — never raw $ P&L — so my absolute balance stays private.

---

## Non-Goals

- **No raw $ P&L exposure.** Not total P&L, not daily P&L, not account balance. Normalized metrics only.
- **No live/real-time performance.** All metrics are rolling-window aggregates (e.g. 90-day). Avoids social pressure of watching a live drawdown.
- **No leaderboard / ranking page.** Emergent from individual profiles is fine; dedicated ranking surface is out of scope.
- **No verification badge / attestation.** ENG-02 (onchain attestations) is the separate surface for provable claims.
- **No copy-trade integration.** Copy-trade is a different product; profile is for credibility, not execution.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add `PerformanceVisibility` toggles to `user_handles`: `show_r_multiple`, `show_win_rate`, `show_profit_factor`, `show_trade_count` — each independent, default false | High | backend |
| FR-2 | Public profile response includes `performance: { r_multiple, win_rate, profit_factor, trade_count } \| null` — each field `null` when its toggle is off | High | backend |
| FR-3 | Metrics computed over a fixed rolling window (90 days) from `trade_groups` where status = closed | High | backend |
| FR-4 | `PerformanceBlock.tsx` renders on `/desk/d/:handle` below the Dignitas block when any performance field is non-null | High | frontend |
| FR-5 | Performance toggles added to the existing IdentitySettings visibility section (same pattern as Dignitas toggles) | High | frontend |
| FR-6 | Minimum trade threshold: metrics return `null` if fewer than 20 closed trades in window — prevents small-sample noise on profile | Medium | backend |
| FR-7 | R-multiple formula documented on profile via hover tooltip: `(profit − loss) / avg_per_trade_risk` | Low | frontend |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Migration: add 4 `show_*` columns to `user_handles`. Extend `IdentityPreferences` + `VisibilityPatch` structs | Toggle storage works end-to-end |
| CP-2 | `compute_performance_metrics(user_id, window_days)` service fn. Unit-tested against fixture `trade_groups` | Metrics match expected ratios on known data |
| CP-3 | `PublicProfile` response carries opt-in `performance` object. Integration test confirms each toggle gates independently | Privacy model holds |
| CP-4 | `PerformanceBlock.tsx` + IdentitySettings toggles. Manual QA via incognito | UI ships |

### Key Types

```rust
// crates/router/src/services/dignitas/handles/mod.rs — extended

pub struct IdentityPreferences {
    // existing fields…
    pub show_r_multiple: bool,
    pub show_win_rate: bool,
    pub show_profit_factor: bool,
    pub show_trade_count: bool,
}

pub struct PerformanceMetrics {
    pub r_multiple: Option<String>,      // e.g. "+15.3"
    pub win_rate: Option<String>,        // e.g. "0.58"
    pub profit_factor: Option<String>,   // e.g. "1.82"
    pub trade_count: Option<i64>,        // e.g. 47
    pub window_days: i64,                // always 90 in v1
}
```

```typescript
// testudo-journal/src/api/client.ts additions

export interface PublicProfile {
  // existing fields…
  performance: {
    r_multiple: string | null
    win_rate: string | null
    profit_factor: string | null
    trade_count: number | null
    window_days: number
  } | null
}
```

### Metric Definitions

- **R-multiple** — sum of per-trade R values over window. Each trade's R = `realized_pnl / initial_risk_amount`. Risk-normalized, unit-free.
- **Win rate** — `winning_trades / total_closed_trades`. Ratio 0–1.
- **Profit factor** — `sum(winning_pnl) / abs(sum(losing_pnl))`. Unitless ratio; >1 means profitable.
- **Trade count** — total closed trade groups in window. Sample size for honesty.

All computed from `trade_groups` closed within the window. Already partially computed for Overview page — reuse the same SQL shape.

### Files

**New (backend):**
- `crates/sqlx_postgres/migrations/NNNN_user_handles_performance_visibility.up.sql` + `.down.sql`
- `crates/router/src/services/dignitas/performance.rs` — metric computation
- Extend `crates/router/src/routes/public_profile.rs` — add performance to response
- Extend `crates/router/src/services/dignitas/handles/mod.rs` — new visibility flags

**New (frontend):**
- `testudo-journal/src/components/profile/PerformanceBlock.tsx`

**Modified:**
- `testudo-journal/src/pages/PublicProfile.tsx` — render PerformanceBlock when populated
- `testudo-journal/src/components/account/IdentitySettings.tsx` — add 4 new toggles
- `testudo-journal/src/api/client.ts` — extend PublicProfile type, add toggles to patch

### Paved Roads

- **Overview page metrics** already compute win_rate, profit_factor, expectancy, R-multiple. Refactor shared calc into `common_utils::performance` so Overview + profile share one source of truth.
- **ENG-01b visibility pattern** — reuse toggle pattern exactly. 4 new flags, same UI component.
- **Existing rate-limit on public profile** covers the new fields — no new anti-abuse surface.

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] All 4 toggles default false; fresh claimed handle returns `performance: null` in public response
- [ ] Each toggle independently gates its field (turning one on leaves others null)
- [ ] Profile with < 20 closed trades returns `performance: null` regardless of toggles (FR-6 honesty floor)
- [ ] R-multiple formula matches Overview-page calculation to 2 decimals on fixture data
- [ ] Manual QA: one account with performance opted-in visible via incognito, metrics match Overview values
- [ ] `cd testudo-exchange && cargo clippy --all-targets && cargo test`
- [ ] `cd testudo-journal && bunx tsc --noEmit`

---

## Risks

1. **Small-sample gaming.** User takes 3 lucky trades, wins 2, shares a 67% win-rate profile. *Mitigation:* FR-6 20-trade floor. Consider 50 for v2.
2. **Paper-trade inflation.** User uses Shadow mode to build a pristine history. *Mitigation:* metrics computed from `trade_groups` where `is_shadow = false` (only live trades count toward profile). Enforce at query level.
3. **Performance encourages gambling.** Users chase visible metrics at cost of Dignitas. *Mitigation:* profile shows both side-by-side; disciplined losses still show high Dignitas + negative R. The contrast is the signal.
4. **Rolling window gaming.** User times disclosure to windows that look good. *Mitigation:* accept at MVP; window is displayed alongside metrics ("last 90d") for honesty. v2 can offer "lifetime" as additional window.
5. **Regulatory noise.** Publishing performance could trip investment-advice or solicitation rules in some jurisdictions. *Mitigation:* terms of service clarifies users publish their own data voluntarily; no claims of future returns; no solicitation. Consult legal before enabling in restricted jurisdictions.

---

## Completion Signal

**This spec is PARKED.** Completion is not scheduled. Revive when:

1. A referral pipeline (REF-01) or equivalent distribution hypothesis is shipping — gives the performance block a downstream conversion action.
2. User count crosses a threshold (e.g. 50 active accounts with real trades) — ensures FR-6 20-trade floor doesn't silence everyone.
3. Product decision is made that public profile is the primary growth loop surface.

Until then: backend shape exists in ENG-01b, no wiring needed. This spec is a bookmark.

---

## Session Context (2026-04-22)

Drafted during ENG-01b claim-bug diagnosis session. User flagged core tension: "dignitas doesn't have to connect to pnl but if we are posting shareable content nobody cares about dignitas, they care about if they can make money or not." This spec is the documented response to that insight, parked until distribution mechanics justify the build.
