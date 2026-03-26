# Specification: Dynamic Stepper Onboarding Flow

**Spec ID:** ONBOARD-01-stepper-onboarding
**Date:** 2026-03-26
**Status:** Draft
**Class:** Feature / Frontend UX
**Priority:** P1 — The Desk currently has no guided path from first connection to populated dashboard. Users must discover the correct sequence on their own. This stepper makes the activation path explicit and highlights the next action.
**Depends on:** DESK-01-unified-dashboard, HIST-01-exchange-history-import
**Series:** ONBOARD-01 (standalone)

---

## Problem Statement

New users who connect their wallet to the Desk land on an empty dashboard with no guidance on what to do next. The correct activation sequence is: connect wallet → add exchange API keys → import history → pair extension. But nothing in the UI communicates this order, and each step lives in a different tab or page.

The onboarding needs to be brief, first-time only, and non-intrusive — a persistent stepper bar at the top of the Desk that highlights the next incomplete step and collapses after completion. A static marketing version of the same stepper appears on the landing page to preview the journey before sign-up.

This is a lightweight UX component, not a wizard. It reads existing state (auth, credentials, trades, pairing) and renders accordingly. No new backend endpoints required — all detection uses existing API responses.

---

## User Stories

- **As a new user**, I want to see a clear numbered sequence of setup steps, so that I know exactly what to do next.
- **As a returning user**, I want the stepper to remember my progress and highlight the next incomplete step, so that I can resume where I left off.
- **As a user who has completed all steps**, I want the stepper to disappear, so that it doesn't waste screen space.
- **As a landing page visitor**, I want to see the onboarding journey previewed, so that I understand what setup involves before committing.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Create a `Stepper` component with 4 steps: (1) Connect Wallet, (2) Add Exchanges, (3) Import History, (4) Pair Extension. Each step shows a number, label, and status (complete/active/pending). | High | testudo-journal |
| FR-2 | The active step (next incomplete) is visually highlighted. Completed steps show a checkmark. Pending steps are dimmed. | High | testudo-journal |
| FR-3 | Step completion detection: (1) `isAuthenticated` signal from AuthContext, (2) user has ≥1 exchange credential (existing `/exchanges` response), (3) user has ≥1 imported trade (`source != 'testudo'` or any `journal_trades` row), (4) user has an active extension pairing. | High | testudo-journal |
| FR-4 | Clicking an active or pending step navigates to the relevant Desk section: step 1 triggers wallet connect modal, step 2 navigates to Account tab, step 3 triggers import (auto on exchange add per HIST-01), step 4 shows pairing instructions with install link as helper text. | Medium | testudo-journal |
| FR-5 | The stepper renders as a horizontal bar at the top of the Desk content area, below the header. It does not replace or overlap the navigation tabs. | High | testudo-journal |
| FR-6 | When all 4 steps are complete, the stepper collapses with a brief "Setup complete" message, then hides permanently. Completion state stored in `localStorage`. | High | testudo-journal |
| FR-7 | The stepper only appears for users who have not completed all steps. Users who completed onboarding before ONBOARD-01 was deployed do not see it (detect via existing data — if they have trades and a paired extension, they're done). | Medium | testudo-journal |
| FR-8 | A static (non-interactive) version of the stepper appears on the landing page with step 1 highlighted. This is a marketing preview — no state detection, no auth required. | Medium | testudo-web / Astro |
| FR-9 | Step 4 (Pair Extension) includes a small helper link: "Don't have the extension? Install from Chrome Web Store" — not a separate step, just inline guidance. | Medium | testudo-journal |

---

## Technical Implementation

### Step Detection Logic (Solid.js)

```typescript
function useOnboardingState() {
  const { isAuthenticated } = useAuth();
  const [exchanges] = createResource(fetchExchanges);
  const [trades] = createResource(fetchTrades);
  const [pairing] = createResource(fetchPairingStatus);

  const steps = () => [
    { label: "Connect Wallet",  complete: isAuthenticated() },
    { label: "Add Exchanges",   complete: (exchanges()?.length ?? 0) > 0 },
    { label: "Import History",  complete: (trades()?.total ?? 0) > 0 },
    { label: "Pair Extension",  complete: pairing()?.is_paired ?? false },
  ];

  const activeStep = () => steps().findIndex(s => !s.complete);
  const allComplete = () => steps().every(s => s.complete);

  return { steps, activeStep, allComplete };
}
```

### Stepper Component

Horizontal bar with 4 numbered circles connected by lines. Responsive: on narrow screens, circles only (labels in tooltip). Matches existing brutalist dark theme — sharp borders, monospace labels, green/amber/dim states.

```
┌─────────────────────────────────────────────────────────────┐
│  ① Connect Wallet  ——  ② Add Exchanges  ——  ③ Import  ——  ④ Pair Extension  │
│       ✓                    ● ACTIVE            ○               ○             │
└─────────────────────────────────────────────────────────────┘
```

### Landing Page Version

Static JSX/HTML. Always renders step 1 as active. No signals, no resources, no auth. Purely visual — shows prospective users what setup looks like.

### Files

**New files:**
- `testudo-journal/src/components/onboarding/Stepper.tsx` — Stepper component
- `testudo-journal/src/components/onboarding/useOnboardingState.ts` — Step detection hook

**Modified files:**
- `testudo-journal/src/components/Layout.tsx` — Mount Stepper above content area
- `testudo-journal/src/api/client.ts` — Add `fetchPairingStatus()` if not already present

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Stepper renders with 4 numbered steps on the Desk
- [ ] Active step is visually distinct (highlighted border/color)
- [ ] Completed steps show checkmark, pending steps are dimmed
- [ ] Step detection correctly reads auth, exchange, trade, and pairing state
- [ ] Clicking active step navigates to relevant section
- [ ] Stepper hides permanently after all steps complete
- [ ] Existing users with complete data do not see the stepper
- [ ] Static stepper renders on landing page with step 1 highlighted
- [ ] `bun run build` passes for testudo-journal

---

## Risks

1. **State detection race conditions** — Resources may load at different speeds, causing the stepper to flash between states. Mitigation: Show stepper only after all resources have resolved (loading state shows skeleton bar).
2. **Import detection timing** — Import runs async (HIST-01). Step 3 may not immediately reflect completion. Mitigation: WebSocket `import_complete` event triggers resource refetch.

---

## Completion Signal

This spec is complete when:
1. New users see the stepper guiding them through the 4-step activation flow
2. Each step correctly detects completion from existing API state
3. The stepper disappears after full completion and does not reappear
4. Landing page shows static preview stepper
5. All acceptance criteria met
6. `bun run build` passes
7. Code committed to master
