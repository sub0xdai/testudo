import { createSignal, createResource, Show, For, onCleanup, onMount } from 'solid-js'
import {
  fetchTradeDetail,
  fetchTags,
  updateTradeNotes,
  addTradeTags,
  removeTradeTag,
  createTag,
  type TradeDetail as TradeDetailType,
  type JournalTag,
} from '../../api/client'
import { MarkdownPreview } from '../journal/MarkdownPreview'
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
import { SkeletonBar } from '../SkeletonBar'
import { createFocusTrap } from '../../lib/createFocusTrap'
import { CLOSE_ANIMATION_MS } from '../../lib/tokens'

export function TradeDetail(props: { tradeId: string; onClose: () => void }) {
  let panelRef!: HTMLDivElement
  const [detail, { refetch }] = createResource(() => props.tradeId, fetchTradeDetail)
  const [allTags] = createResource(fetchTags)
  const [notes, setNotes] = createSignal('')
  const [notesDirty, setNotesDirty] = createSignal(false)
  const [saving, setSaving] = createSignal(false)
  const [showTagPicker, setShowTagPicker] = createSignal(false)
  const [closing, setClosing] = createSignal(false)
  const [previewMode, setPreviewMode] = createSignal(false)
  const [newTagName, setNewTagName] = createSignal('')
  const [creatingTag, setCreatingTag] = createSignal(false)

  createFocusTrap(() => panelRef)

  function requestClose() {
    setClosing(true)
    setTimeout(props.onClose, CLOSE_ANIMATION_MS)
  }

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
    if (e.key === 'Escape') requestClose()
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

  async function handleCreateTag() {
    const name = newTagName().trim()
    if (!name || creatingTag()) return
    setCreatingTag(true)
    try {
      const tag = await createTag({ name })
      await addTradeTags(props.tradeId, [tag.id])
      setNewTagName('')
      setShowTagPicker(false)
      refetch()
    } finally {
      setCreatingTag(false)
    }
  }

  function exportNotes() {
    const d = detail()
    if (!d) return
    const frontmatter = [
      '---',
      `symbol: ${d.symbol}`,
      `side: ${d.side}`,
      `entry: ${d.entry_price}`,
      `exit: ${d.exit_price}`,
      `pnl: ${d.net_pnl}`,
      `date: ${d.closed_at}`,
      `exchange: ${d.exchange}`,
      '---',
    ].join('\n')
    const content = `${frontmatter}\n\n${d.notes || ''}`
    const blob = new Blob([content], { type: 'text/markdown' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `${d.symbol}_${d.closed_at.slice(0, 10)}.md`
    a.click()
    URL.revokeObjectURL(url)
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
        class={`fixed inset-0 bg-black/60 z-40 ${closing() ? 'animate-fade-out' : 'animate-fade-in'}`}
        onClick={requestClose}
      />

      {/* Panel */}
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="trade-detail-title"
        class={`fixed top-0 right-0 h-full w-full max-w-md bg-container-bg border-l border-container-border z-50 overflow-y-auto ${closing() ? 'animate-slide-out-right' : 'animate-slide-in-right'}`}
      >
        {/* Header */}
        <div class="sticky top-0 bg-container-bg border-b border-container-border px-5 py-4 flex items-center justify-between">
          <Show when={detail()} fallback={<div class="flex gap-2"><SkeletonBar width="60px" height="16px" /><SkeletonBar width="32px" height="16px" /><SkeletonBar width="40px" height="16px" /></div>}>
            {(d) => (
              <div id="trade-detail-title">
                <span class="font-mono text-sm text-text-primary">{d().symbol}</span>
                <span class="mx-2 text-text-tertiary">&middot;</span>
                <span class={`font-mono text-sm uppercase ${sideColor(d().side)}`} aria-label={`${d().side} position`}>{d().side}</span>
                <span class="mx-2 text-text-tertiary">&middot;</span>
                <span class="font-mono text-xs text-text-secondary uppercase">{d().exchange}</span>
              </div>
            )}
          </Show>
          <button
            class="text-text-secondary hover:text-text-primary text-lg transition-colors"
            onClick={requestClose}
            aria-label="Close trade detail"
          >
            &times;
          </button>
        </div>

        <Show
          when={!detail.loading && detail()}
          fallback={
            <div class="p-5 space-y-6">
              {/* Date skeleton */}
              <SkeletonBar width="160px" />
              {/* Price grid skeleton */}
              <div class="grid grid-cols-2 gap-x-6 gap-y-2">
                <div class="flex justify-between"><SkeletonBar width="40px" /><SkeletonBar width="64px" /></div>
                <div class="flex justify-between"><SkeletonBar width="32px" /><SkeletonBar width="64px" /></div>
                <div class="flex justify-between"><SkeletonBar width="48px" /><SkeletonBar width="56px" /></div>
                <div class="flex justify-between"><SkeletonBar width="44px" /><SkeletonBar width="56px" /></div>
              </div>
              <div class="border-t border-container-border" />
              {/* P&L grid skeleton */}
              <div class="grid grid-cols-2 gap-x-6 gap-y-2">
                <div class="flex justify-between"><SkeletonBar width="48px" /><SkeletonBar width="72px" /></div>
                <div class="flex justify-between"><SkeletonBar width="56px" /><SkeletonBar width="40px" /></div>
                <div class="flex justify-between"><SkeletonBar width="32px" /><SkeletonBar width="56px" /></div>
                <div class="flex justify-between"><SkeletonBar width="40px" /><SkeletonBar width="48px" /></div>
              </div>
              <div class="border-t border-container-border" />
              {/* Tags row skeleton */}
              <div>
                <SkeletonBar width="36px" height="10px" class="mb-2" />
                <div class="flex gap-1.5">
                  <SkeletonBar width="48px" height="20px" />
                  <SkeletonBar width="56px" height="20px" />
                </div>
              </div>
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
                      class="text-xs font-mono text-text-secondary hover:text-text-primary transition-colors"
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
                    <div
                      role="listbox"
                      aria-label="Available tags"
                      class="mt-2 p-2 bg-elevated border border-container-border rounded shadow-lg shadow-black/30 animate-dropdown-in"
                    >
                      <Show when={availableTags().length > 0}>
                        <div class="flex flex-wrap gap-1.5 mb-2">
                          <For each={availableTags()}>
                            {(tag, i) => (
                              <button role="option" onClick={() => handleAddTag(tag.id)} aria-label={`Add tag ${tag.name}`}>
                                <TagBadge tag={tag} index={i()} />
                              </button>
                            )}
                          </For>
                        </div>
                      </Show>
                      {/* Inline tag creation */}
                      <form
                        onSubmit={(e) => { e.preventDefault(); handleCreateTag() }}
                        class="flex gap-1.5 items-center"
                      >
                        <input
                          type="text"
                          value={newTagName()}
                          onInput={(e) => setNewTagName(e.currentTarget.value)}
                          placeholder="New tag..."
                          class="flex-1 px-2 py-1 bg-main-bg border border-container-border text-xs font-mono text-text-primary placeholder:text-text-tertiary outline-none"
                        />
                        <button
                          type="submit"
                          disabled={!newTagName().trim() || creatingTag()}
                          class="px-2 py-1 text-xs font-mono text-text-primary border border-container-border hover:bg-text-primary hover:text-main-bg transition-colors disabled:opacity-30"
                        >
                          +
                        </button>
                      </form>
                    </div>
                  </Show>
                </div>

                {/* Notes */}
                <div>
                  <div class="flex items-center justify-between mb-2">
                    <span class="text-[10px] font-display font-medium tracking-widest uppercase text-text-tertiary">
                      NOTES
                    </span>
                    <div class="flex items-center gap-2">
                      <button
                        class={`text-[10px] font-mono transition-colors ${previewMode() ? 'text-text-tertiary hover:text-text-secondary' : 'text-text-primary'}`}
                        onClick={() => setPreviewMode(false)}
                      >
                        EDIT
                      </button>
                      <span class="text-text-tertiary text-[10px]">/</span>
                      <button
                        class={`text-[10px] font-mono transition-colors ${previewMode() ? 'text-text-primary' : 'text-text-tertiary hover:text-text-secondary'}`}
                        onClick={() => setPreviewMode(true)}
                      >
                        PREVIEW
                      </button>
                      <Show when={notes()}>
                        <button
                          class="text-[10px] font-mono text-text-tertiary hover:text-text-secondary transition-colors ml-1"
                          onClick={exportNotes}
                          title="Export as .md"
                        >
                          &#8615; .MD
                        </button>
                      </Show>
                    </div>
                  </div>
                  <Show
                    when={!previewMode()}
                    fallback={
                      <div class="px-3 py-2 bg-main-bg border border-container-border rounded min-h-[80px]">
                        <MarkdownPreview content={notes()} />
                      </div>
                    }
                  >
                    <textarea
                      value={notes()}
                      onInput={(e) => {
                        setNotes(e.currentTarget.value)
                        setNotesDirty(true)
                      }}
                      onBlur={() => { if (notesDirty()) saveNotes() }}
                      placeholder="Markdown notes..."
                      class="w-full h-32 px-3 py-2 bg-main-bg border border-container-border rounded text-text-primary text-xs font-mono placeholder:text-text-tertiary resize-y"
                    />
                  </Show>
                  <Show when={notesDirty()}>
                    <button
                      class="mt-1 px-3 py-1 text-xs font-mono border border-text-primary text-text-primary hover:bg-text-primary hover:text-main-bg rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
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
