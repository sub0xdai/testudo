import { createSignal, createResource, Show, For } from 'solid-js'
import { fetchTrades, type JournalTrade } from '../../api/client'
import { formatCurrency, formatDateFull } from '../../lib/formatters'

export function TradeSelector(props: {
  value: JournalTrade | null
  onSelect: (trade: JournalTrade | null) => void
}) {
  const [search, setSearch] = createSignal('')
  const [open, setOpen] = createSignal(false)

  const [trades] = createResource(
    () => ({ symbol: search() || undefined, limit: 20, sort: 'closed_at', order: 'desc' as const }),
    (params) => fetchTrades(params).then((r) => r.trades),
  )

  return (
    <div class="relative">
      <Show
        when={!props.value}
        fallback={
          <div class="flex items-center gap-2 bg-container-bg border border-container-border rounded px-3 py-2">
            <span class="font-mono text-sm text-text-primary flex-1 truncate">
              {props.value!.symbol} {props.value!.side.toUpperCase()} {formatDateFull(props.value!.closed_at)} ({formatCurrency(props.value!.net_pnl)})
            </span>
            <button
              class="font-mono text-xs text-text-tertiary hover:text-signal-red transition-colors"
              onClick={() => props.onSelect(null)}
            >
              &times;
            </button>
          </div>
        }
      >
        <input
          type="text"
          placeholder="Search trades by symbol..."
          class="w-full bg-container-bg border border-container-border rounded px-3 py-2 font-mono text-sm text-text-primary placeholder:text-text-tertiary focus-visible:border-border-active focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-signal-green/30 focus-visible:ring-offset-1 focus-visible:ring-offset-main-bg"
          value={search()}
          onInput={(e) => { setSearch(e.currentTarget.value); setOpen(true) }}
          onFocus={() => setOpen(true)}
        />
      </Show>

      <Show when={open() && !props.value}>
        <div class="absolute z-50 top-full left-0 right-0 mt-1 bg-elevated border border-container-border rounded-lg max-h-48 overflow-y-auto shadow-lg">
          <Show when={trades.loading}>
            <div class="px-3 py-2 font-mono text-xs text-text-tertiary animate-pulse">Loading...</div>
          </Show>
          <Show when={!trades.loading && trades()?.length === 0}>
            <div class="px-3 py-2 font-mono text-xs text-text-tertiary">No trades found</div>
          </Show>
          <For each={trades()}>
            {(trade) => (
              <button
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
