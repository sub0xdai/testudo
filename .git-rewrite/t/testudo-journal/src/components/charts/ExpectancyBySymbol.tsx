import { createResource, createMemo } from 'solid-js'
import { ChartContainer } from './ChartContainer'
import { EChart } from './EChart'
import { useFilters } from '../filterContext'
import { fetchSymbolBreakdown } from '../../api/client'
import { getSignalGreen, getSignalRed, getTextTertiary } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

export function ExpectancyBySymbol() {
  const { filters, setFilters } = useFilters()
  const [data, { refetch }] = createResource(filters, fetchSymbolBreakdown)
  const hasActiveFilters = () => Object.values(filters()).some(Boolean)

  const option = createMemo((): EChartsOption | undefined => {
    const d = data()
    if (!d?.data?.length) return undefined

    const items = d.data
      .map((s) => ({
        symbol: s.symbol,
        expectancy: parseFloat(s.total_pnl) / s.trade_count,
        totalPnl: parseFloat(s.total_pnl),
        tradeCount: s.trade_count,
        winRate: parseFloat(s.win_rate),
      }))
      .sort((a, b) => b.expectancy - a.expectancy)

    return {
      tooltip: {
        trigger: 'axis',
        axisPointer: { type: 'shadow' },
        formatter: (params: any) => {
          const p = Array.isArray(params) ? params[0] : params
          const item = items[p.dataIndex]
          if (!item) return ''
          return [
            `<span style="color:#fff">${item.symbol}</span>`,
            `Expectancy: <b>$${item.expectancy.toFixed(2)}</b>`,
            `Total P&L: $${item.totalPnl.toFixed(2)}`,
            `Trades: ${item.tradeCount}`,
            `Win Rate: ${(item.winRate * 100).toFixed(1)}%`,
          ].join('<br/>')
        },
      },
      grid: { left: 50, right: 20, top: 8, bottom: items.length > 6 ? 60 : 24 },
      xAxis: {
        type: 'category',
        data: items.map((i) => i.symbol),
        axisLabel: {
          fontSize: 10,
          rotate: items.length > 6 ? 45 : 0,
          color: getTextTertiary(),
        },
      },
      yAxis: {
        type: 'value',
        axisLabel: {
          fontSize: 10,
          formatter: (v: number) => `$${v.toFixed(0)}`,
        },
      },
      series: [{
        type: 'bar',
        data: items.map((i) => ({
          value: i.expectancy,
          itemStyle: { color: i.expectancy >= 0 ? getSignalGreen() : getSignalRed() },
        })),
      }],
    }
  })

  return (
    <ChartContainer
      title="EXPECTANCY BY SYMBOL"
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
