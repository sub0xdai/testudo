# Specification: Frontend Web3 Wallet Connection

**Spec ID:** AW-03-frontend-wallet-connection
**Date:** 2026-03-16
**Status:** Draft
**Class:** Feature / Frontend
**Priority:** P0 — user-facing onboarding flow
**Depends on:** AW-01 (agent-key-generation), AW-02 (eip712-approval)
**Series:** AW-01 through AW-05 (Hyperliquid agent wallet authentication)

---

## Problem Statement

With AW-01 and AW-02 providing the backend agent wallet infrastructure, users need a frontend to actually connect their wallets and authorize agent keypairs. The current "Add Exchange Account" form only accepts API key + secret text inputs — there's no wallet connection flow.

The testudo-web app (React 18, Vite, Bun, Tailwind) needs a wagmi/viem integration that:
1. Connects to MetaMask/WalletConnect/etc. via a wallet modal
2. Calls the backend `init` endpoint to generate the agent keypair
3. Prompts the user to sign the EIP-712 typed data via `eth_signTypedData_v4`
4. Submits the signature to the backend `approve` endpoint
5. Shows success/failure state

The browser extension (Manifest V3, Solid.js) delegates wallet setup to the web app — no CSP/MV3 complications for wallet providers.

---

## User Stories

- **As a trader**, I want to click "Connect Wallet" when setting up Hyperliquid, so that I can authorize trading without pasting a private key.
- **As a trader**, I want to see my truncated wallet address in the account list, so that I know which wallet is linked.
- **As a developer**, I want the wallet flow isolated in a dedicated component with clear state machine, so that error recovery is straightforward.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | wagmi + viem + wallet modal library (RainbowKit or ConnectKit) integrated in testudo-web | High | Frontend |
| FR-2 | Selecting "Hyperliquid" exchange shows "Connect Wallet" button instead of API key/secret form | High | Frontend |
| FR-3 | Full flow: connect wallet → init → sign typed data → approve → success | High | Frontend |
| FR-4 | State machine with clear states: Idle → Connecting → InitAgent → Signing → Approving → Success/Error | High | Frontend |
| FR-5 | Error state shows retry button, not a dead end | Medium | Frontend |
| FR-6 | Account list shows truncated wallet address (e.g. `0x1234...abcd`) for agent-wallet accounts | Medium | Frontend |
| FR-7 | Extension delegates to web app for wallet setup (link/redirect, no embedded wallet provider) | Medium | Extension |
| FR-8 | wagmi config targets Arbitrum chain (for wallet connection context) | Medium | Frontend |

---

## Technical Implementation

### Frontend Flow State Machine

```
Idle
  │ user clicks "Connect Wallet"
  ▼
Connecting (wagmi useConnect)
  │ wallet connected, address available
  ▼
InitAgent (POST /api/v1/exchanges/agent-wallet/init { wallet_address })
  │ receives { account_id, agent_address }
  ▼
Signing (wagmi useSignTypedData → eth_signTypedData_v4)
  │ user signs in wallet popup
  ▼
Approving (POST /api/v1/exchanges/agent-wallet/approve { account_id, signature, nonce })
  │ backend submits to Hyperliquid, verifies
  ▼
Success ← account active, agent approved
  │
  └→ Error (at any step) → retry button returns to appropriate state
```

### WalletConnect Component

```tsx
// src/components/WalletConnect.tsx
type WalletFlowState =
  | { step: 'idle' }
  | { step: 'connecting' }
  | { step: 'init-agent'; address: string }
  | { step: 'signing'; accountId: string; agentAddress: string; typedData: any; nonce: number }
  | { step: 'approving'; accountId: string; signature: string; nonce: number }
  | { step: 'success'; accountId: string; agentAddress: string }
  | { step: 'error'; message: string; retryStep: string };
```

### wagmi Configuration

```tsx
// src/config/wagmi.ts
import { createConfig, http } from 'wagmi';
import { arbitrum } from 'wagmi/chains';

export const wagmiConfig = createConfig({
  chains: [arbitrum],
  transports: {
    [arbitrum.id]: http(),
  },
});
```

### API Client Methods

```typescript
// Added to src/api/client.ts
async function initAgentWallet(walletAddress: string): Promise<{
  account_id: string;
  agent_address: string;
}>

async function getApproveData(accountId: string): Promise<{
  typed_data: object;
  nonce: number;
  agent_address: string;
}>

async function approveAgent(accountId: string, signature: string, nonce: number): Promise<{
  success: boolean;
  agent_address: string;
  message: string;
}>
```

### Account Page Integration

In `AccountPage.tsx`, the exchange selection conditional:
- If `exchange_name === "hyperliquid"` → render `<WalletConnect />` component
- Otherwise → render existing API key/secret form

Account list entries with `auth_mode === "agent_wallet"` display:
- Truncated wallet address instead of "API Key configured"
- "Agent Wallet" badge

### Extension Delegation

The browser extension's settings/account page shows a link:
"Set up Hyperliquid → Open Testudo Web App" — opens testudo-web in a new tab.
Once the agent wallet is configured via the web app, the extension automatically picks it up via the shared JWT auth (same user account).

### Files

- **Create:** `testudo-web/src/components/WalletConnect.tsx` — full wallet connection component with state machine
- **Create:** `testudo-web/src/config/wagmi.ts` — wagmi configuration for Arbitrum
- **Modify:** `testudo-web/src/pages/AccountPage.tsx` — conditional wallet vs API key form, wallet address display
- **Modify:** `testudo-web/src/api/client.ts` — `initAgentWallet()`, `getApproveData()`, `approveAgent()` methods
- **Modify:** `testudo-web/src/validation/forms.ts` — conditional validation (no API key required for HL)
- **Modify:** `testudo-web/package.json` — add wagmi, viem, wallet modal dependency

### Dependencies Added

- `wagmi` — React hooks for Ethereum wallet interaction
- `viem` — TypeScript Ethereum library (wagmi peer dependency)
- `@rainbow-me/rainbowkit` — wallet connection modal UI (or `connectkit` as alternative)
- `@tanstack/react-query` — wagmi peer dependency (may already be present)

---

## Acceptance Criteria

- [ ] wagmi + viem + RainbowKit (or ConnectKit) installed and configured
- [ ] Selecting "Hyperliquid" shows "Connect Wallet" instead of API key form
- [ ] Wallet connection via MetaMask works (connect → address available)
- [ ] Init endpoint called with wallet address, agent keypair created
- [ ] EIP-712 signing prompt appears in wallet with correct typed data
- [ ] After signing, approval submitted and account activated
- [ ] Success state displayed with agent address
- [ ] Error states show descriptive messages and retry option
- [ ] Account list shows truncated wallet address for agent-wallet accounts
- [ ] Extension shows "Open Web App" link for Hyperliquid setup
- [ ] `cd testudo-web && bun run build` passes

---

## Risks

1. **Wallet provider CSP conflicts** — RainbowKit injects iframes/popups. Mitigation: testudo-web runs as regular web app (no MV3 CSP restrictions). Extension delegates entirely.
2. **wagmi bundle size** — wagmi + viem add ~50KB gzipped. Mitigation: acceptable for web app; extension doesn't include it.
3. **Multi-wallet UX** — users may have multiple browser wallets. Mitigation: RainbowKit handles wallet selection modal natively.
4. **Network mismatch** — user connected to wrong chain. Mitigation: wagmi `useSwitchChain` hook prompts chain switch if needed (Arbitrum required for context, though signing works on any chain).

---

## Completion Signal

This spec is complete when:
1. Full wallet → init → sign → approve flow works in browser
2. Account list correctly displays agent-wallet accounts
3. Extension delegates to web app
4. All acceptance criteria met
5. `cd testudo-web && bun run build` passes
6. Code committed to master
