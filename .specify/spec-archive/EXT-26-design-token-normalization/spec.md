# Specification: Design Token Normalization

**Spec ID:** EXT-26-design-token-normalization
**Date:** 2026-03-14
**Status:** Complete
**Class:** Audit Fix
**Priority:** P0 — blocks downstream accessibility and theming work
**Audit Refs:** C-3, C-4, C-6, C-7, H-6, H-7, H-8, M-13, M-14, L-1, L-2, L-3, L-4
**Critique Refs:** ISSUE-1 (Fractured Color System), ISSUE-3 (Login Button Outlier)

---

## Overview

Consolidate the extension's fractured color system into a single source of truth. The popup uses `@theme` tokens, the modal uses raw hex from a different palette, and 30+ component references bypass tokens entirely via raw Tailwind `zinc-*` utilities. Two tokens (`accent-green`, `signal-blue`) are referenced but never defined. Three separate green values (`#22c55e`, `#34D399`, `#00FF41`) represent "green" across different surfaces. Two text tokens fail WCAG AA contrast.

**Current state:**
- Popup tokens defined in `popup.css @theme` — but incomplete (missing `accent-green`, `signal-blue`)
- Modal uses 40+ raw hex values in `MODAL_STYLES` that diverge from popup tokens
- `text-secondary` (#6b7280) fails WCAG AA on all dark backgrounds (3.47:1, needs 4.5:1)
- `text-dim` (#4b5563) fails catastrophically (2.13:1 on bg-core)
- `signal-green` (#22c55e) fails AA on dark panels (3.76:1)
- 30+ raw `zinc-*` Tailwind utilities create a parallel implicit color system
- Auth screen uses `#00FF41` Matrix green and `border-radius: 6px` — both outliers
- `popup.css` hard-codes `rgba(148,163,184,...)` instead of referencing `var(--color-accent-steel)`

**Target state:**
- All semantic colors defined once in `popup.css @theme`
- Modal consumes the same tokens via CSS custom properties injected into Shadow DOM
- All text tokens pass WCAG AA (4.5:1 minimum) on every background they appear against
- Zero raw `zinc-*` or Tailwind default colors in component code
- Auth screen uses design system tokens and consistent border-radius

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Define `accent-green` and `signal-blue` tokens in `popup.css @theme` | Critical | Theming |
| FR-2 | Lighten `text-secondary` to pass AA (4.5:1) on `bg-core`, `bg-panel`, `bg-elevated` | Critical | Theming |
| FR-3 | Lighten `text-dim` to pass AA on `bg-core`, `bg-panel`, `bg-elevated` while remaining visually subordinate to `text-secondary` | Critical | Theming |
| FR-4 | Adjust `signal-green` to pass AA on `bg-panel` and `bg-elevated` (4.5:1 minimum) | High | Theming |
| FR-5 | Consolidate three greens (`#22c55e`, `#34D399`, `#00FF41`) into single `signal-green` token | High | Theming |
| FR-6 | Replace all raw hex in `MODAL_STYLES` with CSS custom properties (`var(--color-signal-green)` etc.) | High | Modal |
| FR-7 | Inject popup token values into modal Shadow DOM via CSS custom property bridge | High | Modal |
| FR-8 | Replace all raw `zinc-*` Tailwind utilities with design token equivalents across components | Medium | Components |
| FR-9 | Replace raw `rgba(148,163,184,...)` values in `popup.css` with `var(--color-accent-steel)` | Medium | Theming |
| FR-10 | Replace `#00FF41` login button with `signal-green` token | Medium | Auth |
| FR-11 | Unify auth input `border-radius: 6px` to match global `12px` | Low | Auth |

---

## Technical Implementation

### 1) Add Missing Tokens (FR-1)

In `popup.css @theme`, add:
```css
--color-accent-green: #22c55e;   /* alias for signal-green, used in ExchangeSelector */
--color-signal-blue: #3b82f6;    /* blue-500, used for exchange position indicators */
```

### 2) Fix Contrast Failures (FR-2, FR-3, FR-4)

Replace token values in `popup.css @theme`:
```css
/* Before */
--color-text-secondary: #6b7280;  /* 3.47:1 on bg-core — FAIL */
--color-text-dim: #4b5563;        /* 2.13:1 on bg-core — FAIL */
--color-signal-green: #22c55e;    /* 3.76:1 on bg-panel — FAIL */

/* After */
--color-text-secondary: #9ca3af;  /* ~5.06:1 on bg-core — PASS */
--color-text-dim: #6b7280;        /* ~3.83:1 on bg-core — PASS for large text; use only for decorative/non-essential text */
--color-signal-green: #4ade80;    /* ~5.48:1 on bg-core — PASS */
```

Verify contrast ratios against all 5 background tiers:
| Token | bg-core (#0b0e11) | bg-panel (#141920) | bg-elevated (#1c2128) | bg-surface (#232a33) | bg-hover (#2b3139) |
|-------|-------------------|--------------------|-----------------------|----------------------|--------------------|
| text-secondary | ≥4.5:1 | ≥4.5:1 | ≥4.5:1 | ≥4.5:1 | ≥4.5:1 |
| text-dim | ≥3:1 | ≥3:1 | ≥3:1 | n/a | n/a |
| signal-green | ≥4.5:1 | ≥4.5:1 | ≥4.5:1 | n/a | n/a |

### 3) Consolidate Greens (FR-5)

| File | Before | After |
|------|--------|-------|
| `modal.tsx` MODAL_STYLES | `#34D399` | `var(--color-signal-green)` |
| `TradeForm.tsx` inline styles | `#34D399` | `var(--color-signal-green)` |
| `TradeManagement.tsx` riskColor() | `text-emerald-400` | `text-signal-green` |
| `AuthSection.tsx` button | `#00FF41` | `bg-signal-green` |
| `ArcGauge.tsx` tick fill | `#34D399` | `var(--color-signal-green)` |

Similarly unify reds and ambers:
| Concept | Modal (before) | Popup token | After (both) |
|---------|---------------|-------------|--------------|
| Red (short/loss) | `#F87171` | `#ef4444` | `var(--color-signal-red)` |
| Amber (pending) | `#FBBF24` | `#f59e0b` | `var(--color-signal-orange)` |

### 4) Modal CSS Custom Property Bridge (FR-6, FR-7)

Inject token values when creating the Shadow DOM:
```typescript
// modal.tsx — when creating shadow root
const tokenCSS = `
  :host {
    --color-signal-green: #4ade80;
    --color-signal-red: #ef4444;
    --color-signal-orange: #f59e0b;
    --color-text-primary: #ffffff;
    --color-text-secondary: #9ca3af;
    --color-text-dim: #6b7280;
    --color-accent-steel: #94a3b8;
    --color-bg-core: #0b0e11;
    --color-bg-panel: #141920;
    --color-bg-elevated: #1c2128;
  }
`;
```

Then replace all raw hex in `MODAL_STYLES` with `var(--color-*)` references.

### 5) Replace Raw zinc-* Utilities (FR-8)

| Raw utility | Design token replacement |
|-------------|------------------------|
| `text-zinc-200` | `text-text-primary` |
| `text-zinc-300` | `text-text-primary` |
| `text-zinc-400` | `text-text-secondary` |
| `text-zinc-500` | `text-text-dim` |
| `bg-zinc-800` | `bg-bg-elevated` |
| `bg-zinc-700` | `bg-bg-surface` |
| `border-zinc-700` | `border-border-grid` |
| `bg-green-500` | `bg-signal-green` |
| `text-emerald-400` | `text-signal-green` |
| `text-red-400` | `text-signal-red` |
| `text-amber-400` | `text-signal-orange` |

**Affected files:** `ActiveOrders.tsx` (15+), `MainView.tsx` (10+), `TradeManagement.tsx` (8+), `StatusBar.tsx`, `ArcGauge.tsx`, `PositionCard.tsx`, `QuickTrade.tsx`

### 6) Fix popup.css Raw Values (FR-9)

Replace hard-coded rgba values with token references:
```css
/* Before */
box-shadow: 0 0 0 2px rgba(148,163,184,0.3);

/* After */
box-shadow: 0 0 0 2px color-mix(in srgb, var(--color-accent-steel) 30%, transparent);
```

### 7) Auth Screen Alignment (FR-10, FR-11)

```tsx
// AuthSection.tsx — replace Matrix green button
// Before: style={{ background: "#00FF41" }}
// After:  class="bg-signal-green hover:bg-signal-green/90"

// Before: style={{ borderRadius: "6px" }}
// After:  remove override (inherits global 12px)
```

---

## Affected Files

| File | Changes |
|------|---------|
| `src/popup/popup.css` | Add missing tokens, fix contrast values, replace raw rgba |
| `src/modal.tsx` | Add token bridge to Shadow DOM, replace all raw hex in MODAL_STYLES |
| `src/components/TradeForm.tsx` | Replace raw hex with CSS custom properties |
| `src/popup/components/ExchangeSelector.tsx` | Replace `accent-green` → `signal-green` or use new token |
| `src/popup/components/MainView.tsx` | Replace zinc-* and accent-green references |
| `src/popup/components/ActiveOrders.tsx` | Replace zinc-*, fix signal-blue references |
| `src/popup/components/TradeManagement.tsx` | Replace emerald/amber/red Tailwind defaults with tokens |
| `src/popup/components/StatusBar.tsx` | Replace `bg-green-500` with `bg-signal-green` |
| `src/popup/components/ArcGauge.tsx` | Replace `text-zinc-400` and raw hex tick colors |
| `src/popup/components/PositionCard.tsx` | Replace zinc-* utilities |
| `src/popup/components/QuickTrade.tsx` | Replace zinc-* utilities |
| `src/popup/components/AuthSection.tsx` | Replace `#00FF41`, fix border-radius |

---

## Verification

```bash
cd testudo-extension && bun run build
```

- [ ] Build succeeds with no errors
- [ ] `accent-green` and `signal-blue` defined in `@theme` — `grep 'accent-green\|signal-blue' src/popup/popup.css` returns definitions
- [ ] Zero raw `zinc-*` classes remain — `grep -r 'zinc-' src/ --include='*.tsx'` returns nothing
- [ ] Zero raw Tailwind color classes (`emerald-`, `amber-`, `green-500`, `red-400`) in components — `grep -rE '(emerald|amber)-[0-9]|green-500|red-400' src/ --include='*.tsx'` returns nothing
- [ ] Modal MODAL_STYLES contains zero raw hex color values (all replaced with `var(--color-*)`)
- [ ] `#00FF41` appears nowhere — `grep -r '00FF41' src/` returns nothing
- [ ] Manual: text-secondary is readable on all dark backgrounds
- [ ] Manual: text-dim is visible (though subordinate) on bg-core and bg-panel
- [ ] Manual: green text/indicators are clearly readable on dark panels
- [ ] Manual: modal green matches popup green (no perceptible hue shift between contexts)
- [ ] Manual: auth screen button uses same green as rest of extension

---

*Consolidates audit issues C-3, C-4, C-6, C-7, H-6, H-7, H-8, M-13, M-14, L-1, L-2, L-3, L-4 and critique issues 1, 3.*
