# Specification: Add Motion and Transitions to Panels and Modals

**Spec ID:** UXP-04-motion-and-transitions
**Date:** 2026-03-18
**Status:** Draft
**Class:** Feature / Motion Design
**Priority:** P1 — Instant open/close feels broken; motion provides spatial context
**Depends on:** UXP-01-design-system-alignment
**Series:** UXP-01 through UXP-08 (Journal UX Polish from design critique)

---

## Problem Statement

Every overlay in the journal app (TradeDetail drawer, EntryEditor modal, TagManager modal, TagSelector dropdown, TradeSelector dropdown) appears and disappears instantly. No fade, no slide, no scale. The only motion in the app is `animate-pulse` on loading skeletons and `transition-colors` on hover states.

Instant appearance breaks spatial understanding — users can't tell where a panel came from or where it went. For a data-heavy app where traders click rapidly between trades, the lack of motion makes state changes jarring and disorienting.

The fix: add purposeful entrance/exit animations to all overlays. Use exponential easing (ease-out-expo) for natural deceleration. Keep durations short (150-250ms). Respect `prefers-reduced-motion`.

---

## User Stories

- **As a trader**, I want panels and modals to animate in/out, so that state changes feel smooth and I maintain spatial context.
- **As a trader with motion sensitivity**, I want the option to disable animations, so that the app doesn't cause discomfort.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | TradeDetail drawer slides in from right: `translate-x-full → translate-x-0` over 200ms with ease-out-expo | High | Trades |
| FR-2 | All centered modals (EntryEditor, TagManager) fade+scale: `opacity-0 scale-95 → opacity-100 scale-100` over 150ms | High | Journal |
| FR-3 | Backdrop overlays fade in: `opacity-0 → opacity-100` over 200ms | High | All |
| FR-4 | Dropdown menus (TagSelector, TradeSelector, tag picker in TradeDetail) scale from origin: `scale-95 opacity-0 → scale-100 opacity-100` over 120ms | Medium | Components |
| FR-5 | All animations respect `prefers-reduced-motion: reduce` — skip to final state instantly | High | All |
| FR-6 | Exit animations reverse the entrance (same duration, same easing) before unmounting the element | Medium | All |
| FR-7 | Page transitions: staggered fade-in of main content sections on route change (50ms stagger between cards/sections) | Low | Layout |

---

## Technical Implementation

### Easing Curve

```css
/* Exponential ease-out — fast start, smooth deceleration */
--ease-out-expo: cubic-bezier(0.16, 1, 0.3, 1);
```

### Transition Utility (Solid.js)

Create a `<Transition>` wrapper component using Solid's `createSignal` and CSS transitions:

```tsx
function Transition(props: {
  show: boolean;
  enter: string;      // e.g. "transition-all duration-200 ease-out-expo"
  enterFrom: string;  // e.g. "opacity-0 translate-x-full"
  enterTo: string;    // e.g. "opacity-100 translate-x-0"
  leave: string;
  leaveFrom: string;
  leaveTo: string;
  children: JSX.Element;
}) { ... }
```

Alternatively, use Solid's built-in `<Transition>` from `solid-transition-group` if available, or use CSS keyframes with Tailwind's `animate-*` utilities.

### Implementation Per Component

| Component | Animation | Duration | Easing |
|-----------|-----------|----------|--------|
| TradeDetail drawer | `translate-x-full → translate-x-0` | 200ms | ease-out-expo |
| TradeDetail backdrop | `opacity-0 → opacity-60` | 200ms | ease-out |
| EntryEditor modal | `opacity-0 scale-95 → opacity-100 scale-100` | 150ms | ease-out-expo |
| TagManager modal | `opacity-0 scale-95 → opacity-100 scale-100` | 150ms | ease-out-expo |
| Modal backdrops | `opacity-0 → opacity-60` | 200ms | ease-out |
| TagSelector dropdown | `opacity-0 scale-95 → opacity-100 scale-100` | 120ms | ease-out |
| TradeSelector dropdown | `opacity-0 scale-95 → opacity-100 scale-100` | 120ms | ease-out |

### Reduced Motion

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

### Files

- `testudo-journal/src/styles/app.css` — Add ease-out-expo custom property, reduced motion media query
- `testudo-journal/src/components/Transition.tsx` — New transition wrapper component
- `testudo-journal/src/components/trades/TradeDetail.tsx` — Wrap in slide transition
- `testudo-journal/src/components/journal/EntryEditor.tsx` — Wrap in fade+scale transition
- `testudo-journal/src/components/journal/TagManager.tsx` — Wrap in fade+scale transition
- `testudo-journal/src/components/journal/TagSelector.tsx` — Add dropdown transition
- `testudo-journal/src/components/journal/TradeSelector.tsx` — Add dropdown transition

### Dependencies Added

- `solid-transition-group` — If Solid's built-in transition primitives are insufficient. Check if already available via Solid's core before adding.

---

## Acceptance Criteria

- [ ] TradeDetail drawer slides in from right when opened
- [ ] All modals fade+scale on open and reverse on close
- [ ] Backdrops fade in/out (not instant)
- [ ] Dropdown menus animate from their origin point
- [ ] Setting `prefers-reduced-motion: reduce` in OS disables all animations
- [ ] No animation exceeds 250ms
- [ ] Exit animations complete before DOM elements are unmounted
- [ ] `bun run build` passes

---

## Risks

1. **Exit animations delay unmounting** — Mitigation: Use `onAfterLeave` callback to unmount after transition completes. Keep exit durations short (150ms max).
2. **Solid.js transition API may differ from React** — Mitigation: Use CSS transitions with class toggling as a fallback. Solid's `createEffect` can toggle classes reliably.

---

## Completion Signal

This spec is complete when:
1. All overlays have entrance and exit animations
2. Reduced motion preference is respected
3. Animations feel smooth at 60fps (no jank)
4. `bun run build` passes
5. Code committed to master
