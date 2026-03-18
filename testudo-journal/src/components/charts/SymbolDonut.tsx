import { createResource, createMemo } from 'solid-js'
import { ChartContainer } from './ChartContainer'
import { EChart } from './EChart'
import { useFilters } from '../filterContext'
import { fetchSymbolBreakdown } from '../../api/client'
import { TAG_PALETTE } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

export function SymbolDonut() {
  const { filters, setFilters } = useFilters()
  const [data, { refetch }] = createResource(filters, fetchSymbolBreakdown)
  const hasActiveFilters = () => Object.values(filters()).some(Boolean)

  const option = createMemo((): EChartsOption | undefined => {
    const d = data()
    if (!d?.data?.length) return undefined

    return {
      tooltip: {
        trigger: 'item',
        formatter: (params: any) => {
          const { name, value, percent } = params
          return `<span style="color:#fff">${name}</span><br/>${value} trades (${percent}%)`
        },
      },
      legend: {
        orient: 'vertical',
        right: 10,
        top: 'center',
        textStyle: { fontSize: 11 },
      },
      series: [{
        type: 'pie',
        radius: ['50%', '75%'],
        center: ['35%', '50%'],
        label: { show: false },
        emphasis: {
          itemStyle: { shadowBlur: 10, shadowOffsetX: 0, shadowColor: 'rgba(0,0,0,0.5)' },
        },
        data: d.data.map((s, i) => ({
          name: s.symbol,
          value: s.trade_count,
          itemStyle: { color: TAG_PALETTE[i % TAG_PALETTE.length] },
        })),
      }],
    }
  })

  return (
    <ChartContainer
      title="SYMBOL DISTRIBUTION"
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
