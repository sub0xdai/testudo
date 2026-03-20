# Specification: Journal Notes + Tagging + Reflection

**Spec ID:** JNL-10-journal-notes
**Date:** 2026-03-17
**Status:** Draft
**Class:** Feature / Frontend
**Priority:** P1 — the qualitative journal layer
**Depends on:** JNL-05-journal-api, JNL-07-dashboard-layout
**Series:** Batch 5 — Frontend Journal (JNL-09, JNL-10)

---

## Problem Statement

A trade log without reflection is just data. The journal page provides a markdown-based note-taking system where traders write pre-trade plans, post-trade reviews, daily reflections, and weekly summaries. Notes are linked to specific trades or dates, tagged for categorization, and displayed in a timeline view.

---

## User Stories

- **As a trader**, I want to write markdown notes linked to specific trades to capture my reasoning.
- **As a trader**, I want to write daily/weekly reflections to review my patterns.
- **As a trader**, I want to tag and filter my journal entries to find patterns in my behavior.
- **As a trader**, I want a clean, Obsidian-like writing experience in the brutalist dark theme.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Timeline view of all journal entries (newest first) | High | Journal page |
| FR-2 | Create new entry: select type (note, pre-trade, post-trade, daily, weekly) | High | entry editor |
| FR-3 | Markdown editor with live preview | High | entry editor |
| FR-4 | Link entry to a trade (optional trade selector) | High | entry editor |
| FR-5 | Link entry to a date (for daily/weekly reflections) | High | entry editor |
| FR-6 | Tag management: create, rename, recolor, delete tags | High | tag manager |
| FR-7 | Filter entries by: type, trade, date range, tags | High | Journal page |
| FR-8 | Edit and delete existing entries | High | entry editor |

---

## Technical Implementation

### Journal Page Layout

```
┌─────────────────────────────────────────────────────────────┐
│  JOURNAL                              [+ New Entry]         │
│                                                             │
│  Filters: [All Types ▾] [All Tags ▾] [Date Range]          │
│                                                             │
│  ─── Mar 17, 2026 ────────────────────────────────────────  │
│                                                             │
│  ┌─ POST-TRADE ─────────────────────────────────────────┐  │
│  │  BTC Short Review              ● revenge  ● fomo     │  │
│  │  Linked: BTC_USDT SHORT Mar 17                       │  │
│  │                                                       │  │
│  │  ## What went wrong                                   │  │
│  │  Entered against the trend. Stop was too tight...     │  │
│  │                                                       │  │
│  │  12:34 PM                          [Edit] [Delete]    │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌─ DAILY ──────────────────────────────────────────────┐  │
│  │  End of Day Review                                    │  │
│  │                                                       │  │
│  │  Took 3 trades today. Two were impulsive...           │  │
│  │                                                       │  │
│  │  11:55 PM                          [Edit] [Delete]    │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ─── Mar 16, 2026 ────────────────────────────────────────  │
│  ...                                                        │
└─────────────────────────────────────────────────────────────┘
```

### Entry Editor

Full-screen or panel overlay with markdown editing:

```
┌─────────────────────────────────────────────────────────┐
│  NEW ENTRY                                    [Save]    │
│                                                         │
│  Type: [Post-Trade ▾]                                   │
│  Title: [BTC Short Review                    ]          │
│  Trade: [BTC_USDT SHORT Mar 17 (-$12.50)     ] ✕       │
│  Tags:  ● revenge  ● fomo  [+ Add Tag]                 │
│                                                         │
│  ┌─────────────────────┬───────────────────────────┐   │
│  │  ## What went wrong │ What went wrong            │   │
│  │                     │                             │   │
│  │  Entered against    │ Entered against the trend.  │   │
│  │  the trend. Stop    │ Stop was too tight for the  │   │
│  │  was too tight...   │ timeframe.                  │   │
│  │                     │                             │   │
│  │  EDIT               │ PREVIEW                     │   │
│  └─────────────────────┴───────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### Markdown Rendering

Use a lightweight markdown renderer (e.g., `marked` or `markdown-it`). Support:
- Headers (h1-h3)
- Bold, italic, strikethrough
- Bullet and numbered lists
- Code blocks (inline and fenced)
- Blockquotes
- Links

No images (keep it text-focused like Obsidian).

### Tag Manager

Accessible from a settings/gear icon on the journal page:

```
┌─ TAG MANAGER ───────────────────────┐
│                                     │
│  ● revenge-trade    #FF003C  [✎][✕] │
│  ● fomo             #FF003C  [✎][✕] │
│  ● clean-setup      #00FF41  [✎][✕] │
│  ● trend-follow     #00FF41  [✎][✕] │
│  ● counter-trend    #f59e0b  [✎][✕] │
│                                     │
│  [+ New Tag]                        │
└─────────────────────────────────────┘
```

### Component Structure

```
src/components/journal/
├── JournalTimeline.tsx      — date-grouped list of entries
├── EntryCard.tsx            — single entry in timeline
├── EntryEditor.tsx          — create/edit entry form
├── MarkdownPreview.tsx      — rendered markdown
├── TradeSelector.tsx        — search + select trade to link
├── TagManager.tsx           — CRUD for tags
├── TagSelector.tsx          — add tags to entry/trade
└── EntryTypeFilter.tsx      — filter by entry type
```

### Entry Type Styling

| Type | Badge Color | Label |
|------|-------------|-------|
| note | `#94a3b8` (steel) | NOTE |
| pre-trade | `#f59e0b` (orange) | PRE-TRADE |
| post-trade | `#00FF41` (green) | POST-TRADE |
| daily | `#888888` (secondary) | DAILY |
| weekly | `#888888` (secondary) | WEEKLY |

### Files

- `testudo-journal/src/components/journal/` — all journal components
- `testudo-journal/src/pages/Journal.tsx` — journal page

---

## Acceptance Criteria

- [ ] Timeline displays entries grouped by date, newest first
- [ ] New entry form with type selector, title, markdown body, trade link, tags
- [ ] Markdown renders correctly in preview pane
- [ ] Edit and delete existing entries
- [ ] Tag CRUD works (create, rename, recolor, delete)
- [ ] Entries filterable by type, tags, date range
- [ ] Trade selector shows searchable list of recent trades
- [ ] Entry type badges use correct colors
- [ ] All text styled with Space Grotesk/Space Mono
- [ ] `bun run build` passes

---

## Completion Signal

This spec is complete when:
1. Traders can create, edit, and browse journal entries
2. Entries link to trades and display in timeline
3. Tag system fully functional
4. All acceptance criteria met
5. Code committed to master
