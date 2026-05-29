/** @anchor ui:journal:PnlTreemap
 * @tags ui */

import { createMemo } from 'solid-js'
import { ChartContainer } from './ChartContainer'
import { EChart } from './EChart'
import { useFilters } from '../filterContext'
import { useAuth } from '../../context/AuthContext'
import { fetchSymbolBreakdown } from '../../api/client'
import { useCachedResource, cacheKeyForSection } from '../../lib/cache'
import { getSignalGreen, getSignalRed } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

export function PnlTreemap() {
  const { filters, setFilters } = useFilters()
  const auth = useAuth()
  const data = useCachedResource(
    () => cacheKeyForSection('symbol_breakdown', filters()),
    () => fetchSymbolBreakdown(filters()),
    { staleMs: 30_000, persist: true, identity: auth.user()?.id ?? null },
  )
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
            const symbol = params.name.replace(/_USDT?$/, '')
            return `{name|${symbol}}\n{pnl|${pnlStr}}`
          },
          rich: {
            name: { fontSize: 13, fontWeight: 'bold', fontFamily: "'Space Mono', monospace", lineHeight: 18 },
            pnl: { fontSize: 11, fontFamily: "'Space Mono', monospace", lineHeight: 16, padding: [2, 0, 0, 0] },
          },
          overflow: 'break',
        },
        upperLabel: { show: false },
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
      onRetry={() => data.refetch()}
      hasActiveFilters={hasActiveFilters()}
      onClearFilters={() => setFilters({})}
    >
      <EChart option={option} height="280px" />
    </ChartContainer>
  )
}
