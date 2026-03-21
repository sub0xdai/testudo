import { createSignal, Show, For, onCleanup } from 'solid-js'
import { useFilters } from './filterContext'
import { SymbolSearch } from './SymbolSearch'
import type { SymbolCount } from '../api/client'

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

export function FilterPopout(props: { symbols: SymbolCount[]; onClose: () => void }) {
  const { filters, setFilters } = useFilters()
  const [preset, setPreset] = createSignal<Preset>('all')
  const [customFrom, setCustomFrom] = createSignal('')
  const [customTo, setCustomTo] = createSignal('')

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

  function clearAll() {
    setFilters({})
    setPreset('all')
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
        <div class="max-w-[1400px] mx-auto px-6 py-4 flex items-center gap-4 flex-wrap">
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

          {/* Time presets */}
          <div class="flex items-center gap-1">
            <For each={PRESETS}>
              {(p) => (
                <button
                  class={`font-mono text-xs px-2.5 py-1 rounded transition-colors ${
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

          {/* Custom date inputs */}
          <Show when={preset() === 'custom'}>
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
          </Show>

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
