import { Show } from 'solid-js'

export interface TradeFilterState {
  side?: string
  tag?: string
}

export function TradeFilters(props: {
  filters: TradeFilterState
  onChange: (f: TradeFilterState) => void
  onSync?: () => Promise<void>
  syncing?: boolean
  syncMessage?: string
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
      <label for="trade-filter-tag" class="flex items-center gap-1.5">
        <span class="font-mono text-[10px] text-text-tertiary uppercase tracking-wider">Tag</span>
        <input
          id="trade-filter-tag"
          type="text"
          placeholder="TAG"
          value={props.filters.tag ?? ''}
          onInput={(e) => props.onChange({ ...props.filters, tag: e.currentTarget.value || undefined })}
          class="w-24 px-2 py-1 bg-container-bg border border-container-border text-text-primary text-xs font-mono placeholder:text-text-tertiary"
        />
      </label>

      {/* Clear */}
      {(props.filters.side || props.filters.tag) && (
        <button
          class="px-2 py-1 text-xs font-mono text-text-secondary hover:text-signal-red transition-colors"
          onClick={() => props.onChange({})}
        >
          CLEAR
        </button>
      )}

      {/* Sync now */}
      <Show when={props.onSync}>
        <div class="ml-auto flex items-center gap-2">
          <Show when={props.syncMessage}>
            <span class="text-[10px] font-mono text-text-tertiary">{props.syncMessage}</span>
          </Show>
          <button
            class="px-2 py-1 text-[10px] font-mono border border-container-border text-text-secondary hover:border-text-secondary hover:text-text-primary transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            disabled={props.syncing}
            onClick={() => props.onSync?.()}
          >
            {props.syncing ? 'SYNCING…' : 'SYNC NOW'}
          </button>
        </div>
      </Show>
    </div>
  )
}
