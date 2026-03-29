# Specification: Onboarding UX Polish

**Spec ID:** DEPLOY-04-onboarding-polish
**Date:** 2026-03-28
**Status:** Complete
**Class:** UX Polish
**Priority:** P2 — Functional but rough. First impressions matter.
**Depends on:** DEPLOY-02-cross-origin-cookies (exchange dropdown fix)

---

## Problem Statement

The desk onboarding flow (Account page) needs visual polish to match the brutalist aesthetic of the landing page. Current issues:

1. **Stepper strip** across the top is visually heavy and not integrated with the page design
2. **Exchange form** appears as raw form fields without the translucent card treatment used elsewhere
3. **No exchange cards** visible yet (depends on DEPLOY-02 cookie fix)
4. **Step transitions** feel abrupt with no visual continuity

---

## Design Direction

Apply the same design language as the landing page and lock screen:
- Translucent cards (`bg-main-bg/75 backdrop-blur-md`) with thin borders
- Ghost labels (`// STEP_01`, `// EXCHANGE_CONFIG`)
- Monospace typography for labels, display font for descriptions
- `[ ACTION ]` button style matching pricing cards

---

## Implementation

### T1: Stepper redesign

Replace the horizontal strip with a more discreet vertical or inline indicator. Options:
- Vertical sidebar steps (desktop), collapsed to current-step-only on mobile
- Or: Remove stepper entirely, show context inline on each step card

### T2: Exchange selection card

Wrap the exchange selection form in a translucent card:
- Ghost label: `// SELECT_EXCHANGE`
- Exchange dropdown styled with dark bg, thin border
- Description text in `text-text-secondary`

### T3: Credential input card

When an exchange is selected, show a second card:
- Ghost label: `// API_CREDENTIALS`
- Input fields styled consistently (dark bg, thin border, monospace)
- OKX shows 3 fields (key, secret, passphrase), others show 2
- Hyperliquid shows the wallet connect flow instead

### T4: Exchange cards grid

After adding exchanges, show them in the same card format as the pricing page:
- Heartbeat indicator (green dot)
- Exchange name + type badge (CEX/DEX)
- Balance display
- Kebab menu (test, import, delete)
- Hover lift effect

### T5: Empty state

When no exchanges are added, show a single centered card:
- Ghost label: `// NO_EXCHANGES`
- "Add your first exchange to start trading"
- `[ ADD EXCHANGE ]` button

### T6: Verify

- Visual consistency with landing page
- All steps functional
- Mobile responsive
- Dark/light theme support

---

## Acceptance Criteria

- [ ] Stepper is visually discreet / removed
- [ ] Exchange selection uses translucent card treatment
- [ ] Credential inputs styled consistently
- [ ] Exchange cards match pricing page card style
- [ ] Empty state card when no exchanges
- [ ] `bun run build` passes
- [ ] Responsive on mobile
