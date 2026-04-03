# Specification: Desk Re-Authorization UX and Form Deduplication

**Spec ID:** UXA-02-desk-reauth-ux
**Date:** 2026-04-01
**Status:** Draft
**Class:** Feature / Frontend
**Priority:** P0 — Users have no path to fix broken agent wallets from the UI
**Depends on:** UXA-01-agent-wallet-visibility (backend must surface inactive accounts first)
**Series:** UXA-01 through UXA-03 (Agent Wallet Resilience)

---

## Problem Statement

When UXA-01 surfaces inactive agent wallets in the API, the desk frontend needs to render them meaningfully. Currently `ExchangeCard.tsx` has no concept of a degraded/inactive state — it renders every account identically with a green heartbeat dot. There is no "re-authorize" button, no visual warning, and no path to trigger the `WalletConnectFlow` for an existing account.

The Account page also has two independent implementations of the "add exchange" form: `OnboardingFlow.tsx` (first-time users) and an inline form in `Account.tsx:217-306` (subsequent additions). These diverge in styling (input padding, label size, container width), behavior (success confirmation vs. silent close), and security (OnboardingFlow uses `type="password"` for API keys; Account.tsx uses `type="text"`, leaking credentials to shoulder surfers). The `onMigrate` handler at `Account.tsx:206` is a dead TODO — the "Migrate to agent wallet" link does nothing when clicked.

---

## User Stories

- **As a trader**, I want to see a clear visual indicator when my agent wallet needs re-authorization, so that I can fix it without guessing.
- **As a trader**, I want to click a "Re-authorize" button on my exchange card and complete the MetaMask signing flow, so that I can restore trading in under 30 seconds.
- **As a trader**, I want the same quality of exchange setup experience whether I'm adding my first or fifth exchange, so that the product feels consistent.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `ExchangeCard` renders a degraded state when `requires_reauthorization = true`: amber/yellow border, pulsing warning indicator (replacing green heartbeat), "REAUTHORIZE" badge | High | ExchangeCard |
| FR-2 | Degraded `ExchangeCard` shows a prominent "REAUTHORIZE" button that opens `WalletConnectFlow` for the existing account | High | ExchangeCard |
| FR-3 | `WalletConnectFlow` accepts an optional `existingAccountId` prop to re-approve an existing agent wallet instead of creating a new one (skips `initAgentWallet`, goes directly to `getApproveData`) | High | WalletConnectFlow |
| FR-4 | After successful re-authorization, `ExchangeCard` transitions from degraded to active state (green heartbeat, reauthorization badge removed) | High | Account |
| FR-5 | Extract shared `AddExchangeForm` component from `OnboardingFlow` and `Account.tsx` inline form, eliminating duplication | Medium | Components |
| FR-6 | All API key inputs use `type="password"` (fix `Account.tsx` `type="text"` leak) | High | Account |
| FR-7 | Remove dead `onMigrate` TODO — either implement migration handler or hide the button | Medium | ExchangeCard |
| FR-8 | `[ BRACKET ]` notation applied consistently: primary action buttons use brackets, secondary/cancel buttons do not | Low | Styling |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | FR-1 + FR-2: ExchangeCard degraded state + reauth button | Visual indicator renders for inactive agent wallets |
| CP-2 | FR-3 + FR-4: WalletConnectFlow re-approve path + state transition | MetaMask re-signing flow works end-to-end |
| CP-3 | FR-5 + FR-6 + FR-7 + FR-8: Form extraction + cleanup | No duplicate forms, no security leak, no dead buttons |

### ExchangeCard Degraded State

```tsx
// ExchangeCard.tsx — new conditional rendering
const needsReauth = () => props.account.requires_reauthorization === true

return (
  <div class={`border ${
    needsReauth()
      ? 'border-signal-amber bg-signal-amber/5'
      : 'border-container-border bg-container-bg'
  } p-5 flex flex-col gap-4`}>
    {/* Header: heartbeat indicator changes */}
    <span class={`inline-block w-2.5 h-2.5 rounded-full ${
      needsReauth()
        ? 'bg-signal-amber animate-pulse'
        : props.account.is_active
          ? 'bg-signal-green animate-pulse'
          : 'bg-signal-red'
    }`} />

    {/* Reauth badge */}
    <Show when={needsReauth()}>
      <span class="text-[10px] text-signal-amber font-mono bg-signal-amber/10 px-2 py-0.5 border border-signal-amber/30">
        REAUTH REQUIRED
      </span>
    </Show>

    {/* Reauth button replaces balance when degraded */}
    <Show when={needsReauth()}>
      <button
        onClick={() => props.onReauthorize()}
        class="w-full py-3 border border-signal-amber text-signal-amber font-mono font-bold text-xs tracking-wider hover:bg-signal-amber hover:text-main-bg transition-colors"
      >
        [ REAUTHORIZE ]
      </button>
    </Show>
  </div>
)
```

### WalletConnectFlow Re-Approve Path

```tsx
// WalletConnectFlow.tsx — add optional prop
interface WalletConnectFlowProps {
  onComplete: () => void
  existingAccountId?: string  // NEW: skip init, go straight to approve-data
}

async function startFlow() {
  const address = getConnectedAddress()
  if (!address) return

  try {
    let account_id: string
    let agent_address: string

    if (props.existingAccountId) {
      // Re-approve path: skip init, go straight to approve-data
      account_id = props.existingAccountId
      const approveData = await exchangeApi.getApproveData(account_id)
      agent_address = approveData.agent_address
      // ... continue with signing
    } else {
      // Normal init path (existing behavior)
      const initResult = await exchangeApi.initAgentWallet(address)
      account_id = initResult.account_id
      agent_address = initResult.agent_address
      // ... continue with approve-data + signing
    }
  } catch (err) { /* ... */ }
}
```

### AddExchangeForm Extraction

```tsx
// New file: components/account/AddExchangeForm.tsx
interface AddExchangeFormProps {
  exchanges: ExchangeInfo[]
  onSuccess: () => void
  onCancel?: () => void
}

export function AddExchangeForm(props: AddExchangeFormProps) {
  // Shared state: selectedExchange, apiKey, apiSecret, passphrase, error, submitting
  // Shared rendering: exchange dropdown, conditional WalletConnectFlow, API key/secret inputs
  // Shared submission: exchangeApi.addAccount()
  // Styling: consistent padding (px-4 py-3), type="password" for all credential fields
}
```

Replace both:
- `OnboardingFlow.tsx` form section → `<AddExchangeForm exchanges={...} onSuccess={...} />`
- `Account.tsx:217-306` inline form → `<AddExchangeForm exchanges={...} onSuccess={...} onCancel={...} />`

### Account Page Re-Auth Integration

```tsx
// Account.tsx — add reauth handler + modal state
const [reauthAccountId, setReauthAccountId] = createSignal<string | null>(null)

// In ExchangeCard rendering:
<ExchangeCard
  account={acc}
  onReauthorize={() => setReauthAccountId(acc.id)}
  // ... other props
/>

// Reauth modal (reuses WalletConnectFlow)
<Show when={reauthAccountId()}>
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
    <div class="border border-container-border bg-main-bg/95 backdrop-blur-md p-8 max-w-lg w-full">
      <WalletConnectFlow
        existingAccountId={reauthAccountId()!}
        onComplete={() => { setReauthAccountId(null); refetchAccounts() }}
      />
    </div>
  </div>
</Show>
```

### Paved Roads

- `WalletConnectFlow.tsx` — existing 4-step state machine. The re-approve path reuses steps 2-4, skipping step 1 (init).
- `ExchangeCard.tsx` — existing card component with heartbeat indicator and kebab menu.
- `OnboardingFlow.tsx` — existing onboarding with exchange dropdown + WalletConnectFlow integration.
- `signal-amber` — existing Tailwind token already used for migration prompts in `ExchangeCard.tsx:211`.

### Files

- `testudo-journal/src/components/account/ExchangeCard.tsx` — add degraded state, reauth button, `onReauthorize` prop
- `testudo-journal/src/components/account/WalletConnectFlow.tsx` — add `existingAccountId` prop, skip-init path
- `testudo-journal/src/components/account/AddExchangeForm.tsx` — **new file**, extracted shared form
- `testudo-journal/src/components/account/OnboardingFlow.tsx` — replace inline form with `AddExchangeForm`
- `testudo-journal/src/pages/Account.tsx` — replace inline form with `AddExchangeForm`, add reauth modal, remove dead `onMigrate` TODO

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Inactive agent wallet accounts render with amber border, amber heartbeat, and "REAUTH REQUIRED" badge
- [ ] Clicking "REAUTHORIZE" opens WalletConnectFlow and prompts MetaMask for EIP-712 signature
- [ ] After successful re-authorization, card transitions to green/active state without page reload
- [ ] `WalletConnectFlow` with `existingAccountId` skips init and goes directly to approve-data
- [ ] `AddExchangeForm` is used by both onboarding and account management — no duplicate form code
- [ ] All API key/secret inputs are `type="password"`
- [ ] "Migrate to agent wallet" link either works or is removed
- [ ] `bun run build` passes in testudo-journal

---

## Risks

1. **Backend must return `requires_reauthorization` first** — This spec depends on UXA-01. If backend API doesn't include the field, the frontend has no signal to render degraded state. Mitigation: UXA-01 is P0 and implemented first in the series.
2. **Re-approve flow assumes existing agent key is decryptable** — If the stored agent key can't be decrypted (vault key rotation), `getApproveData` will fail. Mitigation: `WalletConnectFlow` already handles errors with retry button; failed decrypt falls back to full init flow.
3. **Modal re-auth interrupts account page context** — Opening WalletConnectFlow in a modal on the Account page requires MetaMask interaction. Mitigation: Use overlay modal (not full-page redirect) so user retains context. WalletConnectFlow already handles its own error/retry states.

---

## Completion Signal

This spec is complete when:
1. Degraded ExchangeCard renders for inactive agent wallets with reauth button
2. Re-authorization flow works end-to-end (click → MetaMask → active)
3. Duplicate add-exchange forms consolidated into single component
4. All acceptance criteria met
5. `bun run build` passes
6. Code committed to master
