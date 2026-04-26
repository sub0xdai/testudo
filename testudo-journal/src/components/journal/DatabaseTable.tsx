import { createSignal, createMemo, For, Show } from 'solid-js'
import type { JournalEntry, JournalTag } from '../../api/client'
import { TagBadge } from '../trades/TagBadge'
import { getEntryTypeColors } from '../../lib/tokens'
import { formatDateFull } from '../../lib/formatters'

type SortKey = 'created_at' | 'title' | 'entry_type' | 'asset'

const TYPE_LABELS: Record<string, string> = {
  'note': 'NOTE',
  'pre-trade': 'PRE-TRADE',
  'post-trade': 'POST-TRADE',
  'daily-review': 'DAILY',
  'weekly-review': 'WEEKLY',
}

const PAGE_SIZES = [25, 50, 100] as const

/** Strip markdown syntax and return plain text */
function stripMarkdown(md: string): string {
  return md
    .replace(/!\[.*?\]\(.*?\)/g, '')       // images
    .replace(/\[([^\]]*)\]\(.*?\)/g, '$1') // links
    .replace(/[*_~`#>-]/g, '')             // formatting chars
    .replace(/\n+/g, ' ')                  // newlines
    .trim()
}

export function DatabaseTable(props: {
  entries: JournalEntry[]
  getEntryTags: (entry: JournalEntry) => JournalTag[]
  getTradeLabel: (entry: JournalEntry) => string | undefined
  onEdit: (entry: JournalEntry) => void
  onDelete: (entry: JournalEntry) => void
}) {
  const [sortBy, setSortBy] = createSignal<SortKey>('created_at')
  const [sortDir, setSortDir] = createSignal<'asc' | 'desc'>('desc')
  const [page, setPage] = createSignal(1)
  const [pageSize, setPageSize] = createSignal<number>(25)
  const [selectedIds, setSelectedIds] = createSignal<Set<string>>(new Set())
  let lastClickedIndex = -1

  // Reset page when entries change
  const entryCount = createMemo(() => props.entries.length)
  let prevCount = entryCount()
  createMemo(() => {
    const count = entryCount()
    if (count !== prevCount) {
      setPage(1)
      prevCount = count
    }
  })

  const sortedEntries = createMemo(() => {
    const entries = [...props.entries]
    const key = sortBy()
    const dir = sortDir()

    entries.sort((a, b) => {
      let aVal: string
      let bVal: string

      if (key === 'asset') {
        aVal = (props.getTradeLabel(a) ?? '').toLowerCase()
        bVal = (props.getTradeLabel(b) ?? '').toLowerCase()
      } else {
        aVal = (a[key] ?? '').toLowerCase()
        bVal = (b[key] ?? '').toLowerCase()
      }

      if (aVal < bVal) return dir === 'asc' ? -1 : 1
      if (aVal > bVal) return dir === 'asc' ? 1 : -1
      return 0
    })

    return entries
  })

  const totalPages = createMemo(() => Math.max(1, Math.ceil(props.entries.length / pageSize())))

  const paginatedEntries = createMemo(() => {
    const start = (page() - 1) * pageSize()
    return sortedEntries().slice(start, start + pageSize())
  })

  function handleSort(key: SortKey) {
    if (sortBy() === key) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'))
    } else {
      setSortBy(key)
      setSortDir('desc')
    }
    setPage(1)
  }

  function toggleSelect(id: string, index: number, shiftKey: boolean) {
    const paginated = paginatedEntries()

    if (shiftKey && lastClickedIndex >= 0) {
      const from = Math.min(lastClickedIndex, index)
      const to = Math.max(lastClickedIndex, index)
      setSelectedIds((prev) => {
        const next = new Set(prev)
        for (let i = from; i <= to; i++) {
          next.add(paginated[i].id)
        }
        return next
      })
    } else {
      setSelectedIds((prev) => {
        const next = new Set(prev)
        if (next.has(id)) next.delete(id)
        else next.add(id)
        return next
      })
    }

    lastClickedIndex = index
  }

  function toggleSelectAll() {
    const ids = paginatedEntries().map((e) => e.id)
    const selected = selectedIds()
    const allSelected = ids.length > 0 && ids.every((id) => selected.has(id))

    if (allSelected) {
      setSelectedIds((prev) => {
        const next = new Set(prev)
        ids.forEach((id) => next.delete(id))
        return next
      })
    } else {
      setSelectedIds((prev) => {
        const next = new Set(prev)
        ids.forEach((id) => next.add(id))
        return next
      })
    }
  }

  function isAllSelected(): boolean {
    const ids = paginatedEntries().map((e) => e.id)
    const selected = selectedIds()
    return ids.length > 0 && ids.every((id) => selected.has(id))
  }

  function getTypeStyle(entryType: string) {
    const colors = getEntryTypeColors()
    return {
      color: colors[entryType] ?? colors['note'],
      label: TYPE_LABELS[entryType] ?? entryType.toUpperCase(),
    }
  }

  function SortHeader(p: { label: string; column: SortKey; width?: string }) {
    return (
      <th
        class="px-3 py-2 text-left cursor-pointer select-none hover:bg-bg-hover transition-colors"
        style={p.width ? { width: p.width } : undefined}
        onClick={() => handleSort(p.column)}
      >
        <span class="font-mono text-[10px] text-text-tertiary uppercase tracking-wider">
          {p.label}
          <Show when={sortBy() === p.column}>
            <span class="ml-1 text-text-secondary">{sortDir() === 'asc' ? '\u2191' : '\u2193'}</span>
          </Show>
        </span>
      </th>
    )
  }

  return (
    <div>
      {/* Selection info */}
      <Show when={selectedIds().size > 0}>
        <div class="flex items-center gap-3 mb-3 px-3 py-2 bg-container-bg border border-container-border">
          <span class="font-mono text-xs text-text-secondary">
            {selectedIds().size} selected
          </span>
          <button
            class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors"
            onClick={() => setSelectedIds(new Set())}
          >
            [Clear]
          </button>
        </div>
      </Show>

      {/* Table */}
      <div class="overflow-x-auto border border-container-border">
        <table class="w-full border-collapse">
          <thead>
            <tr class="border-b border-container-border bg-container-bg">
              <th class="w-10 px-3 py-2">
                <input
                  type="checkbox"
                  class="accent-text-primary"
                  checked={isAllSelected()}
                  onChange={toggleSelectAll}
                />
              </th>
              <SortHeader label="Date" column="created_at" width="110px" />
              <SortHeader label="Title" column="title" />
              <SortHeader label="Type" column="entry_type" width="100px" />
              <SortHeader label="Asset" column="asset" width="110px" />
              <th class="px-3 py-2 text-left" style={{ width: '150px' }}>
                <span class="font-mono text-[10px] text-text-tertiary uppercase tracking-wider">Tags</span>
              </th>
              <th class="px-3 py-2 text-left" style={{ width: '200px' }}>
                <span class="font-mono text-[10px] text-text-tertiary uppercase tracking-wider">Preview</span>
              </th>
            </tr>
          </thead>
          <tbody>
            <For each={paginatedEntries()}>
              {(entry, index) => {
                const typeStyle = () => getTypeStyle(entry.entry_type)
                const entryTags = () => props.getEntryTags(entry)
                const tradeLabel = () => props.getTradeLabel(entry)
                const preview = () => {
                  const text = stripMarkdown(entry.body)
                  return text.length > 80 ? text.slice(0, 80) + '\u2026' : text
                }

                return (
                  <tr
                    class="border-b border-container-border hover:bg-bg-hover transition-colors cursor-pointer"
                    classList={{ 'bg-bg-hover/50': selectedIds().has(entry.id) }}
                    onClick={() => props.onEdit(entry)}
                  >
                    <td class="px-3 py-2" onClick={(e) => e.stopPropagation()}>
                      <input
                        type="checkbox"
                        class="accent-text-primary"
                        checked={selectedIds().has(entry.id)}
                        onChange={(e) => toggleSelect(entry.id, index(), (e as unknown as MouseEvent).shiftKey)}
                      />
                    </td>
                    <td class="px-3 py-2">
                      <span class="font-mono text-xs text-text-secondary whitespace-nowrap">
                        {formatDateFull(entry.created_at)}
                      </span>
                    </td>
                    <td class="px-3 py-2">
                      <span class="font-display text-sm text-text-primary truncate block max-w-[300px]">
                        {entry.title}
                      </span>
                    </td>
                    <td class="px-3 py-2">
                      <span
                        class="font-mono text-[10px] tracking-[0.15em] font-bold px-2 py-0.5 inline-block"
                        style={{ color: typeStyle().color, background: `${typeStyle().color}15` }}
                      >
                        {typeStyle().label}
                      </span>
                    </td>
                    <td class="px-3 py-2">
                      <span class="font-mono text-xs text-text-secondary whitespace-nowrap">
                        {tradeLabel() ?? '\u2014'}
                      </span>
                    </td>
                    <td class="px-3 py-2">
                      <Show when={entryTags().length > 0} fallback={<span class="text-text-tertiary text-xs">{'\u2014'}</span>}>
                        <div class="flex gap-1 flex-wrap">
                          <For each={entryTags().slice(0, 3)}>
                            {(tag, i) => <TagBadge tag={tag} index={i()} />}
                          </For>
                          <Show when={entryTags().length > 3}>
                            <span class="font-mono text-[10px] text-text-tertiary">+{entryTags().length - 3}</span>
                          </Show>
                        </div>
                      </Show>
                    </td>
                    <td class="px-3 py-2">
                      <span class="font-mono text-xs text-text-tertiary truncate block max-w-[200px]">
                        {preview()}
                      </span>
                    </td>
                  </tr>
                )
              }}
            </For>
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      <div class="flex items-center justify-between mt-4">
        <div class="flex items-center gap-2">
          <span class="font-mono text-xs text-text-tertiary">
            {props.entries.length} entries
          </span>
          <select
            class="bg-elevated border border-container-border px-2 py-1 font-mono text-xs text-text-primary"
            value={pageSize()}
            onChange={(e) => {
              setPageSize(Number(e.currentTarget.value))
              setPage(1)
            }}
          >
            <For each={PAGE_SIZES}>
              {(size) => <option value={size}>{size} / page</option>}
            </For>
          </select>
        </div>

        <div class="flex items-center gap-2">
          <button
            class="px-2 py-1 border border-container-border font-mono text-xs text-text-secondary hover:text-text-primary hover:border-border-active transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
            disabled={page() <= 1}
            onClick={() => setPage((p) => Math.max(1, p - 1))}
          >
            &laquo; Prev
          </button>
          <span class="font-mono text-xs text-text-secondary">
            {page()} / {totalPages()}
          </span>
          <button
            class="px-2 py-1 border border-container-border font-mono text-xs text-text-secondary hover:text-text-primary hover:border-border-active transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
            disabled={page() >= totalPages()}
            onClick={() => setPage((p) => Math.min(totalPages(), p + 1))}
          >
            Next &raquo;
          </button>
        </div>
      </div>
    </div>
  )
}
