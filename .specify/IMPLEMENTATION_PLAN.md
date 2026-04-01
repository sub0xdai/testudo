# Implementation Plan

> Last updated: 2026-04-01
> Current spec: UXA-02-desk-reauth-ux
> Phase: COMPLETE

---

## Active Spec: UXA-02-desk-reauth-ux

Desk re-authorization UX: degraded ExchangeCard state, WalletConnectFlow re-approve path, form deduplication.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | CP-1: Add `requires_reauthorization` to frontend `ExchangeAccount` type + ExchangeCard degraded state + reauth button + `onReauthorize` prop (FR-1, FR-2) | complete | medium | — |
| T2 | CP-2: WalletConnectFlow `existingAccountId` prop (skip init, go to approve-data) + Account.tsx reauth modal + state transition on success (FR-3, FR-4) | complete | medium | T1 |
| T3 | CP-3: Extract `AddExchangeForm` from OnboardingFlow + Account.tsx inline form + fix `type="text"` security leak + remove dead `onMigrate` TODO + bracket consistency (FR-5, FR-6, FR-7, FR-8) | complete | medium | — |
| T4 | Validate: `bun run build` in testudo-journal, commit | complete | low | T1, T2, T3 |

### Key Decisions

- `AddExchangeForm` accepts `initialExchange` prop so Account.tsx can pre-select Hyperliquid for migration flow
- WalletConnectFlow re-auth path uses 3-step progress bar (Connect, Sign, Approve) — skipping Initialize step
- Re-auth button uses amber styling (border-signal-amber) to match degraded card state; normal authorize uses text-primary with glow-pulse
- OnboardingFlow simplified from multi-step state machine to binary success/form state — AddExchangeForm handles all form logic internally
- Dead `onMigrate` TODO replaced with working handler that opens form pre-selected to Hyperliquid

### Discoveries

- Account.tsx API key input used `type="text"` while OnboardingFlow used `type="password"` — security leak fixed by shared AddExchangeForm using `type="password"` for all credential fields
- Account.tsx had unused signals: `showWalletConnect`, `formApiKey`, `formSecret`, `formPassphrase`, `formSubmitting`, `needsPassphrase` — all removed with form extraction
- OnboardingFlow's `handleWalletComplete` constructed a fake ExchangeAccount object — simplified to just signal success state

---

## Completed Specs

- UX-01-pair-page (COMPLETE)
- UX-02-overview-polish (COMPLETE)
- REL-02-hl-journal-pipeline (COMPLETE)
- REL-03-hl-group-reconciliation (COMPLETE)
- CON-01a-daily-stats-regression (COMPLETE)
- UXA-01-agent-wallet-visibility (COMPLETE)
