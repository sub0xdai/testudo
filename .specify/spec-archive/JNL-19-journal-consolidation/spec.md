# Specification: Journal Consolidation — Trades + Notes Unified

**Spec ID:** JNL-19-journal-consolidation
**Date:** 2026-03-27
**Status:** Draft
**Class:** Feature / UX Refactor
**Priority:** P1 — The current separation between TRADES (table + detail sidebar) and JOURNAL (timeline + entry editor) splits what should be one experience. The trades table with its slide-out detail panel is exactly how the journal should work — notes and tags per trade, inline.
**Depends on:** None
**Series:** JNL-19 (standalone)

---

## Problem Statement

The Desk has two separate tabs doing overlapping work. TRADES shows the trade history table with a slide-out detail sidebar (entry/exit, P&L, tags, notes). JOURNAL shows a separate timeline of markdown entries with a modal editor. Users should not have to switch between two pages to journal about their trades.

The TRADES page already has the right UX — click a trade row, see details in the sidebar, add tags and notes inline. What's missing is: (1) the notes field is plain text, not markdown, (2) there's no way to create new tags from the sidebar (only select existing ones), and (3) there's no export.

Additionally, the old JOURNAL page's TagManager (create/edit/delete tags) has no equivalent in the trades sidebar.

---

## User Stories

- **As a trader**, I want one tab called JOURNAL that shows my trades with inline notes, so that I don't switch between two pages.
- **As a trader**, I want to write markdown notes in the trade detail sidebar, so that I can format my reflections.
- **As a trader**, I want to create new tags directly from the trade sidebar, so that I don't need a separate tag management page.
- **As a trader**, I want to export a trade's notes as `.md`, so that I can keep local copies.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Rename TRADES → JOURNAL in the nav bar. Update `NAV_ITEMS` in `Layout.tsx`. | High | Navigation |
| FR-2 | Remove the JOURNAL nav item. The `/journal` route can remain but should redirect to `/trades` (or be removed entirely). | High | Navigation |
| FR-3 | Upgrade the NOTES section in `TradeDetail.tsx` sidebar from plain `<textarea>` to a markdown editor with preview toggle. Use the existing `MarkdownPreview` component from `components/journal/`. | High | TradeDetail |
| FR-4 | Add "Export .md" button to the trade detail sidebar. Clicking it downloads the trade's notes as `{symbol}_{date}.md` with trade metadata as YAML frontmatter. | Medium | TradeDetail |
| FR-5 | Add inline tag creation to the trade detail sidebar. When the tag picker shows "No more tags" or the user wants a new one, provide a text input + color picker to create a tag on the spot (calls `createTag` API). | High | TradeDetail |
| FR-6 | Move TagManager functionality (rename, delete, color edit) into a small gear icon or "Manage Tags" link accessible from the tag picker in the sidebar. | Medium | TradeDetail |
| FR-7 | The `/trades` route path stays as-is (URL doesn't need to change). Only the nav label changes. | Low | Navigation |

---

## Technical Implementation

### FR-1 + FR-2: Nav Rename

```typescript
// Layout.tsx
const NAV_ITEMS = [
  { path: '/', label: 'OVERVIEW' },
  { path: '/trades', label: 'JOURNAL' },
  { path: '/account', label: 'ACCOUNT' },
]
```

### FR-3: Markdown Notes

The `MarkdownPreview` component already exists at `components/journal/MarkdownPreview.tsx`. The sidebar's NOTES section currently uses a plain `<textarea>`. Replace with:
- Edit mode: `<textarea>` with monospace font (as-is)
- Preview mode: `<MarkdownPreview body={notes()} />`
- Toggle button: "EDIT" / "PREVIEW" — small tab-style toggle above the textarea

### FR-4: Export .md

```typescript
function exportTradeNotes(trade: TradeDetailType) {
  const frontmatter = [
    '---',
    `symbol: ${trade.symbol}`,
    `side: ${trade.side}`,
    `entry: ${trade.entry_price}`,
    `exit: ${trade.exit_price}`,
    `pnl: ${trade.net_pnl}`,
    `date: ${trade.closed_at}`,
    '---',
  ].join('\n')
  const content = `${frontmatter}\n\n${trade.notes || ''}`
  const blob = new Blob([content], { type: 'text/markdown' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `${trade.symbol}_${trade.closed_at.slice(0, 10)}.md`
  a.click()
  URL.revokeObjectURL(url)
}
```

### FR-5: Inline Tag Creation

Add a text input below the tag picker grid. When user types a name and hits Enter, call `createTag({ name, color })` then `addTradeTags(tradeId, [newTag.id])`. Color can default to a cycle of preset colors or a simple color picker.

### Files

**Modified:**
- `src/components/Layout.tsx` — Nav items
- `src/components/trades/TradeDetail.tsx` — Markdown notes, export, inline tag creation
- `src/App.tsx` (or router config) — Remove/redirect `/journal` route

**Reused:**
- `src/components/journal/MarkdownPreview.tsx` — Already exists

**Potentially removed (later):**
- `src/pages/Journal.tsx` — Becomes dead code
- `src/components/journal/JournalTimeline.tsx` — No longer needed
- `src/components/journal/EntryEditor.tsx` — No longer needed
- `src/components/journal/EntryCard.tsx` — No longer needed
- `src/components/journal/CollectionSidebar.tsx` — No longer needed
- `src/components/journal/DatabaseTable.tsx` — No longer needed

Note: Don't delete the journal page code yet — it can stay as a hidden route for users who have existing journal entries. Remove it in a follow-up cleanup.

---

## Acceptance Criteria

- [ ] Nav shows OVERVIEW, JOURNAL, ACCOUNT (3 tabs, not 4)
- [ ] Clicking JOURNAL goes to the trades table
- [ ] Trade detail sidebar shows markdown-rendered notes with edit/preview toggle
- [ ] "Export .md" downloads trade notes with frontmatter
- [ ] Tags can be created inline from the trade detail sidebar
- [ ] Existing tags can still be selected and removed
- [ ] `bun run build` passes

---

## Risks

1. **Existing journal entries orphaned** — Users who created entries via the old Journal page still have data. Mitigation: Keep the old journal entries accessible via API; don't delete the database tables. Add a "linked entries" section to the trade detail sidebar later if needed.
2. **Markdown preview rendering** — MarkdownPreview may need CSS adjustments for the narrow sidebar context. Mitigation: Test with real notes content.

---

## Completion Signal

This spec is complete when:
1. The nav has 3 tabs: OVERVIEW, JOURNAL, ACCOUNT
2. Trade detail sidebar supports markdown notes with preview
3. Tags can be created inline
4. Export works
5. `bun run build` passes
6. Code committed to master
