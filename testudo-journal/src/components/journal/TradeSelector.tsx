/** @anchor ui:journal:TradeSelector
 * @tags ui */

import { createSignal, createResource, Show, For } from 'solid-js'
import { fetchTrades, type JournalTrade } from '../../api/client'
import { formatCurrency, formatDateFull } from '../../lib/formatters'
import { useEscapeClose } from '../../lib/useEscapeClose'

export function TradeSelector(props: {
  value: JournalTrade | null
  onSelect: (trade: JournalTrade | null) => void
}) {
  const [search, setSearch] = createSignal('')
  const [open, setOpen] = createSignal(false)
  useEscapeClose(() => setOpen(false))

  const [trades] = createResource(
    () => ({ symbol: search() || undefined, limit: 20, sort: 'closed_at', order: 'desc' as const }),
    (params) => fetchTrades(params).then((r) => r.trades),
  )

  return (
    <div class="relative">
      <Show
        when={!props.value}
        fallback={
          <div class="flex items-center gap-2 bg-container-bg border border-container-border px-3 py-2">
            <span class="font-mono text-sm text-text-primary flex-1 truncate">
              {props.value!.symbol} {props.value!.side.toUpperCase()} {formatDateFull(props.value!.closed_at)} ({formatCurrency(props.value!.net_pnl)})
            </span>
            <button
              class="btn-ghost text-text-tertiary hover:text-signal-red transition-colors"
              onClick={() => props.onSelect(null)}
              aria-label="Clear trade selection"
            >
              &times;
            </button>
          </div>
        }
      >
        <input
          type="text"
          placeholder="Search trades by symbol..."
          class="w-full bg-container-bg border border-container-border px-3 py-2 font-mono text-sm text-text-primary placeholder:text-text-tertiary"
          value={search()}
          onInput={(e) => { setSearch(e.currentTarget.value); setOpen(true) }}
          onFocus={() => setOpen(true)}
          aria-haspopup="listbox"
          aria-expanded={open()}
          aria-controls="trade-listbox"
        />
      </Show>

      <Show when={open() && !props.value}>
        <div id="trade-listbox" role="listbox" aria-label="Trade results" class="absolute z-50 top-full left-0 right-0 mt-1 bg-elevated border border-container-border shadow-lg shadow-black/30 max-h-48 overflow-y-auto animate-dropdown-in">
          <Show when={trades.loading}>
            <div class="px-3 py-2 space-y-1.5">
              <div class="h-3 bg-container-border/15 skeleton-shimmer" style={{ width: '80%' }} />
              <div class="h-3 bg-container-border/15 skeleton-shimmer" style={{ width: '60%' }} />
            </div>
          </Show>
          <Show when={!trades.loading && trades()?.length === 0}>
            <div class="px-3 py-2 font-mono text-xs text-text-tertiary">No trades found</div>
          </Show>
          <For each={trades()}>
            {(trade) => (
              <button
                role="option"
                class="w-full text-left px-3 py-2 hover:bg-container-bg-hover transition-colors flex items-center gap-2"
                onClick={() => { props.onSelect(trade); setOpen(false); setSearch('') }}
              >
                <span class="font-mono text-xs text-text-primary">{trade.symbol}</span>
                <span class={`font-mono text-[10px] ${trade.side === 'long' ? 'text-signal-green' : 'text-signal-red'}`}>
                  {trade.side.toUpperCase()}
                </span>
                <span class="font-mono text-xs text-text-tertiary flex-1 text-right">
                  {formatDateFull(trade.closed_at)}
                </span>
                <span class={`font-mono text-xs ${parseFloat(trade.net_pnl) >= 0 ? 'text-signal-green' : 'text-signal-red'}`}>
                  {formatCurrency(trade.net_pnl)}
                </span>
              </button>
            )}
          </For>
        </div>
      </Show>

      {/* Click outside to close */}
      <Show when={open()}>
        <div class="fixed inset-0 z-40" onClick={() => setOpen(false)} />
      </Show>
    </div>
  )
}
