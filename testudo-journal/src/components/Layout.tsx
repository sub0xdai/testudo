import { createSignal, For, Show, onMount, onCleanup, type JSX } from 'solid-js'
import { A } from '@solidjs/router'
import { useAuth } from '../context/AuthContext'
import { appKit } from '../config/wallet'
import { Stepper } from './onboarding/Stepper'

const NAV_ITEMS = [
  { path: '/', label: 'OVERVIEW' },
  { path: '/trades', label: 'TRADES' },
  { path: '/journal', label: 'JOURNAL' },
  { path: '/account', label: 'ACCOUNT' },
]

type Theme = 'amoled' | 'light'
const THEME_CYCLE: Theme[] = ['amoled', 'light']
const THEME_LABELS: Record<Theme, string> = {
  'amoled': 'DARK',
  'light': 'LIGHT',
}

function applyTheme(theme: Theme) {
  if (theme === 'amoled') {
    document.documentElement.removeAttribute('data-theme')
  } else {
    document.documentElement.setAttribute('data-theme', theme)
  }
  localStorage.setItem('testudo-theme', theme)
}

// ─── Wallet Chip ───

function WalletChip() {
  const auth = useAuth()
  const [open, setOpen] = createSignal(false)
  let ref: HTMLDivElement | undefined

  function handleClickOutside(e: MouseEvent) {
    if (ref && !ref.contains(e.target as Node)) setOpen(false)
  }

  onMount(() => document.addEventListener('mousedown', handleClickOutside))
  onCleanup(() => document.removeEventListener('mousedown', handleClickOutside))

  const addr = () => auth.user()?.wallet_address ?? ''
  const truncated = () => {
    const a = addr()
    return a ? `${a.slice(0, 6)}...${a.slice(-4)}` : ''
  }

  return (
    <Show when={auth.isAuthenticated()} fallback={
      <button
        onClick={() => auth.connectWallet()}
        class="font-mono text-xs tracking-wider text-text-primary animate-glow-pulse"
      >
        CONNECT
      </button>
    }>
      <div ref={ref} class="relative">
        <button
          onClick={() => setOpen(!open())}
          class="flex items-center gap-2 px-4 py-1.5 border border-container-border text-text-primary font-mono text-xs tracking-wider hover:border-text-primary transition-colors"
        >
          <span class="inline-block w-2 h-2 rounded-full bg-signal-green animate-pulse" />
          {truncated()}
          <svg width="10" height="10" viewBox="0 0 10 10" class={`text-text-tertiary transition-transform ${open() ? 'rotate-180' : ''}`}>
            <path d="M2 4L5 7L8 4" stroke="currentColor" stroke-width="1.5" fill="none" />
          </svg>
        </button>

        <Show when={open()}>
          <div class="absolute right-0 mt-1 bg-container-bg border border-container-border z-50">
            <button
              onClick={() => { auth.logout(); setOpen(false) }}
              class="text-left px-4 py-2.5 text-xs font-mono text-signal-red hover:bg-signal-red/10 transition-colors whitespace-nowrap"
            >
              DISCONNECT
            </button>
          </div>
        </Show>
      </div>
    </Show>
  )
}

// ─── Lock Screen ───

function LockScreen() {
  return (
    <div class="relative z-10 min-h-[calc(100vh-var(--header-h))] flex flex-col items-center justify-center gap-4 px-6">
      <h2 class="font-mono text-2xl tracking-widest text-text-primary">TESTUDO DESK</h2>
      <p class="font-mono text-sm text-text-secondary max-w-md text-center">
        Connect your wallet to access the trading dashboard, manage exchanges, and view analytics.
      </p>
    </div>
  )
}

function ConnectingScreen() {
  return (
    <div class="relative z-10 min-h-[calc(100vh-var(--header-h))] flex flex-col items-center justify-center gap-4">
      <div class="w-4 h-4 border-2 border-text-secondary border-t-text-primary rounded-full animate-spin" />
      <p class="font-mono text-xs text-text-secondary tracking-wider">VERIFYING WALLET...</p>
    </div>
  )
}

function ErrorScreen(props: { message: string; onRetry: () => void }) {
  return (
    <div class="relative z-10 min-h-[calc(100vh-var(--header-h))] flex flex-col items-center justify-center gap-6 px-6">
      <p class="font-mono text-sm text-signal-red max-w-md text-center">{props.message}</p>
      <button
        onClick={props.onRetry}
        class="px-8 py-3 border border-text-primary text-text-primary font-mono text-sm tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors"
      >
        TRY AGAIN
      </button>
    </div>
  )
}

// ─── Layout ───

export function Layout(props: { children: JSX.Element }) {
  const auth = useAuth()
  const [menuOpen, setMenuOpen] = createSignal(false)
  const [theme, setTheme] = createSignal<Theme>('amoled')

  onMount(() => {
    const stored = localStorage.getItem('testudo-theme') as Theme | null
    if (stored && THEME_CYCLE.includes(stored)) {
      setTheme(stored)
    }
  })

  function cycleTheme() {
    const current = theme()
    const idx = THEME_CYCLE.indexOf(current)
    const next = THEME_CYCLE[(idx + 1) % THEME_CYCLE.length]
    setTheme(next)
    applyTheme(next)
  }

  return (
    <div class="min-h-screen text-text-primary">
      {/* Hadrian's Wall background — shared with landing page */}
      <div class="fixed inset-0 z-0">
        <div
          class="absolute inset-0"
          style={{
            "background-image": "url(/Roman-testudo-Trajan-column-966204074.jpg)",
            "background-size": "cover",
            "background-position": "center",
            "background-repeat": "no-repeat",
          }}
        />
        <div class="absolute inset-0 bg-overlay" />
      </div>

      <header class="fixed top-0 left-0 right-0 z-50 bg-main-bg/60 backdrop-blur-sm border-b border-container-border/30">
        <div class="max-w-[1400px] mx-auto px-6 md:px-8 py-4 flex items-center justify-between">
          <div class="flex items-center gap-3">
            <a href="/" class="font-mono text-lg tracking-widest text-text-primary hover:text-accent-steel transition-colors">
              TESTUDO
            </a>
            <button
              class="text-text-secondary hover:text-text-primary transition-colors"
              onClick={cycleTheme}
              title={`Theme: ${THEME_LABELS[theme()]} (click to toggle)`}
              aria-label="Toggle theme"
            >
              {theme() === 'amoled' ? (
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <circle cx="12" cy="12" r="5" />
                  <line x1="12" y1="1" x2="12" y2="3" />
                  <line x1="12" y1="21" x2="12" y2="23" />
                  <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
                  <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
                  <line x1="1" y1="12" x2="3" y2="12" />
                  <line x1="21" y1="12" x2="23" y2="12" />
                  <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
                  <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
                </svg>
              ) : (
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
                </svg>
              )}
            </button>
          </div>

          {/* Desktop nav */}
          <nav class="hidden md:flex items-center gap-6">
            <Show when={auth.isAuthenticated()}>
              <For each={NAV_ITEMS}>
                {(item) => (
                  <A
                    href={item.path}
                    end={item.path === '/'}
                    class="font-mono text-xs tracking-wider transition-colors"
                    activeClass="text-text-primary"
                    inactiveClass="text-text-secondary hover:text-text-primary"
                  >
                    {item.label}
                  </A>
                )}
              </For>
            </Show>
            <WalletChip />
          </nav>

          {/* Mobile hamburger */}
          <button
            class="md:hidden p-2 min-w-[44px] min-h-[44px] flex items-center justify-center"
            onClick={() => setMenuOpen(!menuOpen())}
            aria-expanded={menuOpen()}
            aria-label="Navigation menu"
          >
            <span class="font-mono text-lg">{menuOpen() ? '\u00D7' : '\u2261'}</span>
          </button>
        </div>

        {/* Mobile nav panel */}
        <Show when={menuOpen()}>
          <nav class="md:hidden border-t border-container-border py-2 bg-main-bg/95 backdrop-blur-sm">
            <Show when={auth.isAuthenticated()}>
              <For each={NAV_ITEMS}>
                {(item) => (
                  <A
                    href={item.path}
                    end={item.path === '/'}
                    class="block px-6 py-3 min-h-[44px] font-mono text-sm tracking-wider transition-colors flex items-center"
                    activeClass="text-text-primary"
                    inactiveClass="text-text-secondary hover:text-text-primary"
                    onClick={() => setMenuOpen(false)}
                  >
                    {item.label}
                  </A>
                )}
              </For>
            </Show>
            <div class="px-6 py-3">
              <WalletChip />
            </div>
          </nav>
        </Show>
      </header>

      {/* Spacer for fixed header */}
      <div style={{ height: 'var(--header-h)' }} />

      {/* Auth-gated content */}
      <Show when={!auth.loading()} fallback={<ConnectingScreen />}>
        <Show when={auth.isAuthenticated()} fallback={
          <Show when={auth.siweError()} fallback={<LockScreen />}>
            {(error) => <ErrorScreen message={error()} onRetry={auth.connectWallet} />}
          </Show>
        }>
          <Stepper />
          <main class="relative z-10 max-w-[1400px] mx-auto px-6 py-6">
            {props.children}
          </main>
        </Show>
      </Show>
    </div>
  )
}
