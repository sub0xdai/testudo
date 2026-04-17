# Specification: Unified Risk Hub — Expand Account into a Live Operational View

**Spec ID:** RSK-01-unified-risk-hub
**Date:** 2026-04-17
**Status:** Draft
**Class:** Feature / Frontend + Backend
**Priority:** P0 — retention-defining pillar; converts Testudo from retrospective journal to operational command center (mentor-validated acquisition wedge)
**Depends on:** None (first in RSK series)
**Series:** RSK-01 through RSK-03 (RSK-01 unified risk hub, RSK-02 setup tagging at entry, RSK-03 AI trade coach)

---

## Problem Statement

Testudo today is a **rearview mirror**: Overview (`/desk/`) is retrospective — Dignitas radar, P&L calendar, closed-trade analytics. It answers "how did I do?" beautifully but not "what am I exposed to *right now*?". Meanwhile the Account page (`pages/Account.tsx`) is reduced to two cards and vast negative space — it shows *which* exchanges are connected but nothing about their live state. Active positions are buried on the Trades page (`components/trades/ActivePositions.tsx`) with 30-second polling, disconnected from the venue context in Account.

The operational pain this leaves on the table is well-defined: **capital fragmentation blindness**. A trader with margin on Bybit, perps on Hyperliquid, and spot on WOO has no single view of aggregate leverage, net delta, correlation stacking, or free capital per venue. This is the loud, external, recognizable pain — the kind a user tweets about — and it's the class of problem they leave open on a second monitor 24/7. Addressing it converts Testudo from "I check it Sunday evening" to "I check it every 30 minutes."

Rather than introduce a new top-nav route (`COMMAND`), this spec **expands the existing Account page in place**. Account is already semantically correct for "my trading presence across venues"; the exchange cards are the natural anchor for per-venue live state. This preserves the 4-item nav, reclaims wasted whitespace, and leaves the pristine Overview untouched. A persistent slim pulse strip in the layout ensures live risk state is always visible without intruding on Overview's aesthetic.

---

## User Stories

- **As a multi-exchange trader**, I want one view showing total capital deployed and free margin across all venues, so I stop flipping between exchange tabs to size my next trade.
- **As a risk-conscious trader**, I want live aggregate leverage and net delta visible on a second monitor, so I know when my "diversified" positions have stacked into a single directional bet.
- **As a Testudo user writing a journal entry**, I want a glanceable live risk indicator on every page, so I don't lose situational awareness while reflecting on a trade.
- **As a trader preparing to deploy a new position**, I want to see which venue has the most free margin, so I pick the right one without leaving Testudo.
- **As an Overview-first user**, I want the retrospective aesthetic I already love to stay exactly the same, so new features don't disrupt the muscle memory I've built.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Account page displays a **Live Risk Strip** above exchange cards showing: net exposure ($), aggregate leverage (x), free margin ($), long/short delta (%) | High | journal/frontend |
| FR-2 | Account page displays **Live Positions grouped by venue** below exchange cards, replacing isolation on Trades page | High | journal/frontend |
| FR-3 | Account page displays a **Margin by Venue** widget: free capital per exchange, sorted descending | High | journal/frontend |
| FR-4 | Account page displays a **Correlation Stack** widget: MVP groups positions by asset family and direction to surface directional stacking (e.g., "3 longs on BTC/ETH/SOL = effective 2.4x BTC-beta long") | Medium | journal/frontend |
| FR-5 | Account page reserves a **Coach Banner slot** (empty in this spec; consumed by RSK-03) | Low | journal/frontend |
| FR-6 | A **Pulse Strip** component appears as a persistent 1-line header in `Layout.tsx` on every page, showing compact net exposure / leverage / free margin. Clicking routes to `/desk/account`. | Medium | journal/frontend |
| FR-7 | Live values update via **WebSocket push** on position-change events; fall back to 30s polling if WS disconnects | High | journal/frontend + router |
| FR-8 | **Empty states** are graceful: no positions → strip shows `0 exp · 0.0x · $X free`; no exchanges → existing AddExchangeCard flow unchanged | High | journal/frontend |
| FR-9 | **Mobile-responsive**: widgets collapse to single column ≤ md breakpoint; pulse strip compresses to `$X / Yx` | Medium | journal/frontend |
| FR-10 | New backend endpoint `GET /api/risk/snapshot` aggregates across all user exchanges and returns a single `RiskSnapshot` payload | High | router |
| FR-11 | Pulse Strip can be hidden per-user via preference toggle (default on) — avoids visual intrusion on Overview for users who prefer minimalism | Low | journal/frontend |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | `LiveRiskStrip.tsx` + `PulseStrip.tsx` rendered with **mock snapshot** on Account page + Layout | Aesthetic/layout fidelity against existing brutalist theme; mobile collapse |
| CP-2 | `GET /api/risk/snapshot` backend endpoint wired to real position + balance aggregation; replaces mock in strip | End-to-end: exchange APIs → router aggregator → frontend render |
| CP-3 | `PositionsByVenue.tsx` composed on Account below exchange cards, pulled from existing `fetchActivePositions` + grouped | Replaces Trades-page isolation; live data rendering per venue |
| CP-4 | `MarginByVenue.tsx` widget shipped using existing `fetchBalance` per account | Per-exchange free capital visibility |
| CP-5 | `CorrelationStack.tsx` MVP: asset-family grouping + directional bar (no statistical correlation yet) | Directional stacking surfaced without scope explosion |
| CP-6 | WS push integration: subscribe to `order.{user_id}` / position updates, debounce, re-fetch snapshot | Replaces polling; sub-second updates on position change |
| CP-7 | `CoachBanner.tsx` placeholder component wired into Account layout (renders null until RSK-03 provides content) | Slot reserved without breaking layout |
| CP-8 | Preference toggle for Pulse Strip in `pages/Account.tsx` (or settings surface) | FR-11 user control |

Each checkpoint is independently testable, committable, and leaves the app in a shippable state.

### Key Types

```typescript
// testudo-journal/src/api/client.ts
export interface RiskSnapshot {
  net_exposure_usd: string       // decimal as string (convention)
  aggregate_leverage: string     // e.g. "2.3"
  free_margin_usd: string
  long_pct: string               // 0..1
  short_pct: string
  net_delta_usd: string          // long_notional - short_notional
  positions_by_venue: VenuePositions[]
  margin_by_venue: VenueMargin[]
  correlation_stack: CorrelationBucket[]
  as_of: string                  // ISO timestamp
}

export interface VenuePositions {
  exchange_id: string
  exchange_name: string          // "BYBIT" | "HYPERLIQUID" | ...
  positions: ActivePosition[]
}

export interface VenueMargin {
  exchange_id: string
  exchange_name: string
  free_usd: string
  used_usd: string
  total_usd: string
}

export interface CorrelationBucket {
  // MVP: group by asset family ("BTC-beta", "ETH-beta", "alt-L1", "stables")
  bucket: string
  direction: 'long' | 'short' | 'mixed'
  effective_notional_usd: string
  contributing_symbols: string[]
}
```

```rust
// testudo-exchange/crates/router/src/services/risk_snapshot.rs
pub struct RiskSnapshot {
    pub net_exposure_usd: Decimal,
    pub aggregate_leverage: Decimal,
    pub free_margin_usd: Decimal,
    pub long_pct: Decimal,
    pub short_pct: Decimal,
    pub net_delta_usd: Decimal,
    pub positions_by_venue: Vec<VenuePositions>,
    pub margin_by_venue: Vec<VenueMargin>,
    pub correlation_stack: Vec<CorrelationBucket>,
    pub as_of: DateTime<Utc>,
}

pub async fn build_snapshot(user_id: Uuid, db: &PgPool) -> Result<RiskSnapshot, RiskError> {
    // Fan-out to existing services:
    // - fetch_balances_for_user (already exists per-account)
    // - fetch_open_positions_for_user
    // - aggregate → compute leverage, delta, correlation
}
```

### Component Composition — Account Page

```
pages/Account.tsx
├─ <PageSubHeader title="ACCOUNT" helpText={...} />
├─ <LiveRiskStrip snapshot={snapshot()} />        ← NEW (FR-1)
├─ <ExchangeCardsGrid />                          ← existing (unchanged)
├─ <PositionsByVenue snapshot={snapshot()} />     ← NEW (FR-2)
├─ <div class="grid grid-cols-1 lg:grid-cols-2">
│    <MarginByVenue snapshot={snapshot()} />      ← NEW (FR-3)
│    <CorrelationStack snapshot={snapshot()} />   ← NEW (FR-4)
│  </div>
└─ <CoachBanner />                                ← NEW placeholder (FR-5)
```

### Paved Roads

- **Aesthetic language** — reuse signal-green/signal-red, `font-mono`, `border-container-border`, `bg-container-bg` tokens. No new CSS primitives.
- **`PageSubHeader` + `HELP`** pattern for headers and tooltips (`src/lib/help-content.ts`).
- **`StatSection`** component (`src/components/StatSection.tsx`) for metric rows inside widgets.
- **`formatCurrency`, `formatPercent`, `pnlColor`, `rColor`** from `src/lib/formatters.ts` — use throughout.
- **`createResource` + WS channel** — existing pattern in `Overview.tsx` and `ActivePositions.tsx`.
- **`PerformanceRadar` layout** — visual density reference for Correlation Stack.
- **`ExchangeCard`** (`components/account/ExchangeCard.tsx`) — template for per-venue groupings.
- **`fetchBalance`, `listAccounts`** endpoints in `api/client.ts` — aggregate source for `MarginByVenue`.
- **`fetchActivePositions`** — source for `PositionsByVenue` grouping.

### Files

**New:**
- `testudo-journal/src/components/account/LiveRiskStrip.tsx` — 1-row strip with 4 metrics
- `testudo-journal/src/components/account/PositionsByVenue.tsx` — grouped active positions
- `testudo-journal/src/components/account/MarginByVenue.tsx` — per-exchange free capital
- `testudo-journal/src/components/account/CorrelationStack.tsx` — directional stacking widget
- `testudo-journal/src/components/account/CoachBanner.tsx` — placeholder slot
- `testudo-journal/src/components/PulseStrip.tsx` — persistent layout header
- `testudo-exchange/crates/router/src/services/risk_snapshot.rs` — aggregator service
- `testudo-exchange/crates/router/src/routes/risk.rs` — route handler
- `testudo-exchange/crates/router/tests/risk_snapshot_test.rs` — integration tests

**Modified:**
- `testudo-journal/src/pages/Account.tsx` — compose new widgets below exchange cards
- `testudo-journal/src/components/Layout.tsx` — mount `PulseStrip` at top
- `testudo-journal/src/api/client.ts` — add `fetchRiskSnapshot`, `RiskSnapshot` type
- `testudo-journal/src/lib/help-content.ts` — add help entries for new sections
- `testudo-exchange/crates/router/src/routes/mod.rs` — wire risk route
- `testudo-exchange/crates/router/src/main.rs` — register route

### Dependencies Added

None expected. `echarts` already present for any visualization; `rust_decimal` already in use.

---

## Acceptance Criteria

- [ ] `LiveRiskStrip` renders above exchange cards with all 4 metrics (exposure, leverage, free margin, long/short %)
- [ ] `PositionsByVenue` renders active positions grouped by exchange on the Account page
- [ ] `MarginByVenue` + `CorrelationStack` render in a 2-col grid, collapse to 1-col on mobile
- [ ] `PulseStrip` appears at top of every page via `Layout.tsx`; hides when user preference is off
- [ ] Values update within 2s of a position change via WS push (verified manually in a live session)
- [ ] Empty state (no open positions) shows `0 exp · 0.0x · $X free` without layout collapse or errors
- [ ] `CoachBanner` renders `null` in this spec — no visual artifact left behind
- [ ] Overview page `/desk/` is byte-identical before and after this spec (snapshot test or visual diff)
- [ ] Backend: `cd testudo-exchange && cargo clippy --all-targets && cargo test` passes
- [ ] Frontend: `cd testudo-journal && bun run build` succeeds
- [ ] New integration test `risk_snapshot_test.rs` exercises multi-exchange aggregation with fixture data
- [ ] Mobile viewport (≤ 768px) does not clip any widget
- [ ] Help tooltips present on each new section via existing `HELP` system

---

## Risks

1. **WebSocket instability degrades the "live" promise.** If the WS disconnects mid-session, stale numbers erode trust fastest on this exact page. *Mitigation:* fall back to 30s polling on WS disconnect, render a subtle `● stale` indicator in the Pulse Strip when `as_of` is older than 60s.
2. **Correlation widget scope creep.** Full statistical correlation (covariance matrix across symbols) is a tarpit. *Mitigation:* MVP is strict — group by asset family (hard-coded mapping), show directional bars. Statistical correlation is deferred to a future spec explicitly.
3. **Account page becomes too dense, sacrificing aesthetic.** The current Account is minimalist; adding 4 widgets risks clutter. *Mitigation:* enforce generous vertical spacing (match Overview's `p-8` rhythm), progressive disclosure on mobile, design-review before merge. If density becomes a problem, defer `CorrelationStack` to its own view.
4. **Pulse strip intrudes on Overview's hero.** Overview is aesthetically complete — adding a header strip above it could hurt the composition. *Mitigation:* FR-11 preference toggle defaults ON but is easy to disable; evaluate auto-hiding the strip when scroll position = 0 on Overview specifically (fade in on scroll).
5. **Correlation hard-coded asset families drift.** New meme tokens, new L1s mean the hard-coded bucket list goes stale. *Mitigation:* bucket list lives in config (`src/config/asset_families.ts`), documented as expected to change, covered by a unit test that asserts "unknown assets fall into `other` bucket."
6. **Backend aggregation latency.** Fan-out to every exchange per snapshot could be slow on users with 5+ venues. *Mitigation:* parallelize with `tokio::join!`, cache snapshot for 5s server-side, stream via WS after initial fetch.

---

## Completion Signal

This spec is complete when:
1. All FR-1 through FR-11 implemented and tested
2. All acceptance criteria checked off
3. Manual verification session: open 2+ positions on 2 different exchanges, observe live aggregation in Account page and Pulse Strip within 2s of fill event
4. Overview page visual diff confirms zero regression on the retrospective view
5. `cargo clippy --all-targets && cargo test` (backend) and `bun run build` (journal) both pass
6. Code committed to master via conventional-commit message: `feat(rsk-01): unified risk hub on Account page + pulse strip`
7. Landing-page copy update queued for web submodule (separate chore): "Command your capital — one unified view across every exchange."
