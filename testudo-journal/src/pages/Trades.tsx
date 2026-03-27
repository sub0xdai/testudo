import { createSignal, Show } from 'solid-js'
import { TradeTable } from '../components/trades/TradeTable'
import { TradeDetail } from '../components/trades/TradeDetail'
import { ActivePositions } from '../components/trades/ActivePositions'
import { PageSubHeader } from '../components/PageSubHeader'

export function Trades() {
  const [selectedTradeId, setSelectedTradeId] = createSignal<string | null>(null)
  const [isActiveTrade, setIsActiveTrade] = createSignal(false)

  return (
    <div>
      <PageSubHeader title="JOURNAL" />
      <ActivePositions onSelectTrade={(id) => {
        setSelectedTradeId(id)
        setIsActiveTrade(true)
      }} />
      <div class="border border-container-border bg-container-bg">
        <TradeTable onSelectTrade={(id) => {
          setSelectedTradeId(id)
          setIsActiveTrade(false)
        }} />
        <Show when={selectedTradeId()}>
          {(id) => (
            <TradeDetail
              tradeId={id()}
              isActive={isActiveTrade()}
              onClose={() => setSelectedTradeId(null)}
            />
          )}
        </Show>
      </div>
    </div>
  )
}
