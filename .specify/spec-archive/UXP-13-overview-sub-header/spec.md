# Specification: Overview Sub-Header with Filter Popout

**Spec ID:** UXP-13-overview-sub-header
**Date:** 2026-03-18
**Status:** Draft
**Class:** Feature / UI
**Priority:** P0 — Primary navigation and filter UX is broken (active state ambiguity, magic number layout)
**Depends on:** None (first in series)
**Series:** UXP-13 through UXP-15 (Overview redesign + critique fixes)

---

## Problem Statement

The current global FilterBar is a persistent horizontal strip below the header that shows on Overview, Charts, and Trades pages. This creates several problems:

1. **Active state ambiguity**: Time preset buttons (1W, 1M, 3M, YTD, ALL, CUSTOM) use `text-text-primary` for active and `hover:text-text-primary` for hover — identical visual treatment. On touch devices, the selected time range is indistinguishable from unselected presets.

2. **Magic number layout**: The Overview sidebar uses `calc(100vh - var(--header-h) - 83px)` where `83px` is an unmaintained approximation of FilterBar height. When filters wrap on narrower screens, the sidebar clips.

3. **Wasted vertical space**: The FilterBar consumes ~50px of vertical real estate permanently, even when no filters are active. Trading dashboards (FXBlue, Myfxbook, TradingView) use compact sub-headers with popout filters to maximize chart/data area.

4. **No page context**: The FilterBar floats between header and content with no visual connection to the page it's filtering. Reference platforms show filters inline with the page title (e.g., "Overview" heading → exchange dropdown → filter button).

This spec replaces the global FilterBar with a page-level sub-header pattern: the page title with an inline exchange dropdown and a popout filter panel containing symbol search and date range controls.

---

## User Stories

- **As a trader**, I want the exchange selector immediately visible below the page title, so that I can see which exchange I'm viewing at a glance without scanning a separate filter bar.
- **As a trader**, I want to click a filter button to reveal symbol and date range controls, so that the main view is uncluttered when I'm not filtering.
- **As a trader**, I want the active time range to be visually obvious, so that I never confuse which period my stats reflect.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Remove `FilterBar` component from `Layout.tsx`. Each data page renders its own sub-header instead. | High | Layout |
| FR-2 | Create `PageSubHeader` component with: page title (h1), inline exchange dropdown, and a "Filter" toggle button. | High | Components |
| FR-3 | The "Filter" toggle button opens a `FilterPopout` panel (positioned below the button, not a modal) containing: SymbolSearch dropdown and date range controls (time presets + custom date inputs). | High | Components |
| FR-4 | `FilterPopout` closes on Escape, outside click, or re-clicking the toggle button. | High | Components |
| FR-5 | Time presets in the popout use a visually distinct active state: `bg-white/10 text-text-primary` for selected vs `text-text-tertiary` for inactive, with `focus-visible` ring replacing `outline-none`. | High | Components |
| FR-6 | The Overview page renders `PageSubHeader` with title "OVERVIEW", exchange dropdown, and filter popout. Sidebar `max-height` uses `calc(100vh - var(--header-h) - var(--subheader-h))` where `--subheader-h` is a CSS variable (not a magic number). | High | Overview |
| FR-7 | The Charts page renders `PageSubHeader` with title "CHARTS", same exchange dropdown and filter popout. | Medium | Charts |
| FR-8 | The Trades page renders `PageSubHeader` with title "TRADES", same exchange dropdown and filter popout. The existing `TradeFilters` (side + tag) remain below as a secondary filter row. | Medium | Trades |
| FR-9 | The Journal page does NOT render `PageSubHeader` — it keeps its own local filter UI inside `JournalTimeline`. | Medium | Journal |
| FR-10 | `FilterPopout` shows an active filter count badge on the toggle button when filters are applied (e.g., "Filter (2)"). | Medium | Components |
| FR-11 | `FilterPopout` includes a "Clear all" action that resets symbol, date range, and time preset to defaults. | Medium | Components |
| FR-12 | All interactive elements in the popout have visible `focus-visible` rings for keyboard navigation. | High | Accessibility |
| FR-13 | `FilterPopout` uses `animate-dropdown-in` for entrance and fades out on close. `prefers-reduced-motion` respected. | Low | Animation |

---

## Technical Implementation

### PageSubHeader Component

```tsx
// testudo-journal/src/components/PageSubHeader.tsx

interface PageSubHeaderProps {
  title: string
  children?: JSX.Element  // slot for page-specific controls (e.g., TradeFilters)
}

export function PageSubHeader(props: PageSubHeaderProps) {
  const { filters, setFilters } = useFilters()
  const [showPopout, setShowPopout] = createSignal(false)
  const [options] = createResource(
    () => filters().exchange,
    (exchange) => fetchFilterOptions(exchange || undefined)
  )

  const activeFilterCount = () => {
    let count = 0
    if (filters().symbol) count++
    if (filters().dateFrom || filters().dateTo) count++
    return count
  }

  return (
    <div class="border-b border-container-border bg-container-bg">
      <div class="max-w-[1400px] mx-auto px-6 py-3 flex items-center gap-4">
        <h1 class="font-display text-lg font-bold tracking-wider">{props.title}</h1>

        {/* Exchange dropdown — always visible */}
        <ExchangeDropdown value={filters().exchange} onChange={...} />

        {/* Filter toggle */}
        <button onClick={() => setShowPopout(!showPopout())}>
          Filter {activeFilterCount() > 0 ? `(${activeFilterCount()})` : ''}
        </button>

        {props.children}
      </div>

      {/* Popout panel */}
      <Show when={showPopout()}>
        <FilterPopout
          symbols={options()?.symbols ?? []}
          onClose={() => setShowPopout(false)}
        />
      </Show>
    </div>
  )
}
```

### FilterPopout Component

```tsx
// testudo-journal/src/components/FilterPopout.tsx

export function FilterPopout(props: { symbols: SymbolCount[]; onClose: () => void }) {
  // Contains:
  // 1. SymbolSearch (reuse existing component)
  // 2. Time preset buttons with proper active state (bg-white/10)
  // 3. Custom date range inputs (shown when CUSTOM selected)
  // 4. "Clear all" button
  // Positioned via absolute/relative to parent, not a modal
}
```

### CSS Variable for Sub-Header Height

```css
:root {
  --header-h: 57px;
  --subheader-h: 49px;  /* py-3 (24px) + content (~25px) */
}
```

### Layout Changes

```tsx
// Layout.tsx — remove FilterBar import and rendering
// The conditional `<Show when={!location.pathname.startsWith('/journal')}>` block is deleted entirely.
```

### Files

- `testudo-journal/src/components/PageSubHeader.tsx` — new component
- `testudo-journal/src/components/FilterPopout.tsx` — new component (extracted from FilterBar logic)
- `testudo-journal/src/components/Layout.tsx` — remove FilterBar rendering
- `testudo-journal/src/components/FilterBar.tsx` — delete file
- `testudo-journal/src/components/Overview.tsx` — add PageSubHeader, replace `83px` with `--subheader-h`
- `testudo-journal/src/components/Charts.tsx` — add PageSubHeader, remove standalone h1
- `testudo-journal/src/components/trades/TradeTable.tsx` — parent page adds PageSubHeader
- `testudo-journal/src/styles/app.css` — add `--subheader-h` CSS variable

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] FilterBar.tsx is deleted; no global filter bar renders below the header
- [ ] Overview, Charts, and Trades pages each render PageSubHeader with page title and exchange dropdown
- [ ] Clicking "Filter" button opens a popout panel with symbol search and date range controls
- [ ] Time preset active state is visually distinct from hover state (background tint, not just color)
- [ ] Filter popout closes on Escape, outside click, or toggle button re-click
- [ ] Active filter count badge shows on the toggle button when filters are applied
- [ ] Overview sidebar `max-height` uses `--subheader-h` CSS variable, not `83px`
- [ ] All popout controls have visible focus-visible rings
- [ ] Journal page is unaffected (no PageSubHeader)
- [ ] `bun run build` passes with zero errors

---

## Risks

1. **Filter context shared across pages** — The existing `FilterProvider` wraps the entire app. Navigating from Overview (exchange=binance) to Charts should retain that filter. This is correct and unchanged — the context persists across routes.
2. **Popout positioning on mobile** — The popout panel must not overflow the viewport. Use `max-w-[calc(100vw-3rem)]` and `max-h-[60vh] overflow-y-auto` to constrain.

---

## Completion Signal

This spec is complete when:
1. PageSubHeader and FilterPopout components are implemented
2. All three data pages render the sub-header pattern
3. FilterBar.tsx is deleted
4. Overview sidebar uses CSS variable instead of magic number
5. `bun run build` passes
6. Code committed to master
