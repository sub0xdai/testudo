import { createSignal } from 'solid-js'
import { useFilters } from './filterContext'

export function FilterBar() {
  const { filters, setFilters } = useFilters()
  const [localExchange, setLocalExchange] = createSignal(filters().exchange ?? '')
  const [localSymbol, setLocalSymbol] = createSignal(filters().symbol ?? '')
  const [localFrom, setLocalFrom] = createSignal(filters().dateFrom ?? '')
  const [localTo, setLocalTo] = createSignal(filters().dateTo ?? '')

  function apply() {
    setFilters({
      exchange: localExchange() || undefined,
      symbol: localSymbol() || undefined,
      dateFrom: localFrom() || undefined,
      dateTo: localTo() || undefined,
    })
  }

  function clear() {
    setLocalExchange('')
    setLocalSymbol('')
    setLocalFrom('')
    setLocalTo('')
    setFilters({})
  }

  return (
    <div class="border-b border-container-border bg-container-bg">
      <div class="max-w-[1400px] mx-auto px-6 py-3 flex items-center gap-4 flex-wrap">
        {/* Exchange */}
        <div class="flex items-center gap-2">
          <label class="font-mono text-xs text-text-tertiary uppercase tracking-wider">Exchange</label>
          <select
            class="bg-elevated border border-container-border text-text-primary font-mono text-sm px-3 py-1.5 rounded focus-visible:border-signal-green focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-signal-green/30 focus-visible:ring-offset-1 focus-visible:ring-offset-main-bg"
            value={localExchange()}
            onChange={(e) => setLocalExchange(e.currentTarget.value)}
          >
            <option value="">ALL</option>
            <option value="woo">WOO</option>
            <option value="binance">BINANCE</option>
            <option value="hyperliquid">HYPERLIQUID</option>
          </select>
        </div>

        {/* Symbol */}
        <div class="flex items-center gap-2">
          <label class="font-mono text-xs text-text-tertiary uppercase tracking-wider">Symbol</label>
          <input
            type="text"
            placeholder="BTC_USDT"
            class="bg-elevated border border-container-border text-text-primary font-mono text-sm px-3 py-1.5 rounded w-32 focus-visible:border-signal-green focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-signal-green/30 focus-visible:ring-offset-1 focus-visible:ring-offset-main-bg placeholder:text-text-tertiary"
            value={localSymbol()}
            onInput={(e) => setLocalSymbol(e.currentTarget.value)}
          />
        </div>

        {/* Date From */}
        <div class="flex items-center gap-2">
          <label class="font-mono text-xs text-text-tertiary uppercase tracking-wider">From</label>
          <input
            type="date"
            class="bg-elevated border border-container-border text-text-primary font-mono text-sm px-3 py-1.5 rounded focus-visible:border-signal-green focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-signal-green/30 focus-visible:ring-offset-1 focus-visible:ring-offset-main-bg"
            value={localFrom()}
            onInput={(e) => setLocalFrom(e.currentTarget.value)}
          />
        </div>

        {/* Date To */}
        <div class="flex items-center gap-2">
          <label class="font-mono text-xs text-text-tertiary uppercase tracking-wider">To</label>
          <input
            type="date"
            class="bg-elevated border border-container-border text-text-primary font-mono text-sm px-3 py-1.5 rounded focus-visible:border-signal-green focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-signal-green/30 focus-visible:ring-offset-1 focus-visible:ring-offset-main-bg"
            value={localTo()}
            onInput={(e) => setLocalTo(e.currentTarget.value)}
          />
        </div>

        {/* Actions */}
        <button
          onClick={apply}
          class="font-mono text-sm px-4 py-1.5 border border-text-primary text-text-primary hover:bg-text-primary hover:text-main-bg rounded transition-colors"
        >
          APPLY
        </button>
        <button
          onClick={clear}
          class="font-mono text-sm px-4 py-1.5 border border-container-border text-text-secondary hover:text-text-primary hover:border-text-secondary rounded transition-colors"
        >
          CLEAR
        </button>
      </div>
    </div>
  )
}
