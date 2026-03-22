# Implementation Plan

> Last updated: 2026-03-22
> Current spec: JNL-17-nested-collections
> Phase: BUILD

---

## Active Spec: JNL-17-nested-collections

Nested collections for hierarchical journal organization — saved database views with tree navigation.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Collection data model + localStorage persistence — types, CRUD helpers, tree builder in `lib/collections.ts` | complete | medium | — |
| T2 | CollectionSidebar component — tree navigation with expand/collapse, rename, delete, create sub-collection | complete | high | T1 |
| T3 | Journal layout integration — sidebar + content flex layout in JournalTimeline, collection→filter bridge, "Save as Collection" button | complete | high | T1, T2 |
| T4 | Build validation + commit | complete | low | T3 |

### Key Decisions

- **localStorage prototype**: No backend collection endpoints exist. Use localStorage for persistence (spec Risk #1 mitigation). Data model matches spec's `JournalCollection` interface for future backend migration.
- **Collection state in JournalTimeline**: JournalTimeline already owns all filter state, data fetching, and modals. Adding collection state here avoids lifting 10+ props to Journal.tsx. Sidebar renders as a sibling inside JournalTimeline's flex container.
- **Flat storage, client-side tree**: Collections stored as flat array in localStorage. `buildTree()` constructs hierarchy from `parent_id` references. Max 3 levels enforced in UI (hide "Add sub-collection" at depth 3).
- **Filter bridge**: Selecting a collection applies its saved `filters` to JournalTimeline's existing `typeFilter`, `tagFilter`, `dateFrom`, `dateTo` signals. "All Entries" clears all filters.
- **No backend changes**: Pure frontend feature. When backend adds `/journal/collections` endpoints, swap localStorage calls for API calls in `lib/collections.ts`.

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| HL-11-status-transition-fix | 2026-03-21 |
| UXP-18-multi-theme | 2026-03-21 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |
| EXT-38-background-decomposition | 2026-03-22 |
| UXP-19-features-layout | 2026-03-22 |
| UXP-22-signal-color-calibration | 2026-03-22 |
| UXP-20-strip-glassmorphism | 2026-03-22 |
| UXP-23-landing-typography | 2026-03-22 |
| UXP-21-light-theme-parity | 2026-03-22 |
| JNL-14-markdown-hardening | 2026-03-22 |
| JNL-15-export-with-images | 2026-03-22 |
| JNL-16-database-view | 2026-03-22 |

---

*This file is persistent state. Vox updates it each iteration.*
