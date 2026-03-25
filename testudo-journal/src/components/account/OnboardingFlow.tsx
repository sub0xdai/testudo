import { createSignal, Show, For } from 'solid-js'
import { WalletConnectFlow } from './WalletConnectFlow'
import { exchangeApi } from '../../api/client'
import type { ExchangeInfo, ExchangeAccount } from '../../api/client'

// ─── Types ───

interface OnboardingFlowProps {
  exchanges: ExchangeInfo[]
  onComplete: (account: ExchangeAccount) => void
}

type OnboardingStep = 'select' | 'credentials' | 'submitting' | 'success'

// ─── Component ───

export function OnboardingFlow(props: OnboardingFlowProps) {
  const [step, setStep] = createSignal<OnboardingStep>('select')
  const [selectedExchange, setSelectedExchange] = createSignal('')
  const [apiKey, setApiKey] = createSignal('')
  const [apiSecret, setApiSecret] = createSignal('')
  const [passphrase, setPassphrase] = createSignal('')
  const [error, setError] = createSignal('')
  const [createdAccount, setCreatedAccount] = createSignal<ExchangeAccount | null>(null)

  const needsPassphrase = () => {
    const ex = selectedExchange()
    return ex === 'okx' || ex === 'kucoin'
  }

  const isHyperliquid = () => selectedExchange() === 'hyperliquid'

  const selectedExchangeInfo = () =>
    props.exchanges.find(e => e.name === selectedExchange())

  function clearForm() {
    setApiKey('')
    setApiSecret('')
    setPassphrase('')
    setError('')
  }

  function handleExchangeChange(value: string) {
    setSelectedExchange(value)
    clearForm()
    if (value) {
      setStep('credentials')
    } else {
      setStep('select')
    }
  }

  async function handleSubmit() {
    const exchange = selectedExchange()
    if (!exchange) return

    const key = apiKey().trim()
    const secret = apiSecret().trim()

    if (!key) {
      setError('API key is required')
      return
    }
    if (!secret) {
      setError('API secret is required')
      return
    }
    if (needsPassphrase() && !passphrase().trim()) {
      setError('Passphrase is required for this exchange')
      return
    }

    setStep('submitting')
    setError('')

    try {
      const info = selectedExchangeInfo()
      const account = await exchangeApi.addAccount({
        exchange_name: exchange,
        account_name: info?.display_name ?? exchange,
        api_key: key,
        api_secret: secret,
        ...(needsPassphrase() ? { passphrase: passphrase().trim() } : {}),
      })
      setCreatedAccount(account)
      setStep('success')
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : 'Failed to add account'
      setError(message)
      setStep('credentials')
    }
  }

  function handleWalletComplete() {
    // Wallet flow handles its own account creation; signal completion upstream
    // We create a minimal account representation since the backend already persisted it
    props.onComplete({
      id: '',
      exchange_name: 'hyperliquid',
      account_name: 'Hyperliquid',
      is_active: true,
      auth_mode: 'agent_wallet',
      created_at: new Date().toISOString(),
    })
  }

  function handleDone() {
    const account = createdAccount()
    if (account) {
      props.onComplete(account)
    }
  }

  // ─── Render ───

  return (
    <div class="space-y-6">
      <div>
        <h2 class="font-display text-xl font-bold text-text-primary">
          GET STARTED
        </h2>
        <p class="font-mono text-sm text-text-secondary mt-2">
          Link your exchange API keys to enable trading through the Testudo extension.
          Your credentials are encrypted and stored securely.
        </p>
      </div>

      {/* Success state */}
      <Show when={step() === 'success'}>
        <div class="text-center py-8 space-y-6">
          <div class="w-16 h-16 mx-auto border-2 border-text-primary flex items-center justify-center">
            <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="text-text-primary">
              <polyline points="20 6 9 17 4 12" />
            </svg>
          </div>
          <h3 class="font-display text-2xl font-bold text-text-primary tracking-wider">
            EXCHANGE CONNECTED
          </h3>
          <p class="font-mono text-sm text-text-secondary max-w-md mx-auto">
            Your exchange has been validated and configured.
          </p>
          <button
            onClick={handleDone}
            class="px-8 py-3 bg-transparent btn-primary font-mono font-bold text-sm"
          >
            VIEW ACCOUNT
          </button>
        </div>
      </Show>

      {/* Exchange selector + form */}
      <Show when={step() !== 'success'}>
        <div class="space-y-4">
          {/* Exchange dropdown */}
          <div>
            <label class="block font-mono text-sm text-text-secondary mb-2">
              EXCHANGE
            </label>
            <select
              value={selectedExchange()}
              onChange={(e) => handleExchangeChange(e.currentTarget.value)}
              class="w-full px-4 py-3 bg-container-bg border border-container-border font-mono text-text-primary focus:border-text-secondary focus:outline-none"
            >
              <option value="">Select exchange...</option>
              <For each={props.exchanges}>
                {(ex) => (
                  <option value={ex.name}>
                    {ex.display_name}
                  </option>
                )}
              </For>
            </select>
          </div>

          {/* Hyperliquid: wallet connect flow */}
          <Show when={selectedExchange() && isHyperliquid()}>
            <WalletConnectFlow onComplete={handleWalletComplete} />
          </Show>

          {/* Traditional exchange: API key form */}
          <Show when={selectedExchange() && !isHyperliquid()}>
            <div class="space-y-4">
              <div>
                <label class="block font-mono text-sm text-text-secondary mb-2">
                  API KEY
                </label>
                <input
                  type="password"
                  value={apiKey()}
                  onInput={(e) => setApiKey(e.currentTarget.value)}
                  class="w-full px-4 py-3 bg-container-bg border border-container-border font-mono text-text-primary placeholder-text-tertiary focus:border-text-secondary focus:outline-none"
                  placeholder="Enter API key"
                  autocomplete="off"
                />
              </div>

              <div>
                <label class="block font-mono text-sm text-text-secondary mb-2">
                  SECRET
                </label>
                <input
                  type="password"
                  value={apiSecret()}
                  onInput={(e) => setApiSecret(e.currentTarget.value)}
                  class="w-full px-4 py-3 bg-container-bg border border-container-border font-mono text-text-primary placeholder-text-tertiary focus:border-text-secondary focus:outline-none"
                  placeholder="Enter API secret"
                  autocomplete="off"
                />
              </div>

              <Show when={needsPassphrase()}>
                <div>
                  <label class="block font-mono text-sm text-text-secondary mb-2">
                    PASSPHRASE
                  </label>
                  <input
                    type="password"
                    value={passphrase()}
                    onInput={(e) => setPassphrase(e.currentTarget.value)}
                    class="w-full px-4 py-3 bg-container-bg border border-container-border font-mono text-text-primary placeholder-text-tertiary focus:border-text-secondary focus:outline-none"
                    placeholder="Enter passphrase"
                    autocomplete="off"
                  />
                </div>
              </Show>

              <Show when={error()}>
                <div class="px-4 py-3 border border-signal-red bg-signal-red/10 font-mono text-sm text-signal-red">
                  {error()}
                </div>
              </Show>

              <button
                onClick={handleSubmit}
                disabled={step() === 'submitting'}
                class="w-full px-8 py-4 bg-transparent btn-primary font-mono font-bold text-lg disabled:opacity-50"
              >
                {step() === 'submitting' ? 'VALIDATING...' : 'CONNECT EXCHANGE'}
              </button>
            </div>
          </Show>
        </div>
      </Show>
    </div>
  )
}
