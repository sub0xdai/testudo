/** @anchor ui:journal:SymbolBreakdown
 * @tags ui */

import { createResource, createMemo } from 'solid-js'
import { ChartContainer } from './ChartContainer'
import { EChart } from './EChart'
import { useFilters } from '../filterContext'
import { fetchSymbolBreakdown } from '../../api/client'
import { getTagPalette, getTextTertiary } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

export function SymbolBreakdown() {
  const { filters, setFilters } = useFilters()
  const [data, { refetch }] = createResource(filters, fetchSymbolBreakdown)
  const hasActiveFilters = () => Object.values(filters()).some(Boolean)

  const option = createMemo((): EChartsOption | undefined => {
    const d = data()
    if (!d?.data?.length) return undefined

    const palette = getTagPalette()
    const textColor = getTextTertiary()

    return {
      tooltip: {
        trigger: 'axis',
        axisPointer: { type: 'shadow' },
        formatter: (params: any) => {
          const p = Array.isArray(params) ? params[0] : params
          return `<span style="color:#fff">${p.name}</span><br/>${p.value} trades`
        },
      },
      grid: { left: 100, right: 40, top: 10, bottom: 10, containLabel: false },
      yAxis: {
        type: 'category',
        data: d.data.map(s => s.symbol),
        axisLabel: {
          fontFamily: "'Space Mono', monospace",
          fontSize: 11,
          color: textColor,
        },
        axisTick: { show: false },
        axisLine: { show: false },
        inverse: true,
      },
      xAxis: {
        type: 'value',
        axisLabel: { show: false },
        splitLine: { show: false },
        axisTick: { show: false },
        axisLine: { show: false },
      },
      series: [{
        type: 'bar',
        data: d.data.map((s, i) => ({
          value: s.trade_count,
          itemStyle: { color: palette[i % palette.length], opacity: 0.7 },
        })),
        barWidth: '40%',
        label: {
          show: true,
          position: 'right',
          formatter: (params: any) => `${params.value}`,
          fontFamily: "'Space Mono', monospace",
          fontSize: 11,
          color: textColor,
        },
      }],
    }
  })

  return (
    <ChartContainer
      title="SYMBOL ALLOCATION"
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
