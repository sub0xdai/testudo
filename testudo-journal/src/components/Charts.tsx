import { EquityCurve } from './charts/EquityCurve'
import { DailyPnl } from './charts/DailyPnl'
import { CumulativeProfit } from './charts/CumulativeProfit'
import { SymbolDonut } from './charts/SymbolDonut'
import { MarketReturn } from './charts/MarketReturn'
import { DurationScatter } from './charts/DurationScatter'
import { ReturnHistogram } from './charts/ReturnHistogram'
import { TimeHeatmap } from './charts/TimeHeatmap'

export function Charts() {
  return (
    <div class="space-y-6">
      {/* Full-width equity curve */}
      <EquityCurve />

      {/* 2-column grid */}
      <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <DailyPnl />
        <CumulativeProfit />
        <SymbolDonut />
        <MarketReturn />
        <DurationScatter />
        <ReturnHistogram />
      </div>

      {/* Full-width heatmap */}
      <TimeHeatmap />
    </div>
  )
}
