import { createSignal, Show, onCleanup } from 'solid-js'
import { useFilters } from './filterContext'
import { SymbolSearch } from './SymbolSearch'
import type { SymbolCount } from '../api/client'

export function FilterPopout(props: { symbols: SymbolCount[]; onClose: () => void }) {
  const { filters, setFilters } = useFilters()
  const [customFrom, setCustomFrom] = createSignal(filters().dateFrom ?? '')
  const [customTo, setCustomTo] = createSignal(filters().dateTo ?? '')

  function selectSymbol(symbol: string) {
    setFilters({ ...filters(), symbol: symbol || undefined })
  }

  function applyCustomFrom(val: string) {
    setCustomFrom(val)
    setFilters({ ...filters(), dateFrom: val || undefined })
  }

  function applyCustomTo(val: string) {
    setCustomTo(val)
    setFilters({ ...filters(), dateTo: val || undefined })
  }

  function clearAll() {
    setFilters({})
    setCustomFrom('')
    setCustomTo('')
    props.onClose()
  }

  // Close on Escape
  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') props.onClose()
  }
  if (typeof window !== 'undefined') {
    window.addEventListener('keydown', handleKeyDown)
    onCleanup(() => window.removeEventListener('keydown', handleKeyDown))
  }

  return (
    <>
      {/* Backdrop for outside click */}
      <div class="fixed inset-0 z-30" onClick={props.onClose} />

      {/* Popout panel */}
      <div class="absolute left-0 right-0 z-40 bg-container-bg border-b border-container-border shadow-lg shadow-black/30 animate-dropdown-in">
        <div class="max-w-[1800px] mx-auto px-6 py-4 flex items-center gap-4 flex-wrap">
          {/* Symbol search */}
          <div class="flex items-center gap-2">
            <span class="font-mono text-xs text-text-tertiary uppercase tracking-wider" id="symbol-label">Symbol</span>
            <SymbolSearch
              symbols={props.symbols}
              value={filters().symbol ?? ''}
              onSelect={selectSymbol}
            />
          </div>

          {/* Separator */}
          <div class="w-px h-6 bg-container-border" />

          {/* Custom date range */}
          <label class="flex items-center gap-2">
            <span class="font-mono text-xs text-text-tertiary uppercase tracking-wider">From</span>
            <input
              type="date"
              class="bg-elevated border border-container-border text-text-primary font-mono text-sm px-3 py-1.5 rounded"
              value={customFrom()}
              onInput={(e) => applyCustomFrom(e.currentTarget.value)}
            />
          </label>
          <label class="flex items-center gap-2">
            <span class="font-mono text-xs text-text-tertiary uppercase tracking-wider">To</span>
            <input
              type="date"
              class="bg-elevated border border-container-border text-text-primary font-mono text-sm px-3 py-1.5 rounded"
              value={customTo()}
              onInput={(e) => applyCustomTo(e.currentTarget.value)}
            />
          </label>

          {/* Clear all */}
          <button
            class="font-mono text-xs text-text-tertiary hover:text-signal-red transition-colors ml-auto"
            onClick={clearAll}
          >
            Clear all
          </button>
        </div>
      </div>
    </>
  )
}
