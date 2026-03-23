import { createSignal, Show } from 'solid-js'
import { SymbolDonut } from './charts/SymbolDonut'
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
  { value: 'daily-pnl', label: 'Daily P&L' },
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
    <div>
      <div class="flex items-center gap-3 mb-4">
        <select
          value={selected()}
          onChange={(e) => setSelected(e.currentTarget.value as ChartOption)}
          class="font-mono text-xs border border-container-border bg-elevated text-text-primary px-3 py-1.5"
          aria-label="Select chart type"
        >
          {CHART_OPTIONS.map((opt) => (
            <option value={opt.value}>{opt.label}</option>
          ))}
        </select>
      </div>

      <Show when={selected() === 'symbol'}><SymbolDonut /></Show>
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
  )
}
