import { createMemo } from 'solid-js'
import { EChart } from './EChart'
import type { PerformanceStats, RiskStats } from '../../api/client'
import { getAccentPrimary, accentPrimaryAlpha, getTextTertiary } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

interface PerformanceRadarProps {
  performance: PerformanceStats
  risk: RiskStats
}

export function PerformanceRadar(props: PerformanceRadarProps) {
  const option = createMemo((): EChartsOption => {
    const p = props.performance
    const r = props.risk

    const accent = getAccentPrimary()
    const accentFill = accentPrimaryAlpha(0.15)
    const tertiary = getTextTertiary()

    // Normalize all values to 0-100 scale
    const winRate = clamp(parseFloat(p.win_rate))
    const profitFactor = clamp(parseFloat(p.profit_factor) * 20)      // 5.0 = 100
    const avgR = clamp(parseFloat(p.avg_r_multiple) * 25)             // 4.0R = 100
    const maxDD = clamp(100 - parseFloat(r.max_drawdown_pct))         // INVERTED: low DD = high
    const consistency = clamp((winRate * parseFloat(p.profit_factor)) / 2)
    const recovery = clamp(
      (r.best_streak / (Math.abs(r.worst_streak) + 1)) * 20
    )

    return {
      radar: {
        shape: 'circle',
        splitNumber: 4,
        center: ['50%', '55%'],
        radius: '65%',
        indicator: [
          { name: 'Win Rate', max: 100 },
          { name: 'Profit Factor', max: 100 },
          { name: 'Consistency', max: 100 },
          { name: 'Max DD', max: 100 },
          { name: 'Avg R', max: 100 },
          { name: 'Recovery', max: 100 },
        ],
        name: {
          textStyle: {
            color: tertiary,
            fontSize: 10,
            fontFamily: "'Space Mono', monospace",
          },
        },
        axisLine: { lineStyle: { color: 'rgba(255,255,255,0.1)' } },
        splitLine: { lineStyle: { color: 'rgba(255,255,255,0.08)' } },
        splitArea: { show: false },
      },
      series: [{
        type: 'radar',
        data: [{
          value: [winRate, profitFactor, consistency, maxDD, avgR, recovery],
          name: 'Performance',
        }],
        lineStyle: { color: accent, width: 2 },
        areaStyle: { color: accentFill },
        itemStyle: { color: accent },
        symbol: 'circle',
        symbolSize: 4,
      }],
    }
  })

  return (
    <div>
      <div class="font-display text-xs font-bold tracking-section text-text-secondary uppercase px-6 py-4 border-b border-container-border/50">
        PERFORMANCE PROFILE
      </div>
      <EChart option={option} height="260px" />
    </div>
  )
}

function clamp(v: number): number {
  if (isNaN(v)) return 0
  return Math.min(Math.max(v, 0), 100)
}
