# Implementation Plan

> Last updated: 2026-03-23
> Current spec: ANL-01-bloomberg-charts
> Phase: BUILD

---

## Active Spec: ANL-01-bloomberg-charts

Bloomberg-grade analytics charts — 8 new chart types added to the ChartSelector dropdown on the Overview page. Phase 1 (frontend-only) uses existing data; Phase 2 adds 4 new backend endpoints.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Register new ECharts components (`TreemapChart`, `LineChart`, `CalendarComponent`, `MarkLineComponent`) in `echarts-setup.ts` | complete | low | — |
| T2 | C1 Drawdown Chart — `DrawdownChart.tsx` using existing `fetchEquityCurve()` data, inverted area with max DD marker | complete | medium | T1 |
| T3 | C3 P&L Treemap — `PnlTreemap.tsx` using existing `fetchSymbolBreakdown()` data, rectangles sized by abs(P&L) | complete | medium | T1 |
| T4 | C7 Expectancy by Symbol — `ExpectancyBySymbol.tsx` using existing `fetchSymbolBreakdown()` data, bars colored by sign | complete | medium | T1 |
| T5 | C8 Holding Period Analysis — `HoldingPeriodAnalysis.tsx` using existing `fetchDurationProfit()` data, avg P&L per duration bucket | complete | medium | T1 |
| T6 | Wire Phase 1 charts into `ChartSelector.tsx` + build validation | complete | low | T2–T5 |
| T7 | Backend: `calendar_pnl()` endpoint — SQL GROUP BY DATE(closed_at), add route + handler + service method | pending | medium | — |
| T8 | Backend: `streaks()` endpoint — iterate trades, group consecutive wins/losses, add route + handler + service method | pending | medium | — |
| T9 | Backend: `r_distribution()` endpoint — CASE bucket query on r_multiple, add route + handler + service method | pending | medium | — |
| T10 | Backend: `exposure_timeline()` endpoint — count concurrent positions per date, add route + handler + service method | pending | high | — |
| T11 | C2 Calendar Heatmap — `CalendarHeatmap.tsx` with ECharts calendar layout | pending | medium | T1, T7 |
| T12 | C4 Win/Loss Streaks — `StreakWaterfall.tsx` with waterfall bars | pending | medium | T8 |
| T13 | C5 R-Multiple Distribution — `RDistribution.tsx` with histogram + P&L overlay line | pending | medium | T1, T9 |
| T14 | C6 Exposure Timeline — `ExposureTimeline.tsx` with stacked area | pending | medium | T1, T10 |
| T15 | Wire Phase 2 charts into `ChartSelector.tsx`, frontend API functions, full build validation + commit | pending | low | T11–T14 |

### Key Decisions

- **Phase 1 first**: C1, C3, C7, C8 need zero backend work — ship immediately from existing data.
- **EChart theme**: All new charts use existing `TESTUDO_THEME` via `EChart` wrapper component pattern.
- **ChartContainer**: All charts use `ChartContainer` wrapper (already exists) for consistent title + loading + error states.
- **Chart backgrounds**: Use `getChartBg()` token (bg-elevated) not transparent — avoids Hadrian's Wall bleedthrough.
- **Filters**: All charts respect existing `StatsFilter` (exchange, symbol, date range) via `useFilters()` context.
- **Backend pattern**: New endpoints follow existing `TimeSeriesService` + `journal.rs` handler pattern with `DataWrapper<Vec<T>>` response.

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| JNL-18-storage-quotas | 2026-03-22 |
| JNL-17-nested-collections | 2026-03-22 |
| JNL-16-database-view | 2026-03-22 |
| JNL-15-export-with-images | 2026-03-22 |
| JNL-14-markdown-hardening | 2026-03-22 |
| UXP-21-light-theme-parity | 2026-03-22 |
| UXP-23-landing-typography | 2026-03-22 |
| UXP-20-strip-glassmorphism | 2026-03-22 |
| UXP-22-signal-color-calibration | 2026-03-22 |
| UXP-19-features-layout | 2026-03-22 |
| EXT-38-background-decomposition | 2026-03-22 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |
| UXP-18-multi-theme | 2026-03-21 |
| HL-11-status-transition-fix | 2026-03-21 |

---

*This file is persistent state. Vox updates it each iteration.*
