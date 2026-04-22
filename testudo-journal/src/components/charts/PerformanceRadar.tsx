import { createMemo, createResource, Show } from 'solid-js'
import { EChart } from './EChart'
import { HelpTip } from '../HelpTip'
import { HELP } from '../../lib/help-content'
import { useAuth } from '../../context/AuthContext'
import { fetchDignitasMe } from '../../api/client'
import { getAccentPrimary, accentPrimaryAlpha, getTextTertiary, getBorder } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

export function PerformanceRadar() {
  const auth = useAuth()
  const [data] = createResource(
    () => auth.isAuthenticated() || undefined,
    () => fetchDignitasMe(),
  )

  const option = createMemo((): EChartsOption => {
    const d = data()
    const accent = getAccentPrimary()
    const accentFill = accentPrimaryAlpha(0.25)
    const tertiary = getTextTertiary()
    const border = getBorder()

    const emptyIndicators = [
      { name: 'Drawdown', max: 100 },
      { name: 'Sizing', max: 100 },
      { name: 'Setup', max: 100 },
      { name: 'Coach', max: 100 },
      { name: 'Journal', max: 100 },
    ]

    const baseRadar = {
      shape: 'circle' as const,
      splitNumber: 4,
      center: ['50%', '52%'],
      radius: '55%',
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
    }

    if (!d) {
      return {
        radar: { ...baseRadar, indicator: emptyIndicators },
        series: [{
          type: 'radar',
          data: [{ value: [0, 0, 0, 0, 0] }],
          lineStyle: { color: accent, width: 2 },
          areaStyle: { color: accentFill },
          itemStyle: { color: accent },
          symbol: 'circle',
          symbolSize: 4,
        }],
      }
    }

    const c = d.contributions
    const coachAlignment = (1 - parseFloat(c.coach_severity_penalty)) * 100
    const values = [
      parseFloat(c.drawdown_adherence) * 100,
      parseFloat(c.risk_per_trade_consistency) * 100,
      parseFloat(c.setup_adherence) * 100,
      coachAlignment,
      parseFloat(c.journal_consistency) * 100,
    ]

    const indicator = [
      { name: 'Drawdown', max: 100 },
      { name: 'Sizing', max: 100 },
      { name: 'Setup', max: 100 },
      {
        name: d.cold_start ? 'Coach (—)' : 'Coach',
        max: 100,
        ...(d.cold_start ? { color: tertiary } : {}),
      },
      { name: 'Journal', max: 100 },
    ]

    return {
      radar: { ...baseRadar, indicator },
      series: [{
        type: 'radar',
        data: [{ value: values, name: 'Dignitas' }],
        lineStyle: { color: accent, width: 2 },
        areaStyle: { color: accentFill },
        itemStyle: { color: accent },
        symbol: 'circle',
        symbolSize: 4,
      }],
    }
  })

  const score = createMemo(() => {
    const d = data()
    return d ? parseFloat(d.score) : null
  })

  const scoreColor = createMemo(() => {
    const s = score()
    if (s === null) return 'text-text-tertiary'
    if (s >= 60) return 'text-signal-green'
    if (s >= 30) return 'text-signal-amber'
    return 'text-signal-red'
  })

  return (
    <div class="bg-elevated">
      <div class="font-display text-xs font-bold tracking-section text-text-secondary uppercase px-8 py-5 border-b border-container-border">
        DIGNITAS <HelpTip text={HELP['radar.dignitas']} position="below" />
      </div>
      <EChart option={option} height="240px" />
      <div class="px-8 pb-5 flex items-center justify-between">
        <span class="font-display text-xs text-text-secondary">Score</span>
        <Show
          when={score() !== null}
          fallback={<span class="font-mono text-2xl font-bold text-text-tertiary">—</span>}
        >
          <span class={`font-mono text-2xl font-bold ${scoreColor()}`}>
            {score()!.toFixed(1)}
          </span>
        </Show>
      </div>
    </div>
  )
}
