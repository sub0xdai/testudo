import { For, Show, type JSX } from 'solid-js'
import { A, useLocation } from '@solidjs/router'
import { FilterBar } from './FilterBar'

const NAV_ITEMS = [
  { path: '/', label: 'OVERVIEW' },
  { path: '/charts', label: 'CHARTS' },
  { path: '/trades', label: 'TRADES' },
  { path: '/journal', label: 'JOURNAL' },
]

export function Layout(props: { children: JSX.Element }) {
  const location = useLocation()

  return (
    <div class="min-h-screen bg-main-bg text-text-primary">
      <header class="fixed top-0 left-0 right-0 z-50 bg-main-bg/60 backdrop-blur-sm border-b border-container-border/30">
        <div class="max-w-[1400px] mx-auto px-6 md:px-8 py-4 flex items-center justify-between">
          <A href="/" class="font-mono text-lg tracking-widest text-text-primary hover:text-text-primary">
            TESTUDO
          </A>

          <nav class="flex items-center gap-6">
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
        </div>
      </header>

      {/* Spacer for fixed header */}
      <div class="h-[57px]" />

      <Show when={!location.pathname.startsWith('/journal')}>
        <FilterBar />
      </Show>

      <main class="max-w-[1400px] mx-auto px-6 py-6">
        {props.children}
      </main>
    </div>
  )
}
