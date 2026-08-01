# Specification: Replace Hero Equity Curve with P&L Calendar + Radar Chart

**Spec ID:** UX-06-overview-calendar-radar
**Date:** 2026-04-09
**Status:** Draft
**Class:** Feature / UX
**Priority:** P1 — overview is the primary shareability surface; calendar is industry standard hero
**Depends on:** None (equity curve remains available in chart selector)
**Series:** UX-06 (standalone)

---

## Problem Statement

The overview hero section currently shows a full-width equity curve (lightweight-charts baseline series). Competitor research across 8 trading journal platforms (TradeZella, Tradervue, Edgewonk, Kinfo, Myfxbook, etc.) reveals that no major platform uses an equity curve as the hero element. The industry standard is:

1. **KPI cards** (top) — already implemented in Testudo's sidebar
2. **P&L Calendar** (hero) — color-coded monthly grid showing daily P&L, trade count per cell
3. **Radar/spider chart** (sidebar/supporting) — composite performance profile

The equity curve is already accessible via the chart selector dropdown (as "Cumulative Profit"). Having it as both the hero AND a selector option is redundant. Replacing it with a P&L calendar and adding a radar chart creates a more information-dense, scannable overview that matches trader expectations.

Both chart types are native Apache ECharts components — no new dependencies needed.

---

## User Stories

- **As a trader**, I want to see my monthly P&L at a glance as a calendar grid, so that I can spot winning/losing streaks and patterns by day of week.
- **As a trader**, I want a radar chart showing my performance profile, so that I can see strengths and weaknesses across multiple metrics simultaneously.
- **As a potential user viewing a screenshot**, I want the overview to look data-rich and distinctive, so that I'm motivated to try the product.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | P&L Calendar replaces HeroEquityCurve in Overview — monthly grid with day cells | High | Overview |
| FR-2 | Calendar cells show P&L amount + trade count, color-coded green (profit) / red (loss) | High | PnlCalendar |
| FR-3 | Month navigation: ← / → arrows + "This month" button | High | PnlCalendar |
| FR-4 | Weekly P&L summary column to the right of the calendar grid | Medium | PnlCalendar |
| FR-5 | Radar chart in stats sidebar showing 6-axis performance profile | High | PerformanceRadar |
| FR-6 | Radar axes: Win Rate, Profit Factor, Consistency, Max Drawdown (inverted), Avg R-Multiple, Recovery Factor | High | PerformanceRadar |
| FR-7 | Calendar click handler: clicking a day filters the journal/trades to that date | Medium | PnlCalendar |
| FR-8 | Both charts respect theme changes (dark/light/amoled) | High | Both |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | PnlCalendar component: ECharts calendar heatmap with month navigation, replaces HeroEquityCurve in Overview | Calendar renders with daily P&L data, month nav works |
| CP-2 | PerformanceRadar component: ECharts radar chart in sidebar using existing overview stats | Radar renders with 6 axes from OverviewResponse data |
| CP-3 | Polish: weekly summaries, click-to-filter, empty states | Full feature parity with spec |

### CP-1: P&L Calendar Component

**New file: `components/charts/PnlCalendar.tsx`**

Uses the existing `EChart` wrapper component and `fetchDailyPnl` endpoint.

```tsx
import { createResource, createSignal, createMemo } from 'solid-js'
import { EChart } from './EChart'
import { useFilters } from '../filterContext'
import { fetchDailyPnl } from '../../api/client'
import type { EChartsOption } from 'echarts'

interface PnlCalendarProps {
  onDayClick?: (date: string) => void
}

export function PnlCalendar(props: PnlCalendarProps) {
  // Month navigation state
  const [viewMonth, setViewMonth] = createSignal(new Date())

  // Fetch daily P&L data
  const { filters } = useFilters()
  const [data] = createResource(filters, fetchDailyPnl)

  const option = createMemo((): EChartsOption | undefined => {
    const d = data()
    if (!d?.data?.length) return undefined

    const month = viewMonth()
    const year = month.getFullYear()
    const monthNum = month.getMonth() + 1
    const rangeStr = `${year}-${String(monthNum).padStart(2, '0')}`

    // Map data to [date, pnl] pairs
    const calendarData = d.data
      .filter(p => p.date.startsWith(rangeStr))
      .map(p => [p.date, parseFloat(p.pnl)])

    // Also prepare label data with trade counts
    const tradeCountMap = new Map(d.data.map(p => [p.date, p.trade_count]))

    return {
      calendar: {
        range: rangeStr,
        cellSize: ['auto', 60],
        orient: 'horizontal',
        splitLine: { show: true, lineStyle: { color: 'rgba(var(--border), 0.3)', width: 1 } },
        itemStyle: { borderWidth: 1, borderColor: 'transparent' },
        dayLabel: {
          show: true, firstDay: 0,
          nameMap: ['S', 'M', 'T', 'W', 'T', 'F', 'S'],
          color: getTextTertiary()
        },
        monthLabel: { show: false },
        yearLabel: { show: false },
      },
      visualMap: {
        show: false,
        min: -maxAbsPnl,
        max: maxAbsPnl,
        inRange: {
          color: [getSignalRed(), 'transparent', getSignalGreen()]
        },
        // Use opacity to create subtle tints, not solid fills
      },
      series: [{
        type: 'heatmap',
        coordinateSystem: 'calendar',
        data: calendarData,
        label: {
          show: true,
          formatter: (params) => {
            const [date, pnl] = params.value as [string, number]
            const count = tradeCountMap.get(date) || 0
            if (count === 0) return ''
            const sign = pnl >= 0 ? '+' : ''
            return `{pnl|${sign}$${Math.abs(pnl).toFixed(0)}}\n{count|${count} trade${count > 1 ? 's' : ''}}`
          },
          rich: {
            pnl: { fontSize: 11, fontFamily: 'Space Mono', fontWeight: 'bold', lineHeight: 16 },
            count: { fontSize: 9, fontFamily: 'Space Mono', color: getTextTertiary(), lineHeight: 14 },
          }
        }
      }]
    }
  })

  // Month navigation helpers
  function prevMonth() { ... }
  function nextMonth() { ... }
  function thisMonth() { setViewMonth(new Date()) }

  return (
    <div>
      {/* Month nav header */}
      <div class="flex items-center gap-3 px-8 py-3">
        <button onClick={prevMonth} aria-label="Previous month">←</button>
        <span class="font-mono text-sm text-text-primary">
          {viewMonth().toLocaleDateString('en-US', { month: 'long', year: 'numeric' })}
        </span>
        <button onClick={nextMonth} aria-label="Next month">→</button>
        <button onClick={thisMonth} class="font-mono text-xs ...">This month</button>
      </div>

      {/* Calendar chart */}
      <EChart option={option} height="320px" />
    </div>
  )
}
```

**Key ECharts config decisions:**
- `cellSize: ['auto', 60]` — auto-width to fill container, 60px height for readable text
- `visualMap` with piecewise or continuous green-to-red mapping
- `label.rich` for multi-line cell content (P&L amount + trade count)
- `monthLabel: { show: false }` — we handle month navigation ourselves
- Click handler via `chart.on('click', ...)` to emit selected date

**Overview.tsx changes:**
- Remove `HeroEquityCurve` import and usage
- Remove `equity` resource (no longer needed at overview level)
- Import and render `PnlCalendar` in its place
- Keep the equity data fetch only if chart selectors still need it — check if ChartSelector handles its own data

### CP-2: Performance Radar Component

**New file: `components/charts/PerformanceRadar.tsx`**

Uses data already available from `fetchOverview` (passed as props from Overview).

```tsx
import { createMemo } from 'solid-js'
import { EChart } from './EChart'
import type { PerformanceStats, RiskStats } from '../../api/client'
import type { EChartsOption } from 'echarts'

interface PerformanceRadarProps {
  performance: PerformanceStats
  risk: RiskStats
}

export function PerformanceRadar(props: PerformanceRadarProps) {
  const option = createMemo((): EChartsOption => {
    const p = props.performance
    const r = props.risk

    // Normalize all values to 0-100 scale
    const winRate = parseFloat(p.win_rate)                           // already 0-100
    const profitFactor = Math.min(parseFloat(p.profit_factor) * 20, 100)  // 5.0 = 100
    const avgR = Math.min(parseFloat(p.avg_r_multiple) * 25, 100)        // 4.0R = 100
    const maxDD = 100 - Math.min(parseFloat(r.max_drawdown_pct), 100)    // INVERTED: low DD = high score
    const consistency = Math.min(winRate * parseFloat(p.profit_factor) / 2, 100)  // composite
    const recovery = /* best_streak / worst_streak or similar */ 50       // placeholder

    return {
      radar: {
        shape: 'circle',
        splitNumber: 4,
        indicator: [
          { name: 'Win Rate', max: 100 },
          { name: 'Profit Factor', max: 100 },
          { name: 'Consistency', max: 100 },
          { name: 'Max Drawdown', max: 100 },
          { name: 'Avg R', max: 100 },
          { name: 'Recovery', max: 100 },
        ],
        name: { textStyle: { color: getTextTertiary(), fontSize: 10, fontFamily: 'Space Mono' } },
        axisLine: { lineStyle: { color: 'rgba(255,255,255,0.1)' } },
        splitLine: { lineStyle: { color: 'rgba(255,255,255,0.08)' } },
        splitArea: { show: false },
      },
      series: [{
        type: 'radar',
        data: [{
          value: [winRate, profitFactor, consistency, maxDD, avgR, recovery],
          name: 'Performance'
        }],
        lineStyle: { color: getAccentPrimary(), width: 2 },
        areaStyle: { color: accentPrimaryAlpha(0.15) },
        itemStyle: { color: getAccentPrimary() },
        symbol: 'circle',
        symbolSize: 4,
      }]
    }
  })

  return <EChart option={option} height="200px" />
}
```

**Overview.tsx changes:**
- Import `PerformanceRadar`
- Render it in the stats sidebar, below the Risk section
- Pass `performance` and `risk` from the overview stats resource

**Normalization logic for radar axes:**
- **Win Rate**: Already 0-100, use directly
- **Profit Factor**: Scale so 5.0 = 100 (multiply by 20, cap at 100)
- **Avg R-Multiple**: Scale so 4.0R = 100 (multiply by 25, cap at 100)
- **Max Drawdown**: INVERT — `100 - drawdown_pct` so low drawdown = high score (outer ring = good)
- **Consistency**: Composite of win rate and profit factor (a simple proxy: `winRate * PF / 2`, capped at 100)
- **Recovery**: `best_streak / (abs(worst_streak) + 1)` normalized, or placeholder until a proper metric is added

### CP-3: Polish

**Weekly summaries (FR-4):**
- Compute weekly P&L sums from the daily data
- Render as a column of small cards to the right of the calendar (same pattern as TradeZella screenshot)
- `Week 1: $X,XXX / N days` etc.

**Click-to-filter (FR-7):**
- Wire `chart.on('click', ...)` in PnlCalendar
- On click, set `dateFrom` and `dateTo` filters to the clicked date
- This filters the trades table and other charts to that single day

**Empty states:**
- Calendar with no data for the month: "No trades this month"
- Radar with insufficient data: "Need more trades for performance profile"

### Paved Roads

- `EChart` wrapper: handles init, resize, theme changes — all new charts just pass `EChartsOption`
- `fetchDailyPnl`: returns exactly what the calendar needs (`date`, `pnl`, `trade_count`)
- `fetchOverview`: returns all data the radar needs (`PerformanceStats`, `RiskStats`)
- Token helpers in `lib/tokens.ts`: `getSignalGreen()`, `getSignalRed()`, `getAccentPrimary()`, `getTextTertiary()`
- `ChartContainer` wrapper: loading/empty/error states — can wrap both new charts
- `useFilters()` context: month navigation can integrate with existing filter system

### Files

- `components/charts/PnlCalendar.tsx` — **new** — calendar heatmap with month nav
- `components/charts/PerformanceRadar.tsx` — **new** — 6-axis radar chart
- `components/Overview.tsx` — **modified** — replace HeroEquityCurve with PnlCalendar, add PerformanceRadar to sidebar
- `components/charts/EChart.tsx` — **may need** click event callback prop

### Dependencies Added

None — ECharts calendar and radar are built-in.

---

## Acceptance Criteria

- [ ] P&L Calendar renders in Overview hero position with current month's daily data
- [ ] Calendar cells show P&L amount + trade count, green tint for profit, red for loss
- [ ] Month navigation (← → / This month) works correctly
- [ ] Weekly P&L summary column renders beside calendar
- [ ] Radar chart renders in sidebar with 6 normalized axes
- [ ] Radar uses accent-primary (rust/copper) for line and fill
- [ ] Max Drawdown axis is inverted (low DD = outer ring)
- [ ] Both charts respect theme changes
- [ ] Clicking a calendar day filters to that date
- [ ] Empty states show guidance text
- [ ] HeroEquityCurve removed from Overview (equity curve still in chart selector)
- [ ] `cd testudo-journal && bun run build` passes
- [ ] No functional regression — data loading, filters, sidebar stats all unchanged

---

## Risks

1. **Calendar cell size on narrow screens** — cells need 60px+ for readable text. On screens < 600px wide, cells will be too small. Mitigation: hide calendar on mobile, show a condensed list view instead (or just the weekly summaries).
2. **Radar normalization is subjective** — "Consistency" is a composite metric, not a raw stat. Mitigation: start with the simple `winRate * PF / 2` proxy, iterate based on trader feedback.
3. **ECharts calendar label rendering** — bug in ECharts < 5.4 where heatmap labels don't render. Mitigation: verify ECharts version is 5.4+.
4. **Monthly data fetch** — `fetchDailyPnl` currently respects the global filter period. The calendar needs full-month data regardless of date filters. May need a separate fetch with explicit date range, or the calendar manages its own fetch independent of the filter context.

---

## Completion Signal

This spec is complete when:
1. Overview hero shows P&L Calendar with monthly navigation
2. Radar chart renders in sidebar with performance profile
3. Both charts theme-aware and responsive
4. `bun run build` passes
5. Code committed to master
