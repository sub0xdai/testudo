import { onMount, onCleanup, createEffect } from 'solid-js'
import { createChart, type IChartApi, type ISeriesApi, LineSeries, AreaSeries } from 'lightweight-charts'
import { ChartContainer } from './ChartContainer'
import type { EquityPoint } from '../../api/client'

export function EquityCurve(props: {
  data?: { data: EquityPoint[] }
  loading: boolean
  error?: string
}) {

  let container!: HTMLDivElement
  let chart: IChartApi | undefined
  let equityLine: ISeriesApi<'Line'> | undefined
  let drawdownArea: ISeriesApi<'Area'> | undefined

  onMount(() => {
    chart = createChart(container, {
      width: container.clientWidth,
      height: 300,
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
