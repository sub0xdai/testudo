import { createResource, Show, For } from 'solid-js'
import { SkeletonBar } from './SkeletonBar'
import { StatSection } from './StatSection'
import { HeroEquityCurve } from './HeroEquityCurve'
import { ChartSelector } from './ChartSelector'
import { PageSubHeader } from './PageSubHeader'
import type { StatItem } from './StatSection'
import { useFilters } from './filterContext'
import { fetchOverview, fetchEquityCurve } from '../api/client'
import { formatCurrency, formatPercent, formatNumber, formatInteger, pnlColor, rColor, streakSign } from '../lib/formatters'

export function Overview() {
  const { filters } = useFilters()

  const [stats, { refetch: refetchStats }] = createResource(filters, fetchOverview)
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
      { label: 'Profit Factor', value: formatNumber(d.performance.profit_factor), colorClass: parseFloat(d.performance.profit_factor) > 1 ? 'text-signal-green' : parseFloat(d.performance.profit_factor) < 1 ? 'text-signal-red' : undefined },
      { label: 'Expectancy', value: formatCurrency(d.performance.expectancy), colorClass: pnlColor(d.performance.expectancy) },
      { label: 'R-Multiple', value: formatNumber(d.performance.avg_r_multiple), colorClass: rColor(d.performance.avg_r_multiple) },
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
      <PageSubHeader title="OVERVIEW" />

      {/* Loading state — structural skeleton */}
      <Show when={stats.loading && !stats()}>
        <div aria-live="polite" aria-busy="true" class="flex gap-0">
          {/* Stats sidebar skeleton */}
          <div class="w-64 shrink-0 border-r border-container-border hidden md:block">
            <For each={['ACCOUNT', 'PERFORMANCE', 'RISK']}>
              {(section) => (
                <div class="px-4 py-3 border-b border-container-border">
                  <span class="font-display text-xs tracking-section text-text-tertiary uppercase">
                    {section}
                  </span>
                  <div class="mt-3 space-y-2">
                    <For each={Array(4)}>
                      {() => (
                        <div class="flex justify-between">
                          <SkeletonBar width="60px" />
                          <SkeletonBar width="80px" />
                        </div>
                      )}
                    </For>
                  </div>
                </div>
              )}
            </For>
          </div>
          {/* Hero area skeleton */}
          <div class="flex-1 min-w-0">
            <div class="px-6 py-4 border-b border-container-border">
              <SkeletonBar width="200px" height="40px" class="mb-2" />
              <div class="flex gap-6">
                <SkeletonBar width="100px" />
                <SkeletonBar width="100px" />
                <SkeletonBar width="80px" />
              </div>
            </div>
            <div class="relative" style={{ "min-height": "400px" }}>
              {/* Axis lines */}
              <div class="absolute left-8 top-4 bottom-8 w-px bg-container-border/20" />
              <div class="absolute left-8 right-4 bottom-8 h-px bg-container-border/20" />
              <div class="absolute inset-0 skeleton-shimmer" />
            </div>
          </div>
        </div>
      </Show>

      {/* Error state */}
      <Show when={stats.error}>
        <div role="alert" aria-live="assertive" class="bg-elevated border border-container-border p-8 text-center">
          <p class="font-mono text-signal-red text-sm mb-2">FAILED TO LOAD STATS</p>
          <p class="font-mono text-text-tertiary text-xs mb-4">{String(stats.error)}</p>
          <button
            class="font-mono text-xs text-text-secondary hover:text-text-primary transition-colors border border-container-border px-3 py-1.5 rounded"
            onClick={() => refetchStats()}
          >
            Retry
          </button>
        </div>
      </Show>

      {/* Main 2-column layout */}
      <Show when={stats() && !stats.loading}>
        {/* Mobile: condensed stats strip */}
        <div class="md:hidden mb-4">
          <div class="flex items-baseline gap-4 mb-2">
            <span class={`font-mono text-4xl font-bold ${pnlColor(stats()!.account.net_pnl)}`}>
              {formatCurrency(stats()!.account.net_pnl)}
            </span>
            <span class={`font-mono text-2xl font-bold ${pnlColor(parseFloat(stats()!.performance.avg_r_multiple))}`}>
              {formatNumber(stats()!.performance.avg_r_multiple)}R
            </span>
          </div>
          <div class="flex gap-4 font-mono text-xs text-text-secondary">
            <span>Exp <span class="text-text-primary font-bold">{formatCurrency(stats()!.performance.expectancy)}</span></span>
            <span>WR <span class="text-text-primary font-bold">{formatPercent(stats()!.performance.win_rate)}</span></span>
            <span>PF <span class="text-text-primary font-bold">{formatNumber(stats()!.performance.profit_factor)}</span></span>
            <span>Trades <span class="text-text-primary font-bold">{formatInteger(stats()!.account.total_trades)}</span></span>
          </div>
        </div>

        {/* Desktop: 2-column layout */}
        <div class="flex gap-0">
          {/* Left sidebar — stats */}
          <aside class="w-64 shrink-0 overflow-y-auto hidden md:block sticky top-[var(--header-h)] glass-panel border-r-0" style={{ "max-height": "calc(100vh - var(--header-h))" }}>
            <StatSection title="ACCOUNT" items={accountItems()} />
            <StatSection title="PERFORMANCE" items={performanceItems()} />
            <StatSection title="RISK" items={riskItems()} />
          </aside>

          {/* Right main — hero P&L + charts */}
          <div class="flex-1 min-w-0">
            {/* Hero metrics */}
            <div class="px-6 py-4 glass-panel border-0 border-b border-container-border/50">
              <div class="flex items-baseline gap-8 mb-1">
                <div>
                  <span class={`font-mono text-4xl md:text-5xl font-bold ${pnlColor(stats()!.account.net_pnl)}`}>
                    {formatCurrency(stats()!.account.net_pnl)}
                  </span>
                  <span class="font-mono text-sm text-text-secondary ml-3">
                    net P&L
                  </span>
                </div>
                <div>
                  <span class={`font-mono text-4xl md:text-5xl font-bold ${pnlColor(parseFloat(stats()!.performance.avg_r_multiple))}`}>
                    {formatNumber(stats()!.performance.avg_r_multiple)}R
                  </span>
                  <span class="font-mono text-sm text-text-secondary ml-3">
                    R-multiple
                  </span>
                </div>
              </div>
            </div>

            {/* Hero Equity Curve */}
            <div class="px-6 pt-4">
              <span class="font-display text-xs tracking-section text-text-tertiary uppercase">EQUITY CURVE</span>
            </div>
            <HeroEquityCurve
              data={equity()?.data}
              loading={equity.loading}
            />

            {/* Chart selectors — 2-column grid */}
            <div class="grid grid-cols-1 lg:grid-cols-2 gap-6 p-6">
              <ChartSelector defaultChart="symbol" equityData={equity()} equityLoading={equity.loading} />
              <ChartSelector defaultChart="daily-pnl" equityData={equity()} equityLoading={equity.loading} />
            </div>
          </div>
        </div>
      </Show>
    </div>
  )
}
