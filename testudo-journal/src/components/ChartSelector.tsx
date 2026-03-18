import { createSignal, Show } from 'solid-js'
import { SymbolDonut } from './charts/SymbolDonut'
import { MarketReturn } from './charts/MarketReturn'
import { DurationScatter } from './charts/DurationScatter'
import { ReturnHistogram } from './charts/ReturnHistogram'
import { TimeHeatmap } from './charts/TimeHeatmap'

type ChartOption = 'symbol' | 'market' | 'duration' | 'return' | 'heatmap'

const CHART_OPTIONS: { value: ChartOption; label: string }[] = [
  { value: 'symbol', label: 'Symbol Distribution' },
  { value: 'market', label: 'Market Return' },
  { value: 'duration', label: 'Duration / Profitability' },
  { value: 'return', label: 'Return Distribution' },
  { value: 'heatmap', label: 'Time Heatmap' },
]

export function ChartSelector() {
  const [selected, setSelected] = createSignal<ChartOption>('symbol')

  return (
    <div>
      <div class="flex items-center gap-3 mb-4">
        <select
          value={selected()}
          onChange={(e) => setSelected(e.currentTarget.value as ChartOption)}
          class="font-mono text-xs border border-container-border bg-elevated text-text-primary px-3 py-1.5 focus-ring"
        >
          {CHART_OPTIONS.map((opt) => (
            <option value={opt.value}>{opt.label}</option>
          ))}
        </select>
      </div>

      <Show when={selected() === 'symbol'}><SymbolDonut /></Show>
      <Show when={selected() === 'market'}><MarketReturn /></Show>
      <Show when={selected() === 'duration'}><DurationScatter /></Show>
      <Show when={selected() === 'return'}><ReturnHistogram /></Show>
      <Show when={selected() === 'heatmap'}><TimeHeatmap /></Show>
    </div>
  )
}
