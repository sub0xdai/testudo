/** @anchor ui:journal:AddExchangeForm
 * @tags ui */

import { createSignal, Show, For } from 'solid-js'
import { WalletConnectFlow } from './WalletConnectFlow'
import { exchangeApi } from '../../api/client'
import type { ExchangeInfo } from '../../api/client'

interface AddExchangeFormProps {
  exchanges: ExchangeInfo[]
  onSuccess: () => void
  onCancel?: () => void
  initialExchange?: string
}

export function AddExchangeForm(props: AddExchangeFormProps) {
  const [selectedExchange, setSelectedExchange] = createSignal(props.initialExchange ?? '')
  const [apiKey, setApiKey] = createSignal('')
  const [apiSecret, setApiSecret] = createSignal('')
  const [passphrase, setPassphrase] = createSignal('')
  const [error, setError] = createSignal('')
  const [submitting, setSubmitting] = createSignal(false)

  const needsPassphrase = () => ['okx', 'kucoin', 'bitget', 'blofin'].includes(selectedExchange())
  const isHyperliquid = () => selectedExchange() === 'hyperliquid'

  function handleExchangeChange(value: string) {
    setSelectedExchange(value)
    setApiKey('')
    setApiSecret('')
    setPassphrase('')
    setError('')
  }

  async function handleSubmit(e: Event) {
    e.preventDefault()
    const exchange = selectedExchange()
    if (!exchange) return

    const key = apiKey().trim()
    const secret = apiSecret().trim()

    if (!key) { setError('API key is required'); return }
    if (!secret) { setError('API secret is required'); return }
    if (needsPassphrase() && !passphrase().trim()) {
      setError('Passphrase is required for this exchange')
      return
    }

    setSubmitting(true)
    setError('')

    try {
      const info = props.exchanges.find(e => e.id === exchange)
      await exchangeApi.addAccount({
        exchange_name: exchange,
        account_name: info?.name ?? exchange,
        api_key: key,
        secret: secret,
        ...(needsPassphrase() ? { passphrase: passphrase().trim() } : {}),
      })
      props.onSuccess()
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to add account')
      setSubmitting(false)
    }
  }

  return (
    <div class="space-y-4">
      {/* Exchange dropdown */}
      <div>
        <label for="exchange-select" class="block font-mono text-[10px] tracking-widest text-text-tertiary mb-2">
          EXCHANGE
        </label>
        <select
          id="exchange-select"
          aria-label="Exchange"
          value={selectedExchange()}
          onChange={(e) => handleExchangeChange(e.currentTarget.value)}
          class="w-full px-4 py-3 bg-main-bg/50 border border-container-border font-mono text-sm text-text-primary focus:border-text-secondary focus:outline-none"
        >
          <option value="">Select exchange...</option>
          <For each={props.exchanges}>
            {(ex) => (
              <option value={ex.id} class="bg-container-bg text-text-primary">
                {ex.name}
              </option>
            )}
          </For>
        </select>
      </div>

      {/* Hyperliquid: wallet connect flow */}
      <Show when={selectedExchange() && isHyperliquid()}>
        <WalletConnectFlow onComplete={props.onSuccess} />
      </Show>

      {/* Traditional exchange: API key form */}
      <Show when={selectedExchange() && !isHyperliquid()}>
        <form onSubmit={handleSubmit} class="space-y-4">
          <div>
            <label for="exchange-api-key" class="block font-mono text-[10px] tracking-widest text-text-tertiary mb-2">
              API KEY
            </label>
            <input
              id="exchange-api-key"
              type="password"
              value={apiKey()}
              onInput={(e) => setApiKey(e.currentTarget.value)}
              class="w-full px-4 py-3 bg-main-bg/50 border border-container-border font-mono text-sm text-text-primary placeholder-text-tertiary focus:border-text-secondary focus:outline-none"
              placeholder="Enter API key"
              autocomplete="off"
            />
          </div>

          <div>
            <label for="exchange-api-secret" class="block font-mono text-[10px] tracking-widest text-text-tertiary mb-2">
              SECRET
            </label>
            <input
              id="exchange-api-secret"
              type="password"
              value={apiSecret()}
              onInput={(e) => setApiSecret(e.currentTarget.value)}
              class="w-full px-4 py-3 bg-main-bg/50 border border-container-border font-mono text-sm text-text-primary placeholder-text-tertiary focus:border-text-secondary focus:outline-none"
              placeholder="Enter API secret"
              autocomplete="off"
            />
          </div>

          <Show when={needsPassphrase()}>
            <div>
              <label for="exchange-passphrase" class="block font-mono text-[10px] tracking-widest text-text-tertiary mb-2">
                PASSPHRASE
              </label>
              <input
                id="exchange-passphrase"
                type="password"
                value={passphrase()}
                onInput={(e) => setPassphrase(e.currentTarget.value)}
                class="w-full px-4 py-3 bg-main-bg/50 border border-container-border font-mono text-sm text-text-primary placeholder-text-tertiary focus:border-text-secondary focus:outline-none"
                placeholder="Enter passphrase"
                autocomplete="off"
              />
            </div>
          </Show>

          <Show when={error()}>
            <div role="alert" class="px-4 py-3 border border-signal-red bg-signal-red/10 font-mono text-xs text-signal-red">
              {error()}
            </div>
          </Show>

          <div class="flex gap-3">
            <button
              type="submit"
              disabled={submitting() || !apiKey() || !apiSecret()}
              class="btn-primary py-3 px-6 disabled:opacity-50"
            >
              {submitting() ? 'VALIDATING...' : '[ CONNECT EXCHANGE ]'}
            </button>
            <Show when={props.onCancel}>
              <button
                type="button"
                onClick={props.onCancel}
                class="btn-ghost px-6 py-3"
              >
                CANCEL
              </button>
            </Show>
          </div>
        </form>
      </Show>
    </div>
  )
}
