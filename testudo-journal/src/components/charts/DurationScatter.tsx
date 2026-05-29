/** @anchor ui:journal:DurationScatter
 * @tags ui */

import { createMemo } from 'solid-js'
import { ChartContainer } from './ChartContainer'
import { EChart } from './EChart'
import { useFilters } from '../filterContext'
import { useAuth } from '../../context/AuthContext'
import { fetchDurationProfit } from '../../api/client'
import { useCachedResource, cacheKeyForSection } from '../../lib/cache'
import { signalGreenAlpha, signalRedAlpha, getTextTertiary } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

export function DurationScatter() {
  const { filters, setFilters } = useFilters()
  const auth = useAuth()
  const data = useCachedResource(
    () => cacheKeyForSection('duration_profit', filters()),
    () => fetchDurationProfit(filters()),
    { staleMs: 30_000, persist: true, identity: auth.user()?.id ?? null },
  )
  const hasActiveFilters = () => Object.values(filters()).some(Boolean)

  const option = createMemo((): EChartsOption | undefined => {
    const d = data()
    if (!d?.data?.length) return undefined

    const points = d.data.map((p) => ({
      value: [p.duration_secs / 3600, parseFloat(p.pnl)],
      name: p.symbol,
      itemStyle: {
        color: parseFloat(p.pnl) >= 0 ? signalGreenAlpha(0.6) : signalRedAlpha(0.6),
      },
    }))

    return {
      tooltip: {
        trigger: 'item',
        formatter: (params: any) => {
          const [hours, pnl] = params.value as [number, number]
          const sym = params.data.name
          const sign = pnl >= 0 ? '+' : ''
          return `<span style="color:#fff">${sym}</span><br/>Duration: ${hours.toFixed(1)}h<br/>P&L: ${sign}$${pnl.toFixed(2)}`
        },
      },
      grid: { left: 55, right: 20, top: 16, bottom: 40 },
      xAxis: {
        type: 'value',
        name: 'DURATION (HRS)',
        nameLocation: 'middle',
        nameGap: 25,
        nameTextStyle: { color: getTextTertiary(), fontSize: 10 },
      },
      yAxis: {
        type: 'value',
        name: 'P&L ($)',
        nameLocation: 'middle',
        nameGap: 40,
        nameTextStyle: { color: getTextTertiary(), fontSize: 10 },
      },
      series: [{
        type: 'scatter',
        symbolSize: 8,
        data: points,
        emphasis: {
          itemStyle: { borderColor: '#fff', borderWidth: 1 },
          scale: 1.4,
        },
      }],
    }
  })

  return (
    <ChartContainer
      title="DURATION / PROFIT"
      loading={data.loading}
      empty={!data()?.data?.length}
      onRetry={() => data.refetch()}
      hasActiveFilters={hasActiveFilters()}
      onClearFilters={() => setFilters({})}
    >
      <EChart option={option} />
    </ChartContainer>
  )
}
