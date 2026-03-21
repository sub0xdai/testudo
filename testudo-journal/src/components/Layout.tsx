import { createSignal, For, Show, onMount, type JSX } from 'solid-js'
import { A } from '@solidjs/router'

const NAV_ITEMS = [
  { path: '/', label: 'OVERVIEW' },
  { path: '/charts', label: 'ANALYSIS' },
  { path: '/trades', label: 'TRADES' },
  { path: '/journal', label: 'JOURNAL' },
  { path: null, label: 'HOME', external: true },
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

export function Layout(props: { children: JSX.Element }) {
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
    <div class="min-h-screen bg-main-bg text-text-primary">
      <header class="fixed top-0 left-0 right-0 z-50 bg-main-bg/60 backdrop-blur-sm border-b border-container-border/30">
        <div class="max-w-[1400px] mx-auto px-6 md:px-8 py-4 flex items-center justify-between">
          <div class="flex items-center gap-3">
            <A href="/" class="font-mono text-lg tracking-widest text-text-primary hover:text-text-primary">
              TESTUDO
            </A>
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
            <For each={NAV_ITEMS}>
              {(item) => (
                item.external ? (
                  <a
                    href="/"
                    class="font-mono text-xs tracking-wider text-text-secondary hover:text-text-primary transition-colors"
                  >
                    {item.label}
                  </a>
                ) : (
                  <A
                    href={item.path!}
                    end={item.path === '/'}
                    class="font-mono text-xs tracking-wider transition-colors"
                    activeClass="text-text-primary"
                    inactiveClass="text-text-secondary hover:text-text-primary"
                  >
                    {item.label}
                  </A>
                )
              )}
            </For>
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
            <For each={NAV_ITEMS}>
              {(item) => (
                item.external ? (
                  <a
                    href="/"
                    class="block px-6 py-3 min-h-[44px] font-mono text-sm tracking-wider text-text-secondary hover:text-text-primary transition-colors flex items-center"
                    onClick={() => setMenuOpen(false)}
                  >
                    {item.label}
                  </a>
                ) : (
                  <A
                    href={item.path!}
                    end={item.path === '/'}
                    class="block px-6 py-3 min-h-[44px] font-mono text-sm tracking-wider transition-colors flex items-center"
                    activeClass="text-text-primary"
                    inactiveClass="text-text-secondary hover:text-text-primary"
                    onClick={() => setMenuOpen(false)}
                  >
                    {item.label}
                  </A>
                )
              )}
            </For>
          </nav>
        </Show>
      </header>

      {/* Spacer for fixed header */}
      <div style={{ height: 'var(--header-h)' }} />

      <main class="max-w-[1400px] mx-auto px-6 py-6">
        {props.children}
      </main>
    </div>
  )
}
