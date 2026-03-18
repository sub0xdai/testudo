import { createResource, Show } from 'solid-js'
import { StatCard, type StatItem } from './StatCard'
import { useFilters } from './filterContext'
import { fetchOverview } from '../api/client'
import { formatCurrency, formatPercent, formatNumber, formatInteger, pnlColor, streakSign } from '../lib/formatters'

export function Overview() {
  const { filters } = useFilters()

  const [data] = createResource(filters, fetchOverview)

  function accountItems(): StatItem[] {
    const d = data()
    if (!d) return []
    return [
      { label: 'Total P&L', value: formatCurrency(d.account.total_pnl), colorClass: pnlColor(d.account.total_pnl) },
      { label: 'Net P&L', value: formatCurrency(d.account.net_pnl), colorClass: pnlColor(d.account.net_pnl) },
      { label: 'Fees', value: formatCurrency(d.account.total_fees) },
      { label: 'Trades', value: formatInteger(d.account.total_trades) },
    ]
  }

  function performanceItems(): StatItem[] {
    const d = data()
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
    const d = data()
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
      <Show when={data.loading}>
        <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
          <div class="bg-elevated border border-container-border rounded-lg p-5 animate-pulse h-52" />
          <div class="bg-elevated border border-container-border rounded-lg p-5 animate-pulse h-52" />
          <div class="bg-elevated border border-container-border rounded-lg p-5 animate-pulse h-52" />
        </div>
      </Show>

      <Show when={data.error}>
        <div class="bg-elevated border border-container-border rounded-lg p-8 text-center">
          <p class="font-mono text-signal-red text-sm mb-2">FAILED TO LOAD STATS</p>
          <p class="font-mono text-text-tertiary text-xs">{String(data.error)}</p>
        </div>
      </Show>

      <Show when={data() && !data.loading}>
        <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
          <StatCard title="ACCOUNT" items={accountItems()} />
          <StatCard title="PERFORMANCE" items={performanceItems()} />
          <StatCard title="RISK" items={riskItems()} />
        </div>
      </Show>
    </div>
  )
}
