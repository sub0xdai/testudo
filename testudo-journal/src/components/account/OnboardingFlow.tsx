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
    props.exchanges.find(e => e.id === selectedExchange())

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
        account_name: info?.name ?? exchange,
        api_key: key,
        secret: secret,
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
    <div class="flex items-center justify-center min-h-[60vh]">
      {/* Success state */}
      <Show when={step() === 'success'}>
        <div class="border border-text-primary bg-main-bg/75 backdrop-blur-md p-10 md:p-14 max-w-lg w-full text-center">
          <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-6">
            // EXCHANGE_CONNECTED
          </div>
          <div class="w-12 h-12 mx-auto border border-signal-green flex items-center justify-center mb-4">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="text-signal-green">
              <polyline points="20 6 9 17 4 12" />
            </svg>
          </div>
          <h3 class="font-mono text-xl font-bold text-text-primary mb-2 tracking-wider">
            CONNECTED
          </h3>
          <p class="font-mono text-xs text-text-secondary mb-8">
            Your exchange has been validated and configured.
          </p>
          <button
            onClick={handleDone}
            class="w-full py-3 border border-text-primary text-text-primary font-mono font-bold text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors"
          >
            [ VIEW ACCOUNT ]
          </button>
        </div>
      </Show>

      {/* Exchange selector + form */}
      <Show when={step() !== 'success'}>
        <div class="border border-container-border bg-main-bg/75 backdrop-blur-md p-8 md:p-10 max-w-2xl w-full">
          <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-6">
            // ADD_EXCHANGE
          </div>
          <h2 class="font-mono text-xl font-bold text-text-primary mb-2 tracking-wider">
            GET STARTED
          </h2>
          <p class="font-mono text-xs text-text-secondary mb-8 leading-relaxed">
            Connect an exchange to enable trading. Credentials are encrypted with AES-256-GCM.
          </p>

          <div class="space-y-4">
            {/* Exchange dropdown */}
            <div>
              <label class="block font-mono text-[10px] tracking-widest text-text-tertiary mb-2">
                EXCHANGE
              </label>
              <select
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
              <WalletConnectFlow onComplete={handleWalletComplete} />
            </Show>

            {/* Traditional exchange: API key form */}
            <Show when={selectedExchange() && !isHyperliquid()}>
              <div class="space-y-4">
                <div>
                  <label class="block font-mono text-[10px] tracking-widest text-text-tertiary mb-2">
                    API KEY
                  </label>
                  <input
                    type="password"
                    value={apiKey()}
                    onInput={(e) => setApiKey(e.currentTarget.value)}
                    class="w-full px-4 py-3 bg-main-bg/50 border border-container-border font-mono text-sm text-text-primary placeholder-text-tertiary focus:border-text-secondary focus:outline-none"
                    placeholder="Enter API key"
                    autocomplete="off"
                  />
                </div>

                <div>
                  <label class="block font-mono text-[10px] tracking-widest text-text-tertiary mb-2">
                    SECRET
                  </label>
                  <input
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
                    <label class="block font-mono text-[10px] tracking-widest text-text-tertiary mb-2">
                      PASSPHRASE
                    </label>
                    <input
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
                  <div class="px-4 py-3 border border-signal-red bg-signal-red/10 font-mono text-xs text-signal-red">
                    {error()}
                  </div>
                </Show>

                <button
                  onClick={handleSubmit}
                  disabled={step() === 'submitting'}
                  class="w-full py-3 border border-text-primary text-text-primary font-mono font-bold text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors disabled:opacity-50"
                >
                  {step() === 'submitting' ? 'VALIDATING...' : '[ CONNECT EXCHANGE ]'}
                </button>
              </div>
            </Show>
          </div>

          <a
            href="https://testudo.vip/docs/07-exchanges"
            target="_blank"
            rel="noopener noreferrer"
            class="block mt-6 font-mono text-[10px] text-text-tertiary hover:text-text-secondary transition-colors text-center"
          >
            Exchange setup guides &rarr;
          </a>
        </div>
      </Show>
    </div>
  )
}
