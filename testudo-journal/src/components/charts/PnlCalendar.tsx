import { createResource, createSignal, createMemo, Show } from 'solid-js'
import { EChart } from './EChart'
import { useFilters } from '../filterContext'
import { fetchDailyPnl } from '../../api/client'
import type { StatsFilter } from '../../api/client'
import { getSignalGreen, getSignalRed, getTextTertiary, signalGreenAlpha, signalRedAlpha } from '../../lib/tokens'
import type { EChartsOption } from 'echarts'

export function PnlCalendar() {
  const { filters } = useFilters()
  const [viewMonth, setViewMonth] = createSignal(new Date())

  // Derive a filter scoped to the viewed month, preserving exchange/symbol from global filters
  const monthFilter = createMemo((): StatsFilter => {
    const f = filters()
    const m = viewMonth()
    const year = m.getFullYear()
    const month = m.getMonth()
    const firstDay = new Date(year, month, 1)
    const lastDay = new Date(year, month + 1, 0)
    return {
      exchange: f.exchange,
      symbol: f.symbol,
      dateFrom: fmt(firstDay),
      dateTo: fmt(lastDay),
    }
  })

  const [data] = createResource(monthFilter, fetchDailyPnl)

  const option = createMemo((): EChartsOption | undefined => {
    const d = data()
    if (!d?.data) return undefined

    const m = viewMonth()
    const rangeStr = `${m.getFullYear()}-${String(m.getMonth() + 1).padStart(2, '0')}`

    const green = getSignalGreen()
    const red = getSignalRed()
    const tertiary = getTextTertiary()

    // Build data arrays
    const calendarData: [string, number][] = []
    const tradeCountMap = new Map<string, number>()

    for (const p of d.data) {
      const pnl = parseFloat(p.pnl)
      calendarData.push([p.date, pnl])
      tradeCountMap.set(p.date, p.trade_count)
    }

    // Compute max absolute P&L for visualMap scaling
    const maxAbsPnl = calendarData.reduce((max, [, v]) => Math.max(max, Math.abs(v)), 1)

    return {
      tooltip: {
        trigger: 'item',
        formatter: (params: any) => {
          const [date, pnl] = params.value as [string, number]
          const count = tradeCountMap.get(date) || 0
          const sign = pnl >= 0 ? '+' : ''
          return `<span style="color:#fff">${date}</span><br/>${sign}$${pnl.toFixed(2)}<br/>${count} trade${count !== 1 ? 's' : ''}`
        },
      },
      calendar: {
        range: rangeStr,
        cellSize: ['auto', 55],
        orient: 'horizontal',
        left: 30,
        right: 10,
        top: 30,
        bottom: 5,
        splitLine: {
          show: true,
          lineStyle: { color: 'rgba(255,255,255,0.06)', width: 1 },
        },
        itemStyle: {
          borderWidth: 1,
          borderColor: 'rgba(255,255,255,0.04)',
          color: 'transparent',
        },
        dayLabel: {
          show: true,
          firstDay: 0,
          nameMap: ['S', 'M', 'T', 'W', 'T', 'F', 'S'],
          color: tertiary,
          fontSize: 10,
          fontFamily: "'Space Mono', monospace",
        },
        monthLabel: { show: false },
        yearLabel: { show: false },
      },
      visualMap: {
        show: false,
        min: -maxAbsPnl,
        max: maxAbsPnl,
        inRange: {
          color: [red, 'transparent', green],
        },
        type: 'continuous',
      },
      series: [{
        type: 'heatmap',
        coordinateSystem: 'calendar',
        data: calendarData,
        label: {
          show: true,
          formatter: (params: any) => {
            const [date, pnl] = params.value as [string, number]
            const count = tradeCountMap.get(date) || 0
            if (count === 0) return ''
            const sign = pnl >= 0 ? '+' : ''
            return `{pnl|${sign}$${Math.abs(pnl).toFixed(0)}}\n{count|${count} trade${count !== 1 ? 's' : ''}}`
          },
          rich: {
            pnl: {
              fontSize: 11,
              fontFamily: "'Space Mono', monospace",
              fontWeight: 'bold' as const,
              lineHeight: 18,
              color: '#fff',
            },
            count: {
              fontSize: 9,
              fontFamily: "'Space Mono', monospace",
              color: tertiary,
              lineHeight: 14,
            },
          },
        },
        emphasis: {
          itemStyle: {
            borderColor: 'rgba(255,255,255,0.3)',
            borderWidth: 2,
          },
        },
      }],
    }
  })

  const isEmpty = createMemo(() => {
    const d = data()
    return !d?.data?.length
  })

  const monthLabel = createMemo(() => {
    const m = viewMonth()
    return m.toLocaleDateString('en-US', { month: 'long', year: 'numeric' })
  })

  function prevMonth() {
    setViewMonth((prev) => {
      const d = new Date(prev)
      d.setMonth(d.getMonth() - 1)
      return d
    })
  }

  function nextMonth() {
    setViewMonth((prev) => {
      const d = new Date(prev)
      d.setMonth(d.getMonth() + 1)
      return d
    })
  }

  function thisMonth() {
    setViewMonth(new Date())
  }

  return (
    <div class="border-b border-container-border/50">
      {/* Month navigation */}
      <div class="flex items-center gap-3 px-8 py-3">
        <button
          onClick={prevMonth}
          aria-label="Previous month"
          class="font-mono text-sm text-text-secondary hover:text-text-primary transition-colors px-2 py-0.5"
        >
          &larr;
        </button>
        <span class="font-mono text-sm text-text-primary tracking-wider min-w-[160px] text-center">
          {monthLabel()}
        </span>
        <button
          onClick={nextMonth}
          aria-label="Next month"
          class="font-mono text-sm text-text-secondary hover:text-text-primary transition-colors px-2 py-0.5"
        >
          &rarr;
        </button>
        <button
          onClick={thisMonth}
          class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors border border-container-border/50 px-2 py-0.5 ml-2"
        >
          This month
        </button>
      </div>

      {/* Calendar chart or empty state */}
      <Show when={!data.loading && isEmpty()}>
        <div class="flex items-center justify-center" style={{ "min-height": "320px" }}>
          <span class="font-mono text-xs text-text-tertiary">No trades this month</span>
        </div>
      </Show>

      <Show when={data.loading && !data()}>
        <div class="flex items-center justify-center" style={{ "min-height": "320px" }}>
          <div class="w-4 h-4 border-2 border-text-secondary border-t-text-primary rounded-full animate-spin" />
        </div>
      </Show>

      <Show when={!isEmpty() || data()}>
        <EChart option={option} height="320px" />
      </Show>
    </div>
  )
}

/** Format a Date as YYYY-MM-DD */
function fmt(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}
