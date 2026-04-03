import { onMount, onCleanup, createEffect, Show } from 'solid-js'
import { createChart, type IChartApi, type ISeriesApi, BaselineSeries } from 'lightweight-charts'
import type { EquityPoint } from '../api/client'
import { SkeletonBar } from './SkeletonBar'
import { getSignalGreen, getSignalRed, signalGreenAlpha, signalRedAlpha, getGridLineColor, getCrosshairColor, getTextTertiary, getChartBg } from '../lib/tokens'
import { onThemeChange } from '../lib/theme-observer'

interface HeroEquityCurveProps {
  data: EquityPoint[] | undefined
  loading: boolean
}

export function HeroEquityCurve(props: HeroEquityCurveProps) {
  let container!: HTMLDivElement
  let chart: IChartApi | undefined
  let baseline: ISeriesApi<'Baseline'> | undefined

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

    baseline = chart.addSeries(BaselineSeries, {
      baseValue: { type: 'price', price: 0 },
      topLineColor: getSignalGreen(),
      topFillColor1: signalGreenAlpha(0.15),
      topFillColor2: signalGreenAlpha(0),
      bottomLineColor: getSignalRed(),
      bottomFillColor1: signalRedAlpha(0),
      bottomFillColor2: signalRedAlpha(0.15),
      lineWidth: 2,
      priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
    })
  }

  function setChartData(data: EquityPoint[]) {
    if (!baseline) return
    baseline.setData(data.map((p) => ({
      time: p.date as string,
      value: parseFloat(p.cumulative_pnl),
    })))
    chart?.timeScale().fitContent()
  }

  let resizeObserver: ResizeObserver | undefined
  let unsubTheme: (() => void) | undefined

  onMount(() => {
    onCleanup(() => {
      resizeObserver?.disconnect()
      unsubTheme?.()
      chart?.remove()
    })
  })

  createEffect(() => {
    const d = props.data
    if (!d?.length) return
    // container is only in the DOM when data exists (inside <Show>)
    if (!container) return

    if (!chart) {
      initChart()
      resizeObserver = new ResizeObserver(() => {
        chart?.applyOptions({ width: container.clientWidth })
      })
      resizeObserver.observe(container)
      unsubTheme = onThemeChange(() => {
        initChart()
        if (props.data?.length) setChartData(props.data)
      })
    }
    setChartData(d)
  })

  return (
    <div class="border-b border-container-border/50">
      <Show when={props.loading}>
        <div class="relative" style={{ "min-height": "400px" }}>
          <div class="absolute left-2 top-4 bottom-8 w-10 flex flex-col justify-between">
            <SkeletonBar width="36px" height="8px" />
            <SkeletonBar width="30px" height="8px" />
            <SkeletonBar width="34px" height="8px" />
            <SkeletonBar width="28px" height="8px" />
          </div>
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
