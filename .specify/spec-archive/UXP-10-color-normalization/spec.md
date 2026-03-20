# Specification: Color Token Normalization — Purge signal-green from non-financial uses

**Spec ID:** UXP-10-color-normalization
**Date:** 2026-03-18
**Status:** Draft
**Class:** Refactor / Theming
**Priority:** P0 — Affects visual identity across entire journal
**Depends on:** UXP-09-filter-bar-redesign
**Series:** UXP-10 through UXP-12 (Audit remediation from journal UI audit)

---

## Problem Statement

`signal-green` (#00FF41) is used for 15+ distinct purposes across 12 files: positive P&L, LONG side, active sort columns, active pagination, focus rings, EDIT/PREVIEW tabs, tag hover, equity curve line, markdown links, drag-drop borders, and form focus states. When one color means everything, it means nothing. Users cannot distinguish interactive state feedback from financial data.

The global `:focus-visible` ring in `app.css` also uses neon green, making every focused element glow the same color as positive P&L. Per-component focus ring classes are copy-pasted across 12+ files using inconsistent patterns (some use `border-active`, some use `signal-green`).

Additionally, `app.css` hardcodes raw hex values for markdown preview styles and selection colors instead of using design tokens.

---

## Design Principle

**`signal-green` is reserved for money.** It means: positive P&L, profit, winning. Nothing else.

All UI state indicators (active tab, active sort, active page, focus, hover) use `text-text-primary` / `border-active` (white).

---

## Functional Requirements

| ID | Requirement | Files |
|----|-------------|-------|
| FR-1 | Global `:focus-visible` ring in `app.css` uses `rgba(255, 255, 255, 0.4)` instead of `rgba(0, 255, 65, 0.4)` | `app.css` |
| FR-2 | Markdown link color changed from `#00FF41` to `#94A3B8` (accent-steel) | `app.css` |
| FR-3 | Extract shared focus utility `.focus-ring` using `@apply` with `border-active` + white ring. Replace all 12+ per-component focus ring class strings with this single utility | `app.css`, all components with `focus-visible:ring-signal-green` |
| FR-4 | Active sort column header uses `text-text-primary` not `text-signal-green` | `TradeTable.tsx` |
| FR-5 | Active pagination page uses `text-text-primary border-b border-text-primary` not `signal-green` | `Pagination.tsx` |
| FR-6 | EDIT/PREVIEW tab active state uses `text-text-primary` not `text-signal-green` | `EntryEditor.tsx` |
| FR-7 | Tag "Add" hover uses `text-text-primary` not `hover:text-signal-green` | `TagSelector.tsx`, `TradeDetail.tsx` |
| FR-8 | Drag-drop active border uses `border-text-primary` not `border-signal-green` | `EntryEditor.tsx` |
| FR-9 | TagManager "Save" text uses `text-text-primary` not `text-signal-green` | `TagManager.tsx` |
| FR-10 | TradeFilters LONG button active state uses `border-text-primary text-text-primary` not `signal-green` (SHORT keeps `signal-red`) | `TradeFilters.tsx` |
| FR-11 | Markdown preview hardcoded hex values replaced with CSS custom properties referencing design tokens | `app.css` |
| FR-12 | `::selection` colors use token values via CSS custom properties | `app.css` |
| FR-13 | Entry type color for "post-trade" changed from `#00FF41` to `#22C55E` (softer green that isn't the signal color) | `EntryEditor.tsx` |

---

## Signal-green Audit — Keep vs Remove

| Usage | File | Action |
|-------|------|--------|
| Positive P&L color | `formatters.ts:26` | **KEEP** — financial data |
| R-multiple >= 1 | `formatters.ts:70` | **KEEP** — financial data |
| LONG side color | `formatters.ts:76` | **KEEP** — financial data |
| Best streak | `Overview.tsx:49` | **KEEP** — financial data |
| Trade P&L in TradeSelector | `TradeSelector.tsx:75` | **KEEP** — financial data |
| Trade side in TradeSelector | `TradeSelector.tsx:69` | **KEEP** — financial data |
| Equity curve line | `HeroEquityCurve.tsx:40` | **KEEP** — chart data color |
| Active sort column | `TradeTable.tsx:80` | **REMOVE** → `text-text-primary` |
| Active pagination | `Pagination.tsx:44` | **REMOVE** → `text-text-primary` |
| EDIT/PREVIEW tabs | `EntryEditor.tsx:300,310` | **REMOVE** → `text-text-primary` |
| Tag add hover | `TagSelector.tsx:29` | **REMOVE** → `text-text-primary` |
| Tag add hover | `TradeDetail.tsx:223` | **REMOVE** → `text-text-primary` |
| TagManager save | `TagManager.tsx:144` | **REMOVE** → `text-text-primary` |
| LONG filter active | `TradeFilters.tsx:30` | **REMOVE** → `text-text-primary` |
| Drag-drop border | `EntryEditor.tsx:346` | **REMOVE** → `border-text-primary` |
| Focus rings (12+ files) | Multiple | **REMOVE** → `.focus-ring` utility |
| Markdown links | `app.css:42` | **REMOVE** → `#94A3B8` |
| Global focus-visible | `app.css:85` | **REMOVE** → white ring |
| Post-trade entry type | `EntryEditor.tsx:23` | **REMOVE** → `#22C55E` |

---

## Technical Implementation

### Focus Ring Utility (FR-3)

Add to `app.css`:

```css
/* Shared focus ring — replaces per-component focus-visible classes */
.focus-ring {
  @apply focus-visible:border-border-active focus-visible:outline-none
         focus-visible:ring-2 focus-visible:ring-white/20
         focus-visible:ring-offset-1 focus-visible:ring-offset-main-bg;
}
```

Then replace all instances of the long `focus-visible:border-... focus-visible:ring-signal-green/30 ...` class string with just `focus-ring`.

### Files Changed

| File | Change |
|------|--------|
| `src/styles/app.css` | FR-1, FR-2, FR-3, FR-11, FR-12 |
| `src/components/trades/TradeTable.tsx` | FR-4 |
| `src/components/trades/Pagination.tsx` | FR-5 |
| `src/components/trades/TradeFilters.tsx` | FR-10 |
| `src/components/trades/TradeDetail.tsx` | FR-3, FR-7 |
| `src/components/journal/EntryEditor.tsx` | FR-3, FR-6, FR-8, FR-13 |
| `src/components/journal/TagSelector.tsx` | FR-7 |
| `src/components/journal/TagManager.tsx` | FR-3, FR-9 |
| `src/components/journal/JournalTimeline.tsx` | FR-3 |
| `src/components/journal/TradeSelector.tsx` | FR-3 |
| `src/components/FilterBar.tsx` | FR-3 |
| `src/components/SymbolSearch.tsx` | FR-3 |
| `src/components/ChartSelector.tsx` | FR-3 |

---

## Acceptance Criteria

- [ ] `signal-green` grep returns ONLY: `formatters.ts` (pnlColor, rColor, sideColor), `Overview.tsx` (best streak), `TradeSelector.tsx` (trade side/pnl), `HeroEquityCurve.tsx` (chart line)
- [ ] No component has inline `focus-visible:ring-signal-green` — all use `.focus-ring` utility
- [ ] Global `:focus-visible` uses white ring, not green
- [ ] Active sort column, pagination page, EDIT/PREVIEW tabs all use `text-text-primary`
- [ ] Markdown links use `#94A3B8`
- [ ] `bun run build` passes

---

## Completion Signal

This spec is complete when:
1. `signal-green` is used only for financial data
2. All UI state indicators use neutral white
3. Focus ring is extracted to utility class
4. Build passes
5. Code committed to master
