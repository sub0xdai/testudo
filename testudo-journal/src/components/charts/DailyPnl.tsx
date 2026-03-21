import { createResource, onMount, onCleanup, createEffect } from 'solid-js'
import { createChart, type IChartApi, type ISeriesApi, HistogramSeries } from 'lightweight-charts'
import { ChartContainer } from './ChartContainer'
import { useFilters } from '../filterContext'
import { fetchDailyPnl } from '../../api/client'
import { getSignalGreen, getSignalRed, getChartBg, getGridLineColor, getCrosshairColor, getTextTertiary } from '../../lib/tokens'
import { onThemeChange } from '../../lib/theme-observer'

export function DailyPnl() {
  const { filters, setFilters } = useFilters()
  const [data, { refetch }] = createResource(filters, fetchDailyPnl)
  const hasActiveFilters = () => Object.values(filters()).some(Boolean)

  let container!: HTMLDivElement
  let chart: IChartApi | undefined
  let series: ISeriesApi<'Histogram'> | undefined

  function initChart() {
    chart?.remove()
    chart = createChart(container, {
      width: container.clientWidth,
      height: 250,
      layout: {
        background: { color: getChartBg() },
        textColor: getTextTertiary(),
        fontFamily: "'Space Mono', monospace",
        fontSize: 11,
      },
      grid: {
        vertLines: { color: getGridLineColor() },
        horzLines: { color: getGridLineColor() },
      },
      rightPriceScale: { borderColor: getCrosshairColor() },
      timeScale: { borderColor: getCrosshairColor() },
    })

    series = chart.addSeries(HistogramSeries, {
      priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
    })
  }

  onMount(() => {
    initChart()

    const resizeObserver = new ResizeObserver(() => {
      chart?.applyOptions({ width: container.clientWidth })
    })
    resizeObserver.observe(container)

    const unsubTheme = onThemeChange(() => {
      initChart()
      const d = data()
      if (d?.data?.length && series) {
        series.setData(d.data.map((p) => {
          const value = parseFloat(p.pnl)
          return { time: p.date as string, value, color: value >= 0 ? getSignalGreen() : getSignalRed() }
        }))
        chart?.timeScale().fitContent()
      }
    })

    onCleanup(() => {
      resizeObserver.disconnect()
      unsubTheme()
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
        color: value >= 0 ? getSignalGreen() : getSignalRed(),
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
