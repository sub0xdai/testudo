import { createSignal, Show } from 'solid-js'
import { SymbolBreakdown } from './charts/SymbolBreakdown'
import { MarketReturn } from './charts/MarketReturn'
import { DurationScatter } from './charts/DurationScatter'
import { ReturnHistogram } from './charts/ReturnHistogram'
import { TimeHeatmap } from './charts/TimeHeatmap'
import { DailyPnl } from './charts/DailyPnl'
import { CumulativeProfit } from './charts/CumulativeProfit'
import { DrawdownChart } from './charts/DrawdownChart'
import { PnlTreemap } from './charts/PnlTreemap'
import { ExpectancyBySymbol } from './charts/ExpectancyBySymbol'
import { HoldingPeriodAnalysis } from './charts/HoldingPeriodAnalysis'
import type { EquityPoint } from '../api/client'

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

interface ChartSelectorProps {
  defaultChart?: ChartOption
  equityData?: { data: EquityPoint[] }
  equityLoading?: boolean
}

export function ChartSelector(props: ChartSelectorProps) {
  const [selected, setSelected] = createSignal<ChartOption>(props.defaultChart ?? 'symbol')

  return (
    <div class="glass-panel flex flex-col overflow-hidden">
      {/* Panel header with embedded chart selector */}
      <div class="flex items-center justify-between border-b border-container-border/50 px-5 py-3">
        <select
          value={selected()}
          onChange={(e) => setSelected(e.currentTarget.value as ChartOption)}
          class="bg-main-bg font-mono text-xs text-text-tertiary uppercase outline-none cursor-pointer hover:text-text-primary transition-colors"
          aria-label="Select chart type"
        >
          {CHART_OPTIONS.map((opt) => (
            <option value={opt.value} class="bg-main-bg text-text-primary">{opt.label}</option>
          ))}
        </select>
      </div>

      {/* Chart content */}
      <div class="p-5 flex-grow relative min-h-[250px]">
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
      </div>
    </div>
  )
}
