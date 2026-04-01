# Specification: Polish Overview Dashboard Layout and Charts

**Spec ID:** UX-02-overview-polish
**Date:** 2026-03-30
**Status:** Draft
**Class:** Feature / UX Polish
**Priority:** P1 — Dashboard is the first thing traders see; inconsistent spacing, competing background texture, and confusing dual-line equity curve hurt readability and trust.
**Depends on:** None
**Series:** UX-01 through UX-02 (UX polish)

---

## Problem Statement

The Overview page (`testudo-journal/src/components/Overview.tsx`) has several data visualization and layout issues that degrade the trading dashboard experience.

**Spacing inconsistency**: The page mixes `p-4`, `p-5`, `p-6`, `px-6 py-4`, and `px-4 py-2` across different panels. The chart grid uses `gap-6 p-6` while chart interiors use `p-4`, creating a jarring mismatch where outer spacing doesn't match inner spacing. Sidebar stat sections use `px-4` while the main content uses `px-6`.

**Background texture competing with data**: The main content area renders charts directly over the background image/texture with no blur layer, making data hard to read at a glance. The sidebar uses `glass-panel` (blur + semi-transparent bg) but the main content column does not — it only applies glass-panel to the hero metrics bar (`Overview.tsx:148`), leaving the equity curve and chart grid on raw background.

**Equity curve confusion**: `HeroEquityCurve.tsx` renders two overlapping series — a green `LineSeries` for cumulative P&L and a red `AreaSeries` for drawdown — on the same Y-axis. These compete visually and make it unclear whether the chart shows profit or loss at any given point. The standard approach is a single baseline series that colors green above zero and red below.

**Symbol distribution donut chart**: `SymbolDonut.tsx` uses a pie/donut chart which is poor for precise comparison when values are close. A horizontal bar chart with labels on the Y-axis is more readable, especially on a trading dashboard. The chart selector should default to this bar chart view.

**Chart selector defaults**: The left chart defaults to `symbol` (donut) and the right to `daily-pnl`. Better defaults: left = `symbol` (as horizontal bar), right = `daily-pnl` (already good). The title "DAILY P&L" should read "DAILY P&L HISTORY" for clarity.

---

## User Stories

- **As a trader**, I want all dashboard panels to have uniform spacing and a consistent blur backdrop, so that I can read my performance data without visual noise from the background texture.
- **As a trader**, I want the equity curve to clearly show profit above zero in green and loss below zero in red, so that I can instantly assess my P&L trajectory.
- **As a trader**, I want symbol allocation displayed as horizontal bars, so that I can precisely compare my distribution across assets.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | All horizontal padding across sidebar, hero, equity label, chart grid, chart selector header, and chart selector content normalised to `p-5` (20px). Grid gap normalised to `gap-5`. | High | Overview, ChartSelector |
| FR-2 | Main content column (`flex-1 min-w-0` in Overview.tsx:146) wrapped with `glass-panel` class, providing `backdrop-filter: blur(12px)` and semi-transparent background over the texture. | High | Overview |
| FR-3 | `HeroEquityCurve` replaced from dual-series (LineSeries + AreaSeries) to single `BaselineSeries` with `baseValue: { type: 'price', price: 0 }`, green fill above zero, red fill below zero. | High | HeroEquityCurve |
| FR-4 | `SymbolDonut.tsx` chart type changed from `pie` (donut) to horizontal `bar` with `yAxis: category`, symbols on Y-axis, bar values extending right. Rename component file to `SymbolBreakdown.tsx`. | High | SymbolDonut |
| FR-5 | StatSection title styling changed from `text-text-tertiary` to `text-text-secondary font-bold` for stronger visual section breaks. | Medium | StatSection |
| FR-6 | `DailyPnl.tsx` chart title changed from "DAILY P&L" to "DAILY P&L HISTORY". | Low | DailyPnl |
| FR-7 | Consistent border treatment: all section separators use `border-container-border/50` (half-opacity) instead of mixing full-opacity and half-opacity borders. | Medium | Overview |
| FR-8 | Mobile layout (single-column) retains functional parity — no layout shift or broken spacing on small screens. | Medium | Overview |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Normalise spacing + glass-panel blur on content area (FR-1, FR-2, FR-7) | Visual: uniform 20px spacing, blur backdrop |
| CP-2 | Replace equity curve with BaselineSeries (FR-3) | Visual: single green/red line at zero |
| CP-3 | Convert symbol donut to horizontal bar + stat title styling + daily pnl rename (FR-4, FR-5, FR-6) | Visual: bar chart, bolder titles, label update |

### Spacing Normalisation Map

| Element | File | Current | New |
|---------|------|---------|-----|
| Chart grid | `Overview.tsx:179` | `gap-6 p-6` | `gap-5 p-5` |
| Hero metrics | `Overview.tsx:148` | `px-6 py-4` | `px-5 py-4` |
| Equity label | `Overview.tsx:170` | `px-6 pt-4` | `px-5 pt-4` |
| StatSection title | `StatSection.tsx:17` | `px-4 py-3` | `px-5 py-3` |
| StatSection items | `StatSection.tsx:23` | `px-4 py-1.5` | `px-5 py-1.5` |
| ChartSelector header | `ChartSelector.tsx:46` | `px-4 py-2` | `px-5 py-3` |
| ChartSelector content | `ChartSelector.tsx:60` | `p-4` | `p-5` |
| Loading skeleton sidebar | `Overview.tsx:64` | `px-4 py-3` | `px-5 py-3` |
| Loading skeleton hero | `Overview.tsx:84` | `px-6 py-4` | `px-5 py-4` |

### Equity Curve — BaselineSeries

`lightweight-charts@^5.0.0` provides `BaselineSeries` which natively splits a line at a base value with different colors above and below:

```typescript
// HeroEquityCurve.tsx — replace dual series with single baseline
import { BaselineSeries } from 'lightweight-charts'

// Remove: equityLine (LineSeries) + drawdownArea (AreaSeries)
// Add:
const baseline = chart.addSeries(BaselineSeries, {
  baseValue: { type: 'price', price: 0 },
  topLineColor: getSignalGreen(),
  topFillColor1: signalGreenAlpha(0.15),
  topFillColor2: signalGreenAlpha(0),
  bottomLineColor: getSignalRed(),
  bottomFillColor1: signalRedAlpha(0),
  bottomFillColor2: signalRedAlpha(0.15),
  lineWidth: 2,
  priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
})

// Data: only cumulative_pnl (no separate drawdown series)
baseline.setData(data.map(p => ({
  time: p.date as string,
  value: parseFloat(p.cumulative_pnl),
})))
```

### Symbol Horizontal Bar Chart

```typescript
// SymbolBreakdown.tsx (renamed from SymbolDonut.tsx)
return {
  tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
  grid: { left: 100, right: 20, top: 10, bottom: 20 },
  yAxis: {
    type: 'category',
    data: d.data.map(s => s.symbol),
    axisLabel: { fontFamily: "'Space Mono', monospace", fontSize: 11 },
    inverse: true,
  },
  xAxis: {
    type: 'value',
    axisLabel: { show: false },
    splitLine: { show: false },
  },
  series: [{
    type: 'bar',
    data: d.data.map((s, i) => ({
      value: s.trade_count,
      itemStyle: { color: palette[i % palette.length] },
    })),
    barWidth: '60%',
    label: {
      show: true,
      position: 'right',
      formatter: (params: any) => `${params.value}`,
      fontFamily: "'Space Mono', monospace",
      fontSize: 11,
    },
  }],
}
```

### Paved Roads

- **`.glass-panel` CSS class** (`styles/app.css:61-65`): Already provides `backdrop-filter: blur(12px)` + `rgb(var(--bg-panel) / 0.7)` bg + `border: 1px solid rgb(var(--border) / 0.5)`. Reuse directly.
- **`BaselineSeries`** from `lightweight-charts@^5.0.0` (already in `package.json:18`): Native green/red split at base value. No new dependency.
- **`signalGreenAlpha()` / `signalRedAlpha()`** (`lib/tokens.ts:83,92`): Alpha-channel color helpers for fill colors. Already used by HeroEquityCurve.
- **`EChart` component** (`charts/EChart.tsx`): Existing ECharts wrapper used by SymbolDonut. Reuse for bar chart (only the options change).
- **`ChartContainer` component** (`charts/ChartContainer.tsx`): Wraps all standalone charts. No changes needed.

### Files

- `testudo-journal/src/components/Overview.tsx` — Normalise spacing values, add glass-panel to content column, update border consistency
- `testudo-journal/src/components/HeroEquityCurve.tsx` — Replace LineSeries + AreaSeries with BaselineSeries
- `testudo-journal/src/components/ChartSelector.tsx` — Normalise header/content padding
- `testudo-journal/src/components/StatSection.tsx` — Bolder section titles
- `testudo-journal/src/components/charts/SymbolDonut.tsx` — Rename to `SymbolBreakdown.tsx`, change from pie to horizontal bar
- `testudo-journal/src/components/charts/DailyPnl.tsx` — Rename title to "DAILY P&L HISTORY"

### Dependencies Added

None. `lightweight-charts@^5.0.0` and `echarts` are already in `package.json`.

---

## Acceptance Criteria

- [ ] All panels and sections use uniform 20px (`p-5` / `gap-5`) horizontal padding and grid gaps.
- [ ] Main content area has blur backdrop — background texture visible but softened behind chart data.
- [ ] Equity curve renders as a single BaselineSeries: green fill above zero, red fill below zero, no overlapping dual series.
- [ ] Symbol chart displays as horizontal bars with symbol labels on Y-axis, not a donut/pie.
- [ ] Sidebar stat section titles are `font-bold text-text-secondary` — visually stronger section breaks.
- [ ] DailyPnl chart title reads "DAILY P&L HISTORY".
- [ ] All section dividers use `border-container-border/50` consistently.
- [ ] Mobile single-column layout renders correctly — no overflow or broken spacing.
- [ ] `bun run build` passes with zero errors.

---

## Risks

1. **BaselineSeries API differences** — `lightweight-charts` v5 may have slightly different BaselineSeries options than v4. Mitigation: Check the v5 API docs. The `BaselineSeries` type is exported from `lightweight-charts` — import will fail at build time if the API changed, giving immediate feedback.

2. **ECharts bar chart tooltip/interaction regression** — Changing from pie to bar chart alters tooltip behavior and click-to-filter interactions. Mitigation: The `ChartContainer` wrapper handles loading/error/empty states identically regardless of chart type. Only the ECharts `option` object changes. Test tooltip format string matches the new `trigger: 'axis'` pattern.

---

## Completion Signal

This spec is complete when:
1. All spacing normalised to 20px across Overview, ChartSelector, and StatSection
2. Glass-panel blur applied to main content area
3. Equity curve uses BaselineSeries with green/red zero-split
4. Symbol chart renders as horizontal bars
5. All acceptance criteria met
6. `bun run build` passes
7. Code committed to master
