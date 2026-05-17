import { createResource, createMemo } from 'solid-js'
import { ChartContainer } from './ChartContainer'
import { EChart } from './EChart'
import { useFilters } from '../filterContext'
import { fetchSymbolBreakdown } from '../../api/client'
import { getSignalGreen, getSignalRed } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

export function MarketReturn() {
  const { filters, setFilters } = useFilters()
  const [data, { refetch }] = createResource(filters, fetchSymbolBreakdown)
  const hasActiveFilters = () => Object.values(filters()).some(Boolean)

  const option = createMemo((): EChartsOption | undefined => {
    const d = data()
    if (!d?.data?.length) return undefined

    const sorted = [...d.data].sort((a, b) => parseFloat(a.total_pnl) - parseFloat(b.total_pnl))

    return {
      tooltip: {
        trigger: 'axis',
        axisPointer: { type: 'shadow' },
        formatter: (params: any) => {
          const p = Array.isArray(params) ? params[0] : params
          const val = p.value as number
          const sign = val >= 0 ? '+' : ''
          return `<span style="color:#fff">${p.name}</span><br/>P&L: ${sign}$${val.toFixed(2)}`
        },
      },
      grid: { left: 80, right: 20, top: 8, bottom: 24 },
      xAxis: { type: 'value' },
      yAxis: {
        type: 'category',
        data: sorted.map((s) => s.symbol),
        axisLabel: { fontSize: 11 },
      },
      series: [{
        type: 'bar',
        data: sorted.map((s) => {
          const val = parseFloat(s.total_pnl)
          return { value: val, itemStyle: { color: val >= 0 ? getSignalGreen() : getSignalRed() } }
        }),
        barMaxWidth: 16,
      }],
    }
  })

  return (
    <ChartContainer
      title="MARKET RETURN"
      loading={data.loading}
      empty={!data()?.data?.length}
      onRetry={refetch}
      hasActiveFilters={hasActiveFilters()}
      onClearFilters={() => setFilters({})}
    >
      <EChart option={option} />
    </ChartContainer>
  )
}
