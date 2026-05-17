# Specification: Transform Journal from Timeline to Database View

**Spec ID:** JNL-16-database-view
**Date:** 2026-03-22
**Status:** Draft
**Class:** Feature / UX Redesign
**Priority:** P1 — Core UX shift: entries become structured data, not a blog feed
**Depends on:** JNL-14-markdown-hardening
**Series:** JNL-14 through JNL-18 (Journal audit remediation + database redesign)

---

## Problem Statement

The journal currently renders entries as a chronological timeline in `JournalTimeline.tsx` (333 lines). Entries are grouped by date and displayed as cards with truncated markdown previews. Filtering is limited to entry type, tag, and date range — all applied client-side to a 200-entry fetch.

This timeline view has fundamental UX problems for a trading journal:

1. **No sorting** — entries are fixed in chronological order. A trader reviewing all "BTC" entries must scroll through the entire timeline.
2. **No column visibility** — key metadata (linked asset, P&L, entry type, tags) is buried inside each card rather than scannable in a row.
3. **No grouping** — related entries (e.g., pre-trade + post-trade for the same position) aren't visually connected.
4. **200-entry client-side cap** — pagination is absent; heavy users will hit the limit.

The journal should display entries as a database — a sortable, filterable table where each row is an entry and columns expose metadata. This matches how traders think: "show me all my BTC post-trade reviews sorted by date" is a database query, not a timeline scroll.

---

## User Stories

- **As a trader**, I want to sort and filter my journal entries like a database, so that I can find specific entries by asset, type, date, or tag without scrolling.
- **As a trader**, I want to see entry metadata (type, linked asset, tags, date) in scannable columns, so that I can review my journal efficiently.
- **As a user**, I want to switch between table view and card view, so that I can use whichever fits my current task.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Default journal view is a table with columns: Date, Title, Type, Asset, Tags, Preview | High | Journal page |
| FR-2 | All columns are sortable (click header to toggle asc/desc) | High | Table component |
| FR-3 | Column-level filtering: type dropdown, asset search, tag multi-select, date range | High | Table component |
| FR-4 | Clicking a row opens the entry in the editor modal (existing EntryEditor) | High | Table component |
| FR-5 | View toggle: Table view (default) / Card view (existing timeline layout) | Medium | Journal page |
| FR-6 | Server-side pagination with configurable page size (25/50/100) | High | API client, Table |
| FR-7 | Inline entry type badge with color coding (reuse existing `getEntryTypeColors`) | Medium | Table component |
| FR-8 | Bulk actions: select multiple rows → delete, export, tag | Medium | Table component |
| FR-9 | Empty state guides user to create first entry | Low | Table component |

---

## Technical Implementation

### Table Component

Create a reusable `DatabaseTable` component (new file):

```tsx
// src/components/journal/DatabaseTable.tsx

interface Column<T> {
  key: keyof T
  label: string
  width?: string
  sortable?: boolean
  render?: (value: T[keyof T], row: T) => JSX.Element
}

interface DatabaseTableProps {
  entries: JournalEntry[]
  columns: Column<JournalEntry>[]
  sortBy: string
  sortDir: 'asc' | 'desc'
  onSort: (key: string) => void
  onRowClick: (entry: JournalEntry) => void
  selectedIds: Set<string>
  onSelectionChange: (ids: Set<string>) => void
  page: number
  pageSize: number
  total: number
  onPageChange: (page: number) => void
}
```

### Column Definitions

| Column | Key | Width | Sortable | Render |
|--------|-----|-------|----------|--------|
| Checkbox | — | 40px | No | `<input type="checkbox">` |
| Date | `created_at` | 100px | Yes | `formatDate()` |
| Title | `title` | flex-1 | Yes | Truncated text, clickable |
| Type | `entry_type` | 90px | Yes | Color badge (existing pattern) |
| Asset | `trade_id` → linked trade symbol | 100px | Yes | Symbol or "—" |
| Tags | — | 150px | No | `<TagBadge>` row |
| Preview | `body` | 200px | No | First 80 chars, tertiary color |

### Server-Side Pagination

Update `fetchEntries` in `api/client.ts` to support sort parameters:

```typescript
export async function fetchEntries(params: {
  page?: number
  limit?: number
  sortBy?: string
  sortDir?: 'asc' | 'desc'
  entryType?: string
  tagId?: string
  symbol?: string
  dateFrom?: string
  dateTo?: string
}): Promise<{ entries: JournalEntry[]; total: number }>
```

[CLARIFY] Does the backend `/api/v1/journal/entries` endpoint already support `sort_by` and `sort_dir` query parameters? If not, this spec requires a backend change in testudo-exchange's journal router.

### View Toggle

```tsx
// Journal.tsx — view mode state
const [viewMode, setViewMode] = createSignal<'table' | 'cards'>('table')

// Render
<Show when={viewMode() === 'table'} fallback={<JournalTimeline ... />}>
  <DatabaseTable ... />
</Show>
```

### Files

- `testudo-journal/src/components/journal/DatabaseTable.tsx` — **new** — table view component
- `testudo-journal/src/pages/Journal.tsx` — add view toggle, state management
- `testudo-journal/src/api/client.ts` — update `fetchEntries` with sort/filter params
- `testudo-journal/src/components/journal/JournalTimeline.tsx` — keep as card view option

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Journal page defaults to table view with all 7 columns visible
- [ ] Clicking column headers sorts entries (visual indicator for active sort)
- [ ] Type, asset, and tag columns are filterable via dropdowns
- [ ] Clicking a row opens EntryEditor in edit mode
- [ ] View toggle switches between table and card layout
- [ ] Pagination controls show page/total and allow navigation
- [ ] Bulk selection works with shift-click for ranges
- [ ] Table maintains zero-radius, monochrome-first aesthetic
- [ ] Responsive: table scrolls horizontally on mobile, or collapses to card view
- [ ] `cd testudo-journal && bun run build` passes

---

## Risks

1. **Backend sort/filter support** — If the journal API doesn't support server-side sorting, we must either add it or keep client-side sorting with larger fetches. Mitigation: check backend first; client-side sorting on <500 entries is acceptable as interim.
2. **Trade symbol resolution** — Entries link to trades by `trade_id`, but the table needs the symbol. Requires either a join on the backend or a client-side lookup cache. Mitigation: `JournalTimeline` already has a `tradeDetailCache` pattern that can be reused.
3. **Performance with images** — MarkdownPreview in the preview column would be expensive for 50+ rows. Mitigation: preview column shows plain text (first 80 chars), not rendered markdown.

---

## Completion Signal

This spec is complete when:
1. Table view renders with sortable, filterable columns
2. View toggle between table and cards works
3. Pagination functional
4. All acceptance criteria met
5. `bun run build` passes
6. Code committed to master
