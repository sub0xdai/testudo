/** @anchor ui:journal:CumulativeProfit
 * @tags ui */

import { onMount, onCleanup, createEffect } from 'solid-js'
import { createChart, type IChartApi, type ISeriesApi, AreaSeries } from 'lightweight-charts'
import { ChartContainer } from './ChartContainer'
import type { EquityPoint } from '../../api/client'
import { getSignalGreen, getChartBg, signalGreenAlpha, getGridLineColor, getCrosshairColor, getTextTertiary } from '../../lib/tokens'
import { onThemeChange } from '../../lib/theme-observer'

export function CumulativeProfit(props: {
  data?: { data: EquityPoint[] }
  loading: boolean
  error?: string
}) {

  let container!: HTMLDivElement
  let chart: IChartApi | undefined
  let series: ISeriesApi<'Area'> | undefined

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

    series = chart.addSeries(AreaSeries, {
      topColor: signalGreenAlpha(0.3),
      bottomColor: signalGreenAlpha(0.02),
      lineColor: getSignalGreen(),
      lineWidth: 2,
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
      const d = props.data
      initChart()
      if (d?.data?.length && series) {
        series.setData(d.data.map((p) => ({ time: p.date as string, value: parseFloat(p.cumulative_pnl) })))
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
