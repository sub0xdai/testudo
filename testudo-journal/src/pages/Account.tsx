import { createSignal, createResource, For, Show } from 'solid-js'
import {
  exchangeApi,
  fetchRiskSnapshot,
  type TestConnectionResult,
} from '../api/client'
import { HelpTip } from '../components/HelpTip'
import { HELP } from '../lib/help-content'
import { ExchangeCard } from '../components/account/ExchangeCard'
import { AddExchangeCard } from '../components/account/AddExchangeCard'
import { AddExchangeForm } from '../components/account/AddExchangeForm'
import { OnboardingFlow } from '../components/account/OnboardingFlow'
import { WalletConnectFlow } from '../components/account/WalletConnectFlow'
import { CorrelationStack } from '../components/account/CorrelationStack'
import { CoachBanner } from '../components/account/CoachBanner'
import { IdentitySettings } from '../components/account/IdentitySettings'

export default function Account() {
  const [accounts, { refetch: refetchAccounts }] = createResource(async () => {
    return exchangeApi.listAccounts()
  })

  const [exchanges] = createResource(async () => {
    return exchangeApi.listExchanges()
  })

  const [snapshot] = createResource(fetchRiskSnapshot)

  const [testResults, setTestResults] = createSignal<Record<string, TestConnectionResult>>({})
  const [testingId, setTestingId] = createSignal<string | null>(null)
  const [deletingId, setDeletingId] = createSignal<string | null>(null)
  const [revokingId, setRevokingId] = createSignal<string | null>(null)
  const [importingId, setImportingId] = createSignal<string | null>(null)
  const [showForm, setShowForm] = createSignal(false)
  const [formInitialExchange, setFormInitialExchange] = createSignal('')
  const [reauthAccountId, setReauthAccountId] = createSignal<string | null>(null)

  const [setupComplete, setSetupComplete] = createSignal(false)
  const [error, setError] = createSignal('')

  const isOnboarding = () => !accounts.loading && (accounts()?.length ?? 0) === 0 && !setupComplete()

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
    setImportingId(exchangeName)
    try {
      const API_BASE = import.meta.env.VITE_API_URL || ''
      const res = await fetch(`${API_BASE}/api/v1/trades/import`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({ exchange_name: exchangeName }),
      })
      if (!res.ok) {
        setError('Import failed')
      }
    } catch {
      setError('Failed to trigger import')
    } finally {
      setImportingId(null)
    }
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

      <Show when={error()}>
        <div role="alert" class="border border-signal-red bg-signal-red/10 p-4 mx-8 mt-6 font-mono text-sm text-signal-red">
          {error()}
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
                    isTesting={testingId() === acc.id}
                    isDeleting={deletingId() === acc.id}
                    isRevoking={revokingId() === acc.id}
                    onTest={() => handleTest(acc.id)}
                    onDelete={() => handleDelete(acc.id)}
                    onRevoke={() => handleRevoke(acc.id)}
                    onMigrate={() => openForm('hyperliquid')}
                    onReauthorize={() => setReauthAccountId(acc.id)}
                    onImport={() => handleImport(acc.exchange_name)}
                    isImporting={importingId() === acc.exchange_name}
                  />
                )}
              </For>
              <AddExchangeCard onClick={() => openForm()} />
              <For each={Array.from({ length: Math.max(0, 1 - (accounts()?.length ?? 0)) })}>
                {() => <AddExchangeCard onClick={() => openForm()} />}
              </For>
            </div>
          </div>

          <div class="max-w-7xl mx-auto w-full px-8 pb-10">
            <CoachBanner />
            {/* <div class="pt-12 border-t border-container-border/30">
              <IdentitySettings />
            </div> */}
          </div>
        </Show>
      </Show>
    </div>
  )
}
