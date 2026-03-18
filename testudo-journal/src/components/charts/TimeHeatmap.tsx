import { createResource, createMemo } from 'solid-js'
import { ChartContainer } from './ChartContainer'
import { EChart } from './EChart'
import { useFilters } from '../filterContext'
import { fetchTimeDistribution } from '../../api/client'
import { SIGNAL_GREEN } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

const DAYS = ['SUN', 'MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT']
const HOURS = Array.from({ length: 24 }, (_, i) => String(i).padStart(2, '0'))

export function TimeHeatmap() {
  const { filters, setFilters } = useFilters()
  const [data, { refetch }] = createResource(filters, fetchTimeDistribution)
  const hasActiveFilters = () => Object.values(filters()).some(Boolean)

  const option = createMemo((): EChartsOption | undefined => {
    const d = data()
    if (!d?.data?.length) return undefined

    let maxCount = 0
    const points: [number, number, number][] = []

    for (const slot of d.data) {
      points.push([slot.hour, slot.day_of_week, slot.trade_count])
      if (slot.trade_count > maxCount) maxCount = slot.trade_count
    }

    // Fill missing cells with zero
    const existing = new Set(points.map(([h, d]) => `${h}-${d}`))
    for (let day = 0; day < 7; day++) {
      for (let hour = 0; hour < 24; hour++) {
        if (!existing.has(`${hour}-${day}`)) {
          points.push([hour, day, 0])
        }
      }
    }

    return {
      tooltip: {
        formatter: (params: any) => {
          const [hour, day, count] = params.value as [number, number, number]
          return `<span style="color:#fff">${DAYS[day]} ${String(hour).padStart(2, '0')}:00</span><br/>${count} trades`
        },
      },
      grid: { left: 50, right: 40, top: 8, bottom: 24 },
      xAxis: {
        type: 'category',
        data: HOURS,
        splitArea: { show: true },
        axisLabel: { fontSize: 10, interval: 2 },
      },
      yAxis: {
        type: 'category',
        data: DAYS,
        axisLabel: { fontSize: 10 },
      },
      visualMap: {
        min: 0,
        max: maxCount || 1,
        calculable: false,
        orient: 'vertical',
        right: 0,
        top: 'center',
        itemHeight: 100,
        textStyle: { color: '#555555', fontSize: 10 },
        inRange: { color: ['#1A1A1A', SIGNAL_GREEN] },
      },
      series: [{
        type: 'heatmap',
        data: points,
        emphasis: {
          itemStyle: { borderColor: '#fff', borderWidth: 1 },
        },
      }],
    }
  })

  return (
    <ChartContainer
      title="TIME DISTRIBUTION"
      loading={data.loading}
      empty={!data()?.data?.length}
      onRetry={refetch}
      hasActiveFilters={hasActiveFilters()}
      onClearFilters={() => setFilters({})}
    >
      <EChart option={option} height="250px" />
    </ChartContainer>
  )
}
