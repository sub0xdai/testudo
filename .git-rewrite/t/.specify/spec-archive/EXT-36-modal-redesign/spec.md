# Specification: Align Trade Modal with Web App Design Language

**Spec ID:** EXT-36-modal-redesign
**Date:** 2026-03-17
**Status:** Draft
**Class:** Refactor / Design
**Priority:** P2 — visual consistency between modal and web app
**Depends on:** EXT-35-font-unification (fonts should be unified first)
**Series:** EXT-34 through EXT-36 (extension UX polish)

---

## Problem Statement

The trade confirmation modal (Alt+X on TradingView) uses a softer, rounded design language with muted colors (`#4ade80` green, `#ef4444` red, `16px` border-radius, glass-morphism panels). The web app uses a sharper, more brutalist aesthetic (`#00FF41` neon green, `#FF003C` neon red, `4-12px` border-radius, darker backgrounds, `Space Grotesk`/`Space Mono` fonts).

The modal should feel like it belongs to the same product family as the web app — sharper edges, more contrast, and matching signal colors.

### Current Modal vs Web App

| Property | Modal (current) | Web App | Target |
|----------|----------------|---------|--------|
| Sans font | DM Sans | Space Grotesk | Space Grotesk |
| Mono font | JetBrains Mono | Space Mono | Space Mono |
| Green | #4ade80 (soft) | #00FF41 (neon) | #00FF41 |
| Red | #ef4444 (soft) | #FF003C (neon) | #FF003C |
| Panel bg | rgba(21,25,33,0.95) | #0A0A0A | #0A0A0A/95% |
| Border radius | 16px | 4-12px | 8px (rounded-lg) |
| Border color | rgba(255,255,255,0.08) | #3F3F46 | #3F3F46 |
| Button style | rounded, subtle | sharp, high contrast | sharp, high contrast |

---

## User Stories

- **As a trader**, I want the Alt+X modal to look like part of the same product as the web app, so the experience feels unified.
- **As a trader**, I want clear, high-contrast buy/sell buttons on the modal, so I can quickly confirm trades.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Update modal CSS variables to use web app signal colors (#00FF41, #FF003C) | High | modal.tsx |
| FR-2 | Reduce panel border-radius from 16px to 8px | High | modal.tsx |
| FR-3 | Update panel background to match web app (#0A0A0A at 95% opacity) | High | modal.tsx |
| FR-4 | Update border color to match web app (#3F3F46) | Medium | modal.tsx |
| FR-5 | Update button styling — sharper corners, higher contrast, mono font | High | modal.tsx |
| FR-6 | Update toast styling to use neon signal colors | Medium | modal.tsx |
| FR-7 | Fonts updated via EXT-35 (Space Grotesk/Space Mono) | High | modal.tsx |

---

## Technical Implementation

### CSS Variable Updates (MODAL_STYLES)

```css
:host {
  --color-signal-green: #00FF41;
  --color-signal-red: #FF003C;
  --color-text-primary: #ffffff;
  --color-text-secondary: #888888;
  --color-text-dim: #555555;
  --color-bg-core: #050505;
  --color-bg-panel: #0A0A0A;
  --color-bg-elevated: #111111;
  --color-border: #3F3F46;
}
```

### Panel Styling

```css
.panel {
  background-color: rgba(10, 10, 10, 0.95);
  border: 1px solid #3F3F46;
  border-radius: 8px;
  padding: 22px 26px;
  box-shadow: 0 24px 48px rgba(0,0,0,0.5);
}
```

### Button Styling (Buy/Sell)

```css
.btn-buy {
  background: #00FF41;
  color: #050505;
  font-family: "Space Mono", monospace;
  font-weight: 700;
  border-radius: 6px;
  letter-spacing: 0.05em;
}

.btn-sell {
  background: #FF003C;
  color: #ffffff;
  font-family: "Space Mono", monospace;
  font-weight: 700;
  border-radius: 6px;
  letter-spacing: 0.05em;
}
```

### Toast Updates

```css
.toast.success {
  background: rgba(0, 255, 65, 0.1);
  color: #00FF41;
  border: 1px solid rgba(0, 255, 65, 0.3);
  border-radius: 6px;
}

.toast.error {
  background: rgba(255, 0, 60, 0.1);
  color: #FF003C;
  border: 1px solid rgba(255, 0, 60, 0.3);
  border-radius: 6px;
}
```

### Files

- `testudo-extension/src/modal.tsx` — update MODAL_STYLES CSS variables, panel, buttons, toasts
- `testudo-extension/src/components/TradeForm.tsx` — verify class names still apply correctly

---

## Acceptance Criteria

- [ ] Modal panel uses #0A0A0A background with 8px border-radius
- [ ] Signal green is #00FF41, signal red is #FF003C throughout modal
- [ ] Borders use #3F3F46 color
- [ ] Buy/sell buttons use mono font, sharp corners, high contrast
- [ ] Toast notifications use neon signal colors
- [ ] Fonts match web app (Space Grotesk/Space Mono) per EXT-35
- [ ] Modal still renders correctly in Shadow DOM on TradingView
- [ ] `bun run build` passes

---

## Risks

1. **TradingView CSS conflicts** — sharper design may interact differently with TradingView's dark theme. Mitigation: Shadow DOM isolates styles.
2. **Readability** — neon #00FF41 on dark backgrounds may be too bright for some users. Mitigation: use sparingly for accents, not large text blocks.

---

## Completion Signal

This spec is complete when:
1. Modal visually matches web app design language
2. Side-by-side comparison shows cohesive look
3. All acceptance criteria met
4. `bun run build` passes
5. Code committed to master
