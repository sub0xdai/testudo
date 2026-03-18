import { createSignal } from 'solid-js'

export interface TradeFilterState {
  side?: string
  symbol?: string
  exchange?: string
  tag?: string
}

export function TradeFilters(props: {
  filters: TradeFilterState
  onChange: (f: TradeFilterState) => void
}) {
  const [symbol, setSymbol] = createSignal(props.filters.symbol ?? '')

  function toggleSide(side: string) {
    props.onChange({
      ...props.filters,
      side: props.filters.side === side ? undefined : side,
    })
  }

  return (
    <div class="flex flex-wrap items-center gap-3 py-3 border-b border-container-border">
      {/* Side toggle */}
      <div class="flex gap-1 font-mono text-xs">
        <button
          class={`px-2 py-1 border transition-colors ${
            props.filters.side === 'long'
              ? 'border-signal-green text-signal-green'
              : 'border-container-border text-text-secondary hover:text-text-primary'
          }`}
          onClick={() => toggleSide('long')}
        >
          LONG
        </button>
        <button
          class={`px-2 py-1 border transition-colors ${
            props.filters.side === 'short'
              ? 'border-signal-red text-signal-red'
              : 'border-container-border text-text-secondary hover:text-text-primary'
          }`}
          onClick={() => toggleSide('short')}
        >
          SHORT
        </button>
      </div>

      {/* Symbol search */}
      <input
        type="text"
        placeholder="SYMBOL"
        value={symbol()}
        onInput={(e) => setSymbol(e.currentTarget.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            props.onChange({ ...props.filters, symbol: symbol() || undefined })
          }
        }}
        onBlur={() => props.onChange({ ...props.filters, symbol: symbol() || undefined })}
        class="w-28 px-2 py-1 bg-container-bg border border-container-border text-text-primary text-xs font-mono placeholder:text-text-tertiary focus:border-border-active focus:outline-none"
      />

      {/* Exchange dropdown */}
      <select
        value={props.filters.exchange ?? ''}
        onChange={(e) => props.onChange({ ...props.filters, exchange: e.currentTarget.value || undefined })}
        class="px-2 py-1 bg-container-bg border border-container-border text-text-primary text-xs font-mono focus:border-border-active focus:outline-none appearance-none cursor-pointer"
      >
        <option value="">ALL EXCH</option>
        <option value="WOO">WOO</option>
        <option value="BINANCE">BINANCE</option>
        <option value="HYPERLIQUID">HL</option>
      </select>

      {/* Tag filter */}
      <input
        type="text"
        placeholder="TAG"
        value={props.filters.tag ?? ''}
        onInput={(e) => props.onChange({ ...props.filters, tag: e.currentTarget.value || undefined })}
        class="w-24 px-2 py-1 bg-container-bg border border-container-border text-text-primary text-xs font-mono placeholder:text-text-tertiary focus:border-border-active focus:outline-none"
      />

      {/* Clear */}
      {(props.filters.side || props.filters.symbol || props.filters.exchange || props.filters.tag) && (
        <button
          class="px-2 py-1 text-xs font-mono text-text-secondary hover:text-signal-red transition-colors"
          onClick={() => props.onChange({})}
        >
          CLEAR
        </button>
      )}
    </div>
  )
}
