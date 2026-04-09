import { createSignal, Show } from 'solid-js'
import { TradeTable } from '../components/trades/TradeTable'
import { TradeDetail } from '../components/trades/TradeDetail'
import { PageSubHeader } from '../components/PageSubHeader'

export function Trades() {
  const [selectedTradeId, setSelectedTradeId] = createSignal<string | null>(null)

  return (
    <div class="flex flex-col h-full">
      <PageSubHeader title="JOURNAL" />
      <div class="flex-1 min-h-0 overflow-y-auto border-t-0 border border-container-border bg-container-bg">
        <TradeTable onSelectTrade={(id) => {
          setSelectedTradeId(id)
        }} />
        <Show when={selectedTradeId()}>
          {(id) => (
            <TradeDetail
              tradeId={id()}
              isActive={false}
              onClose={() => setSelectedTradeId(null)}
            />
          )}
        </Show>
      </div>
    </div>
  )
}
