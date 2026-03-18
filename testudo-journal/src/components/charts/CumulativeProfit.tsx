import { onMount, onCleanup, createEffect } from 'solid-js'
import { createChart, type IChartApi, type ISeriesApi, AreaSeries } from 'lightweight-charts'
import { ChartContainer } from './ChartContainer'
import type { EquityPoint } from '../../api/client'
import { SIGNAL_GREEN, CHART_BG, signalGreenAlpha } from '../../lib/tokens'

export function CumulativeProfit(props: {
  data?: { data: EquityPoint[] }
  loading: boolean
  error?: string
}) {

  let container!: HTMLDivElement
  let chart: IChartApi | undefined
  let series: ISeriesApi<'Area'> | undefined

  onMount(() => {
    chart = createChart(container, {
      width: container.clientWidth,
      height: 250,
      layout: {
        background: { color: CHART_BG },
        textColor: '#555555',
        fontFamily: "'Space Mono', monospace",
        fontSize: 11,
      },
      grid: {
        vertLines: { color: '#1A1A1A' },
        horzLines: { color: '#1A1A1A' },
      },
      rightPriceScale: { borderColor: '#3F3F46' },
      timeScale: { borderColor: '#3F3F46' },
    })

    series = chart.addSeries(AreaSeries, {
      topColor: signalGreenAlpha(0.3),
      bottomColor: signalGreenAlpha(0.02),
      lineColor: SIGNAL_GREEN,
      lineWidth: 2,
      priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
    })

    const observer = new ResizeObserver(() => {
      chart?.applyOptions({ width: container.clientWidth })
    })
    observer.observe(container)

    onCleanup(() => {
      observer.disconnect()
      chart?.remove()
    })
  })

  createEffect(() => {
    const d = props.data
    if (!d?.data?.length || !series) return

    const points = d.data.map((p) => ({
      time: p.date as string,
      value: parseFloat(p.cumulative_pnl),
    }))

    series.setData(points)
    chart?.timeScale().fitContent()
  })

  return (
    <ChartContainer title="CUMULATIVE PROFIT" loading={props.loading} empty={!props.data?.data?.length} error={props.error}>
      <div ref={container!} />
    </ChartContainer>
  )
}
