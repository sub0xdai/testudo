import { createResource, onMount, onCleanup, createEffect } from 'solid-js'
import { Chart, ArcElement, Tooltip, Legend, DoughnutController } from 'chart.js'
import { ChartContainer } from './ChartContainer'
import { useFilters } from '../filterContext'
import { fetchSymbolBreakdown } from '../../api/client'
import { TAG_PALETTE, CHART_BG } from '../../lib/tokens'

Chart.register(ArcElement, Tooltip, Legend, DoughnutController)

export function SymbolDonut() {
  const { filters } = useFilters()
  const [data] = createResource(filters, fetchSymbolBreakdown)

  let canvas!: HTMLCanvasElement
  let chart: Chart<'doughnut'> | undefined

  onMount(() => {
    chart = new Chart(canvas, {
      type: 'doughnut',
      data: { labels: [], datasets: [{ data: [], backgroundColor: TAG_PALETTE, borderWidth: 0 }] },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        cutout: '60%',
        plugins: {
          legend: {
            position: 'right',
            labels: { color: '#888888', font: { family: "'Space Mono', monospace", size: 11 }, padding: 12 },
          },
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

    chart.data.labels = d.data.map((s) => s.symbol)
    chart.data.datasets[0].data = d.data.map((s) => s.trade_count)
    chart.update()
  })

  return (
    <ChartContainer title="SYMBOL DISTRIBUTION" loading={data.loading} empty={!data()?.data?.length}>
      <div class="h-56"><canvas ref={canvas!} /></div>
    </ChartContainer>
  )
}
