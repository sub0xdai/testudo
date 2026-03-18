import { createResource, Show } from 'solid-js'
import { StatSection } from './StatSection'
import { HeroEquityCurve } from './HeroEquityCurve'
import { ChartSelector } from './ChartSelector'
import { GhostAnnotation } from './GhostAnnotation'
import type { StatItem } from './StatCard'
import { useFilters } from './filterContext'
import { fetchOverview, fetchEquityCurve } from '../api/client'
import { formatCurrency, formatPercent, formatNumber, formatInteger, pnlColor, streakSign } from '../lib/formatters'

export function Overview() {
  const { filters } = useFilters()

  const [stats] = createResource(filters, fetchOverview)
  const [equity] = createResource(filters, fetchEquityCurve)

  function accountItems(): StatItem[] {
    const d = stats()
    if (!d) return []
    return [
      { label: 'Total P&L', value: formatCurrency(d.account.total_pnl), colorClass: pnlColor(d.account.total_pnl) },
      { label: 'Net P&L', value: formatCurrency(d.account.net_pnl), colorClass: pnlColor(d.account.net_pnl) },
      { label: 'Fees', value: formatCurrency(d.account.total_fees) },
      { label: 'Trades', value: formatInteger(d.account.total_trades) },
    ]
  }

  function performanceItems(): StatItem[] {
    const d = stats()
    if (!d) return []
    return [
      { label: 'Win Rate', value: formatPercent(d.performance.win_rate) },
      { label: 'Profit Factor', value: formatNumber(d.performance.profit_factor) },
      { label: 'Expectancy', value: formatCurrency(d.performance.expectancy) },
      { label: 'Avg R', value: formatNumber(d.performance.avg_r_multiple) },
      { label: 'Trades/Day', value: formatNumber(d.performance.trades_per_day, 1) },
    ]
  }

  function riskItems(): StatItem[] {
    const d = stats()
    if (!d) return []
    return [
      { label: 'Max DD', value: formatPercent(d.risk.max_drawdown_pct), colorClass: 'text-signal-red' },
      { label: 'Worst Day', value: formatCurrency(d.risk.worst_day), colorClass: pnlColor(d.risk.worst_day) },
      { label: 'Worst Week', value: formatCurrency(d.risk.worst_week), colorClass: pnlColor(d.risk.worst_week) },
      { label: 'Streak', value: streakSign(d.risk.current_streak), colorClass: pnlColor(d.risk.current_streak) },
      { label: 'Best Streak', value: `+${d.risk.best_streak}`, colorClass: 'text-signal-green' },
    ]
  }

  return (
    <div>
      {/* Loading state */}
      <Show when={stats.loading && !stats()}>
        <div class="flex gap-6">
          <div class="w-64 shrink-0 hidden md:block">
            <div class="bg-elevated border border-container-border animate-pulse h-96" />
          </div>
          <div class="flex-1">
            <div class="bg-elevated border border-container-border animate-pulse h-12 mb-4" />
            <div class="bg-elevated border border-container-border animate-pulse" style={{ "min-height": "400px" }} />
          </div>
        </div>
      </Show>

      {/* Error state */}
      <Show when={stats.error}>
        <div class="bg-elevated border border-container-border p-8 text-center">
          <p class="font-mono text-signal-red text-sm mb-2">FAILED TO LOAD STATS</p>
          <p class="font-mono text-text-tertiary text-xs">{String(stats.error)}</p>
        </div>
      </Show>

      {/* Main 2-column layout */}
      <Show when={stats() && !stats.loading}>
        <div class="mb-4">
          <GhostAnnotation text="ACCOUNT_OVERVIEW" />
          <h1 class="text-2xl md:text-3xl font-display font-bold tracking-tight">OVERVIEW</h1>
        </div>

        {/* Mobile: condensed stats strip */}
        <div class="md:hidden mb-4">
          <div class="flex items-baseline gap-3 mb-2">
            <span class={`font-mono text-4xl font-bold ${pnlColor(stats()!.account.net_pnl)}`}>
              {formatCurrency(stats()!.account.net_pnl)}
            </span>
            <span class="font-mono text-xs text-text-secondary">
              {formatPercent(stats()!.performance.win_rate)} WR
            </span>
          </div>
          <div class="flex gap-4 font-mono text-xs text-text-secondary">
            <span>PF <span class="text-text-primary font-bold">{formatNumber(stats()!.performance.profit_factor)}</span></span>
            <span>Trades <span class="text-text-primary font-bold">{formatInteger(stats()!.account.total_trades)}</span></span>
            <span>DD <span class="text-signal-red font-bold">{formatPercent(stats()!.risk.max_drawdown_pct)}</span></span>
          </div>
        </div>

        {/* Desktop: 2-column layout */}
        <div class="flex gap-0">
          {/* Left sidebar — stats */}
          <aside class="w-64 shrink-0 border-r border-container-border overflow-y-auto hidden md:block" style={{ "max-height": "calc(100vh - 140px)" }}>
            <StatSection title="ACCOUNT" items={accountItems()} />
            <StatSection title="PERFORMANCE" items={performanceItems()} />
            <StatSection title="RISK" items={riskItems()} />
          </aside>

          {/* Right main — hero P&L + charts */}
          <div class="flex-1 min-w-0">
            {/* Hero P&L */}
            <div class="px-6 py-4 border-b border-container-border">
              <div class="flex items-baseline gap-4 mb-1">
                <span class={`font-mono text-4xl md:text-5xl font-bold ${pnlColor(stats()!.account.net_pnl)}`}>
                  {formatCurrency(stats()!.account.net_pnl)}
                </span>
                <span class="font-mono text-sm text-text-secondary">
                  net P&L
                </span>
              </div>
              <div class="flex gap-6 font-mono text-sm">
                <span class="text-text-secondary">
                  Win Rate <span class="text-text-primary font-bold">{formatPercent(stats()!.performance.win_rate)}</span>
                </span>
                <span class="text-text-secondary">
                  Profit Factor <span class="text-text-primary font-bold">{formatNumber(stats()!.performance.profit_factor)}</span>
                </span>
                <span class="text-text-secondary">
                  Trades <span class="text-text-primary font-bold">{formatInteger(stats()!.account.total_trades)}</span>
                </span>
              </div>
            </div>

            {/* Hero Equity Curve — borderless, min 400px */}
            <HeroEquityCurve
              data={equity()?.data}
              loading={equity.loading}
            />

            {/* Secondary chart selector */}
            <div class="p-6">
              <ChartSelector />
            </div>
          </div>
        </div>
      </Show>
    </div>
  )
}
