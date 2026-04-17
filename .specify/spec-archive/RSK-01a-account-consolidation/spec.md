# Specification: Account Consolidation — Per-Venue Dossier Tiles, Kill Redundant Widgets

**Spec ID:** RSK-01a-account-consolidation
**Date:** 2026-04-18
**Status:** Draft
**Class:** Refactor / Frontend
**Priority:** P1 — UX correction on shipped RSK-01; removes structural redundancy before more features land on top
**Depends on:** RSK-01 (shipped, archived) — this spec consumes RSK-01's backend endpoint and deletes/collapses most of its frontend output
**Series:** RSK-01 → RSK-01a → RSK-02 → RSK-03

---

## Problem Statement

RSK-01 shipped all 12 tasks and achieved its functional goal — the Account page surfaces aggregate risk, per-venue positions, margin, and a correlation view powered by a new `/api/v1/risk/snapshot` endpoint. Functionally correct. Structurally redundant.

Critique of the shipped result ([UX critique captured 2026-04-18](../../spec-archive/RSK-01-unified-risk-hub/spec.md)) surfaced one core problem: **the Account page tells a user with one position on one exchange the same fact in six places**. `LiveRiskStrip`, `PulseStrip` (via Layout), the Bybit exchange card, `PositionsByVenue`, `MarginByVenue`, and `CorrelationStack` all render the same underlying data from different angles. Each widget is well-crafted in isolation. Together they produce a page that feels bloated in the sparse-state case and will still read as over-structured in the multi-venue case because the widgets are cross-cutting rather than topical.

The root diagnosis: **per-venue data was rendered in cross-venue widgets instead of being absorbed into the venue it belongs to**. Margin is a property of a venue; positions open on a venue are a property of that venue; the correlation *between* venues is the only cross-venue concept. The shipped layout inverted this — it hoisted per-venue facts into separate bands and left the exchange card impoverished.

This spec corrects the structure. The Exchange Card grows into a **per-venue dossier tile** carrying all of the venue's live state (margin breakdown, positions, credential identity). The cross-venue widgets (`MarginByVenue`, `PositionsByVenue`, `LiveRiskStrip`) are deleted because their content now lives inside the tiles. `CorrelationStack` survives as the only legitimate cross-venue widget, rendered conditionally when there are 2+ buckets. The `PulseStrip` is deleted globally because its metrics now live inline on the Overview hero where they have room, and the Account hero no longer exists to duplicate them.

Result: one topical tile per venue, zero redundancy, same information density — less code.

---

## User Stories

- **As a single-venue user**, I want Account to feel complete rather than sparse, so the page doesn't over-promise structure it doesn't need.
- **As a multi-venue trader**, I want each venue's margin and positions co-located with its credential card, so my mental model of "per-exchange state" matches the UI.
- **As an Overview-first user**, I want aggregate exposure/leverage/free-margin visible inline in the Overview hero, so I don't need a separate strip at the top of every page.
- **As a user on Journal or Pair**, I want no persistent strip above the header, so those pages' aesthetics remain uncluttered.
- **As a user with one asset family**, I want CorrelationStack hidden, so I'm not shown a widget that can't teach me anything yet.
- **As a user with ≥2 asset families**, I want CorrelationStack rendered as a cross-venue summary at the top of Account, so stacking risk is the first thing I see.

---

## Non-Goals (Explicit Anti-Scope)

- **No backend changes.** `/api/v1/risk/snapshot` stays as-is. Field usage on the frontend shifts (some consumers deleted, some added); the wire contract is unchanged.
- **No new widgets.** This is a consolidation — net delete of code, not net add.
- **Not introducing bento/grid frameworks.** The layout uses the same Tailwind grid primitives already in the codebase.
- **Not touching the Overview analytics (calendar, charts, radar).** Only the Overview hero row changes.
- **No changes to the Pair or Journal pages' headers** beyond removing the PulseStrip mount.
- **No Coach Banner content.** The slot stays reserved (RSK-03's concern), but its Account-page placement is revisited (moves to its final position in the new layout).

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Overview hero renders inline aggregate metrics next to existing `net P&L` + `balance`: `net exposure`, `leverage`, `free margin`. Formatted as an extension of the existing mono-typography hero row. | High | journal/frontend |
| FR-2 | `PulseStrip` component is removed from `Layout.tsx` and deleted along with its preference file (`pulse-strip-preference.ts`). The `testudo-pulse-strip` localStorage key is no longer read or written. | High | journal/frontend |
| FR-3 | `LiveRiskStrip` component is removed from `Account.tsx` and deleted. The four-metric hero on Account no longer exists (the data lives on Overview hero only). | High | journal/frontend |
| FR-4 | `ExchangeCard` is expanded to a **rich dossier tile** containing: identity row (already present), margin breakdown (total / free / used with a free-ratio bar), and an inline compact position list for that venue (symbol · side · size · unrealized PnL). | High | journal/frontend |
| FR-5 | `MarginByVenue` component is deleted. All margin data is rendered inside the per-venue Exchange Card tile. | High | journal/frontend |
| FR-6 | `PositionsByVenue` component is deleted. All positions are rendered inside the per-venue Exchange Card tile. Venues with zero positions show an inline "no open positions" hint within the tile (not a separate empty widget). | High | journal/frontend |
| FR-7 | `CorrelationStack` moves to the top of the Account page (above the exchange-tile grid) and renders **only when `snapshot.correlation_stack.length >= 2`**. Single-bucket or zero-bucket case → component returns `null`, reserves no vertical space. | High | journal/frontend |
| FR-8 | Exchange tile grid is a responsive 3-col grid on `lg` breakpoint, 2-col on `md`, 1-col on mobile. Empty "add exchange" slots fill the grid to visual completeness (at least one dashed `+ ADD EXCHANGE` slot always visible; enough slots rendered to reach a minimum of 3 tiles across). | Medium | journal/frontend |
| FR-9 | `CoachBanner` placeholder slot stays on Account, moves to the bottom of the new layout (below the exchange tile grid). Still renders `null` until RSK-03. | Low | journal/frontend |
| FR-10 | `fetchRiskSnapshot` data flow is unchanged. Overview consumes it for hero inline metrics; Account consumes it for correlation stack + exchange tile enrichment (margin + positions lookup per `exchange_id`). WS push + 30s polling fallback (from RSK-01 T10) continues to drive updates; logic moves from `Layout` (where it lived because of PulseStrip) into `Overview.tsx` and `Account.tsx` as the actual consumers. | High | journal/frontend |
| FR-11 | No UI exposes a PulseStrip preference toggle. The toggle on Account (introduced in RSK-01 T11) is removed along with the strip. | High | journal/frontend |
| FR-12 | Overview hero retains an "as of" subtle indicator (e.g., `● live` / `● stale` dot) so the removed PulseStrip's staleness signal still surfaces — just inline. | Medium | journal/frontend |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Overview hero inline metrics + live/stale dot. `fetchRiskSnapshot` ownership moves from Layout into Overview. WS push + polling fallback preserved. | FR-1, FR-12 and the "Overview as single live surface" principle |
| CP-2 | ExchangeCard grown into rich dossier (margin breakdown + free-ratio bar). No positions yet — positions stay in `PositionsByVenue` until CP-3 so that each step leaves the app shippable. | FR-4 (margin half) + visual validation that the tile holds the content |
| CP-3 | ExchangeCard absorbs positions. `PositionsByVenue` + `MarginByVenue` deleted in the same commit. Account page composition updated. | FR-4 (positions half), FR-5, FR-6 |
| CP-4 | `CorrelationStack` moves to top of Account + conditional render. Exchange tile grid structure + "add" slot filler logic. | FR-7, FR-8 |
| CP-5 | `LiveRiskStrip`, `PulseStrip`, `pulse-strip-preference.ts` deleted. Layout and Account cleaned up. | FR-2, FR-3, FR-11 |
| CP-6 | Full verification pass — builds, Overview unchanged below hero, Account tiles correct across breakpoints, Journal + Pair unaffected. | Completion signal |

Each checkpoint is committable in isolation. CP-1 + CP-2 + CP-3 together are the substantive work; CP-4, CP-5, CP-6 are structural tidy-up + verification.

### Architecture Change

**Before (shipped RSK-01):**
```
Layout.tsx
├─ PulseStrip ..................... OWNS fetchRiskSnapshot + WS
├─ header
└─ main
    └─ Account.tsx
        ├─ LiveRiskStrip .......... cross-venue hero
        ├─ ExchangeCard grid ...... identity + balance only
        ├─ PositionsByVenue ....... cross-venue table
        ├─ MarginByVenue .......... cross-venue list
        ├─ CorrelationStack ....... always renders
        └─ CoachBanner
```

**After (RSK-01a):**
```
Layout.tsx ......................... NO live data ownership
├─ header
└─ main
    ├─ Overview.tsx
    │   └─ Hero row .............. OWNS fetchRiskSnapshot + WS, inline metrics + live dot
    └─ Account.tsx
        ├─ CorrelationStack ...... renders only when ≥2 buckets
        ├─ ExchangeCard grid ..... each card = full per-venue dossier (identity + margin + positions)
        │   └─ "+ ADD" slots ..... fill to 3-tile row
        └─ CoachBanner ........... slot, null until RSK-03
```

Account also owns its own `fetchRiskSnapshot` resource (same 5s server-side cache makes this free). The two consumers (Overview hero + Account page) re-fetch independently; no context/provider is introduced.

### ExchangeCard Dossier Layout

Target visual density for the rich tile:

```
┌──────────────────────────────┐
│ ● BYBIT  [CEX]            ⋮  │   ← identity row (existing)
│ Bybit Account                │
│                              │
│ $78.29   total               │
│ ▓▓▓▓▓▓▓▓▓░░   94% free       │   ← margin breakdown (NEW, from FR-4)
│ $74.51 free · $15.20 used    │
│                              │
│ ── 1 POSITION ────────────── │   ← inline positions (NEW, from FR-4)
│ SOL_USDT · LONG              │
│ 1.3 @ 87.85 → 89.26          │
│ +$1.83                       │
└──────────────────────────────┘
```

Zero-position tile:

```
┌──────────────────────────────┐
│ ● HYPERLIQUID  [PERPS]    ⋮  │
│ Main                         │
│                              │
│ $45.00    total              │
│ ▓▓▓▓▓▓▓▓▓▓▓░   100% free     │
│ $45.00 free · $0.00 used     │
│                              │
│ ── NO OPEN POSITIONS ──      │
└──────────────────────────────┘
```

"Add exchange" slot retains its current dashed-outline + "+ ADD EXCHANGE" treatment.

### Overview Hero Extension

Current:
```
+$5.91 net P&L   $77.60 balance
```

Target:
```
+$5.91 net P&L   $77.60 balance   $115 exp   1.5x   $73 free   ● live
```

The `● live` dot uses existing signal color tokens — `signal-green` when WS connected + data fresh, `signal-amber` when polling-only or `as_of` older than 60s. Hover/focus reveals "last updated: {relative time}".

Mobile: collapses gracefully — `+$5.91 · $77.60 · 1.5x · ● live` (drop the labels that won't fit, keep the indicators).

### Paved Roads

- **Existing RSK-01 backend contract** — `RiskSnapshot` type unchanged; `positions_by_venue` + `margin_by_venue` arrays are consumed differently on the frontend but not renamed.
- **ExchangeCard.tsx** — existing identity + balance display is the base; margin and positions added as new sections within the same component file.
- **`formatCurrency`, `formatPercent`, `pnlColor`** — reused throughout.
- **`createResource` + WS channel** — pattern already exists in RSK-01 T10; moves from Layout ownership to Overview + Account ownership.
- **Responsive grid utilities** — existing Tailwind `grid-cols-1 md:grid-cols-2 lg:grid-cols-3` classes.
- **Free-ratio bar** — can reuse the same visual grammar as `CorrelationStack`'s bar (a thin filled rectangle proportional to a ratio) to keep visual language coherent.

### Files

**Deleted:**
- `testudo-journal/src/components/PulseStrip.tsx`
- `testudo-journal/src/components/account/LiveRiskStrip.tsx`
- `testudo-journal/src/components/account/MarginByVenue.tsx`
- `testudo-journal/src/components/account/PositionsByVenue.tsx`
- `testudo-journal/src/lib/pulse-strip-preference.ts`

**Modified:**
- `testudo-journal/src/components/Layout.tsx` — remove PulseStrip mount, remove WS + polling ownership (moved to Overview), remove pulse-preference wiring
- `testudo-journal/src/components/Overview.tsx` — extend hero row with inline exposure/leverage/free + live dot, own `fetchRiskSnapshot` resource + WS + polling fallback
- `testudo-journal/src/components/account/ExchangeCard.tsx` — add margin breakdown section (total / free / used + ratio bar) and inline positions list; derive both from the passed-in `snapshot` for this card's `exchange_id`
- `testudo-journal/src/pages/Account.tsx` — remove LiveRiskStrip / MarginByVenue / PositionsByVenue imports + mounts; move CorrelationStack to top of page; restructure exchange tile grid (3-col lg, 2-col md, 1-col sm with "add" slot fillers); remove PulseStrip preference toggle row
- `testudo-journal/src/components/account/CorrelationStack.tsx` — add conditional early-return when `buckets.length < 2`
- `testudo-journal/src/lib/help-content.ts` — remove `risk.pulse`, `risk.long_short` entries (strip removed); update or remove `risk.positions_by_venue` and `risk.margin_by_venue` entries (widgets deleted — remaining help copy moves to ExchangeCard's new sections if kept)
- `testudo-journal/src/components/account/CoachBanner.tsx` — unchanged file; position in Account.tsx moves to bottom of new layout

### Dependencies Added

None. Net delete.

---

## Acceptance Criteria

### CP-1 (Overview hero)
- [ ] Overview hero row renders `net P&L`, `balance`, `exposure`, `leverage`, `free margin` inline (desktop) / compressed form (mobile)
- [ ] Live dot reflects WS connection state: green/connected, amber/polling or stale
- [ ] Hover on live dot shows last-update relative time
- [ ] Overview's existing P&L calendar + charts + sidebar render unchanged (DOM snapshot diff or visual confirm)

### CP-2/CP-3 (Rich ExchangeCard)
- [ ] Each ExchangeCard shows margin breakdown (total / free / used) with a free-ratio bar
- [ ] Each ExchangeCard shows an inline mini-list of positions for that venue
- [ ] Zero-position venue shows "no open positions" inline within the card
- [ ] `PositionsByVenue`, `MarginByVenue` components no longer exist in the repo
- [ ] The positions list inside a card updates on WS fill events within 2s (via Account's resource refetch)

### CP-4 (CorrelationStack + grid)
- [ ] `CorrelationStack` renders at the top of Account when `buckets.length >= 2`
- [ ] `CorrelationStack` returns `null` (no height, no outline) when `< 2` buckets
- [ ] Exchange tile grid is 3-col `lg`, 2-col `md`, 1-col mobile
- [ ] "Add exchange" dashed slots fill empty grid positions so at least 3 tiles visible per row on `lg`
- [ ] CoachBanner slot sits at the bottom of the page

### CP-5 (PulseStrip removal)
- [ ] `PulseStrip.tsx`, `LiveRiskStrip.tsx`, `pulse-strip-preference.ts` files no longer exist
- [ ] Layout no longer mounts or imports any of the above
- [ ] No `testudo-pulse-strip` localStorage key is read or written anywhere in the codebase
- [ ] Account page no longer shows a pulse-strip preference toggle

### CP-6 (Verification)
- [ ] `cd testudo-journal && bun run build` passes
- [ ] `cd testudo-exchange && cargo check --all-targets` passes (no backend changes — should be clean)
- [ ] Journal page renders unchanged (no regression from PulseStrip removal — it was owned globally in Layout)
- [ ] Pair page renders unchanged (already was outside Layout's PulseStrip branch)
- [ ] Manual QA: single-venue user sees one rich tile + "+ add" slots, no sparse empty widgets
- [ ] Manual QA: multi-venue user (synthesize via fixtures if needed) sees CorrelationStack at top + multiple dossier tiles

---

## Risks

1. **Per-card data lookup misses when `exchange_id` doesn't match.** ExchangeCard now derives its margin + positions from `snapshot` by matching `exchange_id`. A stale card (e.g., deleted exchange that hasn't refetched yet) could receive undefined data. *Mitigation:* defensive lookup with explicit fallback rendering ("margin unavailable" + empty positions list), no thrown errors. Cover with a unit test on the lookup helper.

2. **Overview hero becomes crowded on mobile.** Five metrics + live dot on a small viewport risks wrapping messily. *Mitigation:* explicit mobile collapse rule in FR-1 (drop labels, keep numbers + dot). Test at 320px, 375px, 414px viewports.

3. **Deleting preference storage without migration.** Users who explicitly disabled the pulse strip (via RSK-01 T11) will see their preference silently ignored. *Mitigation:* acceptable — the strip they opted out of no longer exists; nothing breaks. One-time localStorage key left orphaned is harmless and will be GC'd on browser wipe or can be cleaned in a later chore pass.

4. **Loss of at-a-glance metrics on non-Overview pages.** Users on Journal or Pair no longer see live exposure without navigating. *Mitigation:* accepted trade-off per Non-Goals. Overview IS the live-state surface; Journal is for retrospective entries; Pair is standalone for execution. If usage data later shows cross-page state awareness matters, a mini inline indicator in the existing nav can be added as a fast follow (not this spec).

5. **ExchangeCard becomes a large component with multiple concerns.** Identity + margin + positions could bloat the file. *Mitigation:* extract `ExchangeCardMargin.tsx` and `ExchangeCardPositions.tsx` as internal sub-components within the same folder if the single file exceeds ~250 lines. Keep the public surface as `<ExchangeCard snapshot={snapshot} account={account} />`.

6. **Responsive grid with "add" fillers over-commits to a fixed slot count.** Always showing 3 tiles per row could feel forced for a 2-exchange user. *Mitigation:* render `max(accounts.length, 3)` slots on `lg`, `max(accounts.length, 2)` on `md`, exact count on mobile. One or two "add" slots visible is an invitation; five "add" slots would look hollow — the max gate prevents that.

7. **WS ownership move could cause double-subscription if both Overview and Account mount the resource.** *Mitigation:* the WS client is a module-level singleton (check current `lib/ws.ts` implementation — it's designed for single-connection pattern). Each mount subscribes via a callback; multiple callbacks are fine. Unit-verify during CP-1.

---

## Completion Signal

This spec is complete when:
1. All FR-1 through FR-12 implemented and tested
2. All CP-1 through CP-6 acceptance criteria checked off
3. Net lines-of-code delta is **negative** (this is a consolidation — deletions should exceed additions)
4. Manual verification session: open Account with 1 venue → one rich tile + add slots visible; switch to Overview → hero shows all 5 metrics inline with live dot; navigate to Journal/Pair → no strip, no regression
5. `cd testudo-journal && bun run build` passes
6. Commits follow conventional format: `refactor(rsk-01a): CP-N — description`
7. Final commit: `refactor(rsk-01a): account consolidation complete`
8. RSK-01a is archived on completion (move to `.specify/spec-archive/`) alongside RSK-01
