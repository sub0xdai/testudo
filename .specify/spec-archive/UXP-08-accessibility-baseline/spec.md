# Specification: Establish Accessibility Baseline for Interactive Components

**Spec ID:** UXP-08-accessibility-baseline
**Date:** 2026-03-18
**Status:** Draft
**Class:** Feature / Accessibility
**Priority:** P2 — Baseline ARIA and keyboard support for all custom interactive patterns
**Depends on:** UXP-04-motion-and-transitions, UXP-07-interaction-consistency
**Series:** UXP-01 through UXP-08 (Journal UX Polish from design critique)

---

## Problem Statement

The journal uses custom interactive components (dropdowns, modals, drawers, sortable table headers) that lack ARIA attributes, focus management, and keyboard navigation. Screen readers cannot identify dropdown state, modal boundaries, or sort direction. The `outline-none` class on all inputs removes the browser's default focus ring without providing a visible custom replacement in all contexts. Color is the only differentiator for several semantic states (LONG/SHORT, positive/negative P&L, active nav).

This spec establishes the minimum accessibility baseline: ARIA roles on custom widgets, focus trapping in modals, visible focus indicators, and text supplements for color-only information.

---

## User Stories

- **As a keyboard-only user**, I want visible focus indicators on all interactive elements, so that I always know where I am.
- **As a screen reader user**, I want modals announced as dialogs with proper focus management, so that I can navigate them reliably.
- **As a color-blind user**, I want text labels or icons supplementing color-coded information, so that I can read P&L direction and trade side.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | All modals: add `role="dialog"`, `aria-modal="true"`, `aria-labelledby` pointing to the title element | High | Modals |
| FR-2 | Focus trap in modals: Tab cycles within modal boundaries, first focusable element receives focus on open, focus returns to trigger element on close | High | Modals |
| FR-3 | All icon-only buttons (`×` close, pagination arrows) get `aria-label` with descriptive text | High | Buttons |
| FR-4 | Custom dropdowns (TagSelector, TradeSelector): add `role="listbox"`, `aria-expanded`, `role="option"` on items | High | Dropdowns |
| FR-5 | Sortable table headers: add `aria-sort="ascending"`, `"descending"`, or `"none"` to `<th>` elements, wrap sort trigger in `<button>` | High | Table |
| FR-6 | Replace `focus:outline-none` with `focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-signal-green/40 focus-visible:ring-offset-1 focus-visible:ring-offset-main-bg` on all inputs and buttons | High | Global |
| FR-7 | P&L values include direction prefix: `+$2,847` and `-$1,234` (not just color-coded `$2,847` and `$1,234`) | Medium | Formatters |
| FR-8 | Trade side displays "LONG" and "SHORT" text (already does) — ensure text remains when color is removed. Add `aria-label="Long position"` / `aria-label="Short position"` | Low | Table |
| FR-9 | Active nav item: add `aria-current="page"` attribute | Low | Nav |
| FR-10 | Async state changes (loading → loaded, error messages): wrap in `aria-live="polite"` region | Medium | All |

---

## Technical Implementation

### Focus Trap Utility

```tsx
function createFocusTrap(containerRef: () => HTMLElement | undefined) {
  const focusableSelector = [
    'a[href]', 'button:not([disabled])', 'input:not([disabled])',
    'select:not([disabled])', 'textarea:not([disabled])',
    '[tabindex]:not([tabindex="-1"])'
  ].join(', ');

  function trapFocus(e: KeyboardEvent) {
    if (e.key !== 'Tab') return;
    const container = containerRef();
    if (!container) return;

    const focusable = Array.from(container.querySelectorAll(focusableSelector));
    const first = focusable[0] as HTMLElement;
    const last = focusable[focusable.length - 1] as HTMLElement;

    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }

  onMount(() => {
    document.addEventListener('keydown', trapFocus);
    // Focus first element on open
    const container = containerRef();
    const first = container?.querySelector(focusableSelector) as HTMLElement;
    first?.focus();
  });

  onCleanup(() => {
    document.removeEventListener('keydown', trapFocus);
  });
}
```

### Global Focus Ring (app.css)

```css
/* Replace all outline-none patterns */
:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px #050505, 0 0 0 4px rgba(0, 255, 65, 0.4);
}
```

This creates a double-ring effect: inner ring matches background (gap), outer ring is signal-green at 40% opacity. Works on any background color.

### Sortable Header

```tsx
<th>
  <button
    class="font-display text-[10px] tracking-widest uppercase w-full text-left"
    onClick={() => handleSort('net_pnl')}
    aria-sort={sortColumn() === 'net_pnl'
      ? (sortDirection() === 'asc' ? 'ascending' : 'descending')
      : 'none'
    }
  >
    NET P&L {sortColumn() === 'net_pnl' && (sortDirection() === 'asc' ? '▲' : '▼')}
  </button>
</th>
```

### P&L Direction Prefix

```tsx
// formatters.ts — update formatCurrency
export function formatCurrency(value: number): string {
  const prefix = value > 0 ? '+' : '';  // negative sign is automatic
  return `${prefix}$${Math.abs(value).toLocaleString('en-US', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`;
}
// Result: "+$2,847.32" or "-$1,234.56" or "$0.00"
```

### Files

- `testudo-journal/src/styles/app.css` — Global `:focus-visible` ring style
- `testudo-journal/src/lib/createFocusTrap.ts` — New focus trap utility
- `testudo-journal/src/lib/formatters.ts` — Add `+` prefix to positive values
- `testudo-journal/src/components/trades/TradeDetail.tsx` — `role="dialog"`, `aria-modal`, focus trap, `aria-label` on close button
- `testudo-journal/src/components/journal/EntryEditor.tsx` — `role="dialog"`, `aria-modal`, focus trap
- `testudo-journal/src/components/journal/TagManager.tsx` — `role="dialog"`, `aria-modal`, focus trap
- `testudo-journal/src/components/journal/TagSelector.tsx` — `role="listbox"`, `aria-expanded`, `role="option"`
- `testudo-journal/src/components/journal/TradeSelector.tsx` — `role="listbox"`, `aria-expanded`, `role="option"`
- `testudo-journal/src/components/trades/TradeTable.tsx` — `aria-sort` on headers, sort triggers as `<button>`
- `testudo-journal/src/components/trades/Pagination.tsx` — `aria-label` on arrow buttons
- `testudo-journal/src/components/Layout.tsx` — `aria-current="page"` on active nav, `aria-live` on main content area
- `testudo-journal/src/components/Overview.tsx` — `aria-live="polite"` on loading/error containers

---

## Acceptance Criteria

- [ ] All modals have `role="dialog"` and `aria-modal="true"`
- [ ] Tab key cycles within modal boundaries (focus trap)
- [ ] Focus returns to trigger element on modal close
- [ ] All icon-only buttons have `aria-label`
- [ ] Custom dropdowns have `role="listbox"` and `aria-expanded`
- [ ] Sortable headers have `aria-sort` and use `<button>` elements
- [ ] `:focus-visible` ring is visible on all interactive elements (no bare `outline-none`)
- [ ] P&L values show `+`/`-` prefix for direction
- [ ] Active nav has `aria-current="page"`
- [ ] Loading/error state changes are in `aria-live` regions
- [ ] `bun run build` passes

---

## Risks

1. **Focus trap may interfere with browser extensions or dev tools** — Mitigation: Focus trap only activates when modal is open; uses standard DOM APIs.
2. **`+` prefix on positive values changes display width** — Mitigation: The monospace font ensures consistent character widths. Table column widths may need minor adjustment.

---

## Completion Signal

This spec is complete when:
1. All ARIA attributes are applied to custom widgets
2. Focus management works in all modals
3. Focus rings are visible on keyboard navigation
4. Color-coded information has text supplements
5. `bun run build` passes
6. Code committed to master
