# Specification: Recalibrate Signal Colors for Sustained Readability

**Spec ID:** UXP-22-signal-color-calibration
**Date:** 2026-03-22
**Status:** Draft
**Class:** Refactor / Visual Design
**Priority:** P1 — Signal colors are used in every trading interaction; readability directly impacts user safety
**Depends on:** None
**Series:** UXP-19 through UXP-23 (Design critique remediation)

---

## Problem Statement

The dark theme signal green is `#00FF41` (RGB 0, 255, 65) and signal red is `#FF003C` (RGB 255, 0, 60) — both at maximum channel saturation. These are defined in two locations:

- **Extension popup:** `testudo-extension/src/popup/popup.css` lines 41-44 (dark theme `@theme` block)
- **Extension modal:** `testudo-extension/src/modal.tsx` lines 55-56 (Shadow DOM `:host` variables), duplicated at lines 263-264 (toast theme)
- **Landing page:** `testudo-web/src/index.css` lines ~14-15 (dark theme CSS variables)

On AMOLED displays (which the theme is explicitly named for), maximum-saturation colors physically glow due to per-pixel illumination. At small text sizes (11-12px labels in the popup, used extensively in `StatusBar.tsx`, `PositionCard.tsx`, `ActiveOrders.tsx`, `ArcGauge.tsx`), the extreme brightness against `#0a0a0a` backgrounds creates halation — text appears to bleed outward, reducing legibility.

The light theme already applies this principle correctly: signal green becomes `#146426` and signal red becomes `#a00024` — dramatically reduced saturation that maintains semantic meaning. The dark theme should apply the same principle, dialing back from "retina-searing" to "vivid but readable."

The recalibration must be coordinated across both surfaces (extension + web) and all three definition sites (popup.css, modal.tsx inline styles, index.css) to maintain consistency.

---

## User Stories

- **As a trader**, I want profit/loss colors that are immediately recognizable but comfortable during extended sessions, so that I can trade for hours without eye strain.
- **As a user on an AMOLED device**, I want signal colors that don't cause halation at small text sizes, so that I can read precise financial data accurately.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Dark theme signal green reduced from `#00FF41` to a value in the range `#00E639`–`#22C55E` (maintain green identity, reduce max-channel saturation) | High | popup.css, modal.tsx, index.css |
| FR-2 | Dark theme signal red reduced from `#FF003C` to a value in the range `#EF4444`–`#F43F5E` (maintain red identity, reduce max-channel saturation) | High | popup.css, modal.tsx, index.css |
| FR-3 | All dim variants (rgba with 0.12 alpha) updated to match new base colors | High | popup.css |
| FR-4 | ArcGauge.tsx fallback hex values updated to match new colors | Medium | ArcGauge.tsx |
| FR-5 | `--color-accent-green` updated to match new signal green | Medium | popup.css |
| FR-6 | All three definition sites (popup.css, modal.tsx, index.css) use identical values | High | All |
| FR-7 | Light theme signal colors unchanged (already correct) | High | — |

---

## Technical Implementation

### Proposed Color Values

Recommended targets (to be validated visually during implementation):

| Token | Current | Proposed | Rationale |
|-------|---------|----------|-----------|
| `--color-signal-green` | `#00FF41` | `#22C55E` | Tailwind green-500. Saturation 72% vs 100%. Still vivid, no halation. |
| `--color-signal-red` | `#FF003C` | `#EF4444` | Tailwind red-500. Saturation 70% vs 100%. Unmistakably red. |
| `--color-signal-green-dim` | `rgba(0,255,65,0.12)` | `rgba(34,197,94,0.12)` | Matches new base |
| `--color-signal-red-dim` | `rgba(255,0,60,0.12)` | `rgba(239,68,68,0.12)` | Matches new base |
| `--color-accent-green` | `#00FF41` | `#22C55E` | Must match signal-green |

[CLARIFY] The exact values should be tested on an AMOLED screen or display emulator. The range `#00E639`–`#22C55E` for green and `#EF4444`–`#F43F5E` for red are the acceptable bounds. Pick values that maintain clear differentiation from the amber signal (`#f59e0b`).

### Update Sites

**1. Extension popup CSS** (`testudo-extension/src/popup/popup.css`):
```css
/* Lines 41-44: update @theme block */
--color-signal-green: #22C55E;
--color-signal-green-dim: rgba(34, 197, 94, 0.12);
--color-signal-red: #EF4444;
--color-signal-red-dim: rgba(239, 68, 68, 0.12);
/* Line ~48: update accent-green */
--color-accent-green: #22C55E;
```

**2. Extension modal inline styles** (`testudo-extension/src/modal.tsx`):
```css
/* Lines 55-56: :host block */
--color-signal-green: #22C55E;
--color-signal-red: #EF4444;

/* Lines 263-264: toast dark theme */
--color-signal-green: #22C55E;
--color-signal-red: #EF4444;
```

**3. Landing page CSS** (`testudo-web/src/index.css`):
```css
/* Dark theme block: update signal colors */
--signal-green: 34 197 94;    /* was: 0 255 65 */
--signal-red: 239 68 68;      /* was: 255 0 60 */
```

**4. ArcGauge fallbacks** (`testudo-extension/src/popup/components/ArcGauge.tsx`):
```tsx
// Line 25: update fallback
const green = getComputedStyle(el).getPropertyValue('--color-signal-green').trim() || '#22C55E';
// Line 27: update fallback
const red = getComputedStyle(el).getPropertyValue('--color-signal-red').trim() || '#EF4444';
```

**5. RainbowKit accent** (`testudo-web/src/main.tsx`):
```tsx
// Line 20: update accent color
accentColor: '#22C55E',  // was: '#00FF41'
```

### Files

- `testudo-extension/src/popup/popup.css` — primary signal color definitions (lines 41-44, ~48)
- `testudo-extension/src/modal.tsx` — Shadow DOM signal colors (lines 55-56, 263-264)
- `testudo-web/src/index.css` — landing page signal colors
- `testudo-extension/src/popup/components/ArcGauge.tsx` — fallback hex values (lines 25, 27)
- `testudo-web/src/main.tsx` — RainbowKit accent color (line 20)

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Dark theme signal green is in range `#00E639`–`#22C55E` (not `#00FF41`)
- [ ] Dark theme signal red is in range `#EF4444`–`#F43F5E` (not `#FF003C`)
- [ ] All three definition sites use identical color values
- [ ] Dim variants use matching rgba values
- [ ] ArcGauge fallback values match new colors
- [ ] Light theme signal colors unchanged (`#146426` green, `#a00024` red)
- [ ] Signal colors remain clearly distinguishable from amber (`#f59e0b`)
- [ ] `cd testudo-extension && bun run build` passes
- [ ] `cd testudo-web && bun run build` passes

---

## Risks

1. **Brand perception** — The neon green has become associated with the product. Reducing saturation may feel "less Testudo." Mitigation: the proposed colors are still vivid and unmistakably green/red; the change is from "retina burn" to "vivid" — a refinement, not a desaturation.
2. **Color sync drift** — Signal colors are defined in 3+ locations. Missing an update site creates inconsistency. Mitigation: grep for both old hex values (`#00FF41`, `#FF003C`) and their RGB equivalents (`0 255 65`, `255 0 60`) after changes to catch all occurrences.

---

## Completion Signal

This spec is complete when:
1. All signal colors updated across extension and web surfaces
2. Grep for old values (`#00FF41`, `#FF003C`, `0 255 65`, `255 0 60`) returns zero results
3. All acceptance criteria met
4. Both `bun run build` commands pass
5. Code committed to master
