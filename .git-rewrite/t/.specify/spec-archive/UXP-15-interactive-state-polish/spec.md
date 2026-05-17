# Specification: Interactive State and Edge Case Polish

**Spec ID:** UXP-15-interactive-state-polish
**Date:** 2026-03-18
**Status:** Draft
**Class:** Refactor / UX Polish
**Priority:** P1 — Empty states mislead users; focus ring inconsistency breaks keyboard navigation
**Depends on:** UXP-13-overview-sub-header (focus ring work should follow filter restructure)
**Series:** UXP-13 through UXP-15 (Overview redesign + critique fixes)

---

## Problem Statement

The design critique identified several interactive state and edge case problems that undermine the journal's usability:

1. **Two conflicting focus ring styles**: The global `:focus-visible` in `app.css` uses `rgba(255,255,255,0.4)` (40% white), while the `.focus-ring` utility class uses `ring-white/20` (20% white). Inputs, textareas, and selects use `.focus-ring`; everything else uses the global. This creates inconsistent visual feedback for keyboard users navigating between elements.

2. **Empty states provide no guidance**: Chart containers show "NO DATA" with no explanation of why or what to do. Overview error shows "FAILED TO LOAD STATS" with no retry button. Only the journal empty state has a CTA. A trader filtering to "BTCUSDT in last 7 days" who sees blank charts gets no feedback about whether they have zero trades or their filter is too narrow.

3. **Error states have no retry affordance**: When a network error occurs on Overview or Charts, the only recovery is a full page refresh — which loses filter state. The error container should include a retry button.

4. **Delete without confirmation**: `[Delete]` on `EntryCard` fires `onDelete` immediately. This is a destructive action on user-authored content with no undo mechanism.

5. **ChartSelector has no accessible name**: The `<select>` dropdown for choosing secondary charts has no `<label>` or `aria-label`, making it invisible to screen readers.

6. **MarkdownPreview renders unsanitized HTML**: `innerHTML` is set from markdown-parsed content without DOMPurify or equivalent sanitization. If journal entry bodies contain injected script tags (unlikely in a single-user app but still an XSS vector), they execute.

---

## User Stories

- **As a keyboard user**, I want consistent focus rings on all interactive elements, so that I always know which element is focused.
- **As a trader**, I want empty chart states to tell me why they're empty and offer to clear my filters, so that I can distinguish "no data exists" from "my filter is too narrow."
- **As a trader**, I want to retry failed data loads without refreshing the page, so that transient network errors don't lose my filter state.
- **As a trader**, I want a confirmation before deleting journal entries, so that I don't accidentally lose my notes.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Unify focus ring: remove `.focus-ring` utility class from `app.css`. Replace all `.focus-ring` usages with the global `:focus-visible` style. The global style is the canonical one (40% white, double-ring). | High | CSS |
| FR-2 | Ensure no interactive element uses `outline-none` without a replacement focus indicator. Audit and fix all instances. | High | Accessibility |
| FR-3 | Chart empty states show context-aware copy: "No [chart type] data for current filters" with a "Clear filters" text button that resets the filter context. | High | Charts |
| FR-4 | Overview error state includes a "Retry" button that calls `refetch()` on the stats resource. | High | Overview |
| FR-5 | Chart error states include a "Retry" button that calls the chart's refetch function. | High | Charts |
| FR-6 | `EntryCard` delete action shows an inline confirmation: replace `[Delete]` text with `[Confirm?]` for 3 seconds after first click, then revert. Only the second click within the window fires `onDelete`. | Medium | Journal |
| FR-7 | `ChartSelector` select element gets an `aria-label="Select chart type"`. | Medium | Accessibility |
| FR-8 | `MarkdownPreview` sanitizes HTML output. Add `DOMPurify` as a dependency and sanitize before setting `innerHTML`. | Medium | Security |
| FR-9 | `StatCard.tsx` — delete the component if it is not rendered anywhere (only imported for its type). Move the `StatItem` type to `StatSection.tsx` or `types.ts`. | Low | Cleanup |

---

## Technical Implementation

### FR-1 & FR-2: Focus Ring Unification

Remove `.focus-ring` from `app.css`. The global `:focus-visible` already provides the canonical double-ring:

```css
:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px #050505, 0 0 0 4px rgba(255, 255, 255, 0.4);
}
```

Replace usages in:
- `TradeDetail.tsx` textarea: `focus-ring` → remove (global applies)
- `JournalTimeline.tsx` selects and inputs: `focus-ring` → remove
- `EntryEditor.tsx` textarea: `focus-ring` → remove

Audit for `outline-none` without replacement:
- FilterPopout (if UXP-13 creates new buttons, ensure focus-visible)
- Any remaining instances from the old FilterBar

### FR-3: Context-Aware Empty States

```tsx
// In ChartContainer.tsx — replace the current empty fallback
<Show when={!hasData()}>
  <div class="flex flex-col items-center justify-center h-64 text-center">
    <p class="font-mono text-sm text-text-tertiary mb-1">
      No {props.title.toLowerCase()} data
    </p>
    <Show when={hasActiveFilters()}>
      <p class="font-mono text-xs text-text-tertiary mb-3">
        Try adjusting your filters
      </p>
      <button
        class="font-mono text-xs text-text-secondary hover:text-text-primary transition-colors"
        onClick={clearFilters}
      >
        Clear filters
      </button>
    </Show>
  </div>
</Show>
```

This requires `ChartContainer` to accept `useFilters()` context or receive `hasActiveFilters` / `clearFilters` as props.

### FR-4 & FR-5: Error Retry

```tsx
// Overview.tsx error state
<div role="alert" aria-live="assertive" class="...">
  <p class="font-mono text-signal-red text-sm mb-2">FAILED TO LOAD STATS</p>
  <p class="font-mono text-text-tertiary text-xs mb-4">{String(stats.error)}</p>
  <button
    class="font-mono text-xs text-text-secondary hover:text-text-primary transition-colors border border-container-border px-3 py-1.5 rounded"
    onClick={() => refetch()}
  >
    Retry
  </button>
</div>
```

For charts, `ChartContainer` needs a `retry` prop (function) passed from parent.

### FR-6: Delete Confirmation

```tsx
// EntryCard.tsx
const [confirmDelete, setConfirmDelete] = createSignal(false)
let deleteTimer: number | undefined

function handleDeleteClick() {
  if (confirmDelete()) {
    // Second click — actually delete
    props.onDelete()
    setConfirmDelete(false)
  } else {
    // First click — enter confirm state
    setConfirmDelete(true)
    deleteTimer = window.setTimeout(() => setConfirmDelete(false), 3000)
  }
}

onCleanup(() => clearTimeout(deleteTimer))

// In JSX:
<button
  class={`font-mono text-xs transition-colors ${
    confirmDelete()
      ? 'text-signal-red'
      : 'text-text-tertiary hover:text-signal-red'
  }`}
  onClick={handleDeleteClick}
>
  {confirmDelete() ? '[Confirm?]' : '[Delete]'}
</button>
```

### FR-8: MarkdownPreview Sanitization

```bash
cd testudo-journal && bun add dompurify && bun add -d @types/dompurify
```

```tsx
// MarkdownPreview.tsx
import DOMPurify from 'dompurify'

// Before setting innerHTML:
const clean = DOMPurify.sanitize(html)
el.innerHTML = clean
```

### Files

- `testudo-journal/src/styles/app.css` — remove `.focus-ring` class
- `testudo-journal/src/components/trades/TradeDetail.tsx` — remove focus-ring class usage
- `testudo-journal/src/components/journal/JournalTimeline.tsx` — remove focus-ring class usage
- `testudo-journal/src/components/journal/EntryEditor.tsx` — remove focus-ring class usage
- `testudo-journal/src/components/charts/ChartContainer.tsx` — context-aware empty state + retry prop
- `testudo-journal/src/components/Overview.tsx` — error retry button
- `testudo-journal/src/components/journal/EntryCard.tsx` — delete confirmation
- `testudo-journal/src/components/ChartSelector.tsx` — add aria-label
- `testudo-journal/src/components/journal/MarkdownPreview.tsx` — DOMPurify sanitization
- `testudo-journal/src/components/StatCard.tsx` — delete file, move type to StatSection.tsx

### Dependencies Added

- `dompurify` — HTML sanitization for MarkdownPreview (widely-used, no transitive deps)
- `@types/dompurify` — TypeScript types (dev only)

---

## Acceptance Criteria

- [ ] `grep -r 'focus-ring' testudo-journal/src/styles/app.css` returns zero matches
- [ ] `grep -r 'outline-none' testudo-journal/src/components/` returns zero matches without a replacement focus indicator
- [ ] All chart empty states show descriptive copy and "Clear filters" when filters are active
- [ ] Overview and chart error states include a working "Retry" button
- [ ] Deleting a journal entry requires two clicks (first shows "[Confirm?]", second deletes)
- [ ] ChartSelector select has `aria-label`
- [ ] MarkdownPreview uses DOMPurify before innerHTML assignment
- [ ] `StatCard.tsx` is deleted; `StatItem` type lives in `StatSection.tsx`
- [ ] `bun run build` passes with zero errors

---

## Risks

1. **DOMPurify bundle size** — DOMPurify adds ~15KB gzipped. Acceptable for XSS protection. If size is a concern, consider `isomorphic-dompurify` or a simpler regex sanitizer, but DOMPurify is the correct choice for security.
2. **Global `:focus-visible` on native selects** — Browser-rendered `<select>` elements may render the box-shadow focus ring differently across browsers. Test in Chrome and Firefox. If the double-ring looks odd on selects specifically, add a targeted override.
3. **ChartContainer accessing filter context** — Currently ChartContainer is a presentation wrapper. Adding `useFilters()` dependency couples it to the app context. Alternative: pass `hasActiveFilters` and `onClearFilters` as optional props.

---

## Completion Signal

This spec is complete when:
1. Focus ring is unified across the entire application
2. All empty states show contextual copy with clear-filter action
3. All error states have retry buttons
4. Delete confirmation pattern works on EntryCard
5. MarkdownPreview is sanitized
6. `bun run build` passes
7. Code committed to master
