# Specification: Chart Component Suite

**Spec ID:** JNL-08-chart-suite
**Date:** 2026-03-17
**Status:** Draft
**Class:** Feature / Frontend
**Priority:** P1 — core visual analytics
**Depends on:** JNL-06-analytics-api, JNL-07-dashboard-layout
**Series:** Batch 4 — Frontend Dashboard (JNL-07, JNL-08)

---

## Problem Statement

The dashboard needs 6-8 chart types to visualize trading performance. Each chart consumes a specific analytics API endpoint and renders in the brutalist dark theme. Charts must be performant with up to 365+ data points.

---

## User Stories

- **As a trader**, I want to see my equity curve to understand overall account trajectory.
- **As a trader**, I want to see which symbols I trade most and which are most profitable.
- **As a trader**, I want to identify patterns in when and how long I hold trades.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Equity curve: line chart with drawdown shading | High | charts/ |
| FR-2 | Daily P&L: bar chart (green/red bars per day) | High | charts/ |
| FR-3 | Cumulative profit: area chart | High | charts/ |
| FR-4 | Symbol distribution: donut chart (trade count per symbol) | High | charts/ |
| FR-5 | Market return: horizontal bar chart (P&L per symbol) | Medium | charts/ |
| FR-6 | Duration vs profitability: scatter plot | Medium | charts/ |
| FR-7 | Return distribution: histogram (daily % return buckets) | Medium | charts/ |
| FR-8 | Time distribution: heatmap (hour × day-of-week) | Low | charts/ |
| FR-9 | All charts respect global filters | High | charts/ |

---

## Technical Implementation

### Chart Library

Use **lightweight-charts** (v5.x, already used in testudo-web) for time-series (equity, daily P&L, cumulative). Use **D3.js** or **Chart.js** for statistical charts (donut, scatter, histogram, heatmap). Prefer lightweight-charts where possible for consistency.

### Component Structure

```
src/components/charts/
├── EquityCurve.tsx          — line + area (drawdown overlay)
├── DailyPnl.tsx             — bar chart
├── CumulativeProfit.tsx     — area chart
├── SymbolDonut.tsx          — donut/pie chart
├── MarketReturn.tsx         — horizontal bar chart
├── DurationScatter.tsx      — scatter plot
├── ReturnHistogram.tsx      — histogram
├── TimeHeatmap.tsx          — hour×day heatmap
└── ChartContainer.tsx       — reusable wrapper (title, loading, empty states)
```

### Charts Page Layout

```
┌─────────────────────────────────────────────────────┐
│  EQUITY CURVE                                        │
│  [━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━]  │
│  drawdown area shaded in red beneath the line        │
├──────────────────────────┬──────────────────────────┤
│  DAILY P&L               │  CUMULATIVE PROFIT       │
│  ▮▮ ▮ ▮▮▮ ▮ ▮▮          │  ░░░░░░░░░░░░░░░░░░░    │
├──────────────────────────┼──────────────────────────┤
│  SYMBOL DISTRIBUTION     │  MARKET RETURN           │
│      ◉ donut             │  ════ horizontal bars    │
├──────────────────────────┼──────────────────────────┤
│  DURATION / PROFIT       │  RETURN DISTRIBUTION     │
│  · · · scatter           │  ▐▐▐▐▐ histogram        │
└──────────────────────────┴──────────────────────────┘
```

- Equity curve spans full width (top)
- Remaining charts in 2-column grid
- Each chart in a card with `#0A0A0A` background, `#3F3F46` border

### Chart Styling

All charts must use the theme colors:
- Line/area positive: `#00FF41`
- Line/area negative: `#FF003C`
- Drawdown fill: `rgba(255, 0, 60, 0.15)`
- Grid lines: `#1A1A1A`
- Axis text: `#555555` (Space Mono)
- Tooltip background: `#111111`
- Tooltip border: `#3F3F46`

### Data Fetching

Each chart component calls its corresponding analytics endpoint:
```typescript
// EquityCurve.tsx
const [data] = createResource(() => filters(), (f) => fetchEquityCurve(f));
```

Charts show a skeleton/loading state while fetching and an empty state when no data.

### Files

- `testudo-journal/src/components/charts/` — all chart components
- `testudo-journal/src/pages/Charts.tsx` — charts page layout

---

## Acceptance Criteria

- [ ] All 8 chart types render correctly with sample data
- [ ] Charts use Testudo neon color scheme (#00FF41, #FF003C)
- [ ] Equity curve shows drawdown overlay as shaded red area
- [ ] Charts respond to global filter changes
- [ ] Loading skeleton shown during fetch
- [ ] Empty state shown when no trades match filter
- [ ] Charts render smoothly with 365+ data points
- [ ] All chart text uses Space Mono for numbers
- [ ] `bun run build` passes

---

## Completion Signal

This spec is complete when:
1. Charts page shows all 8 chart types with live API data
2. Visual style matches brutalist aesthetic
3. All acceptance criteria met
4. Code committed to master
