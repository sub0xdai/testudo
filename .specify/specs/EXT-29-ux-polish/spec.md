# Specification: UX Polish

**Spec ID:** EXT-29-ux-polish
**Date:** 2026-03-14
**Status:** Draft
**Class:** Audit Fix
**Priority:** P2 — quality-of-life improvements
**Audit Refs:** M-1
**Critique Refs:** ISSUE-4 (Empty & Loading States)
**Depends on:** EXT-26 (tokens), EXT-27 (accessibility), EXT-28 (performance)

---

## Overview

Final polish pass addressing reduced-motion accessibility and improving empty/loading state UX. These issues don't block functionality but affect perceived quality and accessibility compliance.

**Current state:**
- `status-blink` pulse, ArcGauge 700ms transitions, and refresh spin ignore `prefers-reduced-motion` (M-1)
- Loading states show plain text ("Loading...", "$--", "SENDING...") with no skeleton or progress indication
- Balance panel shows "$--" with no context when loading
- Empty states (no positions, no exchange) are minimal with no guidance

**Target state:**
- All animations respect `prefers-reduced-motion: reduce`
- Loading states use subtle shimmer/pulse with contextual hint text
- Empty states guide users toward next actions

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add `@media (prefers-reduced-motion: reduce)` block that disables `status-blink`, reduces ArcGauge transition to 0ms, and stops refresh spin animation | High | popup.css |
| FR-2 | Add shimmer animation to "$--" balance placeholder while loading | Medium | MainView |
| FR-3 | Add hint text below loading balance: "Fetching balance..." | Medium | MainView |
| FR-4 | Improve ActiveOrders loading state with skeleton cards or contextual message | Medium | ActiveOrders |
| FR-5 | Improve QuickTrade submission feedback — replace "SENDING..." with a brief spinner + disable button | Medium | QuickTrade |
| FR-6 | Add guidance to empty position state: "No active positions — place a trade from TradingView (Alt+X)" | Low | ActiveOrders |
| FR-7 | Add guidance to no-exchange-connected state: "Connect an exchange account in Settings to start trading" | Low | MainView |

---

## Technical Implementation

### 1) Reduced Motion (FR-1)

```css
/* popup.css — at bottom of file */
@media (prefers-reduced-motion: reduce) {
  .status-blink {
    animation: none;
  }

  * {
    transition-duration: 0.01ms !important;
    animation-duration: 0.01ms !important;
  }
}
```

### 2) Balance Loading Shimmer (FR-2, FR-3)

```css
/* popup.css */
@keyframes shimmer {
  0% { opacity: 0.5; }
  50% { opacity: 1; }
  100% { opacity: 0.5; }
}

.balance-loading {
  animation: shimmer 1.5s ease-in-out infinite;
}
```

```tsx
// MainView.tsx
<Show when={!balanceLoaded()} fallback={<span>{formatBalance(balance())}</span>}>
  <span class="balance-loading text-text-secondary">$--</span>
  <span class="text-xs text-text-dim mt-1">Fetching balance...</span>
</Show>
```

### 3) Improved Empty States (FR-6, FR-7)

```tsx
// ActiveOrders.tsx — empty positions
<Show when={positions().length === 0}>
  <div class="text-center py-8 text-text-secondary text-sm">
    <p>No active positions</p>
    <p class="text-text-dim text-xs mt-1">
      Place a trade from TradingView (Alt+X)
    </p>
  </div>
</Show>
```

### 4) QuickTrade Submit Feedback (FR-5)

```tsx
// QuickTrade.tsx
<button disabled={submitting()} class="...">
  <Show when={submitting()} fallback="EXECUTE">
    <span class="inline-block animate-spin mr-1">⟳</span> Executing...
  </Show>
</button>
```

---

## Affected Files

| File | Changes |
|------|---------|
| `src/popup/popup.css` | Reduced-motion media query, shimmer keyframes |
| `src/popup/components/MainView.tsx` | Balance loading shimmer + hint text, no-exchange guidance |
| `src/popup/components/ActiveOrders.tsx` | Improved empty state text |
| `src/popup/components/QuickTrade.tsx` | Submit button feedback |

---

## Verification

```bash
cd testudo-extension && bun run build
```

- [ ] Build succeeds with no errors
- [ ] `prefers-reduced-motion` block exists — `grep 'prefers-reduced-motion' src/popup/popup.css` returns match
- [ ] Shimmer animation defined — `grep 'shimmer' src/popup/popup.css` returns match
- [ ] Manual (OS reduced motion ON): no blinking, no slow transitions, no spin animations
- [ ] Manual: balance shows "$--" with shimmer while loading, resolves to actual balance
- [ ] Manual: empty positions list shows guidance text
- [ ] Manual: QuickTrade button shows spinner during submission

---

*Consolidates audit issue M-1 and critique issue 4 (Empty & Loading States).*
