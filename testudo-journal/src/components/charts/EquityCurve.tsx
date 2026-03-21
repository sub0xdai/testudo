import { onMount, onCleanup, createEffect } from 'solid-js'
import { createChart, type IChartApi, type ISeriesApi, LineSeries, AreaSeries } from 'lightweight-charts'
import { ChartContainer } from './ChartContainer'
import type { EquityPoint } from '../../api/client'
import { getSignalGreen, getChartBg, signalRedAlpha, getGridLineColor, getCrosshairColor, getTextTertiary } from '../../lib/tokens'
import { onThemeChange } from '../../lib/theme-observer'

export function EquityCurve(props: {
  data?: { data: EquityPoint[] }
  loading: boolean
  error?: string
}) {

  let container!: HTMLDivElement
  let chart: IChartApi | undefined
  let equityLine: ISeriesApi<'Line'> | undefined
  let drawdownArea: ISeriesApi<'Area'> | undefined

  function initChart() {
    chart?.remove()
    chart = createChart(container, {
      width: container.clientWidth,
      height: 300,
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
      crosshair: {
        vertLine: { color: getCrosshairColor() },
        horzLine: { color: getCrosshairColor() },
      },
      rightPriceScale: { borderColor: getCrosshairColor() },
      timeScale: { borderColor: getCrosshairColor() },
    })

    equityLine = chart.addSeries(LineSeries, {
      color: getSignalGreen(),
      lineWidth: 2,
      priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
    })

    drawdownArea = chart.addSeries(AreaSeries, {
      topColor: signalRedAlpha(0),
      bottomColor: signalRedAlpha(0.15),
      lineColor: signalRedAlpha(0.3),
      lineWidth: 1,
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
      if (d?.data?.length && equityLine && drawdownArea) {
        equityLine.setData(d.data.map((p) => ({ time: p.date as string, value: parseFloat(p.cumulative_pnl) })))
        drawdownArea.setData(d.data.map((p) => ({ time: p.date as string, value: -parseFloat(p.drawdown) })))
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
    if (!d?.data?.length || !equityLine || !drawdownArea) return

    const equityData = d.data.map((p) => ({
      time: p.date as string,
      value: parseFloat(p.cumulative_pnl),
    }))

    const ddData = d.data.map((p) => ({
      time: p.date as string,
      value: -parseFloat(p.drawdown),
    }))

    equityLine.setData(equityData)
    drawdownArea.setData(ddData)
    chart?.timeScale().fitContent()
  })

  return (
    <ChartContainer
      title="EQUITY CURVE"
      loading={props.loading}
      empty={!props.data?.data?.length}
      error={props.error}
      class="col-span-full"
    >
      <div ref={container!} />
    </ChartContainer>
  )
}
