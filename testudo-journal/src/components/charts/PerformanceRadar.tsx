import { createMemo } from 'solid-js'
import { EChart } from './EChart'
import { HelpTip } from '../HelpTip'
import { HELP } from '../../lib/help-content'
import type { PerformanceStats, RiskStats } from '../../api/client'
import { getAccentPrimary, accentPrimaryAlpha, getTextTertiary, getBorder } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

interface PerformanceRadarProps {
  performance: PerformanceStats
  risk: RiskStats
}

/** Normalize each axis to 0-100 for fair radar comparison */
function normalizeAxes(p: PerformanceStats, r: RiskStats) {
  const winRate = clamp(parseFloat(p.win_rate))
  const profitFactor = clamp(parseFloat(p.profit_factor) * 20)           // 5.0 PF = 100
  const avgWinLoss = clamp(avgWinLossRatio(p) * 20)                      // 5:1 ratio = 100
  const maxDD = clamp(100 - parseFloat(r.max_drawdown_pct))              // INVERTED: low DD = high
  const avgR = clamp(parseFloat(p.avg_r_multiple) * 25)                  // 4.0R = 100
  const tradesPerDay = clamp(parseFloat(p.trades_per_day) * 20)          // 5 trades/day = 100

  return { winRate, profitFactor, avgWinLoss, maxDD, avgR, tradesPerDay }
}

/** Composite Dignitas Score: weighted average of all 6 axes */
function computeDignitasScore(p: PerformanceStats, r: RiskStats): number {
  const n = normalizeAxes(p, r)
  // Weights: PF and DD matter most, then win/loss, then rest
  const weighted =
    n.winRate * 0.15 +
    n.profitFactor * 0.25 +
    n.avgWinLoss * 0.20 +
    n.maxDD * 0.20 +
    n.avgR * 0.10 +
    n.tradesPerDay * 0.10
  return clamp(weighted)
}

function avgWinLossRatio(p: PerformanceStats): number {
  const avgWin = Math.abs(parseFloat(p.avg_win))
  const avgLoss = Math.abs(parseFloat(p.avg_loss))
  if (avgLoss === 0) return avgWin > 0 ? 5 : 0
  return avgWin / avgLoss
}

export function PerformanceRadar(props: PerformanceRadarProps) {
  const option = createMemo((): EChartsOption => {
    const p = props.performance
    const r = props.risk

    const accent = getAccentPrimary()
    const accentFill = accentPrimaryAlpha(0.25)
    const tertiary = getTextTertiary()
    const border = getBorder()

    const n = normalizeAxes(p, r)

    return {
      radar: {
        shape: 'circle',
        splitNumber: 4,
        center: ['50%', '52%'],
        radius: '55%',
        indicator: [
          { name: 'Win Rate', max: 100 },
          { name: 'Profit Factor', max: 100 },
          { name: 'Avg W/L', max: 100 },
          { name: 'Max DD', max: 100 },
          { name: 'Avg R', max: 100 },
          { name: 'Activity', max: 100 },
        ],
        name: {
          textStyle: {
            color: tertiary,
            fontSize: 10,
            fontFamily: "'Space Mono', monospace",
          },
        },
        axisLine: { lineStyle: { color: border } },
        splitLine: { lineStyle: { color: border } },
        splitArea: { show: false },
      },
      series: [{
        type: 'radar',
        data: [{
          value: [n.winRate, n.profitFactor, n.avgWinLoss, n.maxDD, n.avgR, n.tradesPerDay],
          name: 'Dignitas',
        }],
        lineStyle: { color: accent, width: 2 },
        areaStyle: { color: accentFill },
        itemStyle: { color: accent },
        symbol: 'circle',
        symbolSize: 4,
      }],
    }
  })

  const score = createMemo(() => computeDignitasScore(props.performance, props.risk))

  // Score color: green > 60, amber 30-60, red < 30
  const scoreColor = createMemo(() => {
    const s = score()
    if (s >= 60) return 'text-signal-green'
    if (s >= 30) return 'text-signal-amber'
    return 'text-signal-red'
  })

  return (
    <div class="bg-elevated">
      <div class="font-display text-xs font-bold tracking-section text-text-secondary uppercase px-8 py-5 border-b border-container-border relative z-10" style={{ overflow: 'visible' }}>
        DIGNITAS <HelpTip text={HELP['radar.dignitas']} position="below" />
      </div>
      <EChart option={option} height="240px" />
      {/* Composite score */}
      <div class="px-8 pb-5 flex items-center justify-between">
        <span class="font-display text-xs text-text-secondary">Score</span>
        <span class={`font-mono text-2xl font-bold ${scoreColor()}`}>
          {score().toFixed(1)}
        </span>
      </div>
    </div>
  )
}

function clamp(v: number): number {
  if (isNaN(v)) return 0
  return Math.min(Math.max(v, 0), 100)
}
