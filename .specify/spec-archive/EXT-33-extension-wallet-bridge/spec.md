# Specification: Extension-to-Web Wallet Connection Bridge

**Spec ID:** EXT-33-extension-wallet-bridge
**Date:** 2026-03-17
**Status:** Complete
**Class:** Feature / Integration
**Priority:** P1 — Enables DEX wallet connection from extension without manual tab juggling
**Depends on:** AW-01 through AW-05 (agent wallet lifecycle, complete), EXT-32 (CEX/DEX toggle, complete)
**Series:** Standalone

---

## Problem Statement

Connecting a Hyperliquid agent wallet currently requires the user to manually navigate to the web app (`localhost:3001/account`), complete the 4-step wallet flow (Connect → Initialize → Sign → Approve), then return to the extension and hope the account appears. There is no communication channel between the web app and the extension during this process — the extension only discovers the new account on its next `LIST_EXCHANGE_ACCOUNTS` poll or popup reopen.

This creates a disjointed UX for DEX users. The extension's "Connect Account" CTA in DEX mode opens the web app but has no awareness of when the flow completes. The user must manually close the tab and reopen the popup to see their wallet.

The web app already has wagmi/viem/RainbowKit for wallet interaction. The extension already injects `token-sync.js` on `localhost:3001/*` for JWT synchronization. This spec bridges the two by extending the existing content script to relay wallet connection events from the web app to the extension in real time.

---

## User Stories

- **As a trader**, I want to click "Connect Wallet" in the extension's DEX mode and have my Hyperliquid account appear automatically after completing the web flow, so that I don't have to manually refresh or reopen the popup.
- **As a trader**, I want the wallet connection tab to feel like part of the extension experience, so that the handoff between extension and web app is seamless.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | DEX empty state "Connect Wallet" button opens `{webUrl}/account?source=extension` in a new tab | High | Extension popup |
| FR-2 | Web app fires `window.postMessage({ type: "TESTUDO_ACCOUNT_LINKED", account })` after successful agent wallet approval | High | testudo-web |
| FR-3 | `token-sync.js` content script listens for `TESTUDO_ACCOUNT_LINKED` messages and relays to background worker via `browser.runtime.sendMessage` | High | Extension content script |
| FR-4 | Background worker handles `ACCOUNT_LINKED` message by refreshing exchange accounts from backend and updating storage | High | Extension background |
| FR-5 | Extension popup reactively shows the new Hyperliquid account in the DEX dropdown when storage updates | High | Extension popup |
| FR-6 | `postMessage` origin is validated against the configured `webUrl` to prevent spoofing | High | Extension content script |

---

## Technical Implementation

### A. Content Script — `token-sync.ts`

Extend the existing content script (already injected on `localhost:3001/*`) with a `message` event listener:

```typescript
window.addEventListener("message", (event) => {
  // FR-6: Validate origin
  if (event.origin !== window.location.origin) return;
  if (event.data?.type !== "TESTUDO_ACCOUNT_LINKED") return;

  browser.runtime.sendMessage({
    type: "ACCOUNT_LINKED",
    account: {
      id: event.data.account?.id,
      exchange_name: event.data.account?.exchange_name,
    },
  });
});
```

### B. Background Worker — `background.ts`

Add handler in the message router:

```typescript
if (msg.type === "ACCOUNT_LINKED") {
  const result = await listExchangeAccounts();
  if (result.success && result.data) {
    await browser.storage.local.set({ exchangeAccounts: result.data });
  }
  return { success: true };
}
```

### C. Web App — Agent Approval Success Handler

After the agent approval step succeeds, fire the postMessage:

```typescript
window.postMessage(
  {
    type: "TESTUDO_ACCOUNT_LINKED",
    account: { id: response.id, exchange_name: "hyperliquid" },
  },
  window.location.origin
);
```

### D. Popup Empty State — DEX "Connect Wallet" CTA

The existing DEX empty state CTA in the popup should open the web app account page with a source parameter:

```typescript
const webUrl = await getWebUrl(); // from storage or default
window.open(`${webUrl}/account?source=extension`, "_blank");
```

### Files

- `testudo-extension/src/token-sync.ts` — Add `TESTUDO_ACCOUNT_LINKED` listener (FR-3, FR-6)
- `testudo-extension/src/background.ts` — Add `ACCOUNT_LINKED` message handler (FR-4)
- `testudo-extension/src/popup/components/MainView.tsx` — Wire DEX empty state CTA to open web app (FR-1)
- `testudo-web/src/pages/Account.tsx` (or equivalent) — Fire postMessage on approval success (FR-2)

### Dependencies Added

None. All required libraries (wagmi, viem, webextension-polyfill) are already present in their respective packages.

---

## Acceptance Criteria

- [x] Clicking "Connect Wallet" in DEX empty state opens `{webUrl}/account?source=extension`
- [x] Completing agent wallet flow on web app fires `TESTUDO_ACCOUNT_LINKED` postMessage
- [x] `token-sync.js` relays the message to background worker with origin validation
- [x] Background worker refreshes account list and updates storage
- [x] Extension popup shows new Hyperliquid account in DEX dropdown without manual refresh
- [x] postMessage origin is validated (spoofed origins are rejected)
- [x] Extension build passes: `cd testudo-extension && bun run build`
- [x] Web app build passes: `cd testudo-web && bun run build`

---

## Risks

1. **Content script not injected** — If `token-sync.js` fails to load on the web app page, the bridge won't work. Mitigation: The existing token-sync injection is proven; same mechanism, same manifest match pattern.
2. **Web app not running** — If the web app isn't available at the configured URL, the "Connect Wallet" button opens a dead page. Mitigation: This is the existing behavior for CEX "Connect Account"; no regression. Future: health check before opening.
3. **Race condition on account refresh** — The backend may not have finished persisting the account by the time the extension calls `listExchangeAccounts()`. Mitigation: Add a small delay (500ms) before the refresh call, or retry once on empty result.

---

## Completion Signal

This spec is complete when:
1. A user can click "Connect Wallet" in the extension's DEX mode, complete the agent wallet flow on the web app, and see the Hyperliquid account appear in the extension without any manual action
2. All acceptance criteria are met
3. Both `bun run build` commands pass for extension and web app
4. Code committed to master
