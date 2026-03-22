# Implementation Plan

> Last updated: 2026-03-22
> Current spec: JNL-16-database-view
> Phase: BUILD

---

## Active Spec: JNL-16-database-view

Transform journal from timeline to sortable, filterable database table view.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Create DatabaseTable.tsx — sortable columns, client-side pagination, bulk selection, row click, type badges | complete | high | — |
| T2 | Integrate DatabaseTable into JournalTimeline — view toggle, sort/page state, conditional render | complete | medium | T1 |
| T3 | Build validation + commit | complete | low | T2 |

### Key Decisions

- **Client-side sorting + pagination**: Backend `fetchEntries` only supports `page`/`limit`/`tradeId`. No server-side sort/filter. Risk #1 mitigation: client-side on 200 entries is acceptable.
- **DatabaseTable as pure display component**: Receives entries, tag/trade-label accessors, and callbacks from JournalTimeline. Sort/page state lives inside DatabaseTable.
- **View toggle in JournalTimeline**: Keep all data fetching, filtering, modals in JournalTimeline. Swap display layer (cards vs table) based on viewMode signal. Minimal refactor.
- **Shift-click bulk selection**: Track lastClickedIndex for range selection. Selected IDs exposed for future bulk actions (delete, export, tag).
- **Preview column = plain text**: Strip markdown, show first 80 chars. No MarkdownPreview in table rows (performance).

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

---

*This file is persistent state. Vox updates it each iteration.*
