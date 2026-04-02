import { createResource, createMemo } from 'solid-js'
import { ChartContainer } from './ChartContainer'
import { EChart } from './EChart'
import { useFilters } from '../filterContext'
import { fetchDailyPnl } from '../../api/client'
import { getSignalGreen, getSignalRed, getTextTertiary } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

export function DailyPnl() {
  const { filters, setFilters } = useFilters()
  const [data, { refetch }] = createResource(filters, fetchDailyPnl)
  const hasActiveFilters = () => Object.values(filters()).some(Boolean)

  const option = createMemo((): EChartsOption | undefined => {
    const d = data()
    if (!d?.data?.length) return undefined

    const green = getSignalGreen()
    const red = getSignalRed()
    const textColor = getTextTertiary()

    const dates = d.data.map((p) => p.date)
    const values = d.data.map((p) => {
      const val = parseFloat(p.pnl)
      return {
        value: val,
        itemStyle: { color: val >= 0 ? green : red },
      }
    })

    return {
      tooltip: {
        trigger: 'axis',
        axisPointer: { type: 'shadow' },
        formatter: (params: any) => {
          const p = Array.isArray(params) ? params[0] : params
          const sign = p.value >= 0 ? '+' : ''
          return `<span style="color:#fff">${p.name}</span><br/>${sign}$${p.value.toFixed(2)}`
        },
      },
      grid: { left: 60, right: 20, top: 10, bottom: 30, containLabel: false },
      xAxis: {
        type: 'category',
        data: dates,
        axisLabel: { color: textColor, fontSize: 10, rotate: 45 },
        axisLine: { show: false },
        axisTick: { show: false },
      },
      yAxis: {
        type: 'value',
        axisLabel: {
          color: textColor,
          fontSize: 10,
          formatter: (v: number) => `$${v.toFixed(0)}`,
        },
        splitLine: { lineStyle: { color: 'rgba(255,255,255,0.05)' } },
      },
      series: [{
        type: 'bar',
        data: values,
        barMaxWidth: 20,
      }],
    }
  })

  return (
    <ChartContainer title="DAILY P&L HISTORY" loading={data.loading} empty={!data()?.data?.length} onRetry={refetch} hasActiveFilters={hasActiveFilters()} onClearFilters={() => setFilters({})}>
      <EChart option={option} />
    </ChartContainer>
  )
}
