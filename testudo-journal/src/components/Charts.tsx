import { EquityCurve } from './charts/EquityCurve'
import { DailyPnl } from './charts/DailyPnl'
import { CumulativeProfit } from './charts/CumulativeProfit'
import { SymbolDonut } from './charts/SymbolDonut'
import { MarketReturn } from './charts/MarketReturn'
import { DurationScatter } from './charts/DurationScatter'
import { ReturnHistogram } from './charts/ReturnHistogram'
import { TimeHeatmap } from './charts/TimeHeatmap'
import { GhostAnnotation } from './GhostAnnotation'

export function Charts() {
  return (
    <div class="space-y-6">
      <div>
        <GhostAnnotation text="CHART_SUITE" />
        <h1 class="text-2xl md:text-3xl font-display font-bold tracking-tight">CHARTS</h1>
      </div>

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
