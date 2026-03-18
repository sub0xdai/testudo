import { createResource } from 'solid-js'
import { EquityCurve } from './charts/EquityCurve'
import { DailyPnl } from './charts/DailyPnl'
import { CumulativeProfit } from './charts/CumulativeProfit'
import { SymbolDonut } from './charts/SymbolDonut'
import { MarketReturn } from './charts/MarketReturn'
import { DurationScatter } from './charts/DurationScatter'
import { ReturnHistogram } from './charts/ReturnHistogram'
import { TimeHeatmap } from './charts/TimeHeatmap'
import { PageSubHeader } from './PageSubHeader'
import { useFilters } from './filterContext'
import { fetchEquityCurve } from '../api/client'

export function Charts() {
  const { filters } = useFilters()
  const [equityData] = createResource(filters, fetchEquityCurve)

  return (
    <div class="space-y-6">
      <PageSubHeader title="ANALYSIS" />

      {/* Full-width equity curve */}
      <EquityCurve data={equityData()} loading={equityData.loading} error={equityData.error?.message} />

      {/* 2-column grid */}
      <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <DailyPnl />
        <CumulativeProfit data={equityData()} loading={equityData.loading} error={equityData.error?.message} />
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
