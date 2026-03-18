import { createSignal, createResource, Show, type JSX } from 'solid-js'
import { useFilters } from './filterContext'
import { FilterPopout } from './FilterPopout'
import { fetchFilterOptions } from '../api/client'

interface PageSubHeaderProps {
  title: string
  children?: JSX.Element
}

export function PageSubHeader(props: PageSubHeaderProps) {
  const { filters, setFilters } = useFilters()
  const [showPopout, setShowPopout] = createSignal(false)

  const [options] = createResource(
    () => filters().exchange,
    (exchange) => fetchFilterOptions(exchange || undefined)
  )

  const activeFilterCount = () => {
    let count = 0
    if (filters().symbol) count++
    if (filters().dateFrom || filters().dateTo) count++
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

  return (
    <div class="relative border-b border-container-border bg-container-bg">
      <div class="max-w-[1400px] mx-auto px-6 py-3 flex items-center gap-4">
        <h1 class="font-display text-lg font-bold tracking-wider">{props.title}</h1>

        {/* Exchange dropdown */}
        <select
          class="bg-elevated border border-container-border text-text-primary font-mono text-sm px-3 py-1.5 rounded"
          value={filters().exchange ?? ''}
          onChange={(e) => selectExchange(e.currentTarget.value)}
          aria-label="Exchange filter"
        >
          <option value="">ALL</option>
          <option value="woo">WOO</option>
          <option value="binance">BINANCE</option>
          <option value="hyperliquid">HYPERLIQUID</option>
        </select>

        {/* Filter toggle */}
        <button
          class={`font-mono text-xs px-3 py-1.5 rounded border transition-colors ${
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

      {/* Popout panel */}
      <Show when={showPopout()}>
        <FilterPopout
          symbols={options()?.symbols ?? []}
          onClose={() => setShowPopout(false)}
        />
      </Show>
    </div>
  )
}
