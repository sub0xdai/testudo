import { createSignal, onMount, onCleanup, Show } from 'solid-js'
import { useAuth } from '../context/AuthContext'
import { pairExtension } from '../api/client'

const CODE_TTL_SECONDS = 60

const CHROME_STORE_URL = 'https://chromewebstore.google.com'
const FIREFOX_STORE_URL = 'https://addons.mozilla.org'

export default function Pair() {
  const auth = useAuth()
  const [extensionDetected, setExtensionDetected] = createSignal(false)
  const [code, setCode] = createSignal<string | null>(null)
  const [countdown, setCountdown] = createSignal(0)
  const [generating, setGenerating] = createSignal(false)
  const [error, setError] = createSignal('')
  const [copied, setCopied] = createSignal(false)
  const [expired, setExpired] = createSignal(false)
  let timer: ReturnType<typeof setInterval> | null = null
  let copyTimeout: ReturnType<typeof setTimeout> | null = null

  // Listen for extension content script signal
  function handleMessage(e: MessageEvent) {
    if (e.data?.type === 'TESTUDO_INSTALLED') {
      setExtensionDetected(true)
    }
  }

  onMount(() => {
    window.addEventListener('message', handleMessage)
    // Auto-generate code if already authenticated
    if (auth.isAuthenticated()) {
      generateCode()
    }
  })

  onCleanup(() => {
    window.removeEventListener('message', handleMessage)
    if (timer) clearInterval(timer)
    if (copyTimeout) clearTimeout(copyTimeout)
  })

  async function generateCode() {
    setGenerating(true)
    setError('')
    setExpired(false)
    try {
      const { code: newCode } = await pairExtension()
      setCode(newCode)
      setCountdown(CODE_TTL_SECONDS)

      if (timer) clearInterval(timer)
      timer = setInterval(() => {
        setCountdown((prev) => {
          if (prev <= 1) {
            if (timer) clearInterval(timer)
            timer = null
            setExpired(true)
            setCode(null)
            return 0
          }
          return prev - 1
        })
      }, 1000)
    } catch {
      setError('Failed to generate code')
    } finally {
      setGenerating(false)
    }
  }

  function copyCode() {
    const current = code()
    if (!current) return
    navigator.clipboard.writeText(current)
    setCopied(true)
    if (copyTimeout) clearTimeout(copyTimeout)
    copyTimeout = setTimeout(() => setCopied(false), 1500)
  }

  const minutes = () => Math.floor(countdown() / 60)
  const seconds = () => String(countdown() % 60).padStart(2, '0')

  // After wallet connect succeeds, auto-generate code
  function handleConnect() {
    auth.connectWallet()
    // Watch for auth state change
    const check = setInterval(() => {
      if (auth.isAuthenticated()) {
        clearInterval(check)
        generateCode()
      }
    }, 500)
    // Stop checking after 60s
    setTimeout(() => clearInterval(check), 60000)
  }

  return (
    <div class="min-h-screen text-text-primary">
      {/* Background — same as landing page */}
      <div class="fixed inset-0 z-0">
        <div
          class="absolute inset-0"
          style={{
            'background-image': 'url(https://testudo.vip/Roman-testudo-Trajan-column-966204074.jpg)',
            'background-size': 'cover',
            'background-position': 'center',
            'background-repeat': 'no-repeat',
          }}
        />
        <div class="absolute inset-0 bg-overlay" />
      </div>

      {/* Centered card */}
      <div class="relative z-10 min-h-screen flex flex-col items-center justify-center px-6">
        <div class="border border-container-border bg-main-bg/75 backdrop-blur-md p-10 md:p-14 max-w-lg w-full text-center">
          {/* Logo */}
          <h1 class="font-mono text-2xl md:text-3xl tracking-[0.3em] text-text-primary mb-1">
            TESTUDO
          </h1>
          <p class="font-mono text-[10px] tracking-widest text-text-tertiary mb-8">
            TRADING TERMINAL
          </p>
          <div class="flex items-center justify-center gap-3 mb-10">
            <div class="flex-1 h-px bg-container-border" />
            <div class="w-2 h-2 rotate-45 bg-text-tertiary" />
            <div class="flex-1 h-px bg-container-border" />
          </div>

          {/* State 1: No extension detected */}
          <Show when={!extensionDetected()}>
            <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-6">
              // CONNECT_EXTENSION
            </div>
            <p class="font-mono text-sm text-text-secondary mb-8 leading-relaxed">
              Install the Testudo Sniper extension to start trading from any chart.
            </p>
            <a
              href={CHROME_STORE_URL}
              target="_blank"
              rel="noopener noreferrer"
              class="block w-full py-3 mb-3 border border-text-primary text-text-primary font-mono font-bold text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors text-center"
            >
              [ CHROME WEB STORE ]
            </a>
            <a
              href={FIREFOX_STORE_URL}
              target="_blank"
              rel="noopener noreferrer"
              class="block w-full py-3 mb-8 border border-container-border text-text-secondary font-mono font-bold text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors text-center"
            >
              [ FIREFOX ADD-ONS ]
            </a>
            <div class="h-px bg-container-border mb-4" />
            <p class="font-mono text-[10px] text-text-tertiary">
              Already installed?{' '}
              <button
                onClick={() => window.location.reload()}
                class="text-text-secondary hover:text-text-primary underline transition-colors"
              >
                Refresh this page
              </button>
            </p>
          </Show>

          {/* State 2: Extension detected, not authenticated */}
          <Show when={extensionDetected() && !auth.isAuthenticated()}>
            <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-6">
              // AUTHENTICATE
            </div>
            <p class="font-mono text-sm text-text-secondary mb-8 leading-relaxed">
              Connect your wallet to link your extension.
            </p>
            <button
              onClick={handleConnect}
              class="w-full py-3 border border-text-primary text-text-primary font-mono font-bold text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors animate-glow-pulse"
            >
              [ CONNECT WALLET ]
            </button>
          </Show>

          {/* State 3: Authenticated — show pairing code */}
          <Show when={auth.isAuthenticated()}>
            <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-6">
              // PAIR_EXTENSION
            </div>

            <Show when={error()}>
              <p class="font-mono text-xs text-signal-red mb-4">{error()}</p>
            </Show>

            <Show when={generating()}>
              <p class="font-mono text-sm text-text-secondary mb-4">Generating code...</p>
            </Show>

            <Show when={code()}>
              <p class="font-mono text-sm text-text-secondary mb-6 leading-relaxed">
                Enter this code in your extension popup.
              </p>
              <button
                onClick={copyCode}
                class="inline-block mb-4 cursor-pointer group"
                title="Click to copy"
              >
                <div class="flex gap-3 justify-center">
                  {code()!.split('').map((digit) => (
                    <span class="font-mono text-3xl md:text-4xl tracking-widest text-text-primary group-hover:text-accent-steel transition-colors">
                      {digit}
                    </span>
                  ))}
                </div>
              </button>
              <p class="font-mono text-lg text-text-tertiary mb-2">
                {minutes()}:{seconds()}
              </p>
              <p class="font-mono text-[10px] text-text-tertiary mb-4">
                {copied() ? 'Copied!' : 'Click code to copy'}
              </p>
            </Show>

            <Show when={expired()}>
              <p class="font-mono text-sm text-text-secondary mb-6 leading-relaxed">
                Pairing code has expired.
              </p>
              <button
                onClick={generateCode}
                disabled={generating()}
                class="w-full py-3 border border-text-primary text-text-primary font-mono font-bold text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors"
              >
                [ GENERATE NEW CODE ]
              </button>
            </Show>

            <Show when={!code() && !expired() && !generating()}>
              <button
                onClick={generateCode}
                class="w-full py-3 border border-text-primary text-text-primary font-mono font-bold text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors"
              >
                [ GENERATE CODE ]
              </button>
            </Show>
          </Show>
        </div>

        {/* Navigation links */}
        <div class="mt-8 flex gap-4">
          <a
            href="/desk/"
            class="px-6 py-2.5 border border-container-border text-text-secondary font-mono text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors"
          >
            DESK
          </a>
          <a
            href="https://testudo.vip"
            class="px-6 py-2.5 border border-container-border text-text-secondary font-mono text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors"
          >
            TESTUDO.VIP
          </a>
        </div>
      </div>
    </div>
  )
}
