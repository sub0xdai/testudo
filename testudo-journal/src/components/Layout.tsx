import { createSignal, For, Show, type JSX } from 'solid-js'
import { A, useLocation } from '@solidjs/router'

const NAV_ITEMS = [
  { path: '/', label: 'OVERVIEW' },
  { path: '/charts', label: 'ANALYSIS' },
  { path: '/trades', label: 'TRADES' },
  { path: '/journal', label: 'JOURNAL' },
]

export function Layout(props: { children: JSX.Element }) {
  const location = useLocation()
  const [menuOpen, setMenuOpen] = createSignal(false)

  return (
    <div class="min-h-screen bg-main-bg text-text-primary">
      <header class="fixed top-0 left-0 right-0 z-50 bg-main-bg/60 backdrop-blur-sm border-b border-container-border/30">
        <div class="max-w-[1400px] mx-auto px-6 md:px-8 py-4 flex items-center justify-between">
          <A href="/" class="font-mono text-lg tracking-widest text-text-primary hover:text-text-primary">
            TESTUDO
          </A>

          {/* Desktop nav */}
          <nav class="hidden md:flex items-center gap-6">
            <For each={NAV_ITEMS}>
              {(item) => (
                <A
                  href={item.path}
                  class="font-mono text-xs tracking-wider transition-colors"
                  classList={{
                    'text-text-primary': location.pathname === item.path,
                    'text-text-secondary hover:text-text-primary': location.pathname !== item.path,
                  }}
                  aria-current={location.pathname === item.path ? 'page' : undefined}
                >
                  {item.label}
                </A>
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
                <A
                  href={item.path}
                  class="block px-6 py-3 min-h-[44px] font-mono text-sm tracking-wider transition-colors flex items-center"
                  classList={{
                    'text-text-primary': location.pathname === item.path,
                    'text-text-secondary hover:text-text-primary': location.pathname !== item.path,
                  }}
                  aria-current={location.pathname === item.path ? 'page' : undefined}
                  onClick={() => setMenuOpen(false)}
                >
                  {item.label}
                </A>
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
