import { createSignal, createResource, Show, For, createMemo } from 'solid-js'
import {
  fetchEntries,
  fetchTags,
  fetchTradeDetail,
  deleteEntry,
  type JournalEntry,
  type JournalTag,
} from '../../api/client'
import { EntryCard } from './EntryCard'
import { EntryEditor } from './EntryEditor'
import { TagManager } from './TagManager'
import { SkeletonBar } from '../SkeletonBar'

const ENTRY_TYPE_OPTIONS = [
  { value: '', label: 'All Types' },
  { value: 'note', label: 'Note' },
  { value: 'pre-trade', label: 'Pre-Trade' },
  { value: 'post-trade', label: 'Post-Trade' },
  { value: 'daily-review', label: 'Daily' },
  { value: 'weekly-review', label: 'Weekly' },
]

function groupByDate(entries: JournalEntry[]): [string, JournalEntry[]][] {
  const groups = new Map<string, JournalEntry[]>()
  for (const entry of entries) {
    const date = new Date(entry.created_at).toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    })
    const existing = groups.get(date) ?? []
    existing.push(entry)
    groups.set(date, existing)
  }
  return Array.from(groups.entries())
}

export function JournalTimeline() {
  const [typeFilter, setTypeFilter] = createSignal('')
  const [tagFilter, setTagFilter] = createSignal('')
  const [dateFrom, setDateFrom] = createSignal('')
  const [dateTo, setDateTo] = createSignal('')
  const [refreshKey, setRefreshKey] = createSignal(0)
  const [showEditor, setShowEditor] = createSignal(false)
  const [editingEntry, setEditingEntry] = createSignal<JournalEntry | null>(null)
  const [showTagManager, setShowTagManager] = createSignal(false)

  // Fetch all entries (large limit for client-side filtering)
  const [entriesData, { refetch }] = createResource(
    () => ({ limit: 200, _key: refreshKey() }),
    (params) => fetchEntries({ limit: params.limit }),
  )

  const [tags, { refetch: refetchTags }] = createResource(fetchTags)

  // Trade detail cache: tradeId -> { tags, symbol, closed_at }
  const [tradeDetailCache, setTradeDetailCache] = createSignal<
    Record<string, { tags: JournalTag[]; symbol: string; closed_at: string }>
  >({})

  async function loadTradeDetail(tradeId: string) {
    if (tradeDetailCache()[tradeId]) return
    try {
      const detail = await fetchTradeDetail(tradeId)
      setTradeDetailCache((prev) => ({
        ...prev,
        [tradeId]: { tags: detail.tags, symbol: detail.symbol, closed_at: detail.closed_at },
      }))
    } catch {
      // Trade may not exist
    }
  }

  // Client-side filtering
  const filteredEntries = createMemo(() => {
    const data = entriesData()
    if (!data) return []
    let entries = data.entries

    const type = typeFilter()
    if (type) entries = entries.filter((e) => e.entry_type === type)

    const from = dateFrom()
    if (from) entries = entries.filter((e) => e.created_at >= from)

    const to = dateTo()
    if (to) entries = entries.filter((e) => e.created_at <= to + 'T23:59:59')

    const tag = tagFilter()
    if (tag) {
      const cache = tradeDetailCache()
      entries = entries.filter((e) => {
        if (!e.trade_id) return false
        const tradeTags = cache[e.trade_id]?.tags
        return tradeTags?.some((t) => t.name === tag)
      })
    }

    // Load trade tags for entries that have trade_ids
    for (const entry of entries) {
      if (entry.trade_id) loadTradeDetail(entry.trade_id)
    }

    return entries
  })

  const grouped = createMemo(() => groupByDate(filteredEntries()))

  function refresh() {
    setRefreshKey((k) => k + 1)
    refetch()
  }

  function handleNewEntry() {
    setEditingEntry(null)
    setShowEditor(true)
  }

  function handleEdit(entry: JournalEntry) {
    setEditingEntry(entry)
    setShowEditor(true)
  }

  async function handleDelete(entry: JournalEntry) {
    try {
      await deleteEntry(entry.id)
      refresh()
    } catch (e) {
      console.error('Failed to delete entry:', e)
    }
  }

  function handleEditorSave() {
    setShowEditor(false)
    setEditingEntry(null)
    refresh()
  }

  function getEntryTags(entry: JournalEntry): JournalTag[] {
    if (!entry.trade_id) return []
    return tradeDetailCache()[entry.trade_id]?.tags ?? []
  }

  function getTradeLabel(entry: JournalEntry): string | undefined {
    if (!entry.trade_id) return undefined
    const cached = tradeDetailCache()[entry.trade_id]
    if (!cached) return `Trade ${entry.trade_id.slice(0, 8)}...`
    const symbol = cached.symbol.replace('_', '')
    const date = new Date(cached.closed_at).toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
    })
    return `${symbol} ${date}`
  }

  return (
    <div>
      {/* Header */}
      <div class="flex items-center justify-between mb-6">
        <h2 class="font-display text-lg font-bold tracking-wider text-text-primary">
          JOURNAL
        </h2>
        <div class="flex gap-2">
          <button
            class="px-3 py-1.5 border border-container-border text-text-secondary font-mono text-xs rounded hover:border-border-active hover:text-text-primary transition-colors"
            onClick={() => setShowTagManager(true)}
          >
            Tags
          </button>
          <button
            class="px-3 py-1.5 border border-text-primary text-text-primary font-mono text-xs rounded hover:bg-text-primary hover:text-main-bg transition-colors"
            onClick={handleNewEntry}
          >
            + New Entry
          </button>
        </div>
      </div>

      {/* Filters */}
      <div class="flex flex-wrap gap-3 mb-6 p-4 bg-container-bg border border-container-border rounded-lg">
        <label class="flex items-center gap-1.5">
          <span class="font-mono text-[10px] text-text-tertiary uppercase tracking-wider">Type</span>
          <select
            class="bg-elevated border border-container-border rounded px-3 py-1.5 font-mono text-xs text-text-primary"
            value={typeFilter()}
            onChange={(e) => setTypeFilter(e.currentTarget.value)}
          >
            <For each={ENTRY_TYPE_OPTIONS}>
              {(opt) => <option value={opt.value}>{opt.label}</option>}
            </For>
          </select>
        </label>

        <label class="flex items-center gap-1.5">
          <span class="font-mono text-[10px] text-text-tertiary uppercase tracking-wider">Tag</span>
          <select
            class="bg-elevated border border-container-border rounded px-3 py-1.5 font-mono text-xs text-text-primary"
            value={tagFilter()}
            onChange={(e) => setTagFilter(e.currentTarget.value)}
          >
            <option value="">All Tags</option>
            <For each={tags() ?? []}>
              {(tag) => <option value={tag.name}>{tag.name}</option>}
            </For>
          </select>
        </label>

        <label class="flex items-center gap-1.5">
          <span class="font-mono text-[10px] text-text-tertiary uppercase tracking-wider">From</span>
          <input
            type="date"
            class="bg-elevated border border-container-border rounded px-3 py-1.5 font-mono text-xs text-text-primary"
            value={dateFrom()}
            onInput={(e) => setDateFrom(e.currentTarget.value)}
          />
        </label>
        <label class="flex items-center gap-1.5">
          <span class="font-mono text-[10px] text-text-tertiary uppercase tracking-wider">To</span>
          <input
            type="date"
            class="bg-elevated border border-container-border rounded px-3 py-1.5 font-mono text-xs text-text-primary"
            value={dateTo()}
            onInput={(e) => setDateTo(e.currentTarget.value)}
          />
        </label>

        <Show when={typeFilter() || tagFilter() || dateFrom() || dateTo()}>
          <button
            class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors"
            onClick={() => { setTypeFilter(''); setTagFilter(''); setDateFrom(''); setDateTo('') }}
          >
            [Clear]
          </button>
        </Show>
      </div>

      {/* Loading state — structural skeleton */}
      <Show when={entriesData.loading}>
        <div class="space-y-3">
          <For each={[1, 2, 3]}>
            {() => (
              <div class="bg-container-bg border border-container-border rounded-lg overflow-hidden">
                {/* Header bar with left accent */}
                <div class="px-4 py-2 flex items-center gap-3 border-b border-container-border" style={{ 'border-left': '3px solid rgba(148, 163, 184, 0.3)' }}>
                  <SkeletonBar width="56px" height="18px" />
                  <SkeletonBar width="140px" height="14px" />
                </div>
                {/* Body lines */}
                <div class="px-4 py-3 space-y-2">
                  <SkeletonBar width="100%" />
                  <SkeletonBar width="85%" />
                  <SkeletonBar width="60%" />
                </div>
                {/* Footer */}
                <div class="px-4 py-2 border-t border-container-border flex justify-between">
                  <SkeletonBar width="48px" height="10px" />
                  <SkeletonBar width="64px" height="10px" />
                </div>
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* Empty state */}
      <Show when={!entriesData.loading && filteredEntries().length === 0}>
        <div class="text-center py-16">
          <div class="font-mono text-text-tertiary text-sm mb-4">NO ENTRIES YET</div>
          <button
            class="px-4 py-2 border border-text-primary text-text-primary font-mono text-xs rounded hover:bg-text-primary hover:text-main-bg transition-colors"
            onClick={handleNewEntry}
          >
            Write your first entry
          </button>
        </div>
      </Show>

      {/* Timeline */}
      <Show when={!entriesData.loading && filteredEntries().length > 0}>
        <div class="space-y-8">
          <For each={grouped()}>
            {([date, entries]) => (
              <div>
                {/* Date separator */}
                <div class="flex items-center gap-4 mb-4">
                  <div class="h-px bg-container-border flex-1" />
                  <span class="font-mono text-xs text-text-tertiary tracking-wider">{date}</span>
                  <div class="h-px bg-container-border flex-1" />
                </div>

                {/* Entries for this date */}
                <div class="space-y-3">
                  <For each={entries}>
                    {(entry) => (
                      <EntryCard
                        entry={entry}
                        tags={getEntryTags(entry)}
                        tradeLabel={getTradeLabel(entry)}
                        onEdit={() => handleEdit(entry)}
                        onDelete={() => handleDelete(entry)}
                      />
                    )}
                  </For>
                </div>
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* Editor modal */}
      <Show when={showEditor()}>
        <EntryEditor
          entry={editingEntry() ?? undefined}
          linkedTags={editingEntry() ? getEntryTags(editingEntry()!) : undefined}
          onSave={handleEditorSave}
          onClose={() => { setShowEditor(false); setEditingEntry(null) }}
        />
      </Show>

      {/* Tag manager modal */}
      <Show when={showTagManager()}>
        <TagManager
          tags={tags() ?? []}
          onUpdate={() => refetchTags()}
          onClose={() => setShowTagManager(false)}
        />
      </Show>
    </div>
  )
}
