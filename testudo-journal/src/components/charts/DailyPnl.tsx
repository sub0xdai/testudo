import { createResource, onMount, onCleanup, createEffect } from 'solid-js'
import { createChart, type IChartApi, type ISeriesApi, HistogramSeries } from 'lightweight-charts'
import { ChartContainer } from './ChartContainer'
import { useFilters } from '../filterContext'
import { fetchDailyPnl } from '../../api/client'
import { SIGNAL_GREEN, SIGNAL_RED, CHART_BG } from '../../lib/tokens'

export function DailyPnl() {
  const { filters, setFilters } = useFilters()
  const [data, { refetch }] = createResource(filters, fetchDailyPnl)
  const hasActiveFilters = () => Object.values(filters()).some(Boolean)

  let container!: HTMLDivElement
  let chart: IChartApi | undefined
  let series: ISeriesApi<'Histogram'> | undefined

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

    series = chart.addSeries(HistogramSeries, {
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

    const bars = d.data.map((p) => {
      const value = parseFloat(p.pnl)
      return {
        time: p.date as string,
        value,
        color: value >= 0 ? SIGNAL_GREEN : SIGNAL_RED,
      }
    })

    series.setData(bars)
    chart?.timeScale().fitContent()
  })

  return (
    <ChartContainer title="DAILY P&L" loading={data.loading} empty={!data()?.data?.length} onRetry={refetch} hasActiveFilters={hasActiveFilters()} onClearFilters={() => setFilters({})}>
      <div ref={container!} />
    </ChartContainer>
  )
}
