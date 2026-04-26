import { createSignal, Show, For, type JSX } from 'solid-js'
import { useFilters } from './filterContext'
import { FilterPopout } from './FilterPopout'
import { HelpTip } from './HelpTip'
import { fetchFilterOptions } from '../api/client'
import { useCachedResource } from '../lib/cache'

type Preset = '1w' | '1m' | '3m' | 'ytd' | 'all'

const PRESETS: { key: Preset; label: string }[] = [
  { key: '1w', label: '1W' },
  { key: '1m', label: '1M' },
  { key: '3m', label: '3M' },
  { key: 'ytd', label: 'YTD' },
  { key: 'all', label: 'ALL' },
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

interface PageSubHeaderProps {
  title: string
  helpText?: string
  children?: JSX.Element
}

export function PageSubHeader(props: PageSubHeaderProps) {
  const { filters, setFilters } = useFilters()
  const [showPopout, setShowPopout] = createSignal(false)
  const [preset, setPreset] = createSignal<Preset>('all')

  const options = useCachedResource(
    () => 'filter-options:' + (filters().exchange ?? ''),
    () => fetchFilterOptions(filters().exchange || undefined),
    { staleMs: 5 * 60_000 },
  )

  const activeFilterCount = () => {
    let count = 0
    if (filters().symbol) count++
    return count
  }

  function selectExchange(exchange: string) {
    const current = filters()
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

  function selectPreset(key: Preset) {
    setPreset(key)
    const dateFrom = computeDateFrom(key)
    setFilters({ ...filters(), dateFrom, dateTo: undefined })
  }

  return (
    <div class="relative shrink-0 border-b border-container-border bg-container-bg">
      <div class="px-8 py-5 flex items-center gap-4">
        <h1 class="font-display text-lg font-bold tracking-wider">
          {props.title}
          {props.helpText && <HelpTip text={props.helpText} position="below" />}
        </h1>

        {/* Exchange dropdown */}
        <select
          class="bg-elevated border border-container-border text-text-primary font-mono text-sm px-3 py-1.5"
          value={filters().exchange ?? ''}
          onChange={(e) => selectExchange(e.currentTarget.value)}
          aria-label="Exchange filter"
        >
          <option value="">ALL</option>
          <option value="woo">WOO</option>
          <option value="binance">BINANCE</option>
          <option value="bybit">BYBIT</option>
          <option value="okx">OKX</option>
          <option value="bitget">BITGET</option>
          <option value="gate">GATE.IO</option>
          <option value="phemex">PHEMEX</option>
          <option value="blofin">BLOFIN</option>
          <option value="hyperliquid">HYPERLIQUID</option>
        </select>

        {/* Always-visible time presets */}
        <div class="flex items-center gap-1">
          <For each={PRESETS}>
            {(p) => (
              <button
                class={`font-mono text-xs px-2.5 py-1 transition-colors ${
                  preset() === p.key
                    ? 'bg-text-primary/10 text-text-primary'
                    : 'text-text-tertiary hover:text-text-primary'
                }`}
                onClick={() => selectPreset(p.key)}
              >
                {p.label}
              </button>
            )}
          </For>
        </div>

        {/* Filter toggle (symbol search + custom dates) */}
        <button
          class={`font-mono text-xs px-3 py-1.5 border transition-colors ${
            showPopout()
              ? 'border-text-primary text-text-primary'
              : 'border-container-border text-text-secondary hover:text-text-primary hover:border-text-secondary'
          }`}
          onClick={() => setShowPopout(!showPopout())}
          aria-expanded={showPopout()}
          aria-controls="filter-popout"
        >
          Filter{activeFilterCount() > 0 ? ` (${activeFilterCount()})` : ''}
        </button>

        {props.children}
      </div>

      {/* Popout panel (symbol search + custom dates only) */}
      <Show when={showPopout()}>
        <FilterPopout
          symbols={options()?.symbols ?? []}
          onClose={() => setShowPopout(false)}
        />
      </Show>
    </div>
  )
}
