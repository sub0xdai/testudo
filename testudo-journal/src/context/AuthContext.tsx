/** @anchor ui:journal-context:AuthContext
 * @tags ui */

import { createContext, useContext, createSignal, onCleanup, type JSX } from 'solid-js'
import { base58 } from '@scure/base'
import { loadWallet, isWalletLoaded } from '../config/wallet'
import { clearCacheForIdentity } from '../lib/cache'
import type { AppKit } from '@reown/appkit'

export interface User {
  id: string
  wallet_address: string
}

interface AuthContextValue {
  user: () => User | null
  isAuthenticated: () => boolean
  loading: () => boolean
  siweError: () => string | null
  connectWallet: () => Promise<void>
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
  let listenersAttached = false
  let unsubListeners: (() => void) | null = null

  let evmProvider: any = null
  let solanaProvider: any = null

  function attachWalletListeners(kit: AppKit): () => void {
    const unsubProviders = kit.subscribeProviders((state: Record<string, any>) => {
      evmProvider = state['eip155'] ?? null
      solanaProvider = state['solana'] ?? null
    })

    // Subscribe to account state — triggers SIWE/SIWS only after explicit user action.
    // On refresh, wallet auto-reconnects and fires this callback, but we must NOT
    // auto-trigger signing — session cookies from /me handle auth restoration silently.
    //
    // Wallet-switch guard: if the wallet extension reports a different address than
    // the currently authenticated user, the stale server session for the old wallet
    // is revoked and client state cleared. The user explicitly changed which wallet
    // is connected — treat that as intent to re-auth — so we re-enable
    // userInitiatedConnect and fall through to the SIWE check below, which fires a
    // fresh sign prompt for the new address.
    // Notify the Testudo extension (if installed) of session changes via
    // window.postMessage. The extension's content script on desk.testudo.vip
    // listens and invalidates its paired JWT when the web's wallet diverges.
    const unsubAccount = kit.subscribeAccount(async (state: { isConnected: boolean; address?: string }) => {
      const current = user()
      if (
        current &&
        state.isConnected &&
        state.address &&
        current.wallet_address.toLowerCase() !== state.address.toLowerCase()
      ) {
        const prevIdentity = current.id
        await fetchAuth('/logout', { method: 'POST' }).catch(() => {})
        setUser(null)
        clearCacheForIdentity(prevIdentity)
        userInitiatedConnect = true
        notifyExtensionOfWalletChange(state.address.toLowerCase())
      }

      if (state.isConnected && state.address && !user() && !siweInFlight && userInitiatedConnect) {
        const chainNs = kit.getCaipNetwork()?.chainNamespace
        if (chainNs === 'solana' && solanaProvider) {
          runSiws(state.address)
        } else if (evmProvider) {
          runSiwe(state.address)
        }
      }
    })

    return () => { unsubAccount(); unsubProviders() }
  }

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
        // Only disconnect if the wallet bundle was already loaded this session.
        if (isWalletLoaded()) {
          ;(await loadWallet()).disconnect()
        }
      }
    } catch {
      setUser(null)
      if (isWalletLoaded()) {
        ;(await loadWallet()).disconnect()
      }
    } finally {
      setLoading(false)
    }
  }
  checkSession()

  interface SignerConfig {
    chain: 'evm' | 'solana'
    getProvider: () => unknown
    providerPollAttempts?: number
    buildMessage: (address: string, nonce: string, chainId?: number | string) => string
    sign: (provider: unknown, message: string, address: string) => Promise<string>
    verifyEndpoint: string
    verifyExtraFields?: Record<string, unknown>
  }

  async function runAuthFlow(config: SignerConfig, address: string): Promise<void> {
    if (user() || loading() || siweInFlight) return
    siweInFlight = true
    setSiweError(null)

    try {
      let attempts = 0
      const maxAttempts = config.providerPollAttempts ?? 20
      while (!config.getProvider() && attempts < maxAttempts) {
        await new Promise(r => setTimeout(r, 100))
        attempts++
      }
      if (!config.getProvider()) {
        throw new Error(`${config.chain} provider not ready — please try again`)
      }

      if (user()) { siweInFlight = false; return }

      const nonceRes = await fetchAuth('/nonce')
      if (!nonceRes.ok) throw new Error('Failed to get nonce')
      const { nonce } = await nonceRes.json() as { nonce: string }

      let chainId: string | number | undefined
      try {
        chainId = (await loadWallet()).getChainId() ?? undefined
      } catch { /* optional — chainId only used by EVM */ }

      const message = config.buildMessage(address, nonce, chainId)
      const signature = await config.sign(config.getProvider(), message, address)

      const body: Record<string, unknown> = {
        message,
        signature,
        address,
        ...config.verifyExtraFields,
      }
      const verifyRes = await fetchAuth(config.verifyEndpoint, {
        method: 'POST',
        body: JSON.stringify(body),
      })
      if (!verifyRes.ok) {
        const errBody = await verifyRes.text().catch(() => '')
        throw new Error(`Verification failed: ${errBody || verifyRes.statusText}`)
      }

      const { user: u } = await verifyRes.json() as { user: User }
      setUser(u)
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Authentication failed'
      console.error(`[${config.chain.toUpperCase()}] auth failed:`, msg)
      setSiweError(
        /reject|denied|cancel/i.test(msg)
          ? 'Signature rejected — click Connect to retry'
          : msg
      )
      ;(await loadWallet()).disconnect()
    } finally {
      siweInFlight = false
      userInitiatedConnect = false
    }
  }

  // Run SIWE when wallet connects and provider becomes available
  async function runSiwe(address: string) {
    await runAuthFlow({
      chain: 'evm',
      getProvider: () => evmProvider,
      buildMessage: (addr, nonce, chainId) =>
        [
          `${window.location.host} wants you to sign in with your Ethereum account:`,
          addr, '', 'Sign in to Testudo', '',
          `URI: ${window.location.origin}`,
          'Version: 1',
          `Chain ID: ${chainId ?? 1}`,
          `Nonce: ${nonce}`,
          `Issued At: ${new Date().toISOString()}`,
        ].join('\n'),
      sign: (provider, message, addr) =>
        (provider as any).request({
          method: 'personal_sign',
          params: [message, addr],
        }),
      verifyEndpoint: '/verify-siwe',
    }, address)
  }

  // Run SIWS (Sign In With Solana) — thin wrapper around runAuthFlow
  async function runSiws(address: string) {
    await runAuthFlow({
      chain: 'solana',
      getProvider: () => solanaProvider,
      buildMessage: (addr, nonce) =>
        [
          `${window.location.host} wants you to sign in with your Solana account:`,
          addr, '', 'Sign in to Testudo', '',
          `URI: ${window.location.origin}`,
          `Nonce: ${nonce}`,
          `Issued At: ${new Date().toISOString()}`,
        ].join('\n'),
      sign: async (provider, message, _addr) => {
        const encoded = new TextEncoder().encode(message)
        const sig = await (provider as any).signMessage(encoded)
        const sigBytes: Uint8Array = sig instanceof Uint8Array ? sig : sig.signature
        return base58.encode(sigBytes)
      },
      verifyEndpoint: '/verify-siws',
    }, address)
  }

  function notifyExtensionOfWalletChange(address: string | null) {
    try {
      window.postMessage(
        { type: 'TESTUDO_WALLET_CHANGED', wallet_address: address },
        window.location.origin,
      )
    } catch { /* noop — message bus not available */ }
  }

  const connectWallet = async () => {
    setSiweError(null)
    userInitiatedConnect = true
    const kit = await loadWallet()
    if (!listenersAttached) {
      unsubListeners = attachWalletListeners(kit)
      listenersAttached = true
    }
    kit.open()
  }

  const logout = async () => {
    const prevIdentity = user()?.id
    await fetchAuth('/logout', { method: 'POST' }).catch(() => {})
    setUser(null)
    if (prevIdentity) clearCacheForIdentity(prevIdentity)
    if (isWalletLoaded()) {
      ;(await loadWallet()).disconnect()
    }
    notifyExtensionOfWalletChange(null)
  }

  onCleanup(() => {
    unsubListeners?.()
  })

  const value: AuthContextValue = {
    user,
    isAuthenticated: () => user() != null,
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
