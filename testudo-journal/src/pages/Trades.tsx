import { createSignal, Show } from 'solid-js'
import { TradeTable } from '../components/trades/TradeTable'
import { TradeDetail } from '../components/trades/TradeDetail'
import { PageSubHeader } from '../components/PageSubHeader'

export function Trades() {
  const [selectedTradeId, setSelectedTradeId] = createSignal<string | null>(null)

  return (
    <div>
      <PageSubHeader title="JOURNAL" />
      <div class="border border-container-border bg-container-bg">
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
