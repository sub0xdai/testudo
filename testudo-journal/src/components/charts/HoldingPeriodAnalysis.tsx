import { createResource, createMemo } from 'solid-js'
import { ChartContainer } from './ChartContainer'
import { EChart } from './EChart'
import { useFilters } from '../filterContext'
import { fetchDurationProfit } from '../../api/client'
import { getSignalGreen, getSignalRed, getTextTertiary } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

interface Bucket {
  label: string
  minSecs: number
  maxSecs: number
}

const BUCKETS: Bucket[] = [
  { label: '< 5m', minSecs: 0, maxSecs: 300 },
  { label: '5\u201330m', minSecs: 300, maxSecs: 1800 },
  { label: '30m\u20132h', minSecs: 1800, maxSecs: 7200 },
  { label: '2\u20138h', minSecs: 7200, maxSecs: 28800 },
  { label: '8h\u20131d', minSecs: 28800, maxSecs: 86400 },
  { label: '1d+', minSecs: 86400, maxSecs: Infinity },
]

export function HoldingPeriodAnalysis() {
  const { filters, setFilters } = useFilters()
  const [data, { refetch }] = createResource(filters, fetchDurationProfit)
  const hasActiveFilters = () => Object.values(filters()).some(Boolean)

  const option = createMemo((): EChartsOption | undefined => {
    const d = data()
    if (!d?.data?.length) return undefined

    const bucketData = BUCKETS.map((b) => ({ ...b, sum: 0, count: 0 }))

    for (const point of d.data) {
      const secs = point.duration_secs
      const pnl = parseFloat(point.pnl)
      const idx = BUCKETS.findIndex(
        (b) => secs >= b.minSecs && secs < b.maxSecs,
      )
      if (idx !== -1) {
        bucketData[idx].sum += pnl
        bucketData[idx].count += 1
      }
    }

    const labels = bucketData.map((b) => b.label)
    const barData = bucketData.map((b) => {
      const avg = b.count > 0 ? b.sum / b.count : 0
      return {
        value: parseFloat(avg.toFixed(2)),
        count: b.count,
        totalPnl: parseFloat(b.sum.toFixed(2)),
        itemStyle: {
          color: avg >= 0 ? getSignalGreen() : getSignalRed(),
        },
      }
    })

    return {
      tooltip: {
        trigger: 'axis',
        axisPointer: { type: 'shadow' },
        formatter: (params: any) => {
          const p = Array.isArray(params) ? params[0] : params
          const d = p.data as { value: number; count: number; totalPnl: number }
          const sign = d.value >= 0 ? '+' : ''
          const totalSign = d.totalPnl >= 0 ? '+' : ''
          return [
            `<span style="color:#fff">${p.name as string}</span>`,
            `Avg P&L: ${sign}$${d.value.toFixed(2)}`,
            `Trades: ${d.count}`,
            `Total P&L: ${totalSign}$${d.totalPnl.toFixed(2)}`,
          ].join('<br/>')
        },
      },
      grid: { left: 55, right: 20, top: 30, bottom: 24 },
      xAxis: {
        type: 'category',
        data: labels,
        axisLabel: { fontSize: 10, color: getTextTertiary() },
      },
      yAxis: {
        type: 'value',
        name: 'AVG P&L ($)',
        nameLocation: 'center',
        nameGap: 40,
        nameTextStyle: { color: getTextTertiary(), fontSize: 10 },
      },
      series: [{
        type: 'bar',
        data: barData,
        label: {
          show: true,
          position: 'top',
          formatter: (params: any) => {
            const d = params.data as { count: number }
            return d.count > 0 ? `${d.count} trades` : ''
          },
          fontSize: 10,
          color: getTextTertiary(),
        },
      }],
    }
  })

  return (
    <ChartContainer
      title="HOLDING PERIOD ANALYSIS"
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
