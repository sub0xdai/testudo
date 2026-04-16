import { createSignal, Show, lazy, Suspense } from 'solid-js'
import type { EquityPoint } from '../api/client'

// Lazy-load chart components — only fetched when selected
const SymbolBreakdown = lazy(() => import('./charts/SymbolBreakdown').then(m => ({ default: m.SymbolBreakdown })))
const MarketReturn = lazy(() => import('./charts/MarketReturn').then(m => ({ default: m.MarketReturn })))
const DurationScatter = lazy(() => import('./charts/DurationScatter').then(m => ({ default: m.DurationScatter })))
const ReturnHistogram = lazy(() => import('./charts/ReturnHistogram').then(m => ({ default: m.ReturnHistogram })))
const TimeHeatmap = lazy(() => import('./charts/TimeHeatmap').then(m => ({ default: m.TimeHeatmap })))
const DailyPnl = lazy(() => import('./charts/DailyPnl').then(m => ({ default: m.DailyPnl })))
const CumulativeProfit = lazy(() => import('./charts/CumulativeProfit').then(m => ({ default: m.CumulativeProfit })))
const DrawdownChart = lazy(() => import('./charts/DrawdownChart').then(m => ({ default: m.DrawdownChart })))
const PnlTreemap = lazy(() => import('./charts/PnlTreemap').then(m => ({ default: m.PnlTreemap })))
const ExpectancyBySymbol = lazy(() => import('./charts/ExpectancyBySymbol').then(m => ({ default: m.ExpectancyBySymbol })))
const HoldingPeriodAnalysis = lazy(() => import('./charts/HoldingPeriodAnalysis').then(m => ({ default: m.HoldingPeriodAnalysis })))

type ChartOption =
  | 'symbol' | 'market' | 'duration' | 'return' | 'heatmap'
  | 'daily-pnl' | 'cumulative'
  | 'drawdown' | 'treemap' | 'expectancy' | 'holding'

const CHART_OPTIONS: { value: ChartOption; label: string }[] = [
  { value: 'symbol', label: 'Symbol Distribution' },
  { value: 'treemap', label: 'P&L Treemap' },
  { value: 'expectancy', label: 'Expectancy by Symbol' },
  { value: 'daily-pnl', label: 'Daily P&L History' },
  { value: 'cumulative', label: 'Cumulative Profit' },
  { value: 'drawdown', label: 'Drawdown' },
  { value: 'holding', label: 'Holding Period Analysis' },
  { value: 'market', label: 'Market Return' },
  { value: 'duration', label: 'Duration / Profitability' },
  { value: 'return', label: 'Return Distribution' },
  { value: 'heatmap', label: 'Time Heatmap' },
]

function ChartLoading() {
  return (
    <div aria-live="polite" aria-busy="true" class="flex items-center justify-center h-full min-h-[200px]">
      <div class="w-4 h-4 border-2 border-text-secondary border-t-text-primary rounded-full animate-spin" />
    </div>
  )
}

interface ChartSelectorProps {
  defaultChart?: ChartOption
  equityData?: { data: EquityPoint[] }
  equityLoading?: boolean
}

export function ChartSelector(props: ChartSelectorProps) {
  const [selected, setSelected] = createSignal<ChartOption>(props.defaultChart ?? 'symbol')

  return (
    <div class="bg-container-bg flex flex-col overflow-hidden">
      {/* Panel header with embedded chart selector */}
      <div class="flex items-center justify-between border-b border-container-border/50 px-6 py-3">
        <select
          value={selected()}
          onChange={(e) => setSelected(e.currentTarget.value as ChartOption)}
          class="bg-main-bg font-mono text-xs text-text-tertiary uppercase outline-none cursor-pointer hover:text-text-primary transition-colors"
          aria-label="Chart type"
        >
          {CHART_OPTIONS.map((opt) => (
            <option value={opt.value} class="bg-main-bg text-text-primary">{opt.label}</option>
          ))}
        </select>
      </div>

      {/* Chart content */}
      <div class="p-6 flex-grow relative min-h-[250px]">
        <Suspense fallback={<ChartLoading />}>
          <Show when={selected() === 'symbol'}><SymbolBreakdown /></Show>
          <Show when={selected() === 'treemap'}><PnlTreemap /></Show>
          <Show when={selected() === 'expectancy'}><ExpectancyBySymbol /></Show>
          <Show when={selected() === 'daily-pnl'}><DailyPnl /></Show>
          <Show when={selected() === 'cumulative'}>
            <CumulativeProfit
              data={props.equityData}
              loading={props.equityLoading ?? false}
            />
          </Show>
          <Show when={selected() === 'drawdown'}><DrawdownChart /></Show>
          <Show when={selected() === 'holding'}><HoldingPeriodAnalysis /></Show>
          <Show when={selected() === 'market'}><MarketReturn /></Show>
          <Show when={selected() === 'duration'}><DurationScatter /></Show>
          <Show when={selected() === 'return'}><ReturnHistogram /></Show>
          <Show when={selected() === 'heatmap'}><TimeHeatmap /></Show>
        </Suspense>
      </div>
    </div>
  )
}
