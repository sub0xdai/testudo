# Specification: Nested Collections for Hierarchical Journal Organization

**Spec ID:** JNL-17-nested-collections
**Date:** 2026-03-22
**Status:** Draft
**Class:** Feature / Data Model
**Priority:** P2 — Requires database view (JNL-16) to be useful
**Depends on:** JNL-16-database-view
**Series:** JNL-14 through JNL-18 (Journal audit remediation + database redesign)

---

## Problem Statement

The current journal has flat organization: all entries live in one list, filterable by type, tag, and date. Tags exist but are treated as flat labels, not organizational units. A trader with 200+ entries across 15 assets and 6 months has no way to create structured views like:

- "BTC Journal" — all entries linked to BTC trades
- "March 2026" — all entries from a specific period
- "Losing Trades" — entries tagged with specific analysis tags
- "BTC Journal → Post-Trade Reviews" — a sub-view within a collection

The concept is **saved database views** (collections) that can be nested. A collection is a named filter preset with its own sort/column configuration. Collections can contain sub-collections, creating a hierarchy like:

```
My Journal
├── By Asset
│   ├── BTC                  (filter: asset = BTC)
│   ├── ETH                  (filter: asset = ETH)
│   └── SOL                  (filter: asset = SOL)
├── Reviews
│   ├── Daily Reviews        (filter: type = daily-review)
│   └── Weekly Reviews       (filter: type = weekly-review)
└── By Tag
    ├── Breakout Trades      (filter: tag = "breakout")
    └── Revenge Trades       (filter: tag = "revenge")
```

Each collection is a database view — same underlying entries, different filter/sort/group configuration. Nesting is organizational, not data duplication.

---

## User Stories

- **As a trader**, I want to save filter configurations as named collections, so that I can quickly access "all BTC post-trade reviews" without re-creating filters every time.
- **As a trader**, I want to nest collections hierarchically, so that I can organize my journal by asset, then by entry type within each asset.
- **As a user**, I want a sidebar showing my collection tree, so that I can navigate my journal like a file system.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Users can create named collections with saved filter/sort configuration | High | Collections API + UI |
| FR-2 | Collections can be nested (parent-child relationship, max 3 levels deep) | High | Data model |
| FR-3 | Sidebar navigation shows collection tree with expand/collapse | High | Layout |
| FR-4 | "All Entries" is the root view (no filters applied) | High | Navigation |
| FR-5 | Collections are stored server-side per user | High | Backend API |
| FR-6 | Collections can be renamed, reordered, and deleted | Medium | UI |
| FR-7 | Clicking a collection loads the database table with that collection's saved filters | High | Journal page |
| FR-8 | "Save as Collection" button creates a collection from current active filters | Medium | Table toolbar |
| FR-9 | Auto-generated smart collections: one per asset with entries, one per entry type | Low | Backend or frontend |
| FR-10 | Collection entry counts shown in sidebar (badge) | Low | Sidebar |

---

## Technical Implementation

### Data Model

```typescript
// New type in api/client.ts
export interface JournalCollection {
  id: string
  user_id: string
  parent_id: string | null       // null = root level
  name: string
  icon?: string                   // optional emoji or icon key
  sort_order: number              // position within siblings
  filters: CollectionFilters      // saved filter state
  sort_by: string                 // saved sort column
  sort_dir: 'asc' | 'desc'
  created_at: string
  updated_at: string
}

export interface CollectionFilters {
  entry_type?: string             // e.g., "post-trade"
  tag_ids?: string[]              // filter by tag
  symbol?: string                 // filter by linked trade asset
  date_from?: string
  date_to?: string
}
```

### Backend API Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/journal/collections` | GET | List all user collections (flat, client builds tree) |
| `/journal/collections` | POST | Create collection |
| `/journal/collections/:id` | PUT | Update collection (name, filters, sort, parent_id) |
| `/journal/collections/:id` | DELETE | Delete collection (children become root-level) |
| `/journal/collections/reorder` | PUT | Batch update sort_order for siblings |

[CLARIFY] Does the backend need a new `journal_collections` table, or can collections be stored as a JSON blob in user preferences? A dedicated table is cleaner for querying entry counts per collection.

### Sidebar Component

```tsx
// src/components/journal/CollectionSidebar.tsx

function CollectionSidebar(props: {
  collections: JournalCollection[]
  activeId: string | null
  onSelect: (collection: JournalCollection | null) => void
  onCreate: () => void
}) {
  const tree = buildTree(props.collections)

  return (
    <nav class="w-56 border-r border-container-border flex-shrink-0 overflow-y-auto">
      <button onClick={() => props.onSelect(null)}>All Entries</button>
      <For each={tree}>
        {(node) => <CollectionNode node={node} depth={0} ... />}
      </For>
      <button onClick={props.onCreate}>+ New Collection</button>
    </nav>
  )
}
```

### Tree Building

```typescript
interface TreeNode {
  collection: JournalCollection
  children: TreeNode[]
}

function buildTree(collections: JournalCollection[]): TreeNode[] {
  const map = new Map<string, TreeNode>()
  const roots: TreeNode[] = []

  // Sort by sort_order within each parent group
  const sorted = [...collections].sort((a, b) => a.sort_order - b.sort_order)

  for (const c of sorted) {
    map.set(c.id, { collection: c, children: [] })
  }

  for (const c of sorted) {
    const node = map.get(c.id)!
    if (c.parent_id && map.has(c.parent_id)) {
      map.get(c.parent_id)!.children.push(node)
    } else {
      roots.push(node)
    }
  }

  return roots
}
```

### Journal Page Layout Change

```
Before:
┌─────────────────────────────┐
│ [Filters]  [+ New Entry]    │
│ ─────────────────────────── │
│ Entry cards / timeline      │
└─────────────────────────────┘

After:
┌───────────┬─────────────────┐
│ Sidebar   │ Database Table  │
│           │                 │
│ All       │ [Sort] [Filter] │
│ ├ BTC     │ ─────────────── │
│ ├ ETH     │ row row row     │
│ └ Reviews │ row row row     │
│   ├ Daily │                 │
│   └ Weekly│                 │
│           │                 │
│ + New     │                 │
└───────────┴─────────────────┘
```

### Files

- `testudo-journal/src/components/journal/CollectionSidebar.tsx` — **new** — collection tree navigation
- `testudo-journal/src/pages/Journal.tsx` — add sidebar layout, collection state
- `testudo-journal/src/api/client.ts` — add collection CRUD functions and types
- Backend: new `journal_collections` table + REST endpoints (separate backend spec if needed)

### Dependencies Added

None (frontend). Backend requires a migration for the collections table.

---

## Acceptance Criteria

- [ ] Users can create, rename, and delete collections
- [ ] Collections can be nested up to 3 levels deep
- [ ] Sidebar shows collection tree with expand/collapse
- [ ] Selecting a collection loads its saved filters into the database table
- [ ] "All Entries" shows unfiltered view
- [ ] "Save as Collection" captures current filter/sort state
- [ ] Deleting a parent moves children to root level (no orphans)
- [ ] Collections persist across sessions (server-side storage)
- [ ] Sidebar is collapsible on mobile
- [ ] `cd testudo-journal && bun run build` passes

---

## Risks

1. **Backend dependency** — This spec requires a new database table and REST endpoints. Mitigation: frontend can be built first with localStorage-based collections as a prototype; migrate to server-side when backend is ready.
2. **Complexity creep** — Nested collections can become arbitrarily deep. Mitigation: hard limit at 3 levels; UI enforces this by hiding "Add sub-collection" at depth 3.
3. **Entry count queries** — Showing counts per collection requires running each collection's filter query. Mitigation: lazy-load counts on sidebar expand, cache for 60 seconds.

---

## Completion Signal

This spec is complete when:
1. Collection CRUD works (create, rename, nest, delete)
2. Sidebar navigation renders collection tree
3. Selecting a collection applies its filters to the database view
4. All acceptance criteria met
5. `bun run build` passes
6. Code committed to master
