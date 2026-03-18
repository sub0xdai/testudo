import { createSignal, createMemo, Show, For } from 'solid-js'
import type { SymbolCount } from '../api/client'
import { useEscapeClose } from '../lib/useEscapeClose'

export function SymbolSearch(props: {
  symbols: SymbolCount[]
  value: string
  onSelect: (symbol: string) => void
}) {
  const [open, setOpen] = createSignal(false)
  const [search, setSearch] = createSignal('')
  const [highlightIdx, setHighlightIdx] = createSignal(0)
  let inputRef: HTMLInputElement | undefined

  useEscapeClose(() => {
    if (open()) setOpen(false)
  })

  const totalCount = createMemo(() =>
    props.symbols.reduce((sum, s) => sum + s.count, 0)
  )

  const filtered = createMemo(() => {
    const q = search().toLowerCase()
    const list = q
      ? props.symbols.filter((s) => s.symbol.toLowerCase().includes(q))
      : props.symbols
    return list.slice(0, 10)
  })

  function openDropdown() {
    setOpen(true)
    setSearch('')
    setHighlightIdx(0)
  }

  function select(symbol: string) {
    props.onSelect(symbol)
    setOpen(false)
    setSearch('')
  }

  function onKeyDown(e: KeyboardEvent) {
    const items = filtered()
    const max = items.length // +1 for ALL option handled via index offset
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setHighlightIdx((i) => Math.min(i + 1, max))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setHighlightIdx((i) => Math.max(i - 1, 0))
    } else if (e.key === 'Enter') {
      e.preventDefault()
      const idx = highlightIdx()
      if (idx === 0) {
        select('')
      } else if (items[idx - 1]) {
        select(items[idx - 1].symbol)
      }
    }
  }

  function handleInput(val: string) {
    setSearch(val)
    setHighlightIdx(0)
    if (!open()) setOpen(true)
  }

  // Close on outside click
  function handleBackdropClick() {
    setOpen(false)
    setSearch('')
  }

  const hasSymbols = () => props.symbols.length > 0
  const displayValue = () => {
    if (props.value) return props.value.toUpperCase()
    return hasSymbols() ? 'ALL' : '—'
  }

  return (
    <div class="relative">
      <button
        class="bg-elevated border border-container-border text-text-primary font-mono text-sm px-3 py-1.5 rounded flex items-center gap-1.5 hover:border-text-secondary transition-colors min-w-[120px] text-left focus-visible:border-text-secondary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-text-secondary/30 focus-visible:ring-offset-1 focus-visible:ring-offset-main-bg"
        onClick={() => hasSymbols() && openDropdown()}
        aria-haspopup="listbox"
        aria-expanded={open()}
        aria-labelledby="symbol-label"
        disabled={!hasSymbols()}
      >
        <span class="flex-1 truncate">{displayValue()}</span>
        <span class="text-text-tertiary text-xs">▾</span>
      </button>

      <Show when={open()}>
        <div
          class="absolute z-50 top-full left-0 mt-1 bg-elevated border border-container-border rounded shadow-lg shadow-black/30 min-w-[220px] animate-dropdown-in"
        >
          {/* Search input */}
          <div class="p-2 border-b border-container-border">
            <input
              ref={inputRef}
              type="text"
              placeholder="Search symbols..."
              class="w-full bg-container-bg border border-container-border text-text-primary font-mono text-xs px-2 py-1.5 rounded focus-visible:border-text-secondary focus-visible:outline-none placeholder:text-text-tertiary"
              value={search()}
              onInput={(e) => handleInput(e.currentTarget.value)}
              onKeyDown={onKeyDown}
              aria-haspopup="listbox"
              aria-expanded={open()}
            />
          </div>

          {/* Options list */}
          <div role="listbox" aria-label="Symbol filter" class="max-h-60 overflow-y-auto">
            {/* ALL option */}
            <button
              role="option"
              class={`w-full text-left px-3 py-2 hover:bg-container-bg-hover transition-colors flex items-center justify-between ${
                highlightIdx() === 0 ? 'bg-container-bg-hover' : ''
              }`}
              onClick={() => select('')}
            >
              <span class="font-mono text-xs text-text-primary">ALL</span>
              <span class="font-mono text-xs text-text-tertiary">{totalCount()}</span>
            </button>

            <For each={filtered()}>
              {(item, i) => (
                <button
                  role="option"
                  class={`w-full text-left px-3 py-2 hover:bg-container-bg-hover transition-colors flex items-center justify-between ${
                    highlightIdx() === i() + 1 ? 'bg-container-bg-hover' : ''
                  }`}
                  onClick={() => select(item.symbol)}
                >
                  <span class="font-mono text-xs text-text-primary">{item.symbol.toUpperCase()}</span>
                  <span class="font-mono text-xs text-text-tertiary">({item.count})</span>
                </button>
              )}
            </For>

            <Show when={filtered().length === 0 && search()}>
              <div class="px-3 py-2 font-mono text-xs text-text-tertiary">No matching symbols</div>
            </Show>
          </div>
        </div>
        <div class="fixed inset-0 z-40" onClick={handleBackdropClick} />
      </Show>
    </div>
  )
}
