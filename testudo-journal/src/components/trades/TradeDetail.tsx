import { createSignal, createResource, Show, For, onCleanup } from 'solid-js'
import {
  fetchTradeDetail,
  fetchTags,
  updateTradeNotes,
  addTradeTags,
  removeTradeTag,
  type TradeDetail as TradeDetailType,
  type JournalTag,
} from '../../api/client'
import { TagBadge } from './TagBadge'
import {
  formatCurrency,
  formatPrice,
  formatNumber,
  formatPercent,
  formatDuration,
  formatDateFull,
  pnlColor,
  rColor,
  sideColor,
} from '../../lib/formatters'

export function TradeDetail(props: { tradeId: string; onClose: () => void }) {
  const [detail, { refetch }] = createResource(() => props.tradeId, fetchTradeDetail)
  const [allTags] = createResource(fetchTags)
  const [notes, setNotes] = createSignal('')
  const [notesDirty, setNotesDirty] = createSignal(false)
  const [saving, setSaving] = createSignal(false)
  const [showTagPicker, setShowTagPicker] = createSignal(false)

  // Sync notes from loaded detail
  const syncNotes = () => {
    const d = detail()
    if (d && !notesDirty()) {
      setNotes(d.notes ?? '')
    }
  }

  // Watch for detail changes
  createResource(() => detail(), () => { syncNotes(); return null })

  // Close on Escape
  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') props.onClose()
  }
  if (typeof window !== 'undefined') {
    window.addEventListener('keydown', handleKeyDown)
    onCleanup(() => window.removeEventListener('keydown', handleKeyDown))
  }

  async function saveNotes() {
    if (!detail()) return
    setSaving(true)
    try {
      await updateTradeNotes(props.tradeId, notes() || null)
      setNotesDirty(false)
      refetch()
    } finally {
      setSaving(false)
    }
  }

  async function handleAddTag(tagId: string) {
    await addTradeTags(props.tradeId, [tagId])
    setShowTagPicker(false)
    refetch()
  }

  async function handleRemoveTag(tagId: string) {
    await removeTradeTag(props.tradeId, tagId)
    refetch()
  }

  const availableTags = () => {
    const all = allTags() ?? []
    const existing = detail()?.tags?.map((t) => t.id) ?? []
    return all.filter((t) => !existing.includes(t.id))
  }

  return (
    <>
      {/* Backdrop */}
      <div
        class="fixed inset-0 bg-black/60 z-40"
        onClick={props.onClose}
      />

      {/* Panel */}
      <div class="fixed top-0 right-0 h-full w-full max-w-md bg-container-bg border-l border-container-border z-50 overflow-y-auto">
        {/* Header */}
        <div class="sticky top-0 bg-container-bg border-b border-container-border px-5 py-4 flex items-center justify-between">
          <Show when={detail()} fallback={<div class="h-5 w-40 bg-container-border/20 animate-pulse rounded" />}>
            {(d) => (
              <div>
                <span class="font-mono text-sm text-text-primary">{d().symbol}</span>
                <span class="mx-2 text-text-tertiary">&middot;</span>
                <span class={`font-mono text-sm uppercase ${sideColor(d().side)}`}>{d().side}</span>
                <span class="mx-2 text-text-tertiary">&middot;</span>
                <span class="font-mono text-xs text-text-secondary uppercase">{d().exchange}</span>
              </div>
            )}
          </Show>
          <button
            class="text-text-secondary hover:text-text-primary text-lg transition-colors"
            onClick={props.onClose}
          >
            &times;
          </button>
        </div>

        <Show
          when={!detail.loading && detail()}
          fallback={
            <div class="p-5 space-y-4">
              <For each={Array(8)}>
                {() => <div class="h-4 bg-container-border/20 rounded animate-pulse" />}
              </For>
            </div>
          }
        >
          {(d) => {
            // Sync notes on load
            if (!notesDirty()) setNotes(d().notes ?? '')

            return (
              <div class="p-5 space-y-6">
                {/* Dates */}
                <div class="text-xs font-mono text-text-secondary">
                  {formatDateFull(d().closed_at)} &middot; {formatDuration(d().duration_secs)}
                </div>

                {/* Price grid */}
                <div class="grid grid-cols-2 gap-x-6 gap-y-2">
                  <DetailRow label="Entry" value={formatPrice(d().entry_price)} />
                  <DetailRow label="Exit" value={formatPrice(d().exit_price)} />
                  <Show when={d().stop_price}>
                    <DetailRow label="Stop" value={formatPrice(d().stop_price!)} />
                  </Show>
                  <Show when={d().target_price}>
                    <DetailRow label="Target" value={formatPrice(d().target_price!)} />
                  </Show>
                  <DetailRow label="Quantity" value={formatNumber(d().quantity, 4)} />
                  <DetailRow label="Leverage" value={`${d().leverage}x`} />
                </div>

                {/* Divider */}
                <div class="border-t border-container-border" />

                {/* P&L section */}
                <div class="grid grid-cols-2 gap-x-6 gap-y-2">
                  <DetailRow
                    label="Net P&L"
                    value={formatCurrency(d().net_pnl)}
                    valueClass={pnlColor(d().net_pnl)}
                  />
                  <Show when={d().r_multiple}>
                    <DetailRow
                      label="R-Multiple"
                      value={`${parseFloat(d().r_multiple!).toFixed(1)}R`}
                      valueClass={rColor(d().r_multiple)}
                    />
                  </Show>
                  <DetailRow label="Fees" value={formatCurrency(d().fees)} />
                  <DetailRow
                    label="Return"
                    value={formatPercent(d().realized_pnl_pct)}
                    valueClass={pnlColor(d().realized_pnl_pct)}
                  />
                </div>

                {/* Divider */}
                <div class="border-t border-container-border" />

                {/* Tags */}
                <div>
                  <div class="flex items-center gap-2 mb-2">
                    <span class="text-[10px] font-display font-medium tracking-widest uppercase text-text-tertiary">
                      TAGS
                    </span>
                    <button
                      class="text-xs font-mono text-text-secondary hover:text-signal-green transition-colors"
                      onClick={() => setShowTagPicker(!showTagPicker())}
                    >
                      + Add
                    </button>
                  </div>
                  <div class="flex flex-wrap gap-1.5">
                    <For each={d().tags}>
                      {(tag, i) => (
                        <TagBadge
                          tag={tag}
                          index={i()}
                          onRemove={() => handleRemoveTag(tag.id)}
                        />
                      )}
                    </For>
                    <Show when={!d().tags.length && !showTagPicker()}>
                      <span class="text-xs font-mono text-text-tertiary">No tags</span>
                    </Show>
                  </div>

                  {/* Tag picker */}
                  <Show when={showTagPicker()}>
                    <div class="mt-2 p-2 border border-container-border bg-main-bg">
                      <Show
                        when={availableTags().length > 0}
                        fallback={<span class="text-xs font-mono text-text-tertiary">No more tags</span>}
                      >
                        <div class="flex flex-wrap gap-1.5">
                          <For each={availableTags()}>
                            {(tag, i) => (
                              <button onClick={() => handleAddTag(tag.id)}>
                                <TagBadge tag={tag} index={i()} />
                              </button>
                            )}
                          </For>
                        </div>
                      </Show>
                    </div>
                  </Show>
                </div>

                {/* Notes */}
                <div>
                  <span class="text-[10px] font-display font-medium tracking-widest uppercase text-text-tertiary block mb-2">
                    NOTES
                  </span>
                  <textarea
                    value={notes()}
                    onInput={(e) => {
                      setNotes(e.currentTarget.value)
                      setNotesDirty(true)
                    }}
                    onBlur={() => { if (notesDirty()) saveNotes() }}
                    placeholder="Quick note..."
                    class="w-full h-20 px-3 py-2 bg-main-bg border border-container-border text-text-primary text-xs font-mono placeholder:text-text-tertiary resize-none focus-visible:border-border-active focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-signal-green/30 focus-visible:ring-offset-1 focus-visible:ring-offset-main-bg"
                  />
                  <Show when={notesDirty()}>
                    <button
                      class="mt-1 px-3 py-1 text-xs font-mono border border-text-primary text-text-primary hover:bg-text-primary hover:text-main-bg rounded transition-colors disabled:opacity-50"
                      onClick={saveNotes}
                      disabled={saving()}
                    >
                      {saving() ? 'SAVING...' : 'SAVE'}
                    </button>
                  </Show>
                </div>

                {/* Journal entries */}
                <Show when={d().entries.length > 0}>
                  <div>
                    <span class="text-[10px] font-display font-medium tracking-widest uppercase text-text-tertiary block mb-2">
                      JOURNAL ENTRIES
                    </span>
                    <div class="space-y-1.5">
                      <For each={d().entries}>
                        {(entry) => (
                          <div class="flex items-center gap-2 px-3 py-2 border border-container-border/50 hover:border-container-border transition-colors">
                            <span class="text-text-tertiary text-xs">&#9654;</span>
                            <span class="text-xs font-mono text-text-primary flex-1 truncate">
                              {entry.title}
                            </span>
                            <span class="text-[10px] font-mono text-text-tertiary whitespace-nowrap">
                              {formatDateFull(entry.created_at)}
                            </span>
                          </div>
                        )}
                      </For>
                    </div>
                  </div>
                </Show>
              </div>
            )
          }}
        </Show>
      </div>
    </>
  )
}

function DetailRow(props: { label: string; value: string; valueClass?: string }) {
  return (
    <div class="flex justify-between items-baseline">
      <span class="text-xs font-display text-text-secondary">{props.label}</span>
      <span class={`text-sm font-mono ${props.valueClass ?? 'text-text-primary'}`}>{props.value}</span>
    </div>
  )
}
