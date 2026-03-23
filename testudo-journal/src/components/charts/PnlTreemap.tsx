import { createResource, createMemo } from 'solid-js'
import { ChartContainer } from './ChartContainer'
import { EChart } from './EChart'
import { useFilters } from '../filterContext'
import { fetchSymbolBreakdown } from '../../api/client'
import { getSignalGreen, getSignalRed } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

export function PnlTreemap() {
  const { filters, setFilters } = useFilters()
  const [data, { refetch }] = createResource(filters, fetchSymbolBreakdown)
  const hasActiveFilters = () => Object.values(filters()).some(Boolean)

  const option = createMemo((): EChartsOption | undefined => {
    const d = data()
    if (!d?.data?.length) return undefined

    return {
      tooltip: {
        formatter: (params: any) => {
          const { data: item } = params
          if (!item) return ''
          const pnl = parseFloat(item.pnl)
          const pnlStr = pnl >= 0 ? `+$${pnl.toFixed(2)}` : `-$${Math.abs(pnl).toFixed(2)}`
          const winRate = (parseFloat(item.winRate) * 100).toFixed(1)
          return [
            `<span style="color:#fff;font-weight:600">${item.name}</span>`,
            `P&L: ${pnlStr}`,
            `Trades: ${item.tradeCount}`,
            `Win rate: ${winRate}%`,
          ].join('<br/>')
        },
      },
      series: [{
        type: 'treemap',
        roam: false,
        breadcrumb: { show: false },
        label: {
          show: true,
          formatter: (params: any) => {
            const pnl = parseFloat(params.data.pnl)
            const pnlStr = pnl >= 0 ? `+$${pnl.toFixed(2)}` : `-$${Math.abs(pnl).toFixed(2)}`
            return `${params.name}\n${pnlStr}`
          },
          fontSize: 12,
        },
        data: d.data.map((s) => {
          const pnl = parseFloat(s.total_pnl)
          return {
            name: s.symbol,
            value: Math.abs(pnl),
            pnl: s.total_pnl,
            tradeCount: s.trade_count,
            winRate: s.win_rate,
            itemStyle: {
              color: pnl >= 0 ? getSignalGreen() : getSignalRed(),
            },
          }
        }),
      }],
    }
  })

  return (
    <ChartContainer
      title="P&L TREEMAP"
      loading={data.loading}
      empty={!data()?.data?.length}
      onRetry={refetch}
      hasActiveFilters={hasActiveFilters()}
      onClearFilters={() => setFilters({})}
    >
      <EChart option={option} height="280px" />
    </ChartContainer>
  )
}
