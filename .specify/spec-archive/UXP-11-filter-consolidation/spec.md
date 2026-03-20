# Specification: Filter Consolidation & Dead Code Cleanup

**Spec ID:** UXP-11-filter-consolidation
**Date:** 2026-03-18
**Status:** Draft
**Class:** Refactor / UX
**Priority:** P1 — Removes user confusion from duplicate filters
**Depends on:** UXP-10-color-normalization
**Series:** UXP-10 through UXP-12 (Audit remediation from journal UI audit)

---

## Problem Statement

The Trades page has two overlapping filter systems:

1. **FilterBar** (global, top of page) — data-driven exchange/symbol dropdowns, time presets, immediate-apply
2. **TradeFilters** (local, Trades page only) — hardcoded exchange dropdown (uppercase values), free-text symbol, LONG/SHORT toggle, tag filter

These conflict: TradeFilters uses uppercase exchange values (`WOO`, `BINANCE`) while FilterBar uses lowercase (`woo`, `binance`). The `TradeTable` merges them with `localFilters().exchange || filters().exchange`, meaning the local filter silently overrides the global one. Users see two exchange dropdowns on the same page.

Additionally, `GhostAnnotation` (`// ACCOUNT_OVERVIEW`, `// CHART_SUITE`) adds decorative noise that provides no information beyond what the page heading already says. And `TradeRow` never receives tags because `TradeTable` doesn't pass them.

---

## Functional Requirements

| ID | Requirement | Priority | Files |
|----|-------------|----------|-------|
| FR-1 | Remove exchange dropdown and symbol text input from `TradeFilters` — these are handled by the global `FilterBar` | High | `TradeFilters.tsx` |
| FR-2 | `TradeFilters` retains only: LONG/SHORT toggle and tag filter | High | `TradeFilters.tsx` |
| FR-3 | `TradeTable` no longer merges local exchange/symbol with global — uses `filters().exchange` and `filters().symbol` directly | High | `TradeTable.tsx` |
| FR-4 | `TradeFilterState` interface drops `exchange` and `symbol` fields | High | `TradeFilters.tsx` |
| FR-5 | Remove `GhostAnnotation` component and all usages | Medium | `GhostAnnotation.tsx`, `Overview.tsx`, `Charts.tsx` |
| FR-6 | `TradeTable` fetches trade tags and passes them to `TradeRow` for display | Medium | `TradeTable.tsx`, API |

---

## Technical Implementation

### FR-1/FR-2/FR-4: Simplify TradeFilters

```tsx
export interface TradeFilterState {
  side?: string
  tag?: string
}

export function TradeFilters(props: {
  filters: TradeFilterState
  onChange: (f: TradeFilterState) => void
}) {
  // Only LONG/SHORT toggle and tag input remain
}
```

### FR-3: TradeTable params simplification

```tsx
const params = (): TradeListParams => ({
  page: page(),
  limit: 50,
  sort: sort().field,
  order: sort().order,
  exchange: filters().exchange,  // global only, no local override
  symbol: filters().symbol,      // global only, no local override
  side: localFilters().side,
  tag: localFilters().tag,
  dateFrom: filters().dateFrom,
  dateTo: filters().dateTo,
})
```

### FR-5: Remove GhostAnnotation

Delete `src/components/GhostAnnotation.tsx`. Remove imports and `<GhostAnnotation>` from `Overview.tsx` and `Charts.tsx`.

### FR-6: TradeRow tags

The backend already returns tags in the trade list via joins. If the current `list_trades` endpoint doesn't include tags, add a subquery or join. If it does, map them through to `TradeRow`.

Check current behavior: `TradeRow` accepts `tags?: JournalTag[]` prop but `TradeTable` never passes it. If the API doesn't return tags in the list response, skip this FR (it would require a backend change and is lower priority).

### Files Changed

| File | Change |
|------|--------|
| `src/components/trades/TradeFilters.tsx` | FR-1, FR-2, FR-4: Remove exchange dropdown, symbol input. Keep side toggle + tag filter |
| `src/components/trades/TradeTable.tsx` | FR-3: Remove local exchange/symbol override. Use global filter directly |
| `src/components/GhostAnnotation.tsx` | FR-5: Delete file |
| `src/components/Overview.tsx` | FR-5: Remove GhostAnnotation import and usage |
| `src/components/Charts.tsx` | FR-5: Remove GhostAnnotation import and usage |

---

## Acceptance Criteria

- [ ] Trades page shows one exchange dropdown (in FilterBar), not two
- [ ] Trades page shows one symbol filter (in FilterBar), not two
- [ ] LONG/SHORT toggle and tag filter still work on Trades page
- [ ] `TradeFilterState` has only `side` and `tag` fields
- [ ] No `GhostAnnotation` references in codebase
- [ ] `GhostAnnotation.tsx` file deleted
- [ ] `bun run build` passes

---

## Completion Signal

This spec is complete when:
1. Duplicate filters removed
2. GhostAnnotation removed
3. Build passes
4. Code committed to master
