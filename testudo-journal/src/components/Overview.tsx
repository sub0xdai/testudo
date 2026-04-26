import { createSignal, createResource, createEffect, Show, For, onMount, onCleanup } from 'solid-js'
import { useCachedBatch } from '../lib/cache'
import { SkeletonBar } from './SkeletonBar'
import { StatSection } from './StatSection'
import { PnlCalendar } from './charts/PnlCalendar'
import { PerformanceRadar } from './charts/PerformanceRadar'
import { ChartSelector } from './ChartSelector'
import { PageSubHeader } from './PageSubHeader'
import type { StatItem } from './StatSection'
import { useFilters } from './filterContext'
import { useAuth } from '../context/AuthContext'
import { exchangeApi, fetchRiskSnapshot, type RiskSnapshot, type OverviewResponse, type EquityPoint } from '../api/client'
import { formatCurrency, formatPercent, formatNumber, formatInteger, pnlColor, rColor, streakSign } from '../lib/formatters'
import { HELP } from '../lib/help-content'
import { createRiskWsClient } from '../lib/ws'

const STALE_THRESHOLD_MS = 60_000
const POLL_FALLBACK_MS = 30_000
const STALE_TICK_MS = 10_000
const REFETCH_DEBOUNCE_MS = 500

function stripSign(formatted: string): string {
  return formatted.replace(/^\+/, '')
}

function relativeTime(snap: RiskSnapshot | null | undefined): string {
  if (!snap) return ''
  const ts = Date.parse(snap.as_of)
  if (Number.isNaN(ts)) return ''
  const diff = Date.now() - ts
  if (diff < 10_000) return 'last updated: just now'
  if (diff < 60_000) return `last updated: ${Math.floor(diff / 1000)}s ago`
  return `last updated: ${Math.floor(diff / 60_000)}m ago`
}

export function Overview() {
  const { filters } = useFilters()
  const auth = useAuth()

  // PERF-02 CP-2: one batched request covers Overview's 4 cold-paint sections.
  // ChartSelector defaults render PnlTreemap (symbol_breakdown) and DailyPnl
  // chart (daily_pnl); both keep their existing useCachedResource calls and
  // get cache HITS from the primed batch entries.
  const batch = useCachedBatch(
    () => ['overview', 'equity_curve', 'symbol_breakdown', 'daily_pnl'],
    filters,
    { staleMs: 30_000, persist: true, identity: auth.user()?.id ?? null },
  )
  // Thin accessors preserve the prior `useCachedResource` API surface so the
  // rest of this component reads unchanged. NB: `Object.assign` invokes
  // getters on the source object and copies the resulting values as plain
  // data properties — that flattens reactivity. Use `defineProperty` so the
  // getters re-run on every read and stay reactive with the underlying signals.
  const statsAccessor = (() => batch.sections.overview() as OverviewResponse | undefined) as {
    (): OverviewResponse | undefined
    readonly loading: boolean
    readonly error: unknown
    refetch: () => void
  }
  Object.defineProperty(statsAccessor, 'loading', {
    get: () => batch.sections.overview.loading, enumerable: true,
  })
  Object.defineProperty(statsAccessor, 'error', {
    get: () => batch.sections.overview.error, enumerable: true,
  })
  Object.defineProperty(statsAccessor, 'refetch', {
    value: () => batch.refetch(), enumerable: true, writable: false,
  })
  const stats = statsAccessor

  const equityAccessor = (() => batch.sections.equity_curve() as { data: EquityPoint[] } | undefined) as {
    (): { data: EquityPoint[] } | undefined
    readonly loading: boolean
    readonly error: unknown
    refetch: () => void
  }
  Object.defineProperty(equityAccessor, 'loading', {
    get: () => batch.sections.equity_curve.loading, enumerable: true,
  })
  Object.defineProperty(equityAccessor, 'error', {
    get: () => batch.sections.equity_curve.error, enumerable: true,
  })
  Object.defineProperty(equityAccessor, 'refetch', {
    value: () => batch.refetch(), enumerable: true, writable: false,
  })
  const equity = equityAccessor

  // Aggregate account balance across all exchanges
  const [totalBalance, setTotalBalance] = createSignal<number | null>(null)
  onMount(async () => {
    try {
      const accounts = await exchangeApi.listAccounts()
      if (!accounts.length) return
      let sum = 0
      const results = await Promise.allSettled(
        accounts.map(acc => exchangeApi.fetchBalance(acc.id))
      )
      for (const r of results) {
        if (r.status !== 'fulfilled') continue
        const primary = r.value.balances.find(b => b.asset === 'USDT' || b.asset === 'USDC')
          || r.value.balances[0]
        if (primary) sum += parseFloat(primary.total) || 0
      }
      setTotalBalance(sum)
    } catch { /* non-blocking */ }
  })

  // RSK-01a T1: Overview owns the live risk snapshot (WS push + 30s polling fallback + stale indicator).
  const [snapshot, { refetch: refetchSnapshot }] = createResource(
    () => auth.isAuthenticated(),
    async (authed: boolean) => (authed ? fetchRiskSnapshot() : null),
  )

  const [now, setNow] = createSignal(Date.now())
  let debounceTimer: ReturnType<typeof setTimeout> | null = null
  let pollTimer: ReturnType<typeof setInterval> | null = null
  let staleTicker: ReturnType<typeof setInterval> | null = null

  function debouncedRefetch() {
    if (debounceTimer) clearTimeout(debounceTimer)
    debounceTimer = setTimeout(() => {
      debounceTimer = null
      refetchSnapshot()
    }, REFETCH_DEBOUNCE_MS)
  }

  const wsClient = createRiskWsClient(debouncedRefetch)

  createEffect(() => {
    const authed = auth.isAuthenticated()
    const uid = auth.user()?.id
    if (authed && uid) {
      wsClient.connect(uid)
    } else {
      wsClient.disconnect()
    }
  })

  createEffect(() => {
    const authed = auth.isAuthenticated()
    if (!authed) {
      if (pollTimer) { clearInterval(pollTimer); pollTimer = null }
      return
    }
    if (wsClient.connected()) {
      if (pollTimer) { clearInterval(pollTimer); pollTimer = null }
    } else if (!pollTimer) {
      pollTimer = setInterval(() => refetchSnapshot(), POLL_FALLBACK_MS)
    }
  })

  onMount(() => {
    staleTicker = setInterval(() => setNow(Date.now()), STALE_TICK_MS)
  })

  onCleanup(() => {
    if (debounceTimer) clearTimeout(debounceTimer)
    if (pollTimer) clearInterval(pollTimer)
    if (staleTicker) clearInterval(staleTicker)
    wsClient.disconnect()
  })

  const isStale = () => {
    const snap = snapshot()
    if (!snap) return false
    now() // reactive dependency on the tick signal
    const ts = Date.parse(snap.as_of)
    if (Number.isNaN(ts)) return false
    return Date.now() - ts > STALE_THRESHOLD_MS
  }

  const heroExposure = () => {
    const s = snapshot()
    return s ? stripSign(formatCurrency(s.net_exposure_usd)) : null
  }
  const heroLeverage = () => {
    const s = snapshot()
    return s ? `${formatNumber(s.aggregate_leverage, 1)}x` : null
  }
  const heroFree = () => {
    const s = snapshot()
    return s ? stripSign(formatCurrency(s.free_margin_usd)) : null
  }

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
      { label: 'Profit Factor', value: parseFloat(d.performance.profit_factor) > 999 ? '∞' : formatNumber(d.performance.profit_factor), colorClass: parseFloat(d.performance.profit_factor) > 1 ? 'text-signal-green' : parseFloat(d.performance.profit_factor) < 1 ? 'text-signal-red' : undefined },
      { label: 'Expectancy', value: formatCurrency(d.performance.expectancy), colorClass: pnlColor(d.performance.expectancy) },
      { label: 'R-Multiple', value: parseFloat(d.performance.avg_r_multiple) ? `${formatNumber(d.performance.avg_r_multiple)}R` : '—', colorClass: parseFloat(d.performance.avg_r_multiple) ? rColor(d.performance.avg_r_multiple) : undefined },
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
    <div class="flex flex-col h-full">
      <PageSubHeader title="OVERVIEW" helpText={HELP['page.overview']} />

      {/* Loading state -- structural skeleton */}
      <Show when={stats.loading && !stats()}>
        <div aria-live="polite" aria-busy="true" class="flex flex-1 min-h-0">
          {/* Stats sidebar skeleton */}
          <div class="w-80 shrink-0 border-r border-container-border hidden md:block bg-container-bg">
            <For each={['ACCOUNT', 'PERFORMANCE', 'RISK']}>
              {(section) => (
                <div class="px-8 py-4 border-b border-container-border/50">
                  <span class="font-display text-xs tracking-section text-text-tertiary uppercase">
                    {section}
                  </span>
                  <div class="mt-3 space-y-3">
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
            <div class="px-10 py-8 border-b border-container-border bg-container-bg">
              <SkeletonBar width="200px" height="40px" class="mb-2" />
              <div class="flex gap-6">
                <SkeletonBar width="100px" />
                <SkeletonBar width="100px" />
                <SkeletonBar width="80px" />
              </div>
            </div>
            <div class="relative" style={{ "min-height": "400px" }}>
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
            class="btn-ghost border border-container-border px-3 py-1.5"
            onClick={() => stats.refetch()}
          >
            Retry
          </button>
        </div>
      </Show>

      {/* Main 2-column layout — fills remaining viewport */}
      <Show when={stats() && !stats.loading}>
        {/* Mobile: condensed stats strip */}
        <div class="md:hidden px-6 py-4 bg-container-bg border-b border-container-border/50">
          <div class="flex items-baseline gap-3 mb-2 flex-wrap">
            <span class={`font-mono text-3xl font-bold ${pnlColor(stats()!.account.net_pnl)}`}>
              {formatCurrency(stats()!.account.net_pnl)}
            </span>
            <Show when={totalBalance() !== null}>
              <span class="font-mono text-xl font-bold text-text-primary">
                ${formatNumber(totalBalance()!)}
              </span>
            </Show>
            <Show when={heroLeverage()}>
              <span class="font-mono text-xl font-bold text-text-primary">
                {heroLeverage()}
              </span>
            </Show>
            <span
              class="inline-block w-1.5 h-1.5 rounded-full self-center"
              classList={{
                'bg-signal-green animate-pulse': !isStale() && wsClient.connected(),
                'bg-signal-green': !isStale() && !wsClient.connected(),
                'bg-signal-amber': isStale(),
              }}
              title={relativeTime(snapshot())}
              aria-label={isStale() ? 'stale' : 'live'}
            />
          </div>
          <div class="flex gap-4 font-mono text-xs text-text-secondary">
            <span>Exp <span class="text-text-primary font-bold">{formatCurrency(stats()!.performance.expectancy)}</span></span>
            <span>WR <span class="text-text-primary font-bold">{formatPercent(stats()!.performance.win_rate)}</span></span>
            <span>PF <span class="text-text-primary font-bold">{formatNumber(stats()!.performance.profit_factor)}</span></span>
            <span>Trades <span class="text-text-primary font-bold">{formatInteger(stats()!.account.total_trades)}</span></span>
          </div>
        </div>

        {/* Desktop: edge-to-edge 2-column layout */}
        <div class="flex flex-1 min-h-0">
          {/* Left sidebar — anchored left, full height */}
          <aside class="w-80 shrink-0 overflow-y-auto hidden md:block bg-container-bg border-r border-container-border">
            <PerformanceRadar />
            <StatSection title="ACCOUNT" items={accountItems()} />
            <StatSection title="PERFORMANCE" items={performanceItems()} />
            <StatSection title="RISK" items={riskItems()} />
          </aside>

          {/* Right main — transparent so Hadrian's Wall shows through */}
          <div class="flex-1 min-w-0 overflow-y-auto">
            {/* Hero metrics — single uniform ticker row spread across the bar */}
            <div class="px-10 py-8 bg-container-bg border-b border-container-border">
              <div class="border-l-2 border-accent-primary pl-8">
                <div class="flex items-baseline justify-between gap-6 flex-wrap">
                  <div>
                    <span class={`font-mono text-3xl md:text-4xl font-bold ${pnlColor(stats()!.account.net_pnl)} ${parseFloat(String(stats()!.account.net_pnl)) >= 0 ? 'hero-glow-green' : 'hero-glow-red'}`}>
                      {formatCurrency(stats()!.account.net_pnl)}
                    </span>
                    <span class="font-mono text-xs text-text-secondary uppercase tracking-wider ml-2">
                      net P&L
                    </span>
                  </div>
                  <Show when={totalBalance() !== null}>
                    <div>
                      <span class="font-mono text-3xl md:text-4xl font-bold text-text-primary">
                        ${formatNumber(totalBalance()!)}
                      </span>
                      <span class="font-mono text-xs text-text-secondary uppercase tracking-wider ml-2">
                        balance
                      </span>
                    </div>
                  </Show>
                  <Show when={heroExposure()}>
                    <div>
                      <span class="font-mono text-3xl md:text-4xl font-bold text-text-primary">
                        {heroExposure()}
                      </span>
                      <span class="font-mono text-xs text-text-secondary uppercase tracking-wider ml-2">
                        exposure
                      </span>
                    </div>
                  </Show>
                  <Show when={heroLeverage()}>
                    <div>
                      <span class="font-mono text-3xl md:text-4xl font-bold text-text-primary">
                        {heroLeverage()}
                      </span>
                      <span class="font-mono text-xs text-text-secondary uppercase tracking-wider ml-2">
                        leverage
                      </span>
                    </div>
                  </Show>
                  <Show when={heroFree()}>
                    <div>
                      <span class="font-mono text-3xl md:text-4xl font-bold text-text-primary">
                        {heroFree()}
                      </span>
                      <span class="font-mono text-xs text-text-secondary uppercase tracking-wider ml-2">
                        free margin
                      </span>
                    </div>
                  </Show>
                  <Show when={snapshot()}>
                    <div class="flex items-center gap-2 self-center">
                      <span
                        class="inline-block w-2 h-2 rounded-full"
                        classList={{
                          'bg-signal-green animate-pulse': !isStale() && wsClient.connected(),
                          'bg-signal-green': !isStale() && !wsClient.connected(),
                          'bg-signal-amber': isStale(),
                        }}
                        title={relativeTime(snapshot())}
                        aria-hidden="true"
                      />
                      <span
                        class="font-mono text-[10px] tracking-wider uppercase"
                        classList={{
                          'text-signal-amber': isStale(),
                          'text-text-tertiary': !isStale(),
                        }}
                      >
                        {isStale() ? 'stale' : 'live'}
                      </span>
                    </div>
                  </Show>
                </div>
              </div>
            </div>

            {/* P&L Calendar — aligned with charts below */}
            <div class="p-8 pb-0">
              <PnlCalendar />
            </div>

            {/* Chart selectors -- 2-column grid, aligned with calendar */}
            <div class="grid grid-cols-1 lg:grid-cols-2 gap-6 p-8">
              <ChartSelector defaultChart="symbol" equityData={equity()} equityLoading={equity.loading} />
              <ChartSelector defaultChart="daily-pnl" equityData={equity()} equityLoading={equity.loading} />
            </div>
          </div>
        </div>
      </Show>
    </div>
  )
}
