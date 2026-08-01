# Specification: Unify Extension Fonts with Web App

**Spec ID:** EXT-35-font-unification
**Date:** 2026-03-17
**Status:** Draft
**Class:** Refactor / Design
**Priority:** P2 — visual consistency between extension and web app
**Depends on:** None
**Series:** EXT-34 through EXT-36 (extension UX polish)

---

## Problem Statement

The extension popup uses **DM Sans** (sans) and **JetBrains Mono** (mono), while the web app uses **Space Grotesk** (display) and **Space Mono** (mono). This creates a visual disconnect — the extension feels like a separate product rather than part of the same platform.

The web app's font choices (Space Grotesk + Space Mono) are more distinctive and aligned with the brutalist/technical brand. The extension should adopt these fonts for a cohesive experience.

Current state:
- Extension: `DM Sans` (sans), `JetBrains Mono` (mono) — self-hosted WOFF2
- Web app: `Space Grotesk` (display/sans), `Space Mono` (mono) — Google Fonts
- Modal (Shadow DOM): inherits extension fonts via injected @font-face

---

## User Stories

- **As a user**, I want the extension to feel like part of the same product as the web app, so that the experience is cohesive.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Replace DM Sans with Space Grotesk in popup CSS | High | popup.css |
| FR-2 | Replace JetBrains Mono with Space Mono in popup CSS | High | popup.css |
| FR-3 | Self-host Space Grotesk and Space Mono as WOFF2 (no Google Fonts CDN in extension) | High | public/popup/fonts/ |
| FR-4 | Update modal.tsx MODAL_STYLES to use Space Grotesk/Space Mono | High | modal.tsx |
| FR-5 | Update Tailwind @theme font-family variables | Medium | popup.css |
| FR-6 | Verify all text renders correctly at existing font sizes | Medium | All components |

---

## Technical Implementation

### Font Files

Download Space Grotesk (400, 500, 600, 700) and Space Mono (400, 700) as WOFF2 variable fonts. Place in `testudo-extension/public/popup/fonts/`.

### popup.css Changes

```css
@font-face {
  font-family: "Space Grotesk";
  src: url("fonts/SpaceGrotesk-Variable.woff2") format("woff2");
  font-weight: 400 700;
  font-display: swap;
}

@font-face {
  font-family: "Space Mono";
  src: url("fonts/SpaceMono-Regular.woff2") format("woff2");
  font-weight: 400;
  font-display: swap;
}

@font-face {
  font-family: "Space Mono";
  src: url("fonts/SpaceMono-Bold.woff2") format("woff2");
  font-weight: 700;
  font-display: swap;
}

@theme {
  --font-family-sans: "Space Grotesk", system-ui, -apple-system, sans-serif;
  --font-family-mono: "Space Mono", ui-monospace, monospace;
}
```

### modal.tsx Changes

Update MODAL_STYLES `@font-face` declarations and `font-family` references to use Space Grotesk and Space Mono. The modal injects font faces into Shadow DOM, so the same WOFF2 files must be referenced via `chrome.runtime.getURL()`.

### Files

- `testudo-extension/public/popup/fonts/` — replace DM Sans/JetBrains Mono with Space Grotesk/Space Mono WOFF2
- `testudo-extension/src/popup/popup.css` — update @font-face and @theme declarations
- `testudo-extension/src/modal.tsx` — update MODAL_STYLES font references

---

## Acceptance Criteria

- [ ] Extension popup renders in Space Grotesk (sans) and Space Mono (mono)
- [ ] Modal on TradingView renders in Space Grotesk/Space Mono
- [ ] No FOUT (flash of unstyled text) — fonts load from local WOFF2
- [ ] All existing text sizes and weights render correctly
- [ ] Old DM Sans and JetBrains Mono font files removed
- [ ] `bun run build` passes
- [ ] Extension bundle size does not increase significantly (Space fonts are similar size)

---

## Risks

1. **Glyph coverage** — Space Grotesk may not cover all characters DM Sans does. Mitigation: system-ui fallback in font stack.
2. **Metric differences** — Space Grotesk has different metrics than DM Sans, potentially breaking layout at tight sizes. Mitigation: visual review of all popup views.

---

## Completion Signal

This spec is complete when:
1. Extension uses Space Grotesk + Space Mono everywhere
2. Visual consistency confirmed between extension and web app
3. All acceptance criteria met
4. `bun run build` passes
5. Code committed to master
