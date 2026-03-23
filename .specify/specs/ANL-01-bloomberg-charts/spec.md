# Specification: Bloomberg-Grade Analytics Charts

**Spec ID:** ANL-01-bloomberg-charts
**Date:** 2026-03-23
**Status:** Draft
**Class:** Feature / Analytics
**Priority:** P1 — Maximizes value of existing trade data; all charts render client-side from existing or new API endpoints
**Depends on:** None (builds on existing ChartSelector infrastructure)
**Series:** ANL-01 (standalone)

---

## Problem Statement

The Overview page currently offers 7 chart types via `ChartSelector`, but these cover only basic analytics. Bloomberg terminals, TradeStation, and professional journaling tools (Tradervue, TradesVault) expose significantly deeper data views that help traders identify behavioral patterns, risk exposure, and edge decay.

The existing trade data (`journal_trades` table) already captures 20+ fields per trade — entry/exit prices, stop/target, R-multiple, fees, leverage, duration, symbol, side, timestamps — but most of this data is only surfaced in the table view. Only a fraction drives the current charts.

This spec adds 8 new chart types to the `ChartSelector` dropdown, each computed from existing trade data. Charts that can be computed entirely client-side from existing endpoints are marked **frontend-only**. Charts requiring new backend aggregation endpoints are marked **backend+frontend**.

---

## User Stories

- **As a trader**, I want to see my drawdown periods visualized with recovery times, so that I can assess my risk tolerance and identify behavioral patterns during losing streaks.
- **As a trader**, I want a calendar heatmap of my P&L by day, so that I can spot day-of-week patterns and avoid trading on historically bad days.
- **As a trader**, I want to see P&L weighted by symbol as a treemap, so that I can instantly see which instruments dominate my book.
- **As a trader**, I want to see my win/loss streaks visualized, so that I can identify tilt patterns and streak clustering.
- **As a trader**, I want an R-multiple distribution histogram, so that I can see if my edge is coming from a few outlier wins or consistent small gains.
- **As a trader**, I want to see how many positions I held concurrently over time, so that I can manage correlation risk and overexposure.
- **As a trader**, I want per-symbol expectancy compared side by side, so that I can focus on my highest-edge instruments.
- **As a trader**, I want to see whether my quick scalps or longer swing trades are more profitable, so that I can optimize my holding period.

---

## Chart Specifications

### C1: Drawdown Chart (frontend-only)

**Type:** ECharts AreaSeries
**Data source:** Existing `fetchEquityCurve()` → `EquityPoint[]` (already has `drawdown`, `drawdown_pct`, `peak`)
**Visualization:**
- X-axis: date
- Y-axis: drawdown percentage (inverted, 0% at top, max DD at bottom)
- Area fill: signal-red gradient (transparent at 0%, opaque at max)
- Horizontal dashed line at max drawdown level
- Tooltip: date, drawdown %, drawdown $, days since peak

**Why it matters:** The equity curve shows drawdown as a secondary overlay, but a dedicated view makes drawdown *depth* and *duration* the primary focus — critical for risk management.

**ECharts components:** `AreaSeries` (already registered), `MarkLine` for max DD marker.

---

### C2: Calendar Heatmap (backend+frontend)

**Type:** ECharts custom calendar layout
**Data source:** New endpoint `GET /api/v1/journal/analytics/calendar-pnl`
**Response type:**
```typescript
interface CalendarPnlDay {
  date: string       // "2026-03-23"
  net_pnl: string    // "-41.06"
  trade_count: number // 3
}
```
**Backend query:**
```sql
SELECT
  DATE(closed_at) as date,
  SUM(net_pnl) as net_pnl,
  COUNT(*) as trade_count
FROM journal_trades
WHERE user_id = $1 AND closed_at >= $2 AND closed_at <= $3
GROUP BY DATE(closed_at)
ORDER BY date
```
**Visualization:**
- GitHub contribution graph style — 7 rows (Mon–Sun) × N weeks
- Color scale: deep red (worst day) → neutral (zero) → deep green (best day)
- Empty cells for no-trade days (subtle border, no fill)
- Tooltip: date, P&L, trade count
- Date range: auto from first trade to last trade (or filter range)

**Why it matters:** Instantly reveals day-of-week patterns. Many traders have statistically worse performance on Mondays or Fridays. Visual pattern recognition is faster than scanning a table.

**ECharts components:** `HeatmapChart` (already registered), `CalendarComponent` (needs registration), `VisualMapComponent` (already registered).

---

### C3: P&L Treemap (frontend-only)

**Type:** ECharts TreemapChart
**Data source:** Existing `fetchSymbolBreakdown()` → `SymbolBreakdownItem[]` (has `symbol`, `trade_count`, `total_pnl`, `win_rate`)
**Visualization:**
- Rectangle size: proportional to `abs(total_pnl)` (bigger = more impact on account)
- Color: green for profitable symbols, red for losing symbols (intensity by magnitude)
- Label: symbol name + P&L value
- Tooltip: symbol, P&L, trade count, win rate
- Drillable: click to filter Overview by that symbol

**Why it matters:** Donut charts show trade *count* distribution well, but treemaps show *impact* — a symbol with 3 trades and -$500 P&L matters more than one with 15 trades and +$50.

**ECharts components:** `TreemapChart` (needs registration).

---

### C4: Win/Loss Streak Waterfall (backend+frontend)

**Type:** ECharts BarChart (waterfall variant)
**Data source:** New endpoint `GET /api/v1/journal/analytics/streaks`
**Response type:**
```typescript
interface StreakSegment {
  start_date: string   // first trade date in streak
  end_date: string     // last trade date in streak
  streak_length: number // positive = wins, negative = losses
  total_pnl: string    // cumulative P&L during streak
}
```
**Backend logic:** Iterate trades ordered by `closed_at`, group consecutive wins/losses into segments.
**Visualization:**
- X-axis: streak index (sequential)
- Y-axis: streak length (positive bars up = win streaks, negative bars down = loss streaks)
- Color: signal-green for wins, signal-red for losses
- Bar width proportional to duration (or fixed)
- Tooltip: streak length, date range, cumulative P&L during streak

**Why it matters:** Streak clustering reveals tilt behavior. A trader who loses 8 in a row then wins 2 then loses 6 has a different problem than one with evenly distributed wins/losses.

**ECharts components:** `BarChart` (already registered).

---

### C5: R-Multiple Distribution (backend+frontend)

**Type:** ECharts BarChart (histogram)
**Data source:** New endpoint `GET /api/v1/journal/analytics/r-distribution`
**Response type:**
```typescript
interface RBucket {
  bucket: string  // "-3R", "-2R", "-1R", "0R", "+1R", "+2R", "+3R", "+4R+"
  count: number
  total_pnl: string
}
```
**Backend query:**
```sql
SELECT
  CASE
    WHEN r_multiple IS NULL THEN 'N/A'
    WHEN CAST(r_multiple AS DECIMAL) <= -3 THEN '-3R'
    WHEN CAST(r_multiple AS DECIMAL) <= -2 THEN '-2R'
    WHEN CAST(r_multiple AS DECIMAL) <= -1 THEN '-1R'
    WHEN CAST(r_multiple AS DECIMAL) <= 0 THEN '0R'
    WHEN CAST(r_multiple AS DECIMAL) <= 1 THEN '+1R'
    WHEN CAST(r_multiple AS DECIMAL) <= 2 THEN '+2R'
    WHEN CAST(r_multiple AS DECIMAL) <= 3 THEN '+3R'
    ELSE '+4R+'
  END as bucket,
  COUNT(*) as count,
  SUM(net_pnl) as total_pnl
FROM journal_trades
WHERE user_id = $1 AND r_multiple IS NOT NULL
GROUP BY bucket
ORDER BY bucket
```
**Visualization:**
- X-axis: R-multiple buckets
- Y-axis: trade count
- Color: red for negative R buckets, green for positive
- Overlay line: cumulative P&L contribution per bucket
- Tooltip: bucket, count, total P&L from that bucket

**Why it matters:** Shows whether edge comes from cutting losers at -1R (discipline) or catching +3R+ runners (selection). Most retail traders have too many -1R and not enough +2R+.

**ECharts components:** `BarChart` (already registered), `LineChart` (needs registration for overlay).

---

### C6: Concurrent Exposure Timeline (backend+frontend)

**Type:** ECharts AreaSeries (stacked)
**Data source:** New endpoint `GET /api/v1/journal/analytics/exposure-timeline`
**Response type:**
```typescript
interface ExposurePoint {
  date: string
  open_positions: number    // count of positions open on this date
  total_exposure: string    // sum of (quantity * entry_price) for open positions
  symbols: string[]         // which symbols were open
}
```
**Backend logic:** For each date in range, count trades where `opened_at <= date AND closed_at >= date`.
**Visualization:**
- X-axis: date
- Y-axis (left): position count
- Y-axis (right): total dollar exposure
- Stacked area by symbol (if feasible) or single area
- Tooltip: date, position count, exposure $, symbol list

**Why it matters:** Overexposure correlates with drawdowns. Seeing 5 positions open simultaneously on a day you lost big reveals a risk management gap.

**ECharts components:** `LineChart` (needs registration for line+area hybrid).

---

### C7: Expectancy by Symbol (frontend-only)

**Type:** ECharts BarChart (grouped)
**Data source:** Existing `fetchSymbolBreakdown()` → `SymbolBreakdownItem[]` (has `symbol`, `trade_count`, `total_pnl`, `win_rate`)
**Derived calculation (client-side):** `expectancy = total_pnl / trade_count`
**Visualization:**
- X-axis: symbol
- Y-axis: expectancy per trade ($)
- Bars colored green (positive expectancy) or red (negative)
- Secondary bars or labels for trade count (to show statistical significance)
- Tooltip: symbol, expectancy, total P&L, trade count, win rate

**Why it matters:** Win rate alone is misleading — 80% win rate with tiny wins and huge losses is negative expectancy. This chart shows which symbols have genuine edge.

**ECharts components:** `BarChart` (already registered).

---

### C8: Holding Period Analysis (frontend-only)

**Type:** ECharts ScatterChart with trend line
**Data source:** Existing `fetchDurationProfit()` → `DurationProfitPoint[]` (has `duration_secs`, `pnl`, `symbol`)
**Derived calculation (client-side):** Bucket trades into holding period ranges and compute average P&L per bucket.
**Visualization:**
- Dual view: scatter plot (existing Duration/Profitability) PLUS a bar overlay
- Bar overlay: average P&L per holding period bucket (< 5min, 5-30min, 30min-2hr, 2-8hr, 8hr-1d, 1d+)
- Color: green/red bars by avg P&L sign
- Tooltip: bucket range, avg P&L, trade count, win rate within bucket

**Why it matters:** Reveals optimal holding period. Many traders have positive expectancy on 2-8hr holds but negative on scalps — or vice versa. Actionable insight: stop doing the unprofitable duration.

**ECharts components:** `ScatterChart` (already registered), `BarChart` (already registered).

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | All 8 charts available in ChartSelector dropdown | High | Frontend |
| FR-2 | C1 (Drawdown) renders from existing equity curve data | High | Frontend |
| FR-3 | C2 (Calendar) new backend endpoint + ECharts calendar component | High | Backend + Frontend |
| FR-4 | C3 (Treemap) renders from existing symbol breakdown data | High | Frontend |
| FR-5 | C4 (Streaks) new backend endpoint + waterfall bars | Medium | Backend + Frontend |
| FR-6 | C5 (R-Distribution) new backend endpoint + histogram | High | Backend + Frontend |
| FR-7 | C6 (Exposure) new backend endpoint + stacked area | Medium | Backend + Frontend |
| FR-8 | C7 (Expectancy) renders from existing symbol breakdown data | High | Frontend |
| FR-9 | C8 (Holding Period) renders from existing duration/profit data | Medium | Frontend |
| FR-10 | All charts respect existing `StatsFilter` (exchange, symbol, date range) | High | Both |
| FR-11 | All charts use `testudo-dark` / `testudo-light` theme tokens | High | Frontend |
| FR-12 | Charts re-render on theme toggle without page reload | Medium | Frontend |
| FR-13 | `ChartContainer` wrapper with solid `bg-elevated` background (not transparent) | High | Frontend |

---

## Technical Implementation

### ECharts Registration (echarts-setup.ts)

Add these chart types and components:

```typescript
import { TreemapChart, LineChart } from 'echarts/charts'
import { CalendarComponent, MarkLineComponent } from 'echarts/components'

echarts.use([
  // ... existing
  TreemapChart,
  LineChart,
  CalendarComponent,
  MarkLineComponent,
])
```

### New Backend Endpoints

| Endpoint | Handler | Service Method |
|----------|---------|----------------|
| `GET /analytics/calendar-pnl` | `calendar_pnl()` | `TimeSeriesService::calendar_pnl()` |
| `GET /analytics/streaks` | `streaks()` | `TimeSeriesService::streaks()` |
| `GET /analytics/r-distribution` | `r_distribution()` | `TimeSeriesService::r_distribution()` |
| `GET /analytics/exposure-timeline` | `exposure_timeline()` | `TimeSeriesService::exposure_timeline()` |

All endpoints follow existing pattern: accept `StatsFilter` query params, return `DataWrapper<Vec<T>>`.

### New Frontend API Functions (client.ts)

```typescript
export async function fetchCalendarPnl(filters: StatsFilter): Promise<{ data: CalendarPnlDay[] }>
export async function fetchStreaks(filters: StatsFilter): Promise<{ data: StreakSegment[] }>
export async function fetchRDistribution(filters: StatsFilter): Promise<{ data: RBucket[] }>
export async function fetchExposureTimeline(filters: StatsFilter): Promise<{ data: ExposurePoint[] }>
```

### New Chart Components (testudo-journal/src/components/charts/)

| File | Chart | Data Source |
|------|-------|------------|
| `DrawdownChart.tsx` | C1 | `fetchEquityCurve()` (existing) |
| `CalendarHeatmap.tsx` | C2 | `fetchCalendarPnl()` (new) |
| `PnlTreemap.tsx` | C3 | `fetchSymbolBreakdown()` (existing) |
| `StreakWaterfall.tsx` | C4 | `fetchStreaks()` (new) |
| `RDistribution.tsx` | C5 | `fetchRDistribution()` (new) |
| `ExposureTimeline.tsx` | C6 | `fetchExposureTimeline()` (new) |
| `ExpectancyBySymbol.tsx` | C7 | `fetchSymbolBreakdown()` (existing) |
| `HoldingPeriodAnalysis.tsx` | C8 | `fetchDurationProfit()` (existing) |

### ChartSelector Update

Add to `CHART_OPTIONS`:
```typescript
{ value: 'drawdown', label: 'Drawdown' },
{ value: 'calendar', label: 'Calendar P&L' },
{ value: 'treemap', label: 'P&L Treemap' },
{ value: 'streaks', label: 'Win/Loss Streaks' },
{ value: 'r-dist', label: 'R-Multiple Distribution' },
{ value: 'exposure', label: 'Exposure Timeline' },
{ value: 'expectancy', label: 'Expectancy by Symbol' },
{ value: 'holding', label: 'Holding Period Analysis' },
```

### Files

**New backend files:**
- `testudo-exchange/crates/router/src/routes/journal.rs` — 4 new handler functions
- `testudo-exchange/crates/router/src/services/time_series.rs` — 4 new service methods
- `testudo-exchange/crates/router/src/main.rs` — register 4 new routes

**New frontend files:**
- `testudo-journal/src/components/charts/DrawdownChart.tsx`
- `testudo-journal/src/components/charts/CalendarHeatmap.tsx`
- `testudo-journal/src/components/charts/PnlTreemap.tsx`
- `testudo-journal/src/components/charts/StreakWaterfall.tsx`
- `testudo-journal/src/components/charts/RDistribution.tsx`
- `testudo-journal/src/components/charts/ExposureTimeline.tsx`
- `testudo-journal/src/components/charts/ExpectancyBySymbol.tsx`
- `testudo-journal/src/components/charts/HoldingPeriodAnalysis.tsx`

**Modified files:**
- `testudo-journal/src/lib/echarts-setup.ts` — register TreemapChart, LineChart, CalendarComponent, MarkLineComponent
- `testudo-journal/src/components/ChartSelector.tsx` — add 8 new options
- `testudo-journal/src/api/client.ts` — add 4 new fetch functions + 4 new interfaces

### Dependencies Added

- No new crate or npm dependencies — ECharts already bundles all required chart types, they just need to be imported and registered.

---

## Implementation Order

Frontend-only charts first (no backend work needed), then backend+frontend:

| Phase | Charts | Backend Work |
|-------|--------|-------------|
| 1 | C1 (Drawdown), C3 (Treemap), C7 (Expectancy), C8 (Holding Period) | None |
| 2 | C5 (R-Distribution), C2 (Calendar), C4 (Streaks), C6 (Exposure) | 4 new endpoints |

---

## Acceptance Criteria

- [ ] All 8 charts render in ChartSelector dropdown
- [ ] C1 Drawdown shows inverted area with max DD marker
- [ ] C2 Calendar renders GitHub-style day grid with green/red color scale
- [ ] C3 Treemap sizes rectangles by abs(P&L), colors by profit/loss
- [ ] C4 Streaks shows positive/negative bars for consecutive win/loss runs
- [ ] C5 R-Distribution shows histogram with P&L contribution overlay
- [ ] C6 Exposure shows concurrent position count over time
- [ ] C7 Expectancy shows per-symbol expectancy bars
- [ ] C8 Holding Period shows avg P&L by duration bucket
- [ ] All charts respect StatsFilter (exchange, symbol, date range)
- [ ] All charts render correctly in both dark and light themes
- [ ] Charts use solid `bg-elevated` background (not transparent)
- [ ] Theme toggle updates chart colors without page reload
- [ ] `bun run build` passes for testudo-journal
- [ ] `cargo clippy --all-targets && cargo test` passes for testudo-exchange

---

## Risks

1. **Calendar component bundle size** — ECharts `CalendarComponent` adds to the bundle. Mitigation: tree-shaking via explicit imports (already the pattern in echarts-setup.ts).
2. **Exposure timeline query performance** — Computing concurrent positions requires a date-range scan. Mitigation: limit to filtered date range, add index on `(user_id, opened_at, closed_at)`.
3. **R-multiple coverage** — Not all trades have `r_multiple` set (nullable field). Mitigation: C5 shows "N/A" bucket count and tooltip explaining missing data.

---

## Completion Signal

This spec is complete when:
1. All 8 chart types render in ChartSelector with correct data
2. 4 new backend analytics endpoints deployed and returning data
3. All acceptance criteria met
4. Verification commands pass
5. Code committed to master
