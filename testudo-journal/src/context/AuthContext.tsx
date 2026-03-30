import { createContext, useContext, createSignal, onCleanup, type JSX } from 'solid-js'
import { appKit } from '../config/wallet'

export interface User {
  id: string
  wallet_address: string
}

interface AuthContextValue {
  user: () => User | null
  isAuthenticated: () => boolean
  loading: () => boolean
  siweError: () => string | null
  connectWallet: () => void
  logout: () => Promise<void>
}

const AuthContext = createContext<AuthContextValue>()

const API_BASE = import.meta.env.VITE_API_URL || ''

async function fetchAuth(path: string, init?: RequestInit) {
  return fetch(`${API_BASE}/api/v1/auth${path}`, {
    ...init,
    credentials: 'include',
    headers: { 'Content-Type': 'application/json', ...init?.headers },
  })
}

export function AuthProvider(props: { children: JSX.Element }) {
  const [user, setUser] = createSignal<User | null>(null)
  const [loading, setLoading] = createSignal(true)
  const [siweError, setSiweError] = createSignal<string | null>(null)
  let siweInFlight = false
  // CON-01: Only trigger SIWE from explicit user action, never from auto-reconnect.
  // This prevents MetaMask popup on every page refresh.
  let userInitiatedConnect = false

  // Track the EVM provider via subscribeProviders (correct AppKit API)
  let evmProvider: any = null
  const unsubProviders = appKit.subscribeProviders((state: Record<string, any>) => {
    evmProvider = state['eip155'] ?? null
  })

  // Check existing cookie session on mount.
  // If access token expired (401), try refresh before giving up.
  async function checkSession() {
    try {
      let res = await fetchAuth('/me')
      if (res.status === 401) {
        // Access token expired — try refresh
        const refreshRes = await fetchAuth('/refresh', { method: 'POST' })
        if (refreshRes.ok) {
          res = await fetchAuth('/me')
        }
      }
      if (res.ok) {
        const data = await res.json() as { user: User }
        setUser(data.user)
      } else {
        setUser(null)
        // No valid session — disconnect wallet so appKit.open() shows
        // the connector picker, not the account/balance modal.
        appKit.disconnect()
      }
    } catch {
      setUser(null)
      appKit.disconnect()
    } finally {
      setLoading(false)
    }
  }
  checkSession()

  // Run SIWE when wallet connects and provider becomes available
  async function runSiwe(address: string) {
    if (user() || loading() || siweInFlight) return
    siweInFlight = true
    setSiweError(null)

    try {
      // Wait briefly for provider to be ready (subscribeProviders may fire after subscribeAccount)
      let attempts = 0
      while (!evmProvider && attempts < 20) {
        await new Promise(r => setTimeout(r, 100))
        attempts++
      }
      if (!evmProvider) throw new Error('Wallet provider not ready — please try again')

      // Re-check after async wait — /me may have resolved and set user()
      if (user()) {
        siweInFlight = false
        return
      }

      // Get nonce from backend
      const nonceRes = await fetchAuth('/nonce')
      if (!nonceRes.ok) throw new Error('Failed to get nonce')
      const { nonce } = await nonceRes.json() as { nonce: string }

      // Build SIWE message
      const message = [
        `${window.location.host} wants you to sign in with your Ethereum account:`,
        address, '', 'Sign in to Testudo', '',
        `URI: ${window.location.origin}`,
        `Version: 1`,
        `Chain ID: 42161`,
        `Nonce: ${nonce}`,
        `Issued At: ${new Date().toISOString()}`,
      ].join('\n')

      // Sign via EVM provider from subscribeProviders
      const signature = await evmProvider.request({
        method: 'personal_sign',
        params: [message, address],
      })

      // Verify with backend
      const verifyRes = await fetchAuth('/verify-siwe', {
        method: 'POST',
        body: JSON.stringify({ message, signature }),
      })
      if (!verifyRes.ok) throw new Error('SIWE verification failed')

      const { user: u } = await verifyRes.json() as { user: User }
      setUser(u)
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Authentication failed'
      console.error('[SIWE] auth failed:', msg)
      setSiweError(
        /reject|denied|cancel/i.test(msg)
          ? 'Signature rejected — click Connect to retry'
          : msg
      )
      appKit.disconnect()
    } finally {
      siweInFlight = false
      userInitiatedConnect = false
    }
  }

  // Subscribe to account state — triggers SIWE only after explicit user action.
  // On refresh, wallet auto-reconnects and fires this callback, but we must NOT
  // auto-trigger SIWE — session cookies from /me handle auth restoration silently.
  const unsubAccount = appKit.subscribeAccount((state: { isConnected: boolean; address?: string }) => {
    if (state.isConnected && state.address && !user() && !siweInFlight && userInitiatedConnect) {
      runSiwe(state.address)
    }
  })

  onCleanup(() => {
    unsubAccount()
    unsubProviders()
  })

  const connectWallet = () => {
    setSiweError(null)
    userInitiatedConnect = true
    appKit.open()
  }

  const logout = async () => {
    await fetchAuth('/logout', { method: 'POST' }).catch(() => {})
    setUser(null)
    appKit.disconnect()
  }

  const value: AuthContextValue = {
    user,
    isAuthenticated: () => user() !== null,
    loading,
    siweError,
    connectWallet,
    logout,
  }

  return (
    <AuthContext.Provider value={value}>
      {props.children}
    </AuthContext.Provider>
  )
}

export function useAuth() {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error('useAuth must be used within AuthProvider')
  return ctx
}
