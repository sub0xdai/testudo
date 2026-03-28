import { createSignal, createResource, For, Show, onMount } from 'solid-js'
import {
  exchangeApi,
  type ExchangeInfo,
  type ExchangeAccount,
  type TestConnectionResult,
  type ExchangeBalanceResponse,
} from '../api/client'
import { ExchangeCard } from '../components/account/ExchangeCard'
import { AddExchangeCard } from '../components/account/AddExchangeCard'
import { ExtensionPairingBanner } from '../components/account/ExtensionPairingBanner'
import { OnboardingFlow } from '../components/account/OnboardingFlow'
import { WalletConnectFlow } from '../components/account/WalletConnectFlow'

export default function Account() {
  const [accounts, { refetch: refetchAccounts }] = createResource(async () => {
    return exchangeApi.listAccounts()
  })

  const [exchanges] = createResource(async () => {
    return exchangeApi.listExchanges()
  })

  const [balances, setBalances] = createSignal<Record<string, ExchangeBalanceResponse>>({})
  const [testResults, setTestResults] = createSignal<Record<string, TestConnectionResult>>({})
  const [testingId, setTestingId] = createSignal<string | null>(null)
  const [deletingId, setDeletingId] = createSignal<string | null>(null)
  const [revokingId, setRevokingId] = createSignal<string | null>(null)
  const [importingId, setImportingId] = createSignal<string | null>(null)
  const [importedExchanges, setImportedExchanges] = createSignal<Set<string>>(new Set())
  const [showForm, setShowForm] = createSignal(false)

  // Check which exchanges have completed imports
  onMount(async () => {
    try {
      const API_BASE = import.meta.env.VITE_API_URL || ''
      const res = await fetch(`${API_BASE}/api/v1/trades/import/status`, { credentials: 'include' })
      if (res.ok) {
        const jobs: Array<{ exchange_name: string; status: string }> = await res.json()
        const done = new Set(jobs.filter(j => j.status === 'completed').map(j => j.exchange_name))
        setImportedExchanges(done)
      }
    } catch { /* non-critical */ }
  })
  const [showWalletConnect, setShowWalletConnect] = createSignal(false)
  const [setupComplete, setSetupComplete] = createSignal(false)
  const [error, setError] = createSignal('')

  // Form state
  const [formExchange, setFormExchange] = createSignal('')
  const [formApiKey, setFormApiKey] = createSignal('')
  const [formSecret, setFormSecret] = createSignal('')
  const [formPassphrase, setFormPassphrase] = createSignal('')
  const [formSubmitting, setFormSubmitting] = createSignal(false)

  // Fetch balances for all accounts
  function fetchBalances(accs: ExchangeAccount[]) {
    for (const acc of accs) {
      exchangeApi.fetchBalance(acc.id)
        .then(b => setBalances(prev => ({ ...prev, [acc.id]: b })))
        .catch(() => {})
    }
  }

  // Refetch when accounts load
  createResource(() => accounts(), (accs) => {
    if (accs) fetchBalances(accs)
    return accs
  })

  const isOnboarding = () => !accounts.loading && (accounts()?.length ?? 0) === 0 && !setupComplete()
  const needsPassphrase = () => formExchange() === 'okx' || formExchange() === 'kucoin'

  async function handleTest(id: string) {
    setTestingId(id)
    try {
      const result = await exchangeApi.testConnection(id)
      setTestResults(prev => ({ ...prev, [id]: result }))
    } catch {
      setTestResults(prev => ({ ...prev, [id]: { success: false, latency_ms: null, error: 'Connection failed' } }))
    } finally {
      setTestingId(null)
    }
  }

  async function handleDelete(id: string) {
    setDeletingId(id)
    try {
      await exchangeApi.deleteAccount(id)
      refetchAccounts()
    } catch {
      setError('Failed to delete account')
    } finally {
      setDeletingId(null)
    }
  }

  async function handleRevoke(id: string) {
    setRevokingId(id)
    try {
      await exchangeApi.revokeAgent(id)
      refetchAccounts()
    } catch {
      setError('Failed to revoke agent')
    } finally {
      setRevokingId(null)
    }
  }

  async function handleImport(exchangeName: string) {
    if (importedExchanges().has(exchangeName)) return // already imported
    setImportingId(exchangeName)
    try {
      const API_BASE = import.meta.env.VITE_API_URL || ''
      const res = await fetch(`${API_BASE}/api/v1/trades/import`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({ exchange_name: exchangeName }),
      })
      if (res.ok) {
        setImportedExchanges(prev => new Set([...prev, exchangeName]))
      }
    } catch {
      setError('Failed to trigger import')
    } finally {
      setImportingId(null)
    }
  }

  async function handleAddAccount(e: Event) {
    e.preventDefault()
    setFormSubmitting(true)
    setError('')
    try {
      await exchangeApi.addAccount({
        exchange_name: formExchange(),
        account_name: `${formExchange()}-main`,
        api_key: formApiKey(),
        api_secret: formSecret(),
        ...(needsPassphrase() ? { passphrase: formPassphrase() } : {}),
      })
      setShowForm(false)
      setFormExchange('')
      setFormApiKey('')
      setFormSecret('')
      setFormPassphrase('')
      setSetupComplete(true)
      refetchAccounts()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to add account')
    } finally {
      setFormSubmitting(false)
    }
  }

  return (
    <div>
      <Show when={!isOnboarding()}>
        <div class="flex items-center justify-between mb-6">
          <h1 class="text-2xl md:text-3xl font-display font-bold tracking-tight text-text-primary">ACCOUNT</h1>
        </div>
      </Show>

      <Show when={error()}>
        <div class="border border-signal-red bg-signal-red/10 p-4 mb-6 font-mono text-sm text-signal-red">
          {error()}
        </div>
      </Show>

      <Show when={!accounts.loading} fallback={
        <div class="flex items-center justify-center py-20">
          <div class="w-4 h-4 border-2 border-text-secondary border-t-text-primary rounded-full animate-spin" />
        </div>
      }>
        {/* Onboarding flow for first-time users */}
        <Show when={isOnboarding()}>
          <Show when={exchanges()}>
            {(exs) => (
              <OnboardingFlow
                exchanges={exs()}
                onComplete={() => {
                  setSetupComplete(true)
                  refetchAccounts()
                }}
              />
            )}
          </Show>
        </Show>

        {/* Normal account management */}
        <Show when={!isOnboarding()}>
          {/* Exchange card grid */}
          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-8">
            <For each={accounts()}>
              {(acc) => (
                <ExchangeCard
                  account={acc}
                  testResult={testResults()[acc.id]}
                  balance={balances()[acc.id]}
                  isTesting={testingId() === acc.id}
                  isDeleting={deletingId() === acc.id}
                  isRevoking={revokingId() === acc.id}
                  onTest={() => handleTest(acc.id)}
                  onDelete={() => handleDelete(acc.id)}
                  onRevoke={() => handleRevoke(acc.id)}
                  onMigrate={() => {/* TODO: migrate to agent wallet */}}
                  onImport={() => handleImport(acc.exchange_name)}
                  isImporting={importingId() === acc.exchange_name}
                  isImported={importedExchanges().has(acc.exchange_name)}
                />
              )}
            </For>
            <AddExchangeCard onClick={() => setShowForm(true)} />
          </div>

          {/* Add exchange form */}
          <Show when={showForm()}>
            <div class="flex justify-center mb-8">
            <div class="border border-container-border bg-main-bg/75 backdrop-blur-md p-8 w-full max-w-lg">
              <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-4">// ADD_EXCHANGE</div>
              <h3 class="font-mono text-sm font-bold text-text-primary mb-4">ADD EXCHANGE</h3>

              <Show when={exchanges()}>
                {(exs) => (
                  <form onSubmit={handleAddAccount}>
                    <div class="mb-4">
                      <label class="block font-mono text-xs text-text-secondary mb-1">EXCHANGE</label>
                      <select
                        value={formExchange()}
                        onInput={(e) => setFormExchange(e.currentTarget.value)}
                        class="w-full bg-main-bg border border-container-border px-3 py-2 font-mono text-sm text-text-primary"
                      >
                        <option value="" class="bg-main-bg text-text-primary">Select exchange...</option>
                        <For each={exs()}>
                          {(ex) => (
                            <option value={ex.id} class="bg-main-bg text-text-primary">{ex.name}</option>
                          )}
                        </For>
                      </select>
                    </div>

                    {/* Hyperliquid uses agent wallet flow instead of API keys */}
                    <Show when={formExchange() === 'hyperliquid'}>
                      <WalletConnectFlow onComplete={() => {
                        setShowForm(false)
                        setFormExchange('')
                        refetchAccounts()
                      }} />
                    </Show>

                    <Show when={formExchange() && formExchange() !== 'hyperliquid'}>
                      <div class="mb-4">
                        <label class="block font-mono text-xs text-text-secondary mb-1">API KEY</label>
                        <input
                          type="text"
                          value={formApiKey()}
                          onInput={(e) => setFormApiKey(e.currentTarget.value)}
                          class="w-full bg-main-bg border border-container-border px-3 py-2 font-mono text-sm text-text-primary"
                          placeholder="Enter API key"
                        />
                      </div>
                      <div class="mb-4">
                        <label class="block font-mono text-xs text-text-secondary mb-1">API SECRET</label>
                        <input
                          type="password"
                          value={formSecret()}
                          onInput={(e) => setFormSecret(e.currentTarget.value)}
                          class="w-full bg-main-bg border border-container-border px-3 py-2 font-mono text-sm text-text-primary"
                          placeholder="Enter API secret"
                        />
                      </div>
                      <Show when={needsPassphrase()}>
                        <div class="mb-4">
                          <label class="block font-mono text-xs text-text-secondary mb-1">PASSPHRASE</label>
                          <input
                            type="password"
                            value={formPassphrase()}
                            onInput={(e) => setFormPassphrase(e.currentTarget.value)}
                            class="w-full bg-main-bg border border-container-border px-3 py-2 font-mono text-sm text-text-primary"
                            placeholder="Enter passphrase"
                          />
                        </div>
                      </Show>
                      <div class="flex gap-3">
                        <button
                          type="submit"
                          disabled={formSubmitting() || !formApiKey() || !formSecret()}
                          class="px-6 py-2 border border-text-primary text-text-primary font-mono text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors disabled:opacity-50"
                        >
                          {formSubmitting() ? 'CONNECTING...' : 'CONNECT EXCHANGE'}
                        </button>
                        <button
                          type="button"
                          onClick={() => setShowForm(false)}
                          class="px-6 py-2 font-mono text-xs text-text-secondary hover:text-text-primary transition-colors"
                        >
                          CANCEL
                        </button>
                      </div>
                    </Show>
                  </form>
                )}
              </Show>
            </div>
            </div>
          </Show>

          {/* Extension pairing */}
          <ExtensionPairingBanner />
        </Show>
      </Show>
    </div>
  )
}
