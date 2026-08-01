# Specification: Accessibility Hardening & Responsive Adaptation

**Spec ID:** UXP-12-accessibility-responsive
**Date:** 2026-03-18
**Status:** Draft
**Class:** Feature / Accessibility / Responsive
**Priority:** P1 — Navigation broken on mobile, form labels missing for screen readers
**Depends on:** UXP-11-filter-consolidation
**Series:** UXP-10 through UXP-12 (Audit remediation from journal UI audit)

---

## Problem Statement

**Accessibility:** Form inputs across TradeFilters, JournalTimeline, and EntryEditor have visible text labels but no programmatic `<label>` associations. Screen readers cannot connect "SYMBOL" text to the input next to it. This violates WCAG 1.3.1 (Info and Relationships) and 4.1.2 (Name, Role, Value).

**Responsive:** The top navigation is a horizontal `flex gap-6` with four text links. On viewports under ~400px, items squeeze together or overflow. There is no hamburger menu or alternative mobile layout. Touch targets for nav links are likely below the 44x44px minimum.

**Fragile layout coupling:** The header spacer (`<div class="h-[57px]" />`) and sidebar max-height (`calc(100vh - 140px)`) use magic numbers that break if header height changes.

---

## Functional Requirements

| ID | Requirement | Priority | Category |
|----|-------------|----------|----------|
| FR-1 | All form inputs have programmatic label associations via wrapping `<label>` elements or `for`/`id` pairs | High | A11y |
| FR-2 | Mobile navigation: at `md` breakpoint and below, nav collapses to a hamburger menu that opens a slide-down panel with nav links | High | Responsive |
| FR-3 | Mobile nav touch targets are minimum 44x44px | High | Responsive |
| FR-4 | Header height defined as CSS custom property `--header-h` used by both the spacer div and sidebar max-height calc | Medium | Responsive |
| FR-5 | FilterBar wraps gracefully on mobile — presets stack below exchange/symbol row | Medium | Responsive |
| FR-6 | Remove unused `onCleanup` import from `SymbolSearch.tsx` | Low | Cleanup |

---

## Technical Implementation

### FR-1: Label Associations

Audit all form controls and add labels. Two patterns:

**Pattern A — Wrapping label (preferred for inline layouts):**
```tsx
<label class="flex items-center gap-2">
  <span class="font-mono text-xs text-text-tertiary uppercase tracking-wider">Symbol</span>
  <input type="text" ... />
</label>
```

**Pattern B — for/id pair (when label and input are separated):**
```tsx
<label for="symbol-filter" class="...">Symbol</label>
<input id="symbol-filter" type="text" ... />
```

**Files requiring label fixes:**
- `TradeFilters.tsx` — symbol input, exchange select, tag input (3 controls)
- `JournalTimeline.tsx` — type select, tag select, date from, date to (4 controls)
- `EntryEditor.tsx` — entry type select, date picker (2 controls)
- `FilterBar.tsx` — exchange select, date from, date to (3 controls — these already have `<label>` elements but no `for`/wrapping association)

### FR-2: Mobile Navigation

Replace fixed horizontal nav with responsive pattern:

```tsx
// Below md: hamburger button + slide-down panel
// Above md: horizontal nav (current behavior)

<header>
  <div class="flex items-center justify-between">
    <A href="/">TESTUDO</A>

    {/* Desktop nav */}
    <nav class="hidden md:flex items-center gap-6">
      <For each={NAV_ITEMS}>...</For>
    </nav>

    {/* Mobile hamburger */}
    <button
      class="md:hidden p-2 min-w-[44px] min-h-[44px]"
      onClick={() => setMenuOpen(!menuOpen())}
      aria-expanded={menuOpen()}
      aria-label="Navigation menu"
    >
      <span class="font-mono text-sm">{menuOpen() ? '×' : '≡'}</span>
    </button>
  </div>

  {/* Mobile nav panel */}
  <Show when={menuOpen()}>
    <nav class="md:hidden border-t border-container-border py-2">
      <For each={NAV_ITEMS}>
        {(item) => (
          <A
            href={item.path}
            class="block px-6 py-3 min-h-[44px] font-mono text-sm"
            onClick={() => setMenuOpen(false)}
          >
            {item.label}
          </A>
        )}
      </For>
    </nav>
  </Show>
</header>
```

### FR-4: Header Height CSS Variable

In `app.css`:
```css
:root {
  --header-h: 57px;
}
```

In `Layout.tsx`:
```tsx
<div style={{ height: 'var(--header-h)' }} />
```

In `Overview.tsx`:
```tsx
style={{ "max-height": "calc(100vh - var(--header-h) - 83px)" }}
```

### Files Changed

| File | Change |
|------|--------|
| `src/components/Layout.tsx` | FR-2, FR-3, FR-4: Mobile nav + header height variable |
| `src/styles/app.css` | FR-4: `--header-h` variable |
| `src/components/trades/TradeFilters.tsx` | FR-1: Label associations |
| `src/components/journal/JournalTimeline.tsx` | FR-1: Label associations |
| `src/components/journal/EntryEditor.tsx` | FR-1: Label associations |
| `src/components/FilterBar.tsx` | FR-1, FR-5: Label wrapping + mobile wrap |
| `src/components/Overview.tsx` | FR-4: Use `--header-h` in sidebar calc |
| `src/components/SymbolSearch.tsx` | FR-6: Remove unused import |

---

## Acceptance Criteria

- [ ] Every `<input>`, `<select>`, and `<textarea>` has a programmatic label association (wrapping `<label>` or `for`/`id`)
- [ ] Mobile nav hamburger appears below `md` breakpoint
- [ ] Mobile nav links have minimum 44x44px touch targets
- [ ] Mobile nav closes when a link is clicked
- [ ] Header spacer and sidebar use `--header-h` CSS variable, not magic numbers
- [ ] FilterBar presets wrap gracefully on narrow viewports
- [ ] No unused imports remain in SymbolSearch
- [ ] `bun run build` passes

---

## Completion Signal

This spec is complete when:
1. All form controls have label associations
2. Mobile nav works
3. Magic numbers replaced with CSS variables
4. Build passes
5. Code committed to master
