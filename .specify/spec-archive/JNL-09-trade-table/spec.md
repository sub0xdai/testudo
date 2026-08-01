# Specification: Trade History Table

**Spec ID:** JNL-09-trade-table
**Date:** 2026-03-17
**Status:** Draft
**Class:** Feature / Frontend
**Priority:** P1 — core data browsing
**Depends on:** JNL-05-journal-api, JNL-07-dashboard-layout
**Series:** Batch 5 — Frontend Journal (JNL-09, JNL-10)

---

## Problem Statement

Traders need to browse, search, and filter their trade history. The trade table is the primary data exploration interface — it shows every closed trade with key metrics and provides drill-down to individual trade detail views.

---

## User Stories

- **As a trader**, I want to see all my trades in a sortable table so I can review my history.
- **As a trader**, I want to filter trades by symbol, exchange, side, tags, and date range.
- **As a trader**, I want to click a trade to see its full details, linked notes, and tags.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Paginated trade table with sortable columns | High | TradeTable.tsx |
| FR-2 | Column set: date, symbol, exchange, side, entry, exit, P&L, R, duration, tags | High | TradeTable.tsx |
| FR-3 | Inline filters: side toggle, symbol search, exchange dropdown | High | TradeTable.tsx |
| FR-4 | Click row → trade detail panel/modal | High | TradeDetail.tsx |
| FR-5 | Trade detail shows: all trade fields, linked journal entries, tags | High | TradeDetail.tsx |
| FR-6 | Add/remove tags from trade detail view | Medium | TradeDetail.tsx |
| FR-7 | Quick note editing from trade detail | Medium | TradeDetail.tsx |

---

## Technical Implementation

### Table Layout

```
┌──────────┬────────┬──────┬──────┬─────────┬─────────┬──────────┬──────┬──────────┬──────────┐
│ Date     │ Symbol │ Exch │ Side │ Entry   │ Exit    │ Net P&L  │ R    │ Duration │ Tags     │
├──────────┼────────┼──────┼──────┼─────────┼─────────┼──────────┼──────┼──────────┼──────────┤
│ Mar 17   │ BTC    │ WOO  │ LONG │ 83,412  │ 84,200  │ +$45.30  │ 2.1R │ 4h 23m  │ ● trend  │
│ Mar 16   │ ETH    │ HL   │ SHRT │ 3,850   │ 3,920   │ -$12.50  │-0.5R │ 1h 12m  │ ● fomo   │
└──────────┴────────┴──────┴──────┴─────────┴─────────┴──────────┴──────┴──────────┴──────────┘
                                                           Page 1 of 5  ← 1 2 3 4 5 →
```

- Side column: green text for LONG, red for SHORT
- P&L column: green for positive, red for negative
- R column: green ≥ 1R, red < 0R, orange 0-1R
- Tags: colored dots with label
- Numbers in Space Mono, labels in Space Grotesk
- Row hover: `#111111` background
- Alternating row tint: subtle `rgba(255,255,255,0.02)`

### Trade Detail Panel

Slides in from right or opens as modal:

```
┌─────────────────────────────────────┐
│  BTC_USDT · LONG · WOO             │
│  Mar 17, 2026 · 4h 23m             │
│                                     │
│  Entry     83,412.00                │
│  Exit      84,200.00                │
│  Stop      82,900.00                │
│  Target    85,000.00                │
│  Quantity  0.0087                    │
│  Leverage  10x                      │
│                                     │
│  ─────────────────────────          │
│  Net P&L       +$45.30              │
│  R-Multiple    2.1R                 │
│  Fees          $1.23                │
│  Return        5.2%                 │
│                                     │
│  TAGS  [+ Add]                      │
│  ● trend-follow  ● clean-setup     │
│                                     │
│  NOTES                              │
│  Quick note: [editable field]       │
│                                     │
│  JOURNAL ENTRIES                    │
│  ▸ Post-trade review (Mar 17)      │
│  ▸ Weekly reflection (Mar 15)      │
└─────────────────────────────────────┘
```

### Component Structure

```
src/components/trades/
├── TradeTable.tsx           — paginated table with sorting
├── TradeRow.tsx             — single row component
├── TradeDetail.tsx          — detail panel/modal
├── TradeFilters.tsx         — inline filter controls
├── TagBadge.tsx             — colored tag pill
└── Pagination.tsx           — page controls
```

### Data Flow

```typescript
// TradeTable.tsx
const [page, setPage] = createSignal(1);
const [sort, setSort] = createSignal({ field: "closed_at", order: "desc" });

const [trades] = createResource(
  () => ({ ...filters(), page: page(), sort: sort(), limit: 50 }),
  (params) => fetchTrades(params)
);
```

### Files

- `testudo-journal/src/components/trades/` — all trade table components
- `testudo-journal/src/pages/Trades.tsx` — trades page

---

## Acceptance Criteria

- [ ] Table displays all trades with correct formatting
- [ ] Columns sortable by clicking header (toggle asc/desc)
- [ ] Pagination works (50 trades per page)
- [ ] Filters narrow results in real-time
- [ ] Trade detail shows all fields, tags, and linked entries
- [ ] Tags can be added/removed from detail view
- [ ] Quick note editable inline
- [ ] Green/red color coding for side and P&L
- [ ] All numbers in Space Mono
- [ ] `bun run build` passes

---

## Completion Signal

This spec is complete when:
1. Trade table renders with paginated, sortable, filterable data
2. Trade detail panel shows complete trade info
3. All acceptance criteria met
4. Code committed to master
