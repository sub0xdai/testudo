import { For, type JSX } from 'solid-js'
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
      <header class="border-b border-container-border bg-container-bg">
        <div class="max-w-[1400px] mx-auto px-6 py-4 flex items-center justify-between">
          <h1 class="font-display text-xl font-bold tracking-wider">
            TESTUDO<span class="text-signal-green">_</span>JOURNAL
          </h1>

          <nav class="flex gap-1">
            <For each={NAV_ITEMS}>
              {(item) => (
                <A
                  href={item.path}
                  class="px-4 py-2 font-mono text-sm tracking-wide transition-colors border border-transparent"
                  classList={{
                    'text-signal-green border-signal-green bg-signal-green/5': location.pathname === item.path,
                    'text-text-secondary hover:text-text-primary hover:border-container-border': location.pathname !== item.path,
                  }}
                >
                  {item.label}
                </A>
              )}
            </For>
          </nav>
        </div>
      </header>

      <FilterBar />

      <main class="max-w-[1400px] mx-auto px-6 py-6">
        {props.children}
      </main>
    </div>
  )
}
