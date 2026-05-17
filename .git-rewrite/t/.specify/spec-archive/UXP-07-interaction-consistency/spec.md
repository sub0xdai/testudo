# Specification: Fix Interaction Inconsistencies and Edge Cases

**Spec ID:** UXP-07-interaction-consistency
**Date:** 2026-03-18
**Status:** Draft
**Class:** Refactor / Polish
**Priority:** P2 — Polish items that accumulate into perceived quality
**Depends on:** UXP-01-design-system-alignment, UXP-04-motion-and-transitions
**Series:** UXP-01 through UXP-08 (Journal UX Polish from design critique)

---

## Problem Statement

The design critique identified a cluster of inconsistencies that individually are minor but collectively erode trust in the interface. Dropdown styles differ between components. Escape key works in some modals but not others. The global FilterBar renders on the Journal page with no effect. Shadow usage is inconsistent. Error handling is missing in Charts. Trade identifiers show raw UUIDs. These are "last mile" problems — the difference between "functional MVP" and "feels polished."

---

## User Stories

- **As a trader**, I want consistent behavior across all interactive elements, so that the interface feels predictable.
- **As a trader**, I want error states on charts, so that I can distinguish "no data" from "failed to load."

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Hide global FilterBar on `/journal` route (Journal has its own filters) | High | Layout |
| FR-2 | Add error state to ChartContainer: distinguish "NO DATA" from "FAILED TO LOAD" with signal-red styling | High | Charts |
| FR-3 | Escape key closes all modals (EntryEditor, TagManager) — not just TradeDetail | High | Journal |
| FR-4 | Unify dropdown styles: all dropdowns use `bg-elevated border border-container-border rounded shadow-lg` | Medium | Components |
| FR-5 | Add `disabled:cursor-not-allowed` to all disabled buttons | Medium | Components |
| FR-6 | Replace raw UUID display in journal trade links with `{symbol} {date}` format | Medium | Journal |
| FR-7 | Remove `shadow-lg` inconsistency: either all dropdowns have shadow, or none do. Decision: all dropdowns get `shadow-lg shadow-black/30` | Low | Components |
| FR-8 | Trades page outer wrapper: add `rounded-lg` to match all other containers, or explicitly document why it's sharp-cornered | Low | Trades |
| FR-9 | TradeDetail notes textarea and SAVE button: add `rounded` to match other form elements | Low | Trades |
| FR-10 | CumulativeProfit and EquityCurve share the same API resource (eliminate duplicate fetch) | Medium | Charts |

---

## Technical Implementation

### FR-1: Conditional FilterBar

```tsx
// Layout.tsx
<Show when={!location.pathname.startsWith('/journal')}>
  <FilterBar />
</Show>
```

### FR-2: Chart Error State

```tsx
// ChartContainer.tsx — add error prop
<Show when={props.error}>
  <div class="flex items-center justify-center h-full">
    <div class="text-center">
      <span class="font-display text-xs tracking-[0.2em] text-signal-red uppercase">
        FAILED TO LOAD
      </span>
      <p class="font-mono text-xs text-text-tertiary mt-1">{props.error}</p>
    </div>
  </div>
</Show>
```

### FR-3: Universal Escape Handler

```tsx
// Shared utility or per-modal
function useEscapeClose(onClose: () => void) {
  const handler = (e: KeyboardEvent) => {
    if (e.key === 'Escape') onClose();
  };
  onMount(() => window.addEventListener('keydown', handler));
  onCleanup(() => window.removeEventListener('keydown', handler));
}
```

Apply to: EntryEditor, TagManager, TagSelector, TradeSelector (in addition to existing TradeDetail).

### FR-6: Trade Identifier Display

```tsx
// Replace: `Trade ${trade.id.slice(0, 8)}...`
// With:
function formatTradeLabel(trade: Trade): string {
  const symbol = trade.symbol.replace('_', '');
  const date = new Date(trade.entry_time).toLocaleDateString('en-US', {
    month: 'short', day: 'numeric'
  });
  return `${symbol} ${date}`;
}
// Result: "BTCUSDT Mar 15" instead of "Trade a1b2c3d4..."
```

### FR-10: Shared Chart Data

```tsx
// Charts.tsx — fetch once, pass to both components
const [equityData] = createResource(() => filters(), fetchEquityCurve);

<EquityCurve data={equityData()} />
<CumulativeProfit data={equityData()} />
```

### Files

- `testudo-journal/src/components/Layout.tsx` — Conditional FilterBar rendering
- `testudo-journal/src/components/charts/ChartContainer.tsx` — Add error state
- `testudo-journal/src/components/Charts.tsx` — Shared data resource, pass error to ChartContainer
- `testudo-journal/src/components/journal/EntryEditor.tsx` — Add Escape handler
- `testudo-journal/src/components/journal/TagManager.tsx` — Add Escape handler
- `testudo-journal/src/components/journal/TagSelector.tsx` — Unify dropdown style
- `testudo-journal/src/components/journal/TradeSelector.tsx` — Unify dropdown style
- `testudo-journal/src/components/trades/TradeDetail.tsx` — Fix dropdown style, add rounded to textarea/button
- `testudo-journal/src/components/journal/JournalTimeline.tsx` — Fix trade label display
- `testudo-journal/src/components/journal/EntryCard.tsx` — Fix trade label display
- `testudo-journal/src/pages/Trades.tsx` — Add `rounded-lg` to outer wrapper
- `testudo-journal/src/lib/useEscapeClose.ts` — New shared hook

---

## Acceptance Criteria

- [ ] FilterBar is not visible on `/journal` route
- [ ] Chart errors display "FAILED TO LOAD" with red text (not "NO DATA")
- [ ] Escape closes EntryEditor and TagManager modals
- [ ] All dropdowns share identical styling (`bg-elevated rounded shadow-lg`)
- [ ] All disabled buttons show `cursor-not-allowed`
- [ ] Journal trade links show "BTCUSDT Mar 15" format, not UUIDs
- [ ] CumulativeProfit does not make its own API call (shares with EquityCurve)
- [ ] Trades outer wrapper has `rounded-lg`
- [ ] TradeDetail textarea and SAVE button have `rounded`
- [ ] `bun run build` passes

---

## Risks

1. **Hiding FilterBar on Journal may confuse users who expect it** — Mitigation: Journal has its own visible filter panel. The global bar was never wired to Journal anyway.
2. **Shared chart data may cause both charts to show error if one data format differs** — Mitigation: Both charts already use the same API response shape; they just render differently.

---

## Completion Signal

This spec is complete when:
1. All 10 functional requirements are implemented
2. No interaction inconsistencies remain from the critique list
3. `bun run build` passes
4. Code committed to master
