# Specification: Harden Journal Accessibility (WCAG AA)

**Spec ID:** A11Y-01-journal-accessibility
**Date:** 2026-04-09
**Status:** Draft
**Class:** Refactor / Accessibility
**Priority:** P2 — no functional impact, improves keyboard/screen reader UX for future public launch
**Depends on:** None
**Series:** A11Y-01 (standalone)

---

## Problem Statement

The testudo-journal frontend has ~20 accessibility gaps identified by audit. Keyboard-only users cannot navigate trade rows or interact with modals properly. Screen reader users encounter unlabeled selects, missing error announcements, and images without alt text. Touch targets on pagination and small buttons are below the 44px recommendation.

These are all cosmetic/attribute changes — zero functional logic is modified. The app works identically before and after; the changes add invisible attributes (aria-labels, roles, tabIndex) and minor spacing/contrast tweaks.

The design system fundamentals are strong (CSS variable tokens, global focus-visible ring, prefers-reduced-motion support, semantic HTML). This spec closes the remaining gaps to reach WCAG AA compliance.

---

## User Stories

- **As a keyboard-only user**, I want to navigate trade rows and open trade details with Enter/Space, so that I can use the journal without a mouse.
- **As a screen reader user**, I want select dropdowns and form inputs to announce their purpose, so that I know what I'm interacting with.
- **As a low-vision user**, I want sufficient contrast on tertiary text and disabled states, so that I can read all content.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Trade rows (`<tr>`) keyboard-accessible: `tabIndex={0}`, `role="button"`, Enter/Space opens detail | High | TradeRow |
| FR-2 | Modal focus moves to first focusable element on open | High | TradeDetail, EntryEditor, TagManager |
| FR-3 | All `<select>` elements have `aria-label` | High | AddExchangeForm, ChartSelector, PageSubHeader, EntryEditor |
| FR-4 | Logo images have descriptive `alt` text | High | Layout |
| FR-5 | Error containers have `role="alert"` | High | Account, AddExchangeForm |
| FR-6 | Form inputs have `id` attrs with matching `htmlFor` on labels | Medium | AddExchangeForm, TradeFilters |
| FR-7 | Kebab menu button has `aria-expanded` and `aria-label` | Medium | ExchangeCard |
| FR-8 | Tertiary text contrast raised to WCAG AA (4.5:1 minimum) | Medium | app.css |
| FR-9 | Pagination and small button touch targets >= 44px | Medium | Pagination, TradeDetail close, TagManager |
| FR-10 | Listbox components have `aria-controls` linking trigger to dropdown | Low | SymbolSearch, TagSelector, TradeSelector |
| FR-11 | Loading spinners have `aria-live="polite"` and `aria-busy` | Low | Account, ChartSelector |
| FR-12 | SymbolSearch input has `aria-label="Search symbols"` | Low | SymbolSearch |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Critical a11y: trade row keyboard, modal focus, aria-labels on selects/images/errors (FR-1 through FR-5) | Keyboard users can navigate trades; screen readers announce all controls |
| CP-2 | High/medium a11y: form labels, aria-expanded, contrast, touch targets (FR-6 through FR-9) | Forms properly labeled; contrast passes WCAG AA; buttons meet 44px |
| CP-3 | Low a11y: listbox aria-controls, loading states, search label (FR-10 through FR-12) | Remaining audit items closed |

### CP-1: Critical Fixes

**TradeRow.tsx** — Add keyboard support to `<tr>`:
```tsx
<tr
  tabIndex={0}
  role="button"
  onClick={() => onSelect(trade)}
  onKeyDown={(e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      onSelect(trade)
    }
  }}
  class="cursor-pointer hover:bg-container-bg-hover focus:bg-container-bg-hover outline-none"
>
```

**TradeDetail.tsx, EntryEditor.tsx, TagManager.tsx** — Move focus on modal open:
```tsx
onMount(() => {
  // Focus first focusable element or close button
  const firstFocusable = panelRef?.querySelector('button, [tabindex="0"]') as HTMLElement
  firstFocusable?.focus()
})
```

**AddExchangeForm.tsx:73** — Add aria-label to exchange select:
```tsx
<select aria-label="Exchange" ...>
```

**ChartSelector.tsx:57** — Add aria-label:
```tsx
<select aria-label="Chart type" ...>
```

**PageSubHeader.tsx:89** — Add aria-label:
```tsx
<select aria-label="Exchange filter" ...>
```

**EntryEditor.tsx:243** — Add aria-label:
```tsx
<select aria-label="Entry type" ...>
```

**Layout.tsx:293, 386** — Fix alt text:
```tsx
<img src="crest.png" alt="Testudo" ... />
<img src="shield.svg" alt="Testudo" ... />
```

**Account.tsx:145, AddExchangeForm.tsx:141** — Add role="alert":
```tsx
<div role="alert" class="text-signal-red ...">
```

### CP-2: Medium Fixes

**AddExchangeForm.tsx** — Add `id` to inputs, `for` to labels:
```tsx
<label for="api-key" ...>API KEY</label>
<input id="api-key" ... />
```

**ExchangeCard.tsx:35-42** — Add aria attrs to kebab menu:
```tsx
<button aria-label="Account options" aria-expanded={menuOpen()} ...>
```

**app.css** — Raise tertiary text contrast:
```css
/* Dark theme: 102 → 130 (achieves ~4.7:1 on #0a0a0a bg) */
--text-tertiary: 130 130 130;

/* Light theme: 120 113 102 → verify passes on #f5f0e8 bg */
```

**Pagination.tsx** — Increase touch targets:
```tsx
{/* Change px-2 py-1 to px-3 py-2 for minimum 44px height */}
<button class="px-3 py-2 min-h-[44px] min-w-[44px] ..." ...>
```

**TradeDetail.tsx close button, TagManager.tsx Edit/Del** — Add padding:
```tsx
<button class="p-2 min-h-[44px] min-w-[44px] ..." aria-label="Close" ...>
```

### CP-3: Low-Priority Fixes

**SymbolSearch.tsx** — Link trigger to listbox + label input:
```tsx
<input aria-label="Search symbols" aria-controls="symbol-listbox" ... />
<div role="listbox" id="symbol-listbox" ...>
```

**TagSelector.tsx, TradeSelector.tsx** — Same `aria-controls` pattern.

**Account.tsx:151, ChartSelector.tsx:36** — Loading aria-live:
```tsx
<div aria-live="polite" aria-busy="true">Loading...</div>
```

### Paved Roads

- Global `focus-visible` ring already in `app.css:176-179` — new focusable elements inherit it
- `createFocusTrap()` already used in modals — just need to add initial focus
- CSS variable token system — contrast change is one line in `:root`
- `prefers-reduced-motion` already handled — no new animations added

### Files

All files are in `testudo-journal/src/`:

- `components/trades/TradeRow.tsx` — **modified** — keyboard support on `<tr>`
- `components/trades/TradeDetail.tsx` — **modified** — modal focus, close button size, aria-label
- `components/journal/EntryEditor.tsx` — **modified** — modal focus, select aria-label
- `components/journal/TagManager.tsx` — **modified** — modal focus, button sizes
- `components/account/AddExchangeForm.tsx` — **modified** — aria-labels, input ids, error role
- `components/account/ExchangeCard.tsx` — **modified** — kebab menu aria-expanded
- `components/ChartSelector.tsx` — **modified** — select aria-label, loading aria-live
- `components/PageSubHeader.tsx` — **modified** — select aria-label
- `components/Pagination.tsx` — **modified** — touch target sizing
- `components/SymbolSearch.tsx` — **modified** — input label, aria-controls
- `components/TagSelector.tsx` — **modified** — aria-controls
- `components/TradeSelector.tsx` — **modified** — aria-controls
- `pages/Account.tsx` — **modified** — error role, loading aria-live
- `pages/Layout.tsx` — **modified** — image alt text
- `styles/app.css` — **modified** — tertiary text contrast value

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Trade rows navigable via Tab, openable via Enter/Space
- [ ] Modal focus moves to first focusable element on open
- [ ] All `<select>` elements announce their purpose in screen readers
- [ ] Logo images have `alt="Testudo"`
- [ ] Error messages announced by screen readers (role="alert")
- [ ] Form inputs properly associated with labels (id/for)
- [ ] Tertiary text passes WCAG AA contrast ratio (4.5:1)
- [ ] Pagination buttons meet 44px minimum touch target
- [ ] No functional regression — app behavior identical
- [ ] `cd testudo-journal && bun run build` passes

---

## Risks

1. **Tertiary text contrast change** — raising from 102 to 130 makes muted text slightly more visible. May subtly alter the visual hierarchy. Mitigation: review in both themes before committing.
2. **Touch target sizing** — larger pagination buttons may shift layout slightly. Mitigation: use `min-h`/`min-w` so they only grow if needed.

---

## Completion Signal

This spec is complete when:
1. All 12 functional requirements implemented
2. `bun run build` passes
3. Manual verification: Tab through trades, open modal with keyboard, screen reader check on selects
4. Code committed to master
