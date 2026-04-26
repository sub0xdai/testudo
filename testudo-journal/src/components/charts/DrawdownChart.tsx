import { createResource, createMemo } from 'solid-js'
import { ChartContainer } from './ChartContainer'
import { EChart } from './EChart'
import { useFilters } from '../filterContext'
import { fetchEquityCurve } from '../../api/client'
import { getSignalRed, signalRedAlpha } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

export function DrawdownChart() {
  const { filters, setFilters } = useFilters()
  const [data, { refetch }] = createResource(filters, fetchEquityCurve)
  const hasActiveFilters = () => Object.values(filters()).some(Boolean)

  const option = createMemo((): EChartsOption | undefined => {
    const d = data()
    if (!d?.data?.length) return undefined

    const dates = d.data.map((p) => p.date)
    const ddPcts = d.data.map((p) => -Math.abs(parseFloat(p.drawdown_pct)))
    const ddAbsolute = d.data.map((p) => parseFloat(p.drawdown))

    const minDD = Math.min(...ddPcts)

    return {
      tooltip: {
        trigger: 'axis',
        formatter: (params: any) => {
          const p = Array.isArray(params) ? params[0] : params
          const idx = p.dataIndex as number
          const pct = ddPcts[idx]
          const abs = ddAbsolute[idx]
          return [
            `<span style="color:#fff">${p.name}</span>`,
            `Drawdown: ${pct.toFixed(2)}%`,
            `Drawdown: $${abs.toFixed(2)}`,
          ].join('<br/>')
        },
      },
      grid: { left: 50, right: 20, top: 8, bottom: 24 },
      xAxis: {
        type: 'category',
        data: dates,
        axisLabel: { fontSize: 10 },
        boundaryGap: false,
      },
      yAxis: {
        type: 'value',
        max: 0,
        axisLabel: {
          fontSize: 10,
          formatter: (v: number) => `${v.toFixed(1)}%`,
        },
      },
      series: [{
        type: 'line',
        data: ddPcts,
        showSymbol: false,
        lineStyle: {
          color: signalRedAlpha(0.5),
          width: 2,
        },
        areaStyle: {
          color: {
            type: 'linear',
            x: 0, y: 0, x2: 0, y2: 1,
            colorStops: [
              { offset: 0, color: signalRedAlpha(0) },
              { offset: 1, color: signalRedAlpha(0.15) },
            ],
          },
        },
        markLine: {
          silent: true,
          symbol: 'none',
          lineStyle: {
            type: 'dashed',
            color: getSignalRed(),
            width: 1,
          },
          data: [{
            yAxis: minDD,
            label: {
              formatter: `Max DD: ${minDD.toFixed(2)}%`,
              fontSize: 10,
              color: getSignalRed(),
              position: 'insideEndTop',
            },
          }],
        },
      }],
    }
  })

  return (
    <ChartContainer
      title="DRAWDOWN"
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
