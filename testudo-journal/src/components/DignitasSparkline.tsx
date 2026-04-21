import { createMemo, Show } from 'solid-js'
import { EChart } from './charts/EChart'
import { getAccentPrimary, accentPrimaryAlpha } from '../lib/tokens'
import type { EChartsOption } from 'echarts'

interface SparklinePoint {
  date: string
  score: string
}

interface Props {
  snapshots: SparklinePoint[]
}

export function DignitasSparkline(props: Props) {
  const option = createMemo((): EChartsOption => {
    const accent = getAccentPrimary()
    const fill = accentPrimaryAlpha(0.1)

    const data = props.snapshots.map((s) => [s.date, parseFloat(s.score)])

    return {
      animation: false,
      grid: { left: 0, right: 0, top: 4, bottom: 0, containLabel: false },
      xAxis: { type: 'time', show: false },
      yAxis: { type: 'value', min: 0, max: 100, show: false },
      series: [
        {
          type: 'line',
          data,
          symbol: 'none',
          lineStyle: { color: accent, width: 1.5 },
          areaStyle: { color: fill },
          smooth: 0.3,
        },
      ],
    }
  })

  return (
    <Show
      when={props.snapshots.length > 0}
      fallback={
        <div class="flex items-center justify-center h-[80px] font-mono text-[10px] text-text-tertiary">
          NO HISTORY
        </div>
      }
    >
      <EChart option={option} height="80px" />
    </Show>
  )
}
