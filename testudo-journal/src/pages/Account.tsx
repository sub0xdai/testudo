/** @anchor ui:journal-page:Account
 * @tags ui */

import { createSignal, createResource, For, Show } from 'solid-js'
import {
  exchangeApi,
  fetchRiskSnapshot,
  type TestConnectionResult,
} from '../api/client'
import { useAsyncAction } from '../lib/useAsyncAction'
import { HelpTip } from '../components/HelpTip'
import { HELP } from '../lib/help-content'
import { ExchangeCard } from '../components/account/ExchangeCard'
import { AddExchangeCard } from '../components/account/AddExchangeCard'
import { AddExchangeForm } from '../components/account/AddExchangeForm'
import { OnboardingFlow } from '../components/account/OnboardingFlow'
import { WalletConnectFlow } from '../components/account/WalletConnectFlow'
import { CorrelationStack } from '../components/account/CorrelationStack'
import { CoachBanner } from '../components/account/CoachBanner'

export default function Account() {
  const [accounts, { refetch: refetchAccounts }] = createResource(async () => {
    return exchangeApi.listAccounts()
  })

  const [exchanges] = createResource(async () => {
    return exchangeApi.listExchanges()
  })

  const [snapshot] = createResource(fetchRiskSnapshot)

  const [testResults, setTestResults] = createSignal<Record<string, TestConnectionResult>>({})
  const action = useAsyncAction()
  const [showForm, setShowForm] = createSignal(false)
  const [formInitialExchange, setFormInitialExchange] = createSignal('')
  const [reauthAccountId, setReauthAccountId] = createSignal<string | null>(null)

  const [setupComplete, setSetupComplete] = createSignal(false)

  const isOnboarding = () => !accounts.loading && (accounts()?.length ?? 0) === 0 && !setupComplete()

  async function handleTest(id: string) {
    await action.run(id, async () => {
      const result = await exchangeApi.testConnection(id)
      setTestResults(prev => ({ ...prev, [id]: result }))
    }, 'Connection failed')
    // Store failure result on error so UI shows the red badge
    if (action.error()) {
      setTestResults(prev => ({ ...prev, [id]: { success: false, latency_ms: null, error: action.error() } }))
    }
  }

  async function handleDelete(id: string) {
    await action.run(id, async () => {
      await exchangeApi.deleteAccount(id)
      refetchAccounts()
    }, 'Failed to delete account')
  }

  async function handleRevoke(id: string) {
    await action.run(id, async () => {
      await exchangeApi.revokeAgent(id)
      refetchAccounts()
    }, 'Failed to revoke agent')
  }

  async function handleImport(exchangeName: string) {
    await action.run(exchangeName, async () => {
      const API_BASE = import.meta.env.VITE_API_URL || ''
      const res = await fetch(`${API_BASE}/api/v1/trades/import`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({ exchange_name: exchangeName }),
      })
      if (!res.ok) throw new Error('Import failed')
    }, 'Failed to trigger import')
  }

  function openForm(initialExchange?: string) {
    setFormInitialExchange(initialExchange ?? '')
    setShowForm(true)
  }

  function handleFormSuccess() {
    setShowForm(false)
    setFormInitialExchange('')
    setSetupComplete(true)
    refetchAccounts()
  }

  return (
    <div class="flex flex-col h-full overflow-y-auto">
      <Show when={!isOnboarding()}>
        <div class="flex items-center justify-between px-8 py-5 shrink-0 border-b border-container-border/50 bg-container-bg">
          <h1 class="font-display text-lg font-bold tracking-wider text-text-primary">
            ACCOUNT
            <HelpTip text={HELP['page.account']} position="below" />
          </h1>
        </div>
      </Show>

      <Show when={action.error()}>
        <div role="alert" class="border border-signal-red bg-signal-red/10 p-4 mx-8 mt-6 font-mono text-sm text-signal-red">
          {action.error()}
        </div>
      </Show>

      <Show when={!accounts.loading} fallback={
        <div aria-live="polite" aria-busy="true" class="flex items-center justify-center py-20">
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
          {/* Correlation Stack — top of Account, conditional on ≥2 buckets */}
          <Show when={snapshot()}>
            {(snap) => (
              <div class="max-w-7xl mx-auto w-full px-8 pt-8">
                <CorrelationStack snapshot={snap()} />
              </div>
            )}
          </Show>

          {/* Infrastructure Zone */}
          <div class="max-w-7xl mx-auto w-full px-8 py-10">
            <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-8">
              <For each={accounts()}>
                {(acc) => (
                  <ExchangeCard
                    account={acc}
                    testResult={testResults()[acc.id]}
                    snapshot={snapshot()}
                    isTesting={action.pending() === acc.id}
                    isDeleting={action.pending() === acc.id}
                    isRevoking={action.pending() === acc.id}
                    onTest={() => handleTest(acc.id)}
                    onDelete={() => handleDelete(acc.id)}
                    onRevoke={() => handleRevoke(acc.id)}
                    onMigrate={() => openForm('hyperliquid')}
                    onReauthorize={() => setReauthAccountId(acc.id)}
                    onImport={() => handleImport(acc.exchange_name)}
                    isImporting={action.pending() === acc.exchange_name}
                  />
                )}
              </For>
              <AddExchangeCard onClick={() => openForm()} />
              <For each={Array.from({ length: Math.max(0, 1 - (accounts()?.length ?? 0)) })}>
                {() => <AddExchangeCard onClick={() => openForm()} />}
              </For>
            </div>

            {/* Add exchange form — centered overlay */}
            <Show when={showForm()}>
              <div
                class="fixed inset-0 z-50 flex items-center justify-center bg-main-bg/80 backdrop-blur-sm"
                onClick={(e) => { if (e.target === e.currentTarget) setShowForm(false) }}
              >
                <div class="relative border border-container-border bg-container-bg p-8 w-full max-w-lg mx-4 shadow-2xl">
                  <button
                    onClick={() => setShowForm(false)}
                    class="absolute top-4 right-4 text-text-tertiary hover:text-text-primary font-mono text-lg leading-none transition-colors"
                    aria-label="Close"
                  >
                    &times;
                  </button>
                  <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-4">// ADD_EXCHANGE</div>
                  <h3 class="font-mono text-sm font-bold text-text-primary mb-4">ADD EXCHANGE</h3>

                  <Show when={exchanges()}>
                    {(exs) => (
                      <AddExchangeForm
                        exchanges={exs()}
                        initialExchange={formInitialExchange()}
                        onSuccess={handleFormSuccess}
                        onCancel={() => setShowForm(false)}
                      />
                    )}
                  </Show>
                </div>
              </div>
            </Show>

            {/* Re-authorization modal */}
            <Show when={reauthAccountId()}>
              <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
                <div class="border border-container-border bg-main-bg/95 backdrop-blur-md p-8 max-w-lg w-full mx-4">
                  <div class="flex justify-between items-center mb-6">
                    <h3 class="font-mono text-sm font-bold text-text-primary tracking-wider">RE-AUTHORIZE AGENT WALLET</h3>
                    <button
                      onClick={() => setReauthAccountId(null)}
                      class="text-text-tertiary hover:text-text-primary font-mono text-xs"
                    >
                      CLOSE
                    </button>
                  </div>
                  <WalletConnectFlow
                    existingAccountId={reauthAccountId()!}
                    onComplete={() => { setReauthAccountId(null); refetchAccounts() }}
                  />
                </div>
              </div>
            </Show>
          </div>

          <div class="max-w-7xl mx-auto w-full px-8 pb-10">
            <CoachBanner />
          </div>
        </Show>
      </Show>
    </div>
  )
}
