# Overview + Analytics Redesign

**Date:** 2026-04-05
**Status:** Design Complete
**Inspiration:** CoinMarketMan (CMM) UX lessons — cognitive separation, symmetrical comparison, ratio visualization

---

## Problem

The Overview page has 14 metrics crammed into a narrow sidebar alongside the equity curve. This causes cognitive overload — traders can't distinguish "North Star" KPIs from secondary stats at a glance. There's no Long/Short performance breakdown despite the data existing in the database. No dedicated space for deep analytics.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| North Star metrics | Net P&L, Win Rate, Expectancy, Profit Factor | How much, how often, how efficiently, how sustainable |
| Where secondary metrics go | New ANALYTICS tab | Clean separation > collapsed drawers |
| Overview layout | Horizontal KPI strip + full-width equity curve | Sidebar wastes space with only 4 stats; strip is the exchange-native pattern |
| Analytics hero element | Long/Short comparison | Unique directional insight, not just "more numbers" |
| Risk of Ruin visibility | Conditional warning on Overview when > 50% | Critical risk metric must surface, not hide in deep stats |
| Filter state | Shared via URL search params | Switching tabs preserves filter context |
| Max DD representation | Show both % and $ | Percentage is meaningful on small accounts, absolute on large |

---

## Overview Page (Redesigned)

### Zone 0 — Risk of Ruin Warning (Conditional)

When Risk of Ruin exceeds 50%, a full-width alert banner appears above the KPI strip:

```
⚠ RISK OF RUIN: 82.4% — current strategy has high probability of account depletion
```

- `signal-orange` background at 10% opacity, `signal-orange` border and text
- Only renders when threshold exceeded, otherwise invisible
- Links to Analytics page for full risk breakdown

### Zone 1 — KPI Strip

Full-width horizontal row, ~80px tall. Four metric blocks separated by subtle vertical dividers:

```
| Net P&L          | Win Rate        | Expectancy      | Profit Factor   |
| -$29.34          | 21.1%           | -$0.51          | 0.50            |
```

- Mono font, large value, small label above
- Net P&L and Expectancy color-coded (green positive / red negative)
- Profit Factor color-coded (green > 1.0 / red < 1.0)
- Filter bar (exchange, time presets) stays above this strip

#### Responsive (mobile)

KPI strip wraps to a 2x2 grid on screens below `md` breakpoint. Each cell maintains the same label-above-value layout.

### Zone 2 — Full-Width Equity Curve

The equity curve expands to full content width (no sidebar stealing 224px). This is the hero element. Chart selectors (Symbol Distribution, Daily P&L, etc.) remain as a 2-column grid below the curve.

### What Moves to Analytics

Streak, R-Multiple, Trades/Day, Fees, Total P&L, Trades count, Max DD, Worst Day, Worst Week, Best Streak — all 10 secondary metrics.

---

## Analytics Page (New)

Route: `/desk/analytics`
Nav position: between OVERVIEW and JOURNAL

### Section 1 — Long/Short Comparison (Hero)

Two mirrored rows with identical columns and integrated ratio bars. Green left-border for Longs, red for Shorts:

```
LONGS   | Count: 42  | Win Rate: [28%]  | Net P&L: +$18.50
SHORTS  | Count: 15  | Win Rate: [13%]  | Net P&L: -$47.84
───────────────────────────────────────────────────────
Long/Short Split  [================----]  74% / 26%
Win/Loss Ratio    [====----------------]  21% / 79%
```

- Win Rate displayed as fill bar within each row
- Each row is a single horizontal card with colored left border
- Symmetrical layout enables instant vertical comparison
- Ratio bars are inline below the comparison rows — part of the same visual section, not a standalone block
- Avg Duration is per-side only (shown in comparison rows, not duplicated in deep stats)

#### Responsive (mobile)

Long/Short rows stack vertically. Each row becomes a full-width card. Ratio bars remain horizontal below.

### Section 2 — Deep Stats Grid

Full-width 2-column table with dotted leader lines:

| Left Column | Right Column |
|-------------|--------------|
| Profit Factor ......... 0.50 | R-Multiple ......... 0.00 |
| Trades/Day ......... 1.8 | Streak ......... -3 |
| Total P&L ......... -$18.64 | Fees ......... +$10.70 |
| Trades ......... 57 | Max DD ......... 952.8% ($27.41) |
| Worst Day ......... -$19.22 | Best Day ......... +$8.12 |
| Worst Week ......... -$25.42 | Best Week ......... +$6.30 |
| Risk of Ruin ......... 82.4% | Best Streak ......... +2 |

- Max DD shows percentage first (primary), dollar amount in parentheses (secondary)
- Avg Duration removed — shown per-side in Long/Short comparison above, not duplicated as an aggregate

#### Responsive (mobile)

Grid collapses to single column. All stat rows stack vertically.

---

## Filter State Sharing

Filters (exchange, time preset, symbol) are persisted in URL search params (`?exchange=hyperliquid&period=1W&symbol=BTC_USDT`). Both Overview and Analytics read from and write to the same params. Navigating between tabs preserves the active filter without additional state management.

The existing `FilterProvider` context should sync with URL params on mount and update params on change. This is a one-time wiring — no new context needed.

---

## Implementation Requirements

### Backend

1. **Extend `StatsFilter`** with `side: Option<String>` — add `AND ($6::TEXT IS NULL OR side = $6)` to existing SQL queries
2. **New endpoint** `GET /api/v1/journal/analytics/side-breakdown` — returns stats computed separately for LONG and SHORT
3. **Ratio computation** — `COUNT(*) FILTER (WHERE side = 'LONG')` / total for Long/Short split; win/loss already exists
4. **Risk of Ruin** — ensure this value is returned in the existing stats response (may already be computed)

### Frontend

1. **New route** `/desk/analytics` in `index.tsx` + nav bar entry
2. **Refactor Overview** — replace sidebar with horizontal KPI strip (swap Streak for Profit Factor), equity curve goes full-width, add conditional Risk of Ruin banner
3. **New `Analytics.tsx` component** — two sections (Long/Short comparison with integrated ratio bars, deep stats grid)
4. **API client** — new `fetchSideBreakdown()` function
5. **Filter sync** — wire `FilterProvider` to URL search params for cross-tab persistence
6. **Responsive breakpoints** — KPI strip → 2x2 grid, Long/Short rows → stacked cards, deep stats → single column

### What Doesn't Change

- Equity curve component (just gets wider)
- Chart selectors below equity curve (same 2-column grid)
- Filter bar component (reused on both pages, now synced to URL)
- All existing backend stats logic (reused with new `side` filter)

### Data Availability

The `journal_trades` table already has `side TEXT NOT NULL` with values LONG/SHORT. All P&L, fees, duration, and risk fields are in place. No migration needed — this is purely query + UI work.

---

## Vertical Checkpoints

| CP | Scope | Validates |
|----|-------|-----------|
| CP-1 | Backend: add `side` to StatsFilter + side-breakdown endpoint | API returns separate Long/Short stats |
| CP-2 | Overview: KPI strip (with Profit Factor) + Risk of Ruin banner + remove sidebar + full-width equity curve | Overview renders with new layout |
| CP-3 | Analytics page: Long/Short comparison with ratio bars + deep stats (Max DD dual format, no duplicate Avg Duration) | New route works end-to-end |
| CP-4 | Filter sync: URL search params shared between Overview and Analytics | Switching tabs preserves filters |
