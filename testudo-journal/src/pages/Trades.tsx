import { createSignal, Show } from 'solid-js'
import { TradeTable } from '../components/trades/TradeTable'
import { TradeDetail } from '../components/trades/TradeDetail'
import { GhostAnnotation } from '../components/GhostAnnotation'

export function Trades() {
  const [selectedTradeId, setSelectedTradeId] = createSignal<string | null>(null)

  return (
    <div>
      <div class="mb-4">
        <GhostAnnotation text="TRADE_HISTORY" />
        <h1 class="text-2xl md:text-3xl font-display font-bold tracking-tight">TRADES</h1>
      </div>
      <div class="border border-container-border bg-container-bg rounded-lg">
        <TradeTable onSelectTrade={(id) => setSelectedTradeId(id)} />
        <Show when={selectedTradeId()}>
          {(id) => (
            <TradeDetail
              tradeId={id()}
              onClose={() => setSelectedTradeId(null)}
            />
          )}
        </Show>
      </div>
    </div>
  )
}
