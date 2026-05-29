/** @anchor ui:journal:SetupBreakdown
 * @tags ui */

import { createResource, createMemo, createSignal, Show, For } from 'solid-js'
import { ChartContainer } from './ChartContainer'
import { EChart } from './EChart'
import { useFilters } from '../filterContext'
import { fetchSetupBreakdown, type SetupBreakdownItem } from '../../api/client'
import { getTagPalette, getTextTertiary } from '../../lib/tokens'
import { formatCurrency, formatPercent, pnlColor, rColor } from '../../lib/formatters'
import type { EChartsOption } from 'echarts'

type SortKey = 'trade_count' | 'win_rate' | 'avg_r_multiple' | 'expectancy'
type SortDir = 'asc' | 'desc'

const COLUMNS: { key: SortKey; label: string }[] = [
  { key: 'trade_count', label: 'Trades' },
  { key: 'win_rate', label: 'Win%' },
  { key: 'avg_r_multiple', label: 'Avg R' },
  { key: 'expectancy', label: 'Exp' },
]

function metricValue(row: SetupBreakdownItem, key: SortKey): number {
  switch (key) {
    case 'trade_count': return row.trade_count
    case 'win_rate': return parseFloat(row.win_rate) || 0
    case 'avg_r_multiple': return row.avg_r_multiple !== null ? parseFloat(row.avg_r_multiple) : 0
    case 'expectancy': return parseFloat(row.expectancy) || 0
  }
}

function metricLabel(row: SetupBreakdownItem, key: SortKey): string {
  switch (key) {
    case 'trade_count': return String(row.trade_count)
    case 'win_rate': return formatPercent(row.win_rate)
    case 'avg_r_multiple': return row.avg_r_multiple !== null ? `${parseFloat(row.avg_r_multiple).toFixed(2)}R` : '—'
    case 'expectancy': return formatCurrency(row.expectancy)
  }
}

export function SetupBreakdown() {
  const { filters, setFilters } = useFilters()
  const [data, { refetch }] = createResource(filters, fetchSetupBreakdown)
  const hasActiveFilters = () => Object.values(filters()).some(Boolean)

  const [sortKey, setSortKey] = createSignal<SortKey>('expectancy')
  const [sortDir, setSortDir] = createSignal<SortDir>('desc')

  const sorted = createMemo<SetupBreakdownItem[]>(() => {
    const rows = data()?.data ?? []
    const key = sortKey()
    const sign = sortDir() === 'asc' ? 1 : -1
    return [...rows].sort((a, b) => {
      const aNull = key === 'avg_r_multiple' && a.avg_r_multiple === null
      const bNull = key === 'avg_r_multiple' && b.avg_r_multiple === null
      if (aNull && bNull) return 0
      if (aNull) return 1
      if (bNull) return -1
      return (metricValue(a, key) - metricValue(b, key)) * sign
    })
  })

  function toggleSort(next: SortKey) {
    if (sortKey() === next) {
      setSortDir(d => (d === 'asc' ? 'desc' : 'asc'))
    } else {
      setSortKey(next)
      setSortDir('desc')
    }
  }

  const option = createMemo((): EChartsOption | undefined => {
    const rows = sorted()
    if (!rows.length) return undefined

    const palette = getTagPalette()
    const textColor = getTextTertiary()
    const key = sortKey()

    return {
      tooltip: {
        trigger: 'axis',
        axisPointer: { type: 'shadow' },
        formatter: (params: any) => {
          const p = Array.isArray(params) ? params[0] : params
          const row = rows[p.dataIndex]
          if (!row) return ''
          return [
            `<span style="color:#fff">${row.setup_tag}</span>`,
            `Trades: ${row.trade_count}`,
            `Win: ${formatPercent(row.win_rate)}`,
            `Avg R: ${row.avg_r_multiple !== null ? `${parseFloat(row.avg_r_multiple).toFixed(2)}R` : '—'}`,
            `Expectancy: ${formatCurrency(row.expectancy)}`,
          ].join('<br/>')
        },
      },
      grid: { left: 120, right: 60, top: 10, bottom: 10, containLabel: false },
      yAxis: {
        type: 'category',
        data: rows.map(r => r.setup_tag),
        axisLabel: {
          fontFamily: "'Space Mono', monospace",
          fontSize: 11,
          color: textColor,
        },
        axisTick: { show: false },
        axisLine: { show: false },
        inverse: true,
      },
      xAxis: {
        type: 'value',
        axisLabel: { show: false },
        splitLine: { show: false },
        axisTick: { show: false },
        axisLine: { show: false },
      },
      series: [{
        type: 'bar',
        data: rows.map((r, i) => ({
          value: metricValue(r, key),
          itemStyle: { color: palette[i % palette.length], opacity: 0.7 },
        })),
        barWidth: '40%',
        label: {
          show: true,
          position: 'right',
          formatter: (params: any) => metricLabel(rows[params.dataIndex], key),
          fontFamily: "'Space Mono', monospace",
          fontSize: 11,
          color: textColor,
        },
      }],
    }
  })

  return (
    <ChartContainer
      title="SETUP BREAKDOWN"
      loading={data.loading}
      empty={!data()?.data?.length}
      onRetry={refetch}
      hasActiveFilters={hasActiveFilters()}
      onClearFilters={() => setFilters({})}
    >
      <div class="flex flex-col gap-3">
        <EChart option={option} />

        <div class="border-t border-container-border/50 pt-3">
          <div class="grid grid-cols-[1fr_auto_auto_auto_auto] gap-x-4 gap-y-1 text-xs font-mono">
            <div class="text-text-tertiary uppercase tracking-section">Setup</div>
            <For each={COLUMNS}>
              {(col) => (
                <button
                  type="button"
                  class="text-right uppercase tracking-section hover:text-text-primary transition-colors"
                  classList={{
                    'text-text-primary': sortKey() === col.key,
                    'text-text-tertiary': sortKey() !== col.key,
                  }}
                  onClick={() => toggleSort(col.key)}
                  aria-label={`Sort by ${col.label}`}
                >
                  {col.label}
                  <Show when={sortKey() === col.key}>
                    <span class="ml-1">{sortDir() === 'desc' ? '↓' : '↑'}</span>
                  </Show>
                </button>
              )}
            </For>

            <For each={sorted()}>
              {(row) => (
                <>
                  <div class="text-text-primary truncate" title={row.setup_tag}>{row.setup_tag}</div>
                  <div class="text-right text-text-secondary">{row.trade_count}</div>
                  <div class="text-right text-text-secondary">{formatPercent(row.win_rate)}</div>
                  <div class={`text-right ${rColor(row.avg_r_multiple)}`}>
                    {row.avg_r_multiple !== null ? `${parseFloat(row.avg_r_multiple).toFixed(2)}R` : '—'}
                  </div>
                  <div class={`text-right ${pnlColor(row.expectancy)}`}>
                    {formatCurrency(row.expectancy)}
                  </div>
                </>
              )}
            </For>
          </div>
        </div>
      </div>
    </ChartContainer>
  )
}
