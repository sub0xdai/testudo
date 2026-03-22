# Specification: Give Light Theme Atmospheric Parity with Dark

**Spec ID:** UXP-21-light-theme-parity
**Date:** 2026-03-22
**Status:** Draft
**Class:** Feature / Visual Design
**Priority:** P2 — Light theme is functional but personality-less compared to dark
**Depends on:** UXP-20-strip-glassmorphism (must resolve blur strategy first)
**Series:** UXP-19 through UXP-23 (Design critique remediation)

---

## Problem Statement

The landing page dark theme has three atmospheric effects that give it personality: a mouse-tracking spotlight (`SpotlightBackground.tsx`), CRT scan lines (`.scan-lines` class in `index.css`), and the Trajan column background photo. When the user switches to light theme, two of these are disabled:

1. **Spotlight disabled** (`SpotlightBackground.tsx` lines 54-57): light mode renders a flat `rgb(var(--bg-core) / 0.80)` wash instead of the radial-gradient mouse tracker.
2. **Scan lines hidden** (`SpotlightBackground.tsx` line 61): `!isLight && <div className="scan-lines" />` — conditionally excluded.

The warm cream palette (`#f5f0e8`) is pleasant but generic. It doesn't share DNA with the dark theme's brutalist identity. Light theme users get a fundamentally different (and lesser) experience — the page just looks like a normal website.

Additionally, the RainbowKit wallet button is hardcoded to `darkTheme()` in `testudo-web/src/main.tsx` (lines 19-23), so the wallet connect UI breaks the light theme completely.

The fix is to give light theme its own atmospheric treatment — not a copy of dark, but an equivalent that maintains the brutalist/tactical identity.

---

## User Stories

- **As a light-theme user**, I want the landing page to feel as intentionally designed as the dark version, so that the product feels premium regardless of my preference.
- **As a developer**, I want the RainbowKit theme to match the active app theme, so that third-party UI integrations don't break visual coherence.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Light theme has a subtle texture overlay replacing scan lines (e.g., paper grain noise, fine dot grid, or light horizontal rules) | High | SpotlightBackground.tsx or index.css |
| FR-2 | Light theme spotlight uses a desaturated warm-tone radial gradient that follows mouse (not disabled) | High | SpotlightBackground.tsx |
| FR-3 | Light theme borders are heavier (2px) to compensate for reduced contrast range | Medium | index.css or tailwind preset |
| FR-4 | RainbowKit theme switches dynamically based on active app theme | High | main.tsx |
| FR-5 | Light theme background image uses a lighter overlay that still reveals texture | Medium | SpotlightBackground.tsx |

---

## Technical Implementation

### Light Theme Texture Overlay

Replace the scan-lines conditional with a theme-aware texture:

```css
/* index.css — light theme texture alternative */
[data-theme="light"] .texture-overlay {
  background: repeating-linear-gradient(
    0deg,
    transparent,
    transparent 3px,
    rgb(var(--bg-core) / 0.06) 3px,
    rgb(var(--bg-core) / 0.06) 4px
  );
  pointer-events: none;
}
```

Or use an SVG noise pattern via CSS `url()` for a paper-grain effect.

### Light Theme Spotlight

```tsx
// SpotlightBackground.tsx — enable spotlight in light mode with warmer tones
background: isLight
  ? `radial-gradient(circle ${spotlightRadius}px at ${mousePos.x}px ${mousePos.y}px,
      rgb(var(--bg-core) / 0.70) 0%,
      rgb(var(--bg-core) / 0.85) 80%,
      rgb(var(--bg-core) / 0.92) 100%)`
  : `radial-gradient(circle ${spotlightRadius}px at ${mousePos.x}px ${mousePos.y}px,
      transparent 0%,
      rgb(var(--bg-core) / 0.85) 80%,
      rgb(var(--bg-core) / 0.95) 100%)`,
```

### RainbowKit Dynamic Theme

```tsx
// main.tsx — switch RainbowKit theme based on app theme
import { darkTheme, lightTheme } from '@rainbow-me/rainbowkit';

// Inside component or useMemo:
const rkTheme = useMemo(() => {
  const theme = localStorage.getItem('testudo-theme');
  return theme === 'light'
    ? lightTheme({ accentColor: '#146426', accentColorForeground: '#f5f0e8', borderRadius: 'none' })
    : darkTheme({ accentColor: '#00FF41', accentColorForeground: '#050505', borderRadius: 'none' });
}, [activeTheme]);
```

Note: `borderRadius: 'none'` aligns with the zero-radius aesthetic.

### Files

- `testudo-web/src/components/ui/SpotlightBackground.tsx` — enable light spotlight, texture overlay
- `testudo-web/src/index.css` — light theme texture class
- `testudo-web/src/main.tsx` — dynamic RainbowKit theme
- `testudo-web/src/components/ui/Header.tsx` — theme toggle may need to trigger RainbowKit re-render

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Light theme has a visible texture overlay (not blank/flat)
- [ ] Light theme spotlight tracks mouse cursor (not flat wash)
- [ ] RainbowKit wallet button uses light theme styling when app is in light mode
- [ ] RainbowKit uses `borderRadius: 'none'` in both themes
- [ ] Light theme borders are visually heavier than dark theme borders
- [ ] Background image is subtly visible through light theme overlay
- [ ] Visual identity is recognizably "Testudo" in both themes
- [ ] `cd testudo-web && bun run build` passes

---

## Risks

1. **RainbowKit re-render** — Changing theme dynamically may require RainbowKitProvider to re-mount. Mitigation: test with theme toggle; may need to lift theme state to provider level or use RainbowKit's built-in theme switching if available.
2. **Light spotlight subtlety** — A warm-tone spotlight may be too subtle on cream backgrounds. Mitigation: increase the opacity differential (0.70 center vs 0.92 edge) and test visually.
3. **Performance** — Mouse-tracking in both themes doubles the requestAnimationFrame usage. Mitigation: this is already running in dark mode with no issues; same code path applies.

---

## Completion Signal

This spec is complete when:
1. Light theme has atmospheric texture and spotlight effects
2. RainbowKit theme dynamically matches app theme
3. All acceptance criteria met
4. `bun run build` passes
5. Code committed to master
