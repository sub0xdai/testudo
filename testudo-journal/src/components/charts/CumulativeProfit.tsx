import { createResource, onMount, onCleanup, createEffect } from 'solid-js'
import { createChart, type IChartApi, type ISeriesApi, AreaSeries } from 'lightweight-charts'
import { ChartContainer } from './ChartContainer'
import { useFilters } from '../filterContext'
import { fetchEquityCurve } from '../../api/client'

export function CumulativeProfit() {
  const { filters } = useFilters()
  const [data] = createResource(filters, fetchEquityCurve)

  let container!: HTMLDivElement
  let chart: IChartApi | undefined
  let series: ISeriesApi<'Area'> | undefined

  onMount(() => {
    chart = createChart(container, {
      width: container.clientWidth,
      height: 250,
      layout: {
        background: { color: '#111111' },
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
      topColor: 'rgba(0, 255, 65, 0.3)',
      bottomColor: 'rgba(0, 255, 65, 0.02)',
      lineColor: '#00FF41',
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
    const d = data()
    if (!d?.data?.length || !series) return

    const points = d.data.map((p) => ({
      time: p.date as string,
      value: parseFloat(p.cumulative_pnl),
    }))

    series.setData(points)
    chart?.timeScale().fitContent()
  })

  return (
    <ChartContainer title="CUMULATIVE PROFIT" loading={data.loading} empty={!data()?.data?.length}>
      <div ref={container!} />
    </ChartContainer>
  )
}
