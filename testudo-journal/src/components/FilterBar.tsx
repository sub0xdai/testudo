import { createSignal, createResource, Show, For } from 'solid-js'
import { useFilters } from './filterContext'
import { fetchFilterOptions } from '../api/client'
import { SymbolSearch } from './SymbolSearch'

type Preset = '1w' | '1m' | '3m' | 'ytd' | 'all' | 'custom'

const PRESETS: { key: Preset; label: string }[] = [
  { key: '1w', label: '1W' },
  { key: '1m', label: '1M' },
  { key: '3m', label: '3M' },
  { key: 'ytd', label: 'YTD' },
  { key: 'all', label: 'ALL' },
  { key: 'custom', label: 'CUSTOM' },
]

function computeDateFrom(key: Preset): string | undefined {
  const now = new Date()
  switch (key) {
    case '1w': {
      const d = new Date(now)
      d.setDate(d.getDate() - 7)
      return d.toISOString().slice(0, 10)
    }
    case '1m': {
      const d = new Date(now)
      d.setDate(d.getDate() - 30)
      return d.toISOString().slice(0, 10)
    }
    case '3m': {
      const d = new Date(now)
      d.setDate(d.getDate() - 90)
      return d.toISOString().slice(0, 10)
    }
    case 'ytd':
      return `${now.getFullYear()}-01-01`
    case 'all':
      return undefined
    default:
      return undefined
  }
}

export function FilterBar() {
  const { filters, setFilters } = useFilters()
  const [preset, setPreset] = createSignal<Preset>('all')
  const [customFrom, setCustomFrom] = createSignal('')
  const [customTo, setCustomTo] = createSignal('')

  // Fetch filter options, re-fetch when exchange changes
  const [options] = createResource(
    () => filters().exchange,
    (exchange) => fetchFilterOptions(exchange || undefined)
  )

  function selectExchange(exchange: string) {
    const current = filters()
    // If symbol is set and not available on new exchange, reset it
    if (current.symbol && exchange) {
      const opts = options()
      const symbolExists = opts?.symbols.some((s) => s.symbol === current.symbol)
      if (!symbolExists) {
        setFilters({ ...current, exchange: exchange || undefined, symbol: undefined })
        return
      }
    }
    setFilters({ ...current, exchange: exchange || undefined })
  }

  function selectSymbol(symbol: string) {
    setFilters({ ...filters(), symbol: symbol || undefined })
  }

  function selectPreset(key: Preset) {
    setPreset(key)
    if (key === 'custom') return
    setCustomFrom('')
    setCustomTo('')
    const dateFrom = computeDateFrom(key)
    setFilters({ ...filters(), dateFrom, dateTo: undefined })
  }

  function applyCustomFrom(val: string) {
    setCustomFrom(val)
    setFilters({ ...filters(), dateFrom: val || undefined })
  }

  function applyCustomTo(val: string) {
    setCustomTo(val)
    setFilters({ ...filters(), dateTo: val || undefined })
  }

  function reset() {
    setFilters({})
    setPreset('all')
    setCustomFrom('')
    setCustomTo('')
  }

  const hasActiveFilters = () => {
    const f = filters()
    return !!(f.exchange || f.symbol || f.dateFrom || f.dateTo)
  }

  return (
    <div class="border-b border-container-border bg-container-bg">
      <div class="max-w-[1400px] mx-auto px-6 py-3 flex items-center gap-4 flex-wrap">
        {/* Exchange dropdown */}
        <label class="flex items-center gap-2">
          <span class="font-mono text-xs text-text-tertiary uppercase tracking-wider">Exchange</span>
          <select
            class="bg-elevated border border-container-border text-text-primary font-mono text-sm px-3 py-1.5 rounded focus-visible:border-text-secondary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-text-secondary/30 focus-visible:ring-offset-1 focus-visible:ring-offset-main-bg"
            value={filters().exchange ?? ''}
            onChange={(e) => selectExchange(e.currentTarget.value)}
          >
            <option value="">ALL</option>
            <option value="woo">WOO</option>
            <option value="binance">BINANCE</option>
            <option value="hyperliquid">HYPERLIQUID</option>
          </select>
        </label>

        {/* Symbol searchable dropdown */}
        <div class="flex items-center gap-2">
          <span class="font-mono text-xs text-text-tertiary uppercase tracking-wider" id="symbol-label">Symbol</span>
          <SymbolSearch
            symbols={options()?.symbols ?? []}
            value={filters().symbol ?? ''}
            onSelect={selectSymbol}
          />
        </div>

        {/* Separator */}
        <div class="w-px h-6 bg-container-border" />

        {/* Time presets */}
        <div class="flex items-center gap-1">
          <For each={PRESETS}>
            {(p) => (
              <button
                class={`font-mono text-xs px-2.5 py-1 transition-colors outline-none ${
                  preset() === p.key
                    ? 'text-text-primary'
                    : 'text-text-tertiary hover:text-text-primary'
                }`}
                onClick={() => selectPreset(p.key)}
              >
                {p.label}
              </button>
            )}
          </For>
        </div>

        {/* Custom date inputs */}
        <Show when={preset() === 'custom'}>
          <label class="flex items-center gap-2">
            <span class="font-mono text-xs text-text-tertiary uppercase tracking-wider">From</span>
            <input
              type="date"
              class="bg-elevated border border-container-border text-text-primary font-mono text-sm px-3 py-1.5 rounded focus-visible:border-text-secondary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-text-secondary/30 focus-visible:ring-offset-1 focus-visible:ring-offset-main-bg"
              value={customFrom()}
              onInput={(e) => applyCustomFrom(e.currentTarget.value)}
            />
          </label>
          <label class="flex items-center gap-2">
            <span class="font-mono text-xs text-text-tertiary uppercase tracking-wider">To</span>
            <input
              type="date"
              class="bg-elevated border border-container-border text-text-primary font-mono text-sm px-3 py-1.5 rounded focus-visible:border-text-secondary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-text-secondary/30 focus-visible:ring-offset-1 focus-visible:ring-offset-main-bg"
              value={customTo()}
              onInput={(e) => applyCustomTo(e.currentTarget.value)}
            />
          </label>
        </Show>

        {/* Reset */}
        <Show when={hasActiveFilters()}>
          <button
            class="font-mono text-xs text-text-tertiary hover:text-signal-red transition-colors ml-auto"
            onClick={reset}
          >
            × reset
          </button>
        </Show>
      </div>
    </div>
  )
}
