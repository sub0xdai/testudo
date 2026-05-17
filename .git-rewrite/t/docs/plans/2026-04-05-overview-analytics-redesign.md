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
| North Star metrics | Net P&L, Win Rate, Expectancy, Streak | How much, how often, how efficiently, what's the momentum |
| Where secondary metrics go | New ANALYTICS tab | Clean separation > collapsed drawers |
| Overview layout | Horizontal KPI strip + full-width equity curve | Sidebar wastes space with only 4 stats; strip is the exchange-native pattern |
| Analytics hero element | Long/Short comparison | Unique directional insight, not just "more numbers" |

---

## Overview Page (Redesigned)

### Zone 1 — KPI Strip

Full-width horizontal row, ~80px tall. Four metric blocks separated by subtle vertical dividers:

```
| Net P&L          | Win Rate        | Expectancy      | Streak          |
| -$29.34          | 21.1%           | -$0.51          | -3              |
```

- Mono font, large value, small label above
- Net P&L and Expectancy color-coded (green positive / red negative)
- Streak shows signed number with color
- Filter bar (exchange, time presets) stays above this strip

### Zone 2 — Full-Width Equity Curve

The equity curve expands to full content width (no sidebar stealing 224px). This is the hero element. Chart selectors (Symbol Distribution, Daily P&L, etc.) remain as a 2-column grid below the curve.

### What Moves to Analytics

Profit Factor, R-Multiple, Trades/Day, Fees, Total P&L, Trades count, Max DD, Worst Day, Worst Week, Best Streak — all 10 secondary metrics.

---

## Analytics Page (New)

Route: `/desk/analytics`
Nav position: between OVERVIEW and JOURNAL

### Section 1 — Long/Short Comparison (Hero)

Two mirrored rows with identical columns. Green left-border for Longs, red for Shorts:

```
LONGS   | Count: 42  | Win Rate: [28%]  | Avg Duration: 2h 14m  | Net P&L: +$18.50
SHORTS  | Count: 15  | Win Rate: [13%]  | Avg Duration: 1h 40m  | Net P&L: -$47.84
```

- Win Rate displayed as circular gauge or fill bar
- Each row is a single horizontal card with colored left border
- Symmetrical layout enables instant vertical comparison

### Section 2 — Ratio Bars

Three horizontal stacked progress bars, each totaling 100%:

```
Long/Short Split  [================----]  74% / 26%
Win/Loss Ratio    [====----------------]  21% / 79%
Profit Factor     [========------------]  0.50
```

- Green fill / red fill on single line
- Labels left, percentages right
- Three bars in ~100px vertical space

### Section 3 — Deep Stats Grid

Full-width 2-column table with dotted leader lines:

| Left Column | Right Column |
|-------------|--------------|
| Profit Factor ......... 0.50 | R-Multiple ......... 0.00 |
| Trades/Day ......... 1.8 | Avg Duration ......... 1h 52m |
| Total P&L ......... -$18.64 | Fees ......... +$10.70 |
| Trades ......... 57 | Max DD ......... $27.41 |
| Worst Day ......... -$19.22 | Best Day ......... +$8.12 |
| Worst Week ......... -$25.42 | Best Week ......... +$6.30 |
| Risk of Ruin ......... 82.4% | Best Streak ......... +2 |

---

## Implementation Requirements

### Backend

1. **Extend `StatsFilter`** with `side: Option<String>` — add `AND ($6::TEXT IS NULL OR side = $6)` to existing SQL queries
2. **New endpoint** `GET /api/v1/journal/analytics/side-breakdown` — returns stats computed separately for LONG and SHORT
3. **Ratio computation** — `COUNT(*) FILTER (WHERE side = 'LONG')` / total for Long/Short split; win/loss already exists

### Frontend

1. **New route** `/desk/analytics` in `index.tsx` + nav bar entry
2. **Refactor Overview** — replace sidebar with horizontal KPI strip, equity curve goes full-width
3. **New `Analytics.tsx` component** — three sections (comparison grid, ratio bars, deep stats)
4. **API client** — new `fetchSideBreakdown()` function

### What Doesn't Change

- Equity curve component (just gets wider)
- Chart selectors below equity curve (same 2-column grid)
- Filter bar component (reused on both pages)
- All existing backend stats logic (reused with new `side` filter)

### Data Availability

The `journal_trades` table already has `side TEXT NOT NULL` with values LONG/SHORT. All P&L, fees, duration, and risk fields are in place. No migration needed — this is purely query + UI work.

---

## Vertical Checkpoints

| CP | Scope | Validates |
|----|-------|-----------|
| CP-1 | Backend: add `side` to StatsFilter + side-breakdown endpoint | API returns separate Long/Short stats |
| CP-2 | Overview: KPI strip + remove sidebar + full-width equity curve | Overview renders with new layout |
| CP-3 | Analytics page: Long/Short grid + ratio bars + deep stats | New route works end-to-end |
