import { createResource, createMemo } from 'solid-js'
import { useAuth } from '../../context/AuthContext'
import { exchangeApi, fetchTrades } from '../../api/client'

const STORAGE_KEY = 'testudo-onboarding-complete'
const PAIRED_KEY = 'testudo-extension-paired'

export interface StepState {
  label: string
  complete: boolean
}

export function useOnboardingState() {
  const auth = useAuth()

  // Only fetch when authenticated
  const [accounts] = createResource(
    () => auth.isAuthenticated(),
    async (authed) => {
      if (!authed) return []
      try {
        return await exchangeApi.listAccounts()
      } catch {
        return []
      }
    }
  )

  const [trades] = createResource(
    () => auth.isAuthenticated(),
    async (authed) => {
      if (!authed) return { trades: [], total: 0 }
      try {
        return await fetchTrades({ page: 1, limit: 1 })
      } catch {
        return { trades: [], total: 0 }
      }
    }
  )

  const isPaired = () => localStorage.getItem(PAIRED_KEY) === 'true'

  const steps = createMemo((): StepState[] => [
    { label: 'Connect Wallet', complete: auth.isAuthenticated() },
    { label: 'Add Exchanges', complete: (accounts()?.length ?? 0) > 0 },
    { label: 'Import History', complete: (trades()?.total ?? 0) > 0 },
    { label: 'Pair Extension', complete: isPaired() },
  ])

  const activeStep = createMemo(() => {
    const s = steps()
    const idx = s.findIndex((step) => !step.complete)
    return idx === -1 ? s.length : idx
  })

  const allComplete = createMemo(() => steps().every((s) => s.complete))

  const loading = () => accounts.loading || trades.loading

  // Check if user already completed onboarding before this feature existed
  const alreadyDismissed = () => localStorage.getItem(STORAGE_KEY) === 'true'

  const dismiss = () => localStorage.setItem(STORAGE_KEY, 'true')

  // Should show: authenticated, not dismissed, not all complete (or recently completed)
  const shouldShow = createMemo(() => {
    if (!auth.isAuthenticated()) return false
    if (alreadyDismissed()) return false
    if (loading()) return false
    return true
  })

  return { steps, activeStep, allComplete, loading, shouldShow, dismiss }
}

/** Mark extension as paired — called from ExtensionPairingBanner */
export function markExtensionPaired() {
  localStorage.setItem(PAIRED_KEY, 'true')
}
