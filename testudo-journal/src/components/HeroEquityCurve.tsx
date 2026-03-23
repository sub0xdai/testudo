import { onMount, onCleanup, createEffect, Show } from 'solid-js'
import { createChart, type IChartApi, type ISeriesApi, LineSeries, AreaSeries } from 'lightweight-charts'
import type { EquityPoint } from '../api/client'
import { SkeletonBar } from './SkeletonBar'
import { getSignalGreen, signalRedAlpha, getGridLineColor, getCrosshairColor, getTextTertiary, getChartBg } from '../lib/tokens'
import { onThemeChange } from '../lib/theme-observer'

interface HeroEquityCurveProps {
  data: EquityPoint[] | undefined
  loading: boolean
}

export function HeroEquityCurve(props: HeroEquityCurveProps) {
  let container!: HTMLDivElement
  let chart: IChartApi | undefined
  let equityLine: ISeriesApi<'Line'> | undefined
  let drawdownArea: ISeriesApi<'Area'> | undefined

  function initChart() {
    chart?.remove()
    chart = createChart(container, {
      width: container.clientWidth,
      height: 400,
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
      if (d?.length && equityLine && drawdownArea) {
        equityLine.setData(d.map((p) => ({ time: p.date as string, value: parseFloat(p.cumulative_pnl) })))
        drawdownArea.setData(d.map((p) => ({ time: p.date as string, value: -parseFloat(p.drawdown) })))
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
    if (!d?.length || !equityLine || !drawdownArea) return

    const equityData = d.map((p) => ({
      time: p.date as string,
      value: parseFloat(p.cumulative_pnl),
    }))

    const ddData = d.map((p) => ({
      time: p.date as string,
      value: -parseFloat(p.drawdown),
    }))

    equityLine.setData(equityData)
    drawdownArea.setData(ddData)
    chart?.timeScale().fitContent()
  })

  return (
    <div class="border-b border-container-border">
      <Show when={props.loading}>
        <div class="relative" style={{ "min-height": "400px" }}>
          {/* Y-axis ticks */}
          <div class="absolute left-2 top-4 bottom-8 w-10 flex flex-col justify-between">
            <SkeletonBar width="36px" height="8px" />
            <SkeletonBar width="30px" height="8px" />
            <SkeletonBar width="34px" height="8px" />
            <SkeletonBar width="28px" height="8px" />
          </div>
          {/* Chart area */}
          <div class="absolute left-14 top-4 right-4 bottom-8 border-l border-b border-container-border/20">
            <div class="absolute inset-0 skeleton-shimmer" />
          </div>
        </div>
      </Show>

      <Show when={!props.loading && (!props.data || props.data.length === 0)}>
        <div class="flex items-center justify-center" style={{ "min-height": "400px" }}>
          <div class="font-mono text-xs text-text-tertiary">NO DATA</div>
        </div>
      </Show>

      <Show when={!props.loading && props.data && props.data.length > 0}>
        <div ref={container!} style={{ "min-height": "400px" }} />
      </Show>
    </div>
  )
}
