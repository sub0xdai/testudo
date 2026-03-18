export interface TradeFilterState {
  side?: string
  tag?: string
}

export function TradeFilters(props: {
  filters: TradeFilterState
  onChange: (f: TradeFilterState) => void
}) {
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
              ? 'border-text-primary text-text-primary'
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

      {/* Tag filter */}
      <input
        type="text"
        placeholder="TAG"
        value={props.filters.tag ?? ''}
        onInput={(e) => props.onChange({ ...props.filters, tag: e.currentTarget.value || undefined })}
        class="w-24 px-2 py-1 bg-container-bg border border-container-border text-text-primary text-xs font-mono placeholder:text-text-tertiary focus-ring"
      />

      {/* Clear */}
      {(props.filters.side || props.filters.tag) && (
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
