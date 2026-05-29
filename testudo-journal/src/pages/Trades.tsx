/** @anchor ui:journal-page:Trades
 * @tags ui */

import { createEffect, createSignal, Show } from 'solid-js'
import { useSearchParams } from '@solidjs/router'
import { TradeTable } from '../components/trades/TradeTable'
import { TradeDetail } from '../components/trades/TradeDetail'
import { PageSubHeader } from '../components/PageSubHeader'
import { HELP } from '../lib/help-content'

export function Trades() {
  const [selectedTradeId, setSelectedTradeId] = createSignal<string | null>(null)
  const [searchParams, setSearchParams] = useSearchParams()

  // Deep-link: /desk/trades?trade={uuid} pre-opens the detail modal (used by coach citations).
  createEffect(() => {
    const raw = searchParams.trade
    const id = Array.isArray(raw) ? raw[0] : raw
    if (id && id !== selectedTradeId()) {
      setSelectedTradeId(id)
    }
  })

  function closeDetail() {
    setSelectedTradeId(null)
    if (searchParams.trade) {
      setSearchParams({ ...searchParams, trade: undefined }, { replace: true })
    }
  }

  return (
    <div class="flex flex-col h-full">
      <PageSubHeader title="JOURNAL" helpText={HELP['page.journal']} />
      <div class="flex-1 min-h-0 overflow-y-auto border-t-0 border border-container-border bg-container-bg">
        <TradeTable onSelectTrade={(id) => {
          setSelectedTradeId(id)
        }} />
        <Show when={selectedTradeId()}>
          {(id) => (
            <TradeDetail
              tradeId={id()}
              isActive={false}
              onClose={closeDetail}
            />
          )}
        </Show>
      </div>
    </div>
  )
}
