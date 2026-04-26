import { createSignal, createResource, Show, For, createMemo } from 'solid-js'
import { useCachedResource, invalidate } from '../../lib/cache'
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
import { DatabaseTable } from './DatabaseTable'
import { CollectionSidebar } from './CollectionSidebar'
import { StorageBar } from './StorageBar'
import { SkeletonBar } from '../SkeletonBar'
import { exportEntries } from '../../lib/export'
import {
  getCollections,
  type JournalCollection,
  type CollectionFilters,
} from '../../lib/collections'

type ViewMode = 'table' | 'cards'

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
  const [viewMode, setViewMode] = createSignal<ViewMode>('table')
  const [exporting, setExporting] = createSignal(false)
  const [exportProgress, setExportProgress] = createSignal('')
  const [storageRefreshKey, setStorageRefreshKey] = createSignal(0)
  const refreshStorage = () => setStorageRefreshKey((k) => k + 1)

  // Collection state
  const [collections, setCollections] = createSignal<JournalCollection[]>(getCollections())
  const [activeCollection, setActiveCollection] = createSignal<JournalCollection | null>(null)
  const [sidebarCollapsed, setSidebarCollapsed] = createSignal(false)

  function refreshCollections() {
    setCollections(getCollections())
  }

  function handleSelectCollection(collection: JournalCollection | null) {
    setActiveCollection(collection)
    if (collection) {
      // Apply collection's saved filters
      setTypeFilter(collection.filters.entry_type ?? '')
      setTagFilter(collection.filters.tag_name ?? '')
      setDateFrom(collection.filters.date_from ?? '')
      setDateTo(collection.filters.date_to ?? '')
    } else {
      // "All Entries" — clear filters
      setTypeFilter('')
      setTagFilter('')
      setDateFrom('')
      setDateTo('')
    }
  }

  const currentFilters = (): CollectionFilters => ({
    entry_type: typeFilter() || undefined,
    tag_name: tagFilter() || undefined,
    date_from: dateFrom() || undefined,
    date_to: dateTo() || undefined,
  })

  // Fetch all entries (large limit for client-side filtering)
  const [entriesData, { refetch }] = createResource(
    () => ({ limit: 200, _key: refreshKey() }),
    (params) => fetchEntries({ limit: params.limit }),
  )

  const tags = useCachedResource(
    () => 'tags:all',
    fetchTags,
    { staleMs: 5 * 60_000 },
  )

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

  async function handleBulkExport() {
    const entries = filteredEntries()
    if (!entries.length) return
    setExporting(true)
    setExportProgress(`0 / ${entries.length}`)
    const tagMap: Record<string, JournalTag[]> = {}
    for (const entry of entries) {
      tagMap[entry.id] = getEntryTags(entry)
    }
    await exportEntries(entries, tagMap, (current, total) => {
      setExportProgress(`${current} / ${total}`)
    })
    setExporting(false)
    setExportProgress('')
  }

  function handleClearFilters() {
    setTypeFilter('')
    setTagFilter('')
    setDateFrom('')
    setDateTo('')
    setActiveCollection(null)
  }

  return (
    <div class="flex" style={{ 'min-height': '400px' }}>
      {/* Sidebar */}
      <CollectionSidebar
        collections={collections()}
        activeId={activeCollection()?.id ?? null}
        onSelect={handleSelectCollection}
        onChange={refreshCollections}
        collapsed={sidebarCollapsed()}
        onToggleCollapse={() => setSidebarCollapsed(!sidebarCollapsed())}
        currentFilters={currentFilters()}
      />

      {/* Main content */}
      <div class="flex-1 min-w-0">
        {/* Header */}
        <div class="flex items-center justify-between mb-6">
          <div class="flex items-center gap-4">
            <h2 class="font-display text-lg font-bold tracking-wider text-text-primary">
              {activeCollection()?.name ?? 'JOURNAL'}
            </h2>
            <StorageBar refreshKey={storageRefreshKey()} />
          </div>
          <div class="flex items-center gap-2">
            {/* View toggle */}
            <div class="flex border border-container-border">
              <button
                class="px-2 py-1.5 font-mono text-xs transition-colors"
                classList={{
                  'bg-text-primary text-main-bg': viewMode() === 'table',
                  'text-text-tertiary hover:text-text-primary': viewMode() !== 'table',
                }}
                onClick={() => setViewMode('table')}
                title="Table view"
              >
                Table
              </button>
              <button
                class="px-2 py-1.5 font-mono text-xs border-l border-container-border transition-colors"
                classList={{
                  'bg-text-primary text-main-bg': viewMode() === 'cards',
                  'text-text-tertiary hover:text-text-primary': viewMode() !== 'cards',
                }}
                onClick={() => setViewMode('cards')}
                title="Card view"
              >
                Cards
              </button>
            </div>
            <Show when={exporting()}>
              <span class="font-mono text-xs text-text-tertiary">Exporting {exportProgress()}...</span>
            </Show>
            <button
              class="btn-secondary px-3 py-1.5 disabled:opacity-50 disabled:cursor-not-allowed"
              onClick={handleBulkExport}
              disabled={exporting() || filteredEntries().length === 0}
            >
              Export All
            </button>
            <button
              class="btn-secondary px-3 py-1.5"
              onClick={() => setShowTagManager(true)}
            >
              Tags
            </button>
            <button
              class="btn-primary px-3 py-1.5 text-xs"
              onClick={handleNewEntry}
            >
              + New Entry
            </button>
          </div>
        </div>

        {/* Filters */}
        <div class="flex flex-wrap gap-3 mb-6 p-4 bg-container-bg border border-container-border">
          <label class="flex items-center gap-1.5">
            <span class="font-mono text-[10px] text-text-tertiary uppercase tracking-wider">Type</span>
            <select
              class="bg-elevated border border-container-border px-3 py-1.5 font-mono text-xs text-text-primary"
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
              class="bg-elevated border border-container-border px-3 py-1.5 font-mono text-xs text-text-primary"
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
              class="bg-elevated border border-container-border px-3 py-1.5 font-mono text-xs text-text-primary"
              value={dateFrom()}
              onInput={(e) => setDateFrom(e.currentTarget.value)}
            />
          </label>
          <label class="flex items-center gap-1.5">
            <span class="font-mono text-[10px] text-text-tertiary uppercase tracking-wider">To</span>
            <input
              type="date"
              class="bg-elevated border border-container-border px-3 py-1.5 font-mono text-xs text-text-primary"
              value={dateTo()}
              onInput={(e) => setDateTo(e.currentTarget.value)}
            />
          </label>

          <Show when={typeFilter() || tagFilter() || dateFrom() || dateTo()}>
            <button
              class="btn-ghost"
              onClick={handleClearFilters}
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
                <div class="bg-container-bg border border-container-border overflow-hidden">
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
            <p class="font-mono text-sm text-text-secondary mb-2">NO JOURNAL ENTRIES</p>
            <p class="font-mono text-xs text-text-tertiary mb-4">
              Click on a trade to open the detail panel, then write your thesis in the notes section.
            </p>
            <button
              class="btn-primary px-4 py-2 text-xs"
              onClick={handleNewEntry}
            >
              Write your first entry
            </button>
          </div>
        </Show>

        {/* Content: Table or Cards */}
        <Show when={!entriesData.loading && filteredEntries().length > 0}>
          <Show
            when={viewMode() === 'table'}
            fallback={
              <div class="space-y-8">
                <For each={grouped()}>
                  {([date, entries]) => (
                    <div>
                      <div class="flex items-center gap-4 mb-4">
                        <div class="h-px bg-container-border flex-1" />
                        <span class="font-mono text-xs text-text-tertiary tracking-wider">{date}</span>
                        <div class="h-px bg-container-border flex-1" />
                      </div>
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
            }
          >
            <DatabaseTable
              entries={filteredEntries()}
              getEntryTags={getEntryTags}
              getTradeLabel={getTradeLabel}
              onEdit={handleEdit}
              onDelete={handleDelete}
            />
          </Show>
        </Show>
      </div>

      {/* Editor modal */}
      <Show when={showEditor()}>
        <EntryEditor
          entry={editingEntry() ?? undefined}
          linkedTags={editingEntry() ? getEntryTags(editingEntry()!) : undefined}
          onSave={handleEditorSave}
          onClose={() => { setShowEditor(false); setEditingEntry(null) }}
          onStorageChange={refreshStorage}
        />
      </Show>

      {/* Tag manager modal */}
      <Show when={showTagManager()}>
        <TagManager
          tags={tags() ?? []}
          onUpdate={() => { invalidate('tags:'); tags.refetch() }}
          onClose={() => setShowTagManager(false)}
        />
      </Show>
    </div>
  )
}
