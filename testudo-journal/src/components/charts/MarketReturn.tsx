import { createResource, onMount, onCleanup, createEffect } from 'solid-js'
import { Chart, BarElement, CategoryScale, LinearScale, Tooltip, BarController } from 'chart.js'
import { ChartContainer } from './ChartContainer'
import { useFilters } from '../filterContext'
import { fetchSymbolBreakdown } from '../../api/client'
import { SIGNAL_GREEN, SIGNAL_RED, CHART_BG } from '../../lib/tokens'

Chart.register(BarElement, CategoryScale, LinearScale, Tooltip, BarController)

export function MarketReturn() {
  const { filters } = useFilters()
  const [data] = createResource(filters, fetchSymbolBreakdown)

  let canvas!: HTMLCanvasElement
  let chart: Chart<'bar'> | undefined

  onMount(() => {
    chart = new Chart(canvas, {
      type: 'bar',
      data: {
        labels: [],
        datasets: [{
          data: [],
          backgroundColor: [],
          borderWidth: 0,
          barThickness: 16,
        }],
      },
      options: {
        indexAxis: 'y',
        responsive: true,
        maintainAspectRatio: false,
        scales: {
          x: {
            grid: { color: '#1A1A1A' },
            ticks: { color: '#555555', font: { family: "'Space Mono', monospace", size: 11 } },
            border: { color: '#3F3F46' },
          },
          y: {
            grid: { display: false },
            ticks: { color: '#888888', font: { family: "'Space Mono', monospace", size: 11 } },
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
          },
        },
      },
    })

    onCleanup(() => chart?.destroy())
  })

  createEffect(() => {
    const d = data()
    if (!d?.data?.length || !chart) return

    const sorted = [...d.data].sort((a, b) => parseFloat(b.total_pnl) - parseFloat(a.total_pnl))
    chart.data.labels = sorted.map((s) => s.symbol)
    const values = sorted.map((s) => parseFloat(s.total_pnl))
    chart.data.datasets[0].data = values
    chart.data.datasets[0].backgroundColor = values.map((v) => (v >= 0 ? SIGNAL_GREEN : SIGNAL_RED))
    chart.update()
  })

  return (
    <ChartContainer title="MARKET RETURN" loading={data.loading} empty={!data()?.data?.length}>
      <div class="h-56"><canvas ref={canvas!} /></div>
    </ChartContainer>
  )
}
