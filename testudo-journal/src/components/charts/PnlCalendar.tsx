import { createResource, createSignal, createMemo, Show, For, Index } from 'solid-js'
import { useFilters } from '../filterContext'
import { fetchDailyPnl } from '../../api/client'
import type { StatsFilter, DailyPnlPoint } from '../../api/client'
import { pnlColor } from '../../lib/formatters'

const DAY_NAMES = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']

/** Format compact P&L: <1000 → $XXX, >=1000 → $X.XK */
function compactPnl(value: number): string {
  const abs = Math.abs(value)
  const sign = value > 0 ? '+' : value < 0 ? '-' : ''
  if (abs >= 1000) return `${sign}$${(abs / 1000).toFixed(1)}K`
  return `${sign}$${abs.toFixed(0)}`
}

interface CalendarCell {
  day: number           // day of month (1-31), 0 for padding cells
  inMonth: boolean      // true if this day belongs to the viewed month
  data?: DailyPnlPoint  // trade data for this day, if any
}

interface WeeklySummary {
  weekNum: number
  pnl: number
  tradingDays: number
}

export function PnlCalendar() {
  const { filters } = useFilters()
  const [viewMonth, setViewMonth] = createSignal(new Date())

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

  // Build a lookup map from date string → DailyPnlPoint
  const dataMap = createMemo(() => {
    const d = data()
    if (!d?.data) return new Map<string, DailyPnlPoint>()
    const map = new Map<string, DailyPnlPoint>()
    for (const p of d.data) map.set(p.date, p)
    return map
  })

  // Build grid cells for the month
  const cells = createMemo((): CalendarCell[] => {
    const m = viewMonth()
    const year = m.getFullYear()
    const month = m.getMonth()
    const firstDay = new Date(year, month, 1)
    const startDow = firstDay.getDay() // 0=Sun
    const daysInMonth = new Date(year, month + 1, 0).getDate()
    const totalCells = Math.ceil((startDow + daysInMonth) / 7) * 7
    const dm = dataMap()

    const result: CalendarCell[] = []
    for (let i = 0; i < totalCells; i++) {
      const dayOfMonth = i - startDow + 1
      if (dayOfMonth < 1 || dayOfMonth > daysInMonth) {
        // Padding cell (before or after month)
        result.push({ day: 0, inMonth: false })
      } else {
        const dateStr = fmt(new Date(year, month, dayOfMonth))
        result.push({
          day: dayOfMonth,
          inMonth: true,
          data: dm.get(dateStr),
        })
      }
    }
    return result
  })

  // Build weekly summaries
  const weeklySummaries = createMemo((): WeeklySummary[] => {
    const c = cells()
    const weeks: WeeklySummary[] = []
    for (let i = 0; i < c.length; i += 7) {
      const weekCells = c.slice(i, i + 7)
      let pnl = 0
      let tradingDays = 0
      for (const cell of weekCells) {
        if (cell.data) {
          pnl += parseFloat(cell.data.pnl)
          tradingDays++
        }
      }
      weeks.push({ weekNum: weeks.length + 1, pnl, tradingDays })
    }
    return weeks
  })

  // Monthly stats
  const monthlyStats = createMemo(() => {
    const d = data()
    if (!d?.data?.length) return null
    let totalPnl = 0
    let tradingDays = 0
    for (const p of d.data) {
      totalPnl += parseFloat(p.pnl)
      if (p.trade_count > 0) tradingDays++
    }
    return { totalPnl, tradingDays }
  })

  const isEmpty = createMemo(() => !data()?.data?.length)

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

  function cellBg(cell: CalendarCell): string {
    if (!cell.data) return ''
    const pnl = parseFloat(cell.data.pnl)
    if (pnl > 0) return 'bg-signal-green/10'
    if (pnl < 0) return 'bg-signal-red/10'
    return ''
  }

  return (
    <div class="border-b border-container-border/50">
      {/* Header: navigation + monthly stats */}
      <div class="flex items-center justify-between px-8 py-3">
        <div class="flex items-center gap-3">
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

        {/* Monthly stats badges */}
        <Show when={monthlyStats()}>
          {(stats) => (
            <div class="hidden sm:flex items-center gap-3">
              <span class={`font-mono text-xs font-bold ${pnlColor(stats().totalPnl)}`}>
                {compactPnl(stats().totalPnl)}
              </span>
              <span class="font-mono text-xs text-text-tertiary">
                {stats().tradingDays} day{stats().tradingDays !== 1 ? 's' : ''}
              </span>
            </div>
          )}
        </Show>
      </div>

      {/* Loading state */}
      <Show when={data.loading && !data()}>
        <div class="flex items-center justify-center" style={{ "min-height": "320px" }}>
          <div class="w-4 h-4 border-2 border-text-secondary border-t-text-primary rounded-full animate-spin" />
        </div>
      </Show>

      {/* Empty state */}
      <Show when={!data.loading && isEmpty()}>
        <div class="flex items-center justify-center" style={{ "min-height": "320px" }}>
          <span class="font-mono text-xs text-text-tertiary">No trades this month</span>
        </div>
      </Show>

      {/* Calendar grid */}
      <Show when={(!data.loading || data()) && !isEmpty()}>
        <div class="px-8 pb-6 overflow-x-auto">
          <div class="grid gap-px min-w-[600px]" style={{ "grid-template-columns": "repeat(7, 1fr) auto" }}>
            {/* Column headers: Sun-Sat + Weekly */}
            <For each={DAY_NAMES}>
              {(name) => (
                <div class="font-mono text-[10px] text-text-tertiary text-center py-2 uppercase tracking-wider">
                  {name}
                </div>
              )}
            </For>
            <div class="font-mono text-[10px] text-text-tertiary text-center py-2 uppercase tracking-wider w-24">
              Week
            </div>

            {/* Calendar cells + weekly summaries */}
            <Index each={cells()}>
              {(cell, i) => {
                const c = cell()
                return (
                  <>
                    <div
                      class={`border border-container-border/30 min-h-[72px] p-1.5 relative ${
                        c.inMonth ? cellBg(c) : 'opacity-30'
                      }`}
                    >
                      {/* Day number */}
                      <Show when={c.day > 0}>
                        <span class="absolute top-1 right-1.5 font-mono text-[10px] text-text-tertiary">
                          {c.day}
                        </span>
                      </Show>

                      {/* P&L data */}
                      <Show when={c.data}>
                        {(d) => {
                          const pnl = parseFloat(d().pnl)
                          return (
                            <div class="flex flex-col items-center justify-center h-full pt-3">
                              <span class={`font-mono text-sm font-bold ${pnlColor(pnl)}`}>
                                {compactPnl(pnl)}
                              </span>
                              <span class="font-mono text-[9px] text-text-tertiary mt-0.5">
                                {d().trade_count} trade{d().trade_count !== 1 ? 's' : ''}
                              </span>
                            </div>
                          )
                        }}
                      </Show>
                    </div>

                    {/* Weekly summary cell — after every 7th cell */}
                    <Show when={(i + 1) % 7 === 0}>
                      {(() => {
                        const weekIdx = Math.floor(i / 7)
                        const week = weeklySummaries()[weekIdx]
                        if (!week || week.tradingDays === 0) {
                          return <div class="w-24 min-h-[72px] border border-container-border/15" />
                        }
                        return (
                          <div class={`w-24 min-h-[72px] border border-container-border/15 flex flex-col items-center justify-center p-1.5 ${
                            week.pnl > 0 ? 'bg-signal-green/5' : week.pnl < 0 ? 'bg-signal-red/5' : ''
                          }`}>
                            <span class="font-mono text-[9px] text-text-tertiary">Wk {week.weekNum}</span>
                            <span class={`font-mono text-xs font-bold ${pnlColor(week.pnl)}`}>
                              {compactPnl(week.pnl)}
                            </span>
                            <span class="font-mono text-[9px] text-text-tertiary">
                              {week.tradingDays}d
                            </span>
                          </div>
                        )
                      })()}
                    </Show>
                  </>
                )
              }}
            </Index>
          </div>
        </div>
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
