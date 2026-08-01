# Specification: Remove Glassmorphism from Landing Page

**Spec ID:** UXP-20-strip-glassmorphism
**Date:** 2026-03-22
**Status:** Draft
**Class:** Refactor / Visual Design
**Priority:** P1 — Glass effects contradict the brutalist aesthetic and create tonal mismatch between landing page and product
**Depends on:** None
**Series:** UXP-19 through UXP-23 (Design critique remediation)

---

## Problem Statement

The Testudo extension commits fully to zero-radius, solid-background, border-driven UI. The landing page hedges with translucent blur effects on nearly every surface:

| File | Line | Effect | Context |
|------|------|--------|---------|
| `testudo-web/src/components/ui/Header.tsx` | 40 | `backdrop-blur-sm` | Fixed header with `bg-main-bg/60` |
| `testudo-web/src/components/ui/Card.tsx` | 22 | `backdrop-blur-md` | Glass variant with `bg-main-bg/80` |
| `testudo-web/src/components/sections/Features.tsx` | 35 | `backdrop-blur-sm` | Feature cards with `bg-main-bg/90` |
| `testudo-web/src/components/sections/Hero.tsx` | 39 | `backdrop-blur-sm` | Data ticker with `bg-main-bg/60` |
| `testudo-web/src/components/sections/Pricing.tsx` | 62 | `backdrop-blur-sm` | Pricing cards with `bg-main-bg/90` |

This creates a tonal mismatch: the product (extension) feels raw and tactical, but the marketing page (landing) feels polished and safe. A visitor's first impression doesn't prepare them for the actual product aesthetic. Glass effects also compete visually with the scan lines and spotlight — both already provide depth and atmosphere.

The fix is to remove `backdrop-blur` from all card/section surfaces and increase background opacity to 95-100%. Keep blur only where it's functionally justified (modal backdrop in extension, not landing page). The scan lines and spotlight already create sufficient visual depth.

---

## User Stories

- **As a visitor**, I want the landing page to feel like the product it's selling, so that my expectations match the actual experience.
- **As a designer**, I want a single coherent visual language across marketing and product surfaces, so that the brand identity is clear.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Remove `backdrop-blur-sm` from Features.tsx cards | High | Features.tsx |
| FR-2 | Remove `backdrop-blur-sm` from Pricing.tsx cards | High | Pricing.tsx |
| FR-3 | Remove `backdrop-blur-sm` from Hero.tsx data ticker | High | Hero.tsx |
| FR-4 | Remove `backdrop-blur-md` from Card.tsx glass variant (replace with solid at 95% opacity) | High | Card.tsx |
| FR-5 | Remove or reduce `backdrop-blur-sm` from Header.tsx (increase bg opacity to 90%+) | Medium | Header.tsx |
| FR-6 | Increase background opacity on all affected elements to 95% minimum | High | All above |

---

## Technical Implementation

### Card.tsx Changes

```tsx
// Before (line 22)
const variantStyles = {
  solid: 'bg-main-bg/95',
  glass: 'bg-main-bg/80 backdrop-blur-md',
}

// After
const variantStyles = {
  solid: 'bg-main-bg/95',
  glass: 'bg-main-bg/95',  // Drop blur, match solid opacity
}
```

Consider removing the `glass` variant entirely if no callers differentiate behavior.

### Header.tsx Changes

```tsx
// Before (line 40)
className="... bg-main-bg/60 backdrop-blur-sm ..."

// After — solid at high opacity
className="... bg-main-bg/90 ..."
```

### Section Component Changes

Replace `bg-main-bg/90 backdrop-blur-sm` with `bg-main-bg/95` in:
- `Features.tsx` line 35
- `Pricing.tsx` line 62
- `Hero.tsx` line 39

### Files

- `testudo-web/src/components/ui/Card.tsx` — remove glass variant blur
- `testudo-web/src/components/ui/Header.tsx` — remove header blur, increase opacity
- `testudo-web/src/components/sections/Features.tsx` — remove card blur
- `testudo-web/src/components/sections/Pricing.tsx` — remove card blur
- `testudo-web/src/components/sections/Hero.tsx` — remove ticker blur

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] No `backdrop-blur` classes remain in landing page section components
- [ ] Card.tsx glass variant uses solid background (no blur)
- [ ] Header uses >=90% opacity background without blur
- [ ] All section cards use >=95% opacity background
- [ ] Scan lines and spotlight remain the sole atmospheric depth effects
- [ ] Visual appearance is cohesive with extension popup aesthetic
- [ ] `cd testudo-web && bun run build` passes

---

## Risks

1. **Spotlight background visibility** — Higher opacity cards may obscure the spotlight effect. Mitigation: test with spotlight active; the spotlight operates on a layer beneath cards, so this should be minimal.
2. **Header readability on scroll** — Removing blur from the fixed header means content scrolling behind it may be more visible. Mitigation: 90% opacity should be sufficient; test with scrolled content.

---

## Completion Signal

This spec is complete when:
1. All `backdrop-blur` removed from landing page components
2. Background opacities increased to 90-95%
3. All acceptance criteria met
4. `bun run build` passes
5. Code committed to master
