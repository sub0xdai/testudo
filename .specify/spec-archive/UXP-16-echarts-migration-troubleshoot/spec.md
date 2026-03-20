# Specification: Migrate Chart.js to Apache ECharts for Analytical Graphics

**Spec ID:** UXP-16-echarts-migration
**Date:** 2026-03-18
**Status:** Draft
**Class:** Refactor / Feature
**Priority:** P1 — Current Chart.js charts are basic dashboard-tier; ECharts enables industry-quality analytical visualizations with built-in dark/light theming
**Depends on:** UXP-15-interactive-state-polish (ChartContainer retry/empty state props must be in place)
**Series:** UXP-16 (standalone — charting infrastructure upgrade)

---

## Problem Statement

The testudo-journal Analysis page uses Chart.js v4 for four analytical charts (SymbolDonut, MarketReturn, DurationScatter, ReturnHistogram) and a custom DOM grid for the TimeHeatmap. Chart.js is a general-purpose dashboard library — it lacks the analytical depth needed for a professional trading journal. Its scatter plots have no regression lines, its histograms have no distribution overlays, its heatmaps don't exist (hence the custom DOM fallback), and its tooltip system is rigid. The charts look functional but not professional.

Apache ECharts provides every analytical chart type out of the box — proper heatmaps with continuous color scales, scatter plots with built-in regression/trend lines, rich tooltip formatters with HTML, treemaps, box plots, radar charts, and calendar heatmaps. It ships with a `'dark'` built-in theme and supports `registerTheme()` for custom branded themes, making the future light-mode addition trivial. ECharts renders on canvas with GPU acceleration and handles thousands of data points smoothly.

The four lightweight-charts components (EquityCurve, DailyPnl, CumulativeProfit, HeroEquityCurve) remain unchanged — lightweight-charts is purpose-built for financial time-series and excels at that specific job. This migration only replaces Chart.js and the custom DOM heatmap with ECharts, then removes the Chart.js dependency entirely.

---

## User Stories

- **As a trader**, I want professional-quality analytical charts with rich tooltips and smooth interactions, so that my trading journal feels like an institutional-grade analytics platform.
- **As a trader**, I want a consistent dark theme across all charts that matches the app's brutalist aesthetic, so that the visualization layer doesn't feel like a bolted-on widget.
- **As a developer**, I want a single charting library (ECharts) for all analytical visualizations with a reusable Solid.js wrapper, so that adding new chart types requires only an options object — no boilerplate.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Create a reusable `EChart` Solid.js wrapper component that handles `echarts.init()`, `setOption()`, `ResizeObserver`, and `dispose()` lifecycle. Accepts `option` accessor and `theme` prop. | High | Infrastructure |
| FR-2 | Create a `testudo-dark` ECharts theme using `echarts.registerTheme()` that matches the app's design tokens: `#050505` background, `#3F3F46` axis/grid lines, `#555555` tick labels, `#888888` secondary text, `Space Mono` font for data, signal-green/red palette. | High | Theming |
| FR-3 | Migrate `SymbolDonut.tsx` from Chart.js doughnut to ECharts pie with `radius: ['50%', '75%']` (ring), right-aligned legend, and `TAG_PALETTE` color scheme. | High | Charts |
| FR-4 | Migrate `MarketReturn.tsx` from Chart.js horizontal bar to ECharts bar with `yAxis.type: 'category'`, horizontal orientation, green/red per-bar coloring based on P&L sign. | High | Charts |
| FR-5 | Migrate `DurationScatter.tsx` from Chart.js scatter to ECharts scatter with custom tooltip formatter showing symbol, duration (hours), and P&L. Green/red point coloring by P&L sign. Point size 8px, hover emphasis. | High | Charts |
| FR-6 | Migrate `ReturnHistogram.tsx` from Chart.js bar to ECharts bar with green/red bucket coloring based on return sign. Zero-gap bars for histogram feel. | High | Charts |
| FR-7 | Migrate `TimeHeatmap.tsx` from custom DOM grid to ECharts heatmap with `xAxis` (hours 0-23), `yAxis` (days SUN-SAT), continuous green color scale using `visualMap`. Tooltip shows day, hour, trade count. | High | Charts |
| FR-8 | Remove `chart.js` dependency from `package.json`. No Chart.js imports should remain in the codebase. | High | Cleanup |
| FR-9 | All migrated charts preserve existing `ChartContainer` integration: `loading`, `empty`, `error`, `onRetry`, `hasActiveFilters`, `onClearFilters` props pass through unchanged. | High | Integration |
| FR-10 | ECharts tooltip styling matches the app: `#111111` background, `#3F3F46` border, `Space Mono` font, white title, `#888888` body text. Configured once in theme, not per-chart. | Medium | Theming |

---

## Technical Implementation

### FR-1: EChart Wrapper Component

A reusable Solid.js component that encapsulates the ECharts lifecycle. Every analytical chart will use this instead of managing `init`/`dispose` manually.

```tsx
// testudo-journal/src/components/charts/EChart.tsx
import { onMount, onCleanup, createEffect, type Accessor } from 'solid-js'
import * as echarts from 'echarts/core'
import type { EChartsOption } from 'echarts'

interface EChartProps {
  option: Accessor<EChartsOption | undefined>
  class?: string
  height?: string  // default '224px' (h-56)
}

export function EChart(props: EChartProps) {
  let container!: HTMLDivElement
  let chart: echarts.ECharts | undefined

  onMount(() => {
    chart = echarts.init(container, 'testudo-dark')

    const observer = new ResizeObserver(() => chart?.resize())
    observer.observe(container)

    onCleanup(() => {
      observer.disconnect()
      chart?.dispose()
    })
  })

  // Reactive option updates
  createEffect(() => {
    const opt = props.option()
    if (opt && chart) {
      chart.setOption(opt, { notMerge: true })
    }
  })

  return (
    <div
      ref={container!}
      class={props.class}
      style={{ height: props.height ?? '224px', width: '100%' }}
    />
  )
}
```

### FR-2: Testudo Dark Theme

Register once at app startup. Centralizes all visual config so individual charts need minimal styling.

```tsx
// testudo-journal/src/lib/echarts-theme.ts
import * as echarts from 'echarts/core'
import { SIGNAL_GREEN, SIGNAL_RED, SIGNAL_AMBER, CHART_BG, TAG_PALETTE } from './tokens'

export const TESTUDO_THEME = 'testudo-dark'

echarts.registerTheme(TESTUDO_THEME, {
  color: TAG_PALETTE,
  backgroundColor: 'transparent',  // ChartContainer handles bg
  textStyle: {
    fontFamily: "'Space Mono', monospace",
    color: '#555555',
    fontSize: 11,
  },
  title: {
    textStyle: { color: '#FFFFFF', fontFamily: "'Space Grotesk', sans-serif" },
  },
  legend: {
    textStyle: { color: '#888888', fontFamily: "'Space Mono', monospace", fontSize: 11 },
  },
  tooltip: {
    backgroundColor: CHART_BG,
    borderColor: '#3F3F46',
    borderWidth: 1,
    textStyle: {
      fontFamily: "'Space Mono', monospace",
      color: '#888888',
      fontSize: 11,
    },
    extraCssText: 'box-shadow: 0 4px 12px rgba(0,0,0,0.5);',
  },
  categoryAxis: {
    axisLine: { lineStyle: { color: '#3F3F46' } },
    axisTick: { lineStyle: { color: '#3F3F46' } },
    axisLabel: { color: '#555555' },
    splitLine: { lineStyle: { color: '#1A1A1A' } },
  },
  valueAxis: {
    axisLine: { lineStyle: { color: '#3F3F46' } },
    axisTick: { lineStyle: { color: '#3F3F46' } },
    axisLabel: { color: '#555555' },
    splitLine: { lineStyle: { color: '#1A1A1A' } },
  },
})
```

### FR-3–FR-7: Chart Migration Mapping

Each chart replaces its Chart.js/DOM implementation with a `createMemo` that computes an ECharts `option` object, passed to the `<EChart>` wrapper.

| Current Component | ECharts series.type | Key Config |
|---|---|---|
| SymbolDonut (doughnut) | `pie` | `radius: ['50%', '75%']`, `label.show: false`, `legend: { orient: 'vertical', right: 10 }` |
| MarketReturn (horiz bar) | `bar` | `yAxis.type: 'category'`, `xAxis.type: 'value'`, per-item `itemStyle.color` green/red |
| DurationScatter (scatter) | `scatter` | `symbolSize: 8`, custom `tooltip.formatter`, green/red by P&L sign |
| ReturnHistogram (bar) | `bar` | `barGap: '0%'`, per-item green/red, `xAxis.type: 'category'` |
| TimeHeatmap (DOM grid) | `heatmap` | `xAxis` (0-23 hours), `yAxis` (SUN-SAT), `visualMap: { min: 0, max, inRange: { color: ['#1A1A1A', SIGNAL_GREEN] } }` |

### ECharts Tree-Shakeable Imports

To minimize bundle size, import only the components used:

```tsx
// testudo-journal/src/lib/echarts-setup.ts
import * as echarts from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { PieChart, BarChart, ScatterChart, HeatmapChart } from 'echarts/charts'
import {
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
  VisualMapComponent,
} from 'echarts/components'

echarts.use([
  CanvasRenderer,
  PieChart,
  BarChart,
  ScatterChart,
  HeatmapChart,
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
  VisualMapComponent,
])

export { echarts }
```

The `EChart.tsx` wrapper and theme file import from this setup module instead of `echarts` directly, ensuring tree-shaking works.

### Files

**New files:**
- `testudo-journal/src/lib/echarts-setup.ts` — tree-shakeable ECharts registration
- `testudo-journal/src/lib/echarts-theme.ts` — `testudo-dark` theme registration
- `testudo-journal/src/components/charts/EChart.tsx` — reusable Solid.js wrapper

**Modified files:**
- `testudo-journal/src/components/charts/SymbolDonut.tsx` — rewrite: Chart.js doughnut → ECharts pie
- `testudo-journal/src/components/charts/MarketReturn.tsx` — rewrite: Chart.js horizontal bar → ECharts bar
- `testudo-journal/src/components/charts/DurationScatter.tsx` — rewrite: Chart.js scatter → ECharts scatter
- `testudo-journal/src/components/charts/ReturnHistogram.tsx` — rewrite: Chart.js bar → ECharts bar
- `testudo-journal/src/components/charts/TimeHeatmap.tsx` — rewrite: custom DOM → ECharts heatmap
- `testudo-journal/src/index.tsx` — import `echarts-setup.ts` + `echarts-theme.ts` at app entry
- `testudo-journal/package.json` — add `echarts`, remove `chart.js`

**Unchanged files:**
- `testudo-journal/src/components/charts/EquityCurve.tsx` — stays lightweight-charts
- `testudo-journal/src/components/charts/DailyPnl.tsx` — stays lightweight-charts
- `testudo-journal/src/components/charts/CumulativeProfit.tsx` — stays lightweight-charts
- `testudo-journal/src/components/HeroEquityCurve.tsx` — stays lightweight-charts
- `testudo-journal/src/components/charts/ChartContainer.tsx` — no changes needed
- `testudo-journal/src/components/ChartSelector.tsx` — no changes needed (same component names)

### Dependencies Added

- `echarts = "^5.5"` — Apache ECharts (tree-shakeable, ~250KB gzipped with selected components)

### Dependencies Removed

- `chart.js = "^4.4.0"` — replaced entirely by ECharts

---

## Acceptance Criteria

- [ ] `grep -r "from 'chart.js'" testudo-journal/src/` returns zero matches
- [ ] `grep -r "chart.js" testudo-journal/package.json` returns zero matches
- [ ] `EChart.tsx` wrapper handles init, reactive option updates, resize, and dispose
- [ ] `testudo-dark` theme is registered and all charts use it (no per-chart color config)
- [ ] SymbolDonut renders as ECharts pie ring with right-aligned legend and TAG_PALETTE colors
- [ ] MarketReturn renders as horizontal ECharts bar with green/red per-bar P&L coloring
- [ ] DurationScatter renders as ECharts scatter with custom tooltip showing symbol/duration/P&L
- [ ] ReturnHistogram renders as ECharts bar with green/red bucket coloring
- [ ] TimeHeatmap renders as ECharts heatmap with continuous green color scale and visualMap
- [ ] All charts retain ChartContainer integration (loading, empty, error, retry, filter props)
- [ ] Tooltips use consistent styling: `#111111` bg, `#3F3F46` border, Space Mono font
- [ ] `bun run build` passes with zero errors
- [ ] Bundle size does not increase by more than 100KB gzipped vs current (Chart.js removal offsets ECharts addition)

---

## Risks

1. **Bundle size regression** — ECharts full library is ~1MB. Mitigation: use tree-shakeable imports (`echarts/core` + individual chart/component modules) to import only PieChart, BarChart, ScatterChart, HeatmapChart and required components. Chart.js removal (~60KB gzipped) partially offsets the addition.

2. **ECharts resize flicker** — ECharts may flicker during container resize if `resize()` is called too frequently. Mitigation: the ResizeObserver in `EChart.tsx` handles this natively; if needed, debounce with `requestAnimationFrame`.

3. **TimeHeatmap data format change** — Current custom DOM heatmap builds a `Map<string, number>` grid. ECharts heatmap expects `[x, y, value]` triples. Mitigation: straightforward data transform in `createMemo` — the API response (`TimeSlot[]`) already has `day_of_week`, `hour`, `trade_count` which map directly.

4. **SSR incompatibility** — ECharts requires DOM access (`echarts.init(container)`). Mitigation: testudo-journal is client-only SPA; no SSR concerns.

---

## Completion Signal

This spec is complete when:
1. EChart wrapper, theme, and tree-shake setup are created and working
2. All 5 Chart.js/DOM charts migrated to ECharts with matching data transforms
3. Chart.js dependency fully removed
4. All acceptance criteria pass
5. `bun run build` passes with zero errors
6. Code committed to master
