import { createMemo } from 'solid-js'
import { ChartContainer } from './ChartContainer'
import { EChart } from './EChart'
import { useFilters } from '../filterContext'
import { fetchReturnDistribution } from '../../api/client'
import { useCachedResource, stableHash } from '../../lib/cache'
import { getSignalGreen, getSignalRed } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

export function ReturnHistogram() {
  const { filters, setFilters } = useFilters()
  const data = useCachedResource(
    () => 'return-distribution:' + stableHash(filters()),
    () => fetchReturnDistribution(filters()),
    { staleMs: 30_000 },
  )
  const hasActiveFilters = () => Object.values(filters()).some(Boolean)

  const option = createMemo((): EChartsOption | undefined => {
    const d = data()
    if (!d?.data?.length) return undefined

    return {
      tooltip: {
        trigger: 'axis',
        axisPointer: { type: 'shadow' },
        formatter: (params: any) => {
          const p = Array.isArray(params) ? params[0] : params
          return `<span style="color:#fff">${p.name}</span><br/>${p.value} trades`
        },
      },
      grid: { left: 40, right: 20, top: 8, bottom: 24 },
      xAxis: {
        type: 'category',
        data: d.data.map((b) => b.bucket),
        axisLabel: { fontSize: 10 },
      },
      yAxis: { type: 'value' },
      series: [{
        type: 'bar',
        data: d.data.map((b) => {
          const num = parseFloat(b.bucket)
          return {
            value: b.count,
            itemStyle: { color: isNaN(num) || num >= 0 ? getSignalGreen() : getSignalRed() },
          }
        }),
        barGap: '0%',
        barCategoryGap: '0%',
      }],
    }
  })

  return (
    <ChartContainer
      title="RETURN DISTRIBUTION"
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
