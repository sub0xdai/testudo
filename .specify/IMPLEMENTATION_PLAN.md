# Implementation Plan

> Last updated: 2026-03-30
> Current spec: UX-02-overview-polish
> Phase: COMPLETE

---

## Active Spec: UX-02-overview-polish

Polish Overview dashboard: standardise spacing, add blur backdrop, replace equity curve, convert symbol chart, bolder titles.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Normalise spacing to 20px (p-5/gap-5) across Overview, ChartSelector, StatSection + glass-panel blur on content column + consistent borders | complete | medium | — |
| T2 | Replace HeroEquityCurve dual-series with single BaselineSeries (green/red split at zero) | complete | medium | T1 |
| T3 | Convert SymbolDonut to horizontal bar chart (rename to SymbolBreakdown), bolder StatSection titles, rename DailyPnl title to "DAILY P&L HISTORY" | complete | medium | T1 |

### Key Decisions

- BaselineSeries from lightweight-charts v5 handles green/red split natively — no custom rendering needed
- Kept SymbolDonut.tsx as dead file (not deleted) — ChartSelector now imports SymbolBreakdown instead
- glass-panel applied to main content column, not individual chart panels (avoids double-blur)

### Discoveries

- lightweight-charts v5 BaselineSeries uses `'Baseline'` as the series type string in ISeriesApi generic
- ECharts horizontal bar needs `inverse: true` on yAxis to show top symbol first
- `containLabel: false` in grid config needed to prevent label overflow with long symbol names

---

## Completed Specs

- UX-01-pair-page (COMPLETE)
- UX-02-overview-polish (COMPLETE)
