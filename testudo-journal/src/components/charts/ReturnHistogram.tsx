import { createResource, onMount, onCleanup, createEffect } from 'solid-js'
import { Chart, BarElement, CategoryScale, LinearScale, Tooltip, BarController } from 'chart.js'
import { ChartContainer } from './ChartContainer'
import { useFilters } from '../filterContext'
import { fetchReturnDistribution } from '../../api/client'
import { SIGNAL_GREEN, SIGNAL_RED, CHART_BG } from '../../lib/tokens'

Chart.register(BarElement, CategoryScale, LinearScale, Tooltip, BarController)

export function ReturnHistogram() {
  const { filters, setFilters } = useFilters()
  const [data, { refetch }] = createResource(filters, fetchReturnDistribution)
  const hasActiveFilters = () => Object.values(filters()).some(Boolean)

  let canvas!: HTMLCanvasElement
  let chart: Chart<'bar'> | undefined

  onMount(() => {
    chart = new Chart(canvas, {
      type: 'bar',
      data: { labels: [], datasets: [{ data: [], backgroundColor: [], borderWidth: 0 }] },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        scales: {
          x: {
            grid: { display: false },
            ticks: { color: '#555555', font: { family: "'Space Mono', monospace", size: 11 } },
            border: { color: '#3F3F46' },
          },
          y: {
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
          },
        },
      },
    })

    onCleanup(() => chart?.destroy())
  })

  createEffect(() => {
    const d = data()
    if (!d?.data?.length || !chart) return

    chart.data.labels = d.data.map((b) => b.bucket)
    chart.data.datasets[0].data = d.data.map((b) => b.count)
    chart.data.datasets[0].backgroundColor = d.data.map((b) => {
      const num = parseFloat(b.bucket)
      return isNaN(num) || num >= 0 ? SIGNAL_GREEN : SIGNAL_RED
    })
    chart.update()
  })

  return (
    <ChartContainer title="RETURN DISTRIBUTION" loading={data.loading} empty={!data()?.data?.length} onRetry={refetch} hasActiveFilters={hasActiveFilters()} onClearFilters={() => setFilters({})}>
      <div class="h-56"><canvas ref={canvas!} /></div>
    </ChartContainer>
  )
}
