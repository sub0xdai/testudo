import { onMount, onCleanup, createEffect, Show } from 'solid-js'
import { createChart, type IChartApi, type ISeriesApi, LineSeries, AreaSeries } from 'lightweight-charts'
import type { EquityPoint } from '../api/client'

interface HeroEquityCurveProps {
  data: EquityPoint[] | undefined
  loading: boolean
}

export function HeroEquityCurve(props: HeroEquityCurveProps) {
  let container!: HTMLDivElement
  let chart: IChartApi | undefined
  let equityLine: ISeriesApi<'Line'> | undefined
  let drawdownArea: ISeriesApi<'Area'> | undefined

  onMount(() => {
    chart = createChart(container, {
      width: container.clientWidth,
      height: 400,
      layout: {
        background: { color: 'transparent' },
        textColor: '#555555',
        fontFamily: "'Space Mono', monospace",
        fontSize: 11,
      },
      grid: {
        vertLines: { color: '#1A1A1A' },
        horzLines: { color: '#1A1A1A' },
      },
      crosshair: {
        vertLine: { color: '#3F3F46' },
        horzLine: { color: '#3F3F46' },
      },
      rightPriceScale: { borderColor: '#3F3F46' },
      timeScale: { borderColor: '#3F3F46' },
    })

    equityLine = chart.addSeries(LineSeries, {
      color: '#00FF41',
      lineWidth: 2,
      priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
    })

    drawdownArea = chart.addSeries(AreaSeries, {
      topColor: 'rgba(255, 0, 60, 0)',
      bottomColor: 'rgba(255, 0, 60, 0.15)',
      lineColor: 'rgba(255, 0, 60, 0.3)',
      lineWidth: 1,
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
        <div class="flex items-center justify-center" style={{ "min-height": "400px" }}>
          <div class="font-mono text-xs text-text-tertiary animate-pulse">LOADING...</div>
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
