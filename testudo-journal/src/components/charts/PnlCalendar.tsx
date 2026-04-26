import { createSignal, createMemo, Show, For } from 'solid-js'
import { useNavigate } from '@solidjs/router'
import { useFilters } from '../filterContext'
import { fetchDailyPnl } from '../../api/client'
import type { StatsFilter, DailyPnlPoint } from '../../api/client'
import { useCachedResource, cacheKeyForSection } from '../../lib/cache'
import { pnlColor } from '../../lib/formatters'
import { useAuth } from '../../context/AuthContext'

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
  dateStr: string       // YYYY-MM-DD for navigation
  inMonth: boolean      // true if this day belongs to the viewed month
  isToday: boolean      // true if this is today's date
  data?: DailyPnlPoint  // trade data for this day, if any
  pnlValue: number      // parsed P&L (0 if no data)
}

interface WeeklySummary {
  weekNum: number
  pnl: number
  tradingDays: number
}

export function PnlCalendar() {
  const { filters, setFilters } = useFilters()
  const navigate = useNavigate()
  const auth = useAuth()
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

  const data = useCachedResource(
    () => cacheKeyForSection('daily_pnl', monthFilter()),
    () => fetchDailyPnl(monthFilter()),
    { staleMs: 30_000, persist: true, identity: auth.user()?.id ?? null },
  )

  // Build a lookup map from date string → DailyPnlPoint
  const dataMap = createMemo(() => {
    const d = data()
    if (!d?.data) return new Map<string, DailyPnlPoint>()
    const map = new Map<string, DailyPnlPoint>()
    for (const p of d.data) map.set(p.date, p)
    return map
  })

  const todayStr = fmt(new Date())

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
        result.push({ day: 0, dateStr: '', inMonth: false, isToday: false, pnlValue: 0 })
      } else {
        const dateStr = fmt(new Date(year, month, dayOfMonth))
        const point = dm.get(dateStr)
        result.push({
          day: dayOfMonth,
          dateStr,
          inMonth: true,
          isToday: dateStr === todayStr,
          data: point,
          pnlValue: point ? parseFloat(point.pnl) : 0,
        })
      }
    }
    return result
  })

  // Compute magnitude thresholds for graduated opacity (percentile-based)
  const intensityThresholds = createMemo(() => {
    const allPnl = cells()
      .filter((c) => c.data)
      .map((c) => Math.abs(c.pnlValue))
      .sort((a, b) => a - b)

    if (allPnl.length === 0) return { p25: 0, p50: 0, p75: 0 }
    const p = (pct: number) => allPnl[Math.min(Math.floor(pct * allPnl.length), allPnl.length - 1)]
    return { p25: p(0.25), p50: p(0.5), p75: p(0.75) }
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
          pnl += cell.pnlValue
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

  // Can't go past the current month
  const isCurrentMonth = createMemo(() => {
    const m = viewMonth()
    const now = new Date()
    return m.getFullYear() === now.getFullYear() && m.getMonth() === now.getMonth()
  })

  function prevMonth() {
    setViewMonth((prev) => {
      const d = new Date(prev)
      d.setMonth(d.getMonth() - 1)
      return d
    })
  }

  function nextMonth() {
    if (isCurrentMonth()) return
    setViewMonth((prev) => {
      const d = new Date(prev)
      d.setMonth(d.getMonth() + 1)
      return d
    })
  }

  function thisMonth() {
    setViewMonth(new Date())
  }

  /** Navigate to /trades filtered to the clicked day */
  function drillDown(dateStr: string) {
    setFilters({ ...filters(), dateFrom: dateStr, dateTo: dateStr })
    navigate('/trades')
  }

  // Graduated background classes (static for Tailwind JIT detection)
  // bg-signal-green/[.08] bg-signal-green/[.15] bg-signal-green/[.25] bg-signal-green/[.40]
  // bg-signal-red/[.08] bg-signal-red/[.15] bg-signal-red/[.25] bg-signal-red/[.40]
  const GREEN_TIERS = ['bg-signal-green/[.08]', 'bg-signal-green/[.15]', 'bg-signal-green/[.25]', 'bg-signal-green/[.40]'] as const
  const RED_TIERS = ['bg-signal-red/[.08]', 'bg-signal-red/[.15]', 'bg-signal-red/[.25]', 'bg-signal-red/[.40]'] as const

  /** Graduated background: maps PnL magnitude to opacity level */
  function cellBg(cell: CalendarCell): string {
    if (!cell.data) return ''
    const abs = Math.abs(cell.pnlValue)
    const { p25, p50, p75 } = intensityThresholds()
    const tiers = cell.pnlValue > 0 ? GREEN_TIERS : cell.pnlValue < 0 ? RED_TIERS : null
    if (!tiers) return ''

    if (abs <= p25) return tiers[0]
    if (abs <= p50) return tiers[1]
    if (abs <= p75) return tiers[2]
    return tiers[3]
  }

  /** Weekly summary glow style */
  function weeklyGlow(pnl: number): string {
    if (pnl > 0) return '0 0 12px rgba(34, 197, 94, 0.2), inset 0 0 8px rgba(34, 197, 94, 0.06)'
    if (pnl < 0) return '0 0 12px rgba(239, 68, 68, 0.2), inset 0 0 8px rgba(239, 68, 68, 0.06)'
    return ''
  }

  return (
    <div class="border border-container-border bg-elevated">
      {/* Header: title + navigation + monthly stats */}
      <div class="flex items-center justify-between px-8 py-3 border-b border-container-border/30">
        <div class="flex items-center gap-4">
          <span class="font-display text-xs tracking-section text-text-tertiary uppercase">P&L Calendar</span>
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
              class="font-mono text-sm text-text-secondary transition-colors px-2 py-0.5"
              classList={{
                'hover:text-text-primary': !isCurrentMonth(),
                'opacity-30 cursor-default': isCurrentMonth(),
              }}
              disabled={isCurrentMonth()}
            >
              &rarr;
            </button>
            <button
              onClick={thisMonth}
              class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors border border-container-border/50 px-2 py-0.5 ml-1"
            >
              This month
            </button>
          </div>
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
        <div class="flex items-center justify-center" style={{ "min-height": "380px" }}>
          <div class="w-4 h-4 border-2 border-text-secondary border-t-text-primary rounded-full animate-spin" />
        </div>
      </Show>

      {/* Empty state — actionable guidance */}
      <Show when={!data.loading && isEmpty()}>
        <div class="flex flex-col items-center justify-center gap-3" style={{ "min-height": "380px" }}>
          <span class="font-mono text-xs text-text-tertiary">No trades this month</span>
          <Show when={!isCurrentMonth()}>
            <button
              onClick={thisMonth}
              class="font-mono text-[10px] text-text-tertiary hover:text-text-primary transition-colors border border-container-border/30 px-3 py-1"
            >
              &rarr; Jump to current month
            </button>
          </Show>
          <Show when={isCurrentMonth()}>
            <span class="font-mono text-[10px] text-text-tertiary/60 max-w-xs text-center leading-relaxed">
              Closed trades will appear here automatically. Try navigating to a month with activity.
            </span>
          </Show>
        </div>
      </Show>

      {/* Calendar grid */}
      <Show when={(!data.loading || data()) && !isEmpty()}>
        <div class="px-8 pb-6 overflow-x-auto">
          <div class="grid gap-px min-w-[700px]" style={{ "grid-template-columns": "repeat(7, 1fr) minmax(100px, 0.6fr)" }}>
            {/* Column headers: Sun-Sat + Weekly */}
            <For each={DAY_NAMES}>
              {(name) => (
                <div class="font-mono text-[10px] text-text-tertiary text-center py-2 uppercase tracking-wider">
                  {name}
                </div>
              )}
            </For>
            <div class="font-mono text-[10px] text-text-tertiary text-center py-2 uppercase tracking-wider">
              Week
            </div>

            {/* Calendar cells + weekly summaries */}
            <For each={cells()}>
              {(c, idx) => {
                const i = idx()
                const hasData = !!c.data
                return (
                  <>
                    <div
                      class={`border border-container-border/50 min-h-[88px] p-2 relative transition-colors ${
                        c.inMonth ? `bg-container-bg ${cellBg(c)}` : 'opacity-20'
                      } ${hasData ? 'cursor-pointer hover:border-text-secondary/50 hover:bg-container-bg-hover' : ''} ${
                        c.isToday ? 'ring-1 ring-inset ring-text-primary/30' : ''
                      }`}
                      onClick={() => hasData && drillDown(c.dateStr)}
                      role={hasData ? 'button' : undefined}
                      tabindex={hasData ? 0 : undefined}
                      onKeyDown={(e) => { if (hasData && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); drillDown(c.dateStr) } }}
                      aria-label={hasData ? `${c.dateStr}: ${compactPnl(c.pnlValue)}, ${c.data!.trade_count} trades — click to view` : undefined}
                    >
                      {/* Day number */}
                      <Show when={c.day > 0}>
                        <span class={`absolute top-1.5 right-2 font-mono text-[10px] ${
                          c.isToday ? 'text-text-primary font-bold' : 'text-text-tertiary'
                        }`}>
                          {c.day}
                        </span>
                      </Show>

                      {/* P&L data */}
                      <Show when={c.data}>
                        {(d) => (
                          <div class="flex flex-col items-center justify-center h-full pt-3">
                            <span class={`font-mono text-sm font-bold ${pnlColor(c.pnlValue)}`}>
                              {compactPnl(c.pnlValue)}
                            </span>
                            <span class="font-mono text-[10px] text-text-tertiary mt-0.5">
                              {d().trade_count} trade{d().trade_count !== 1 ? 's' : ''}
                            </span>
                          </div>
                        )}
                      </Show>
                    </div>

                    {/* Weekly summary cell — after every 7th cell */}
                    <Show when={(i + 1) % 7 === 0}>
                      {(() => {
                        const weekIdx = Math.floor(i / 7)
                        const week = weeklySummaries()[weekIdx]
                        if (!week || week.tradingDays === 0) {
                          return <div class="min-h-[88px] border border-container-border/30" />
                        }
                        return (
                          <div
                            class={`min-h-[88px] border border-container-border/30 flex flex-col items-center justify-center p-2 bg-container-bg ${
                              week.pnl > 0 ? 'bg-signal-green/5' : week.pnl < 0 ? 'bg-signal-red/5' : ''
                            }`}
                            style={{ "box-shadow": weeklyGlow(week.pnl) }}
                          >
                            <span class="font-mono text-[10px] text-text-tertiary">Wk {week.weekNum}</span>
                            <span class={`font-mono text-sm font-bold ${pnlColor(week.pnl)}`}>
                              {compactPnl(week.pnl)}
                            </span>
                            <span class="font-mono text-[10px] text-text-tertiary">
                              {week.tradingDays}d
                            </span>
                          </div>
                        )
                      })()}
                    </Show>
                  </>
                )
              }}
            </For>
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
