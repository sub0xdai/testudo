import { createResource, onMount, onCleanup, createEffect } from 'solid-js'
import { Chart, PointElement, LinearScale, Tooltip, ScatterController } from 'chart.js'
import { ChartContainer } from './ChartContainer'
import { useFilters } from '../filterContext'
import { fetchDurationProfit } from '../../api/client'
import { CHART_BG, signalGreenAlpha, signalRedAlpha } from '../../lib/tokens'

Chart.register(PointElement, LinearScale, Tooltip, ScatterController)

export function DurationScatter() {
  const { filters } = useFilters()
  const [data] = createResource(filters, fetchDurationProfit)

  let canvas!: HTMLCanvasElement
  let chart: Chart<'scatter'> | undefined

  onMount(() => {
    chart = new Chart(canvas, {
      type: 'scatter',
      data: { datasets: [] },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        scales: {
          x: {
            title: { display: true, text: 'DURATION (HRS)', color: '#555555', font: { family: "'Space Mono', monospace", size: 10 } },
            grid: { color: '#1A1A1A' },
            ticks: { color: '#555555', font: { family: "'Space Mono', monospace", size: 11 } },
            border: { color: '#3F3F46' },
          },
          y: {
            title: { display: true, text: 'P&L ($)', color: '#555555', font: { family: "'Space Mono', monospace", size: 10 } },
            grid: { color: '#1A1A1A' },
            ticks: { color: '#555555', font: { family: "'Space Mono', monospace", size: 11 } },
            border: { color: '#3F3F46' },
          },
        },
        plugins: {
          legend: { display: false },
          tooltip: {
            backgroundColor: CHART_BG,
            borderColor: '#3F3F46',
            borderWidth: 1,
            titleFont: { family: "'Space Mono', monospace" },
            bodyFont: { family: "'Space Mono', monospace" },
            titleColor: '#FFFFFF',
            bodyColor: '#888888',
            callbacks: {
              label: (ctx) => {
                const raw = ctx.raw as { x: number; y: number; symbol: string }
                return `${raw.symbol}: ${raw.x.toFixed(1)}h / $${raw.y.toFixed(2)}`
              },
            },
          },
        },
      },
    })

    onCleanup(() => chart?.destroy())
  })

  createEffect(() => {
    const d = data()
    if (!d?.data?.length || !chart) return

    const points = d.data.map((p) => ({
      x: p.duration_secs / 3600,
      y: parseFloat(p.pnl),
      symbol: p.symbol,
    }))

    chart.data.datasets = [{
      data: points,
      backgroundColor: points.map((p) => (p.y >= 0 ? signalGreenAlpha(0.6) : signalRedAlpha(0.6))),
      pointRadius: 5,
      pointHoverRadius: 7,
    }]
    chart.update()
  })

  return (
    <ChartContainer title="DURATION / PROFIT" loading={data.loading} empty={!data()?.data?.length}>
      <div class="h-56"><canvas ref={canvas!} /></div>
    </ChartContainer>
  )
}
