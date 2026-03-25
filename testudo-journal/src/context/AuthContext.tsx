import { createContext, useContext, createSignal, createEffect, onCleanup, type JSX } from 'solid-js'
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

  // Check existing cookie session on mount
  fetchAuth('/me')
    .then(r => r.ok ? r.json() : Promise.reject())
    .then((data: { user: User }) => setUser(data.user))
    .catch(() => setUser(null))
    .finally(() => setLoading(false))

  // Subscribe to AppKit account state for wallet connect/disconnect
  const unsubAccount = appKit.subscribeAccount(async (state) => {
    if (!state.isConnected || !state.address) return
    if (user() || loading() || siweInFlight) return

    siweInFlight = true
    setSiweError(null)

    try {
      // Get nonce from backend
      const nonceRes = await fetchAuth('/nonce')
      if (!nonceRes.ok) throw new Error('Failed to get nonce')
      const { nonce } = await nonceRes.json() as { nonce: string }

      // Build SIWE message
      const message = [
        `${window.location.host} wants you to sign in with your Ethereum account:`,
        state.address, '', 'Sign in to Testudo', '',
        `URI: ${window.location.origin}`,
        `Version: 1`,
        `Chain ID: 42161`,
        `Nonce: ${nonce}`,
        `Issued At: ${new Date().toISOString()}`,
      ].join('\n')

      // Sign via AppKit provider
      const provider = appKit.getProvider()
      if (!provider) throw new Error('No wallet provider available')

      const signature = await (provider as any).request({
        method: 'personal_sign',
        params: [message, state.address],
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
    }
  })

  onCleanup(() => unsubAccount())

  const connectWallet = () => {
    setSiweError(null)
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
