# Specification: Unified Desk Dashboard — Wallet Auth + Account Management

**Spec ID:** DESK-01-unified-dashboard
**Date:** 2026-03-25
**Status:** Draft
**Class:** Feature / Architecture
**Priority:** P0 — The Desk becomes the single authenticated surface for all Testudo functionality. Currently split across two apps (React testudo-web for account management, Solid.js testudo-journal for analytics) forcing users to bounce between separate shells.
**Depends on:** None (first in series)
**Series:** DESK-01 through DESK-02 (unified dashboard migration)

---

## Problem Statement

Testudo's authenticated functionality is split across two separate applications built in different frameworks. The AccountPage (React, testudo-web, port 3001) handles exchange management, extension pairing, and agent wallet setup. The Desk dashboard (Solid.js, testudo-journal, port 3002, served at `/desk/`) handles trade analytics, charts, and journal entries. Users must navigate between two separate shells with their own headers, navigation, and auth awareness — the web Header links to `/desk/` (opens new tab), and the Desk Layout links HOME back to `/`.

This creates three UX problems: (1) traders lose context when bouncing between apps to manage exchanges vs. view analytics, (2) the wallet connection and SIWE authentication live in testudo-web while the primary app experience lives in testudo-journal, forcing users through a separate "gateway" before reaching the Desk, and (3) two separate auth-aware surfaces with their own headers, theme toggles, and wallet chips duplicate code and create inconsistency.

The fix: make the Desk the single authenticated dApp. Move wallet connection (via Reown AppKit / Web3Modal), SIWE authentication, exchange management, extension pairing, and agent wallet setup into testudo-journal. The Desk becomes a persistent terminal with three states: LOCKED (no wallet), CONNECTING (SIWE in progress), and ACTIVE (full dashboard). The AccountPage content becomes a fourth tab in the Desk navigation.

---

## User Stories

- **As a trader**, I want to manage my exchange connections and view my analytics in the same interface, so that I don't lose context switching between apps.
- **As a new user**, I want to connect my wallet directly in the Desk, so that I can start using the app without visiting a separate account page first.
- **As a trader**, I want to pair my browser extension from within the Desk, so that I can complete setup in one place.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add `@reown/appkit` and `@reown/appkit-adapter-wagmi` (or vanilla JS adapter) to testudo-journal. Configure with WalletConnect project ID, Arbitrum chain, and app metadata. | High | Auth infra |
| FR-2 | Create a Solid.js `AuthProvider` context that manages the SIWE lifecycle: wallet connect → nonce → sign → verify → cookie session. Port logic from testudo-web's `AuthContext.tsx` using Solid.js primitives (`createSignal`, `createEffect`, refs). Expose: `user`, `isAuthenticated`, `loading`, `siweError`, `logout`, `connectWallet`. | High | Auth |
| FR-3 | Implement three Desk states in `Layout.tsx`: **LOCKED** (connect prompt overlay when no wallet), **CONNECTING** (spinner with "VERIFYING WALLET..." during SIWE), **ACTIVE** (full dashboard with all tabs). The dashboard shell (header, background) always renders — only the content area is gated. | High | Layout |
| FR-4 | Add wallet chip to Desk header. When connected: truncated address with green pulse dot, click opens dropdown with DISCONNECT. When disconnected: "CONNECT" button that triggers Reown AppKit modal. | High | Layout |
| FR-5 | Add `/desk/account` route rendering an Account page with three sections: exchange card grid, extension pairing banner, and onboarding flow (conditional). | High | Account |
| FR-6 | Port `ExchangeCard` from React to Solid.js. Preserve: heartbeat dot, CEX/DEX badge, balance display, kebab menu with test/delete/revoke actions, inline confirmation for destructive actions. | High | Account |
| FR-7 | Port `AddExchangeCard` ghost card from React to Solid.js. On click, show inline form for API key entry with exchange type selector and conditional passphrase field (OKX, KuCoin). | High | Account |
| FR-8 | Port `ExtensionPairingBanner` from React to Solid.js. Preserve: 6-digit code display, click-to-copy, countdown timer (5 min TTL), "NEW CODE" regeneration, "EXTENSION LINKED" success state. | High | Account |
| FR-9 | Port `WalletConnect` agent wallet flow from React to Solid.js. State machine: idle → connecting → init-agent → signing → approving → success → error. Use `window.ethereum.request('eth_signTypedData_v4')` for EIP-712 signing (not wagmi hooks — framework-agnostic). | High | Account |
| FR-10 | Port onboarding flow: when user has zero exchange accounts, show step-by-step setup (select exchange → enter credentials → test connection) instead of the card grid. Transition to normal view after first account added. | Medium | Account |
| FR-11 | Add exchange management API methods to testudo-journal's `api/client.ts`: `listExchanges`, `listAccounts`, `addAccount`, `deleteAccount`, `testConnection`, `fetchBalance`, `initAgentWallet`, `getApproveData`, `approveAgent`, `migrateToAgentWallet`, `revokeAgent`, `pairExtension`. All use `fetchWithCredentials` (cookie auth). | High | API |
| FR-12 | Update Desk nav from `OVERVIEW · TRADES · JOURNAL` to `OVERVIEW · TRADES · JOURNAL · ACCOUNT`. Active state highlighting matches existing pattern. | Medium | Layout |
| FR-13 | SIWE failure handling: if user rejects signature or nonce fails, show error message on the LOCKED screen with "TRY AGAIN" button. Do not redirect away from the Desk. | Medium | Auth |

---

## Technical Implementation

### Auth Provider (Solid.js)

Port of `testudo-web/src/context/AuthContext.tsx` using Solid.js reactivity:

```typescript
// testudo-journal/src/context/AuthContext.tsx
import { createContext, useContext, createSignal, createEffect, onCleanup } from 'solid-js'
import type { JSX } from 'solid-js'

interface AuthContextValue {
  user: () => User | null
  isAuthenticated: () => boolean
  loading: () => boolean
  siweError: () => string | null
  logout: () => Promise<void>
  connectWallet: () => void  // triggers Reown AppKit modal
}

// SIWE flow mirrors testudo-web exactly:
// 1. GET /api/v1/auth/nonce
// 2. User signs EIP-191 message (via Reown AppKit's provider)
// 3. POST /api/v1/auth/verify-siwe { message, signature }
// 4. Backend sets HttpOnly cookie
// 5. createSignal(user) set on success
```

### Reown AppKit Integration

```typescript
// testudo-journal/src/config/wallet.ts
import { createAppKit } from '@reown/appkit'
import { arbitrum } from '@reown/appkit/networks'

const projectId = import.meta.env.VITE_WALLETCONNECT_PROJECT_ID || ''

export const appKit = createAppKit({
  projectId,
  networks: [arbitrum],
  metadata: {
    name: 'Testudo',
    description: 'Automated risk management for crypto trading',
    url: window.location.origin,
    icons: ['/testudo-icon.png'],
  },
  themeMode: 'dark',
})
```

### Desk State Machine (Layout.tsx)

```typescript
// In Layout.tsx render:
<Show when={!auth.loading()} fallback={<LoadingScreen />}>
  <Show when={auth.isAuthenticated()} fallback={
    <Show when={!auth.siweError()} fallback={<ErrorOverlay onRetry={auth.connectWallet} />}>
      <LockScreen onConnect={auth.connectWallet} />
    </Show>
  }>
    {props.children}
  </Show>
</Show>
```

### Exchange API Methods

Add to `testudo-journal/src/api/client.ts` — identical endpoints to testudo-web, using `fetchWithCredentials`:

| Method | Endpoint | HTTP |
|--------|----------|------|
| `listExchanges()` | `/api/v1/exchanges` | GET |
| `listAccounts()` | `/api/v1/exchanges/accounts` | GET |
| `addAccount(payload)` | `/api/v1/exchanges/accounts` | POST |
| `deleteAccount(id)` | `/api/v1/exchanges/accounts/${id}` | DELETE |
| `testConnection(id)` | `/api/v1/exchanges/accounts/${id}/test` | POST |
| `fetchBalance(id)` | `/api/v1/exchanges/accounts/${id}/balance` | GET |
| `initAgentWallet(addr)` | `/api/v1/exchanges/agent-wallet/init` | POST |
| `getApproveData(id)` | `/api/v1/exchanges/agent-wallet/approve-data` | POST |
| `approveAgent(id, sig, nonce)` | `/api/v1/exchanges/agent-wallet/approve` | POST |
| `revokeAgent(id)` | `/api/v1/exchanges/agent-wallet/${id}/revoke` | DELETE |
| `pairExtension()` | `/api/v1/auth/pair-extension` | POST |

### Account Page Component Tree

```
AccountPage
├── Show when={isOnboarding}
│   └── OnboardingFlow (exchange selector → form → test)
├── Show when={!isOnboarding}
│   ├── ExchangeCardGrid
│   │   ├── For each={accounts}
│   │   │   └── ExchangeCard (heartbeat, badge, balance, kebab)
│   │   └── AddExchangeCard (ghost card → inline form)
│   └── ExtensionPairingBanner (6-digit code, copy, countdown)
└── Show when={showWalletConnect}
    └── WalletConnectFlow (agent wallet state machine)
```

### Files

**Create:**
- `testudo-journal/src/context/AuthContext.tsx` — Solid.js auth provider with SIWE (FR-2)
- `testudo-journal/src/config/wallet.ts` — Reown AppKit initialization (FR-1)
- `testudo-journal/src/pages/Account.tsx` — Account page with grid + pairing + onboarding (FR-5)
- `testudo-journal/src/components/account/ExchangeCard.tsx` — Exchange card with kebab menu (FR-6)
- `testudo-journal/src/components/account/AddExchangeCard.tsx` — Ghost add card + inline form (FR-7)
- `testudo-journal/src/components/account/ExtensionPairingBanner.tsx` — Pairing code banner (FR-8)
- `testudo-journal/src/components/account/WalletConnectFlow.tsx` — Agent wallet state machine (FR-9)
- `testudo-journal/src/components/account/OnboardingFlow.tsx` — First-time setup wizard (FR-10)

**Modify:**
- `testudo-journal/src/index.tsx` — Add AuthProvider wrapper, import Reown AppKit (FR-1, FR-2)
- `testudo-journal/src/components/Layout.tsx` — Add LOCKED/CONNECTING/ACTIVE states, wallet chip, ACCOUNT nav link (FR-3, FR-4, FR-12)
- `testudo-journal/src/api/client.ts` — Add exchange + auth API methods (FR-11)
- `testudo-journal/package.json` — Add @reown/appkit dependency (FR-1)

### Dependencies Added

- `@reown/appkit` — framework-agnostic wallet connection modal (replaces RainbowKit)
- `@reown/appkit-adapter-wagmi` — adapter for EVM wallet interaction (if needed; may use vanilla JS adapter instead)

---

## Acceptance Criteria

- [ ] Visiting `/desk/` without a wallet connected shows the LOCKED screen with "CONNECT WALLET" button (FR-3)
- [ ] Clicking "CONNECT WALLET" opens the Reown AppKit modal with MetaMask, WalletConnect, and other detected wallets (FR-1)
- [ ] After wallet connection, SIWE auto-triggers and "VERIFYING WALLET..." spinner shows (FR-2, FR-3)
- [ ] Successful SIWE sets HttpOnly cookie and unlocks full dashboard (FR-2)
- [ ] Failed SIWE shows error with "TRY AGAIN" button on the Desk — does not redirect (FR-13)
- [ ] Wallet chip in header shows truncated address with DISCONNECT dropdown (FR-4)
- [ ] DISCONNECT clears session and returns to LOCKED state (FR-4)
- [ ] Desk nav shows OVERVIEW · TRADES · JOURNAL · ACCOUNT (FR-12)
- [ ] `/desk/account` renders exchange card grid with heartbeat, badge, balance, kebab menu (FR-5, FR-6)
- [ ] AddExchangeCard opens inline form for API key entry (FR-7)
- [ ] Extension pairing banner shows 6-digit code with copy, countdown, and regeneration (FR-8)
- [ ] Hyperliquid wallet connect flow completes: init → sign EIP-712 → approve → success (FR-9)
- [ ] Onboarding flow shows when zero accounts exist (FR-10)
- [ ] All exchange API endpoints work with cookie auth from SIWE session (FR-11)
- [ ] Existing Desk functionality (Overview, Trades, Journal) is unaffected
- [ ] Theme toggle (amoled/light) works across all states including LOCKED screen
- [ ] `cd testudo-journal && bun run build` passes

---

## Risks

1. **Reown AppKit + Solid.js compatibility** — Reown AppKit is framework-agnostic but its examples focus on React/Vue/vanilla. Solid.js integration may require manual provider setup or wrapper components. Mitigation: use the vanilla JS API (`createAppKit()`) and wire events to Solid.js signals manually — avoid any React-specific adapters.

2. **EIP-712 signing without wagmi** — The current `WalletConnect.tsx` uses `window.ethereum.request('eth_signTypedData_v4')` directly (not wagmi hooks), so this is already framework-agnostic. The port is a direct copy. Mitigation: verify the Reown AppKit provider exposes the same `window.ethereum` interface or provides its own signing method.

3. **Cookie auth domain** — In development, testudo-web (port 3001) and testudo-journal (port 3002) are different origins. Cookies set by SIWE on port 3002 won't work for the journal's API calls if the backend is on port 8080. Mitigation: the journal's vite proxy already routes `/api` to the backend (same origin from the browser's perspective). SIWE verify endpoint must also be proxied.

4. **Scope creep** — Porting 6 React components to Solid.js (ExchangeCard, AddExchangeCard, ExtensionPairingBanner, WalletConnectFlow, OnboardingFlow, AccountPage) is significant. Mitigation: each component is self-contained with clear props. Port one at a time, verify each independently.

---

## Completion Signal

This spec is complete when:
1. Reown AppKit installed and configured in testudo-journal
2. Solid.js AuthProvider with SIWE flow working (connect → sign → cookie)
3. Desk three-state lifecycle (LOCKED → CONNECTING → ACTIVE) implemented
4. Wallet chip in Desk header with connect/disconnect
5. `/desk/account` route with exchange card grid, pairing banner, and onboarding
6. All 6 React components ported to Solid.js and functional
7. Exchange API methods added to journal's client.ts
8. ACCOUNT tab in Desk nav
9. All existing Desk functionality (Overview, Trades, Journal) unaffected
10. `bun run build` passes
11. Code committed to master
