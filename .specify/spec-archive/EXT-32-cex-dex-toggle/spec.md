# Specification: CEX/DEX Mode Toggle + Empty State CTA

**Spec ID:** EXT-32-cex-dex-toggle
**Date:** 2026-03-17
**Status:** Complete
**Class:** Feature / UX
**Priority:** P1 — Required for Hyperliquid launch; users need to switch between exchange types
**Depends on:** HL-05 (exchange routing), AW-03 (frontend wallet connection)
**Series:** EXT-32 (standalone)

---

## Problem Statement

The extension popup has no concept of exchange type. After integrating Hyperliquid (a DEX), users may have both CEX accounts (WOO, Binance) and DEX accounts (Hyperliquid) connected simultaneously. The `ExchangeSelector` shows all accounts in a flat list with no type differentiation.

Additionally, when no exchange account is connected, the popup displays raw text ("Connect an exchange account in Settings to start trading") in the balance hero area. This violates empty-state UX best practices: no visual hierarchy, no clear action affordance, no contextual guidance.

The extension needs a mode toggle to separate CEX and DEX trading contexts, and a proper empty-state CTA to guide users toward account setup.

---

## User Stories

- **As a trader**, I want to toggle between CEX and DEX mode, so that I see only the relevant exchange accounts and don't accidentally trade on the wrong exchange type.
- **As a new user**, I want a clear call-to-action when no exchange is connected, so that I know exactly what to do to start trading.
- **As a returning user**, I want the extension to remember my last-used mode and account per mode, so that switching between CEX and DEX preserves my context.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add `[CEX \| DEX]` segmented pill toggle to `HeaderBar`, positioned left of `ExchangeSelector` | High | Extension UI |
| FR-2 | Persist `exchangeMode` ("cex" \| "dex") in `browser.storage.local`; default to "cex" | High | Extension State |
| FR-3 | Store separate active account IDs per mode: `activeCexAccountId` and `activeDexAccountId` | High | Extension State |
| FR-4 | `getActiveExchangeId()` reads `exchangeMode` and returns the corresponding per-mode ID | High | Background Worker |
| FR-5 | `ExchangeSelector` filters displayed accounts by current mode (derive type from `exchange_name`) | High | Extension UI |
| FR-6 | When toggling mode, if the per-mode ID is null/stale, auto-select the first account of that type | Medium | Extension State |
| FR-7 | Replace raw "Connect an exchange..." text with an inline CTA card: icon + one-line copy + Connect button | High | Extension UI |
| FR-8 | CTA card copy is mode-aware: "No CEX exchange linked" vs "No wallet connected" | Medium | Extension UI |
| FR-9 | CTA Connect button opens the web app `/account` page in a new tab | Medium | Extension UI |
| FR-10 | Migrate existing `activeExchangeId` to the appropriate per-mode key on first load | Medium | Extension State |

---

## Technical Implementation

### Exchange Type Derivation

Client-side lookup — zero backend changes needed:

```typescript
// src/utils.ts
const DEX_EXCHANGES = new Set(["hyperliquid"]);

export type ExchangeType = "cex" | "dex";

export function getExchangeType(exchangeName: string): ExchangeType {
  return DEX_EXCHANGES.has(exchangeName.toLowerCase()) ? "dex" : "cex";
}
```

### Storage Model

```
browser.storage.local:
  exchangeMode:        "cex" | "dex"      (default: "cex")
  activeCexAccountId:  string | null       (UUID of last-used CEX account)
  activeDexAccountId:  string | null       (UUID of last-used DEX account)
  activeExchangeId:    REMOVED (migrated)  (legacy key, one-time migration)
```

### Background Worker Changes (`src/background.ts`)

```typescript
// Updated getActiveExchangeId()
async function getActiveExchangeId(): Promise<string | null> {
  const { exchangeMode = "cex" } = await browser.storage.local.get("exchangeMode");
  const key = exchangeMode === "dex" ? "activeDexAccountId" : "activeCexAccountId";
  const result = await browser.storage.local.get(key);
  return result[key] ?? null;
}

// Updated setActiveExchangeId()
async function setActiveExchangeId(id: string | null): Promise<void> {
  const { exchangeMode = "cex" } = await browser.storage.local.get("exchangeMode");
  const key = exchangeMode === "dex" ? "activeDexAccountId" : "activeCexAccountId";
  await browser.storage.local.set({ [key]: id });
}

// New: one-time migration from legacy key
async function migrateActiveExchangeId(): Promise<void> {
  const { activeExchangeId, activeCexAccountId, activeDexAccountId } =
    await browser.storage.local.get(["activeExchangeId", "activeCexAccountId", "activeDexAccountId"]);

  if (activeExchangeId && !activeCexAccountId && !activeDexAccountId) {
    // Determine type from account list
    const accounts = await fetchExchangeAccounts();
    const account = accounts?.find(a => a.id === activeExchangeId);
    const type = account ? getExchangeType(account.exchange_name) : "cex";
    const key = type === "dex" ? "activeDexAccountId" : "activeCexAccountId";
    await browser.storage.local.set({ [key]: activeExchangeId, exchangeMode: type });
    await browser.storage.local.remove("activeExchangeId");
  }
}
```

### New Component: ExchangeToggle (`src/popup/components/ExchangeToggle.tsx`)

```tsx
// Solid.js segmented pill: [CEX | DEX]
// Reads/writes exchangeMode from browser.storage.local
// Emits storage change which HeaderBar, ExchangeSelector, MainView all listen to
```

### HeaderBar Layout Change

```
Before: [StatusBar]                    [ExchangeSelector] [Settings]
After:  [StatusBar]  [CEX|DEX toggle]  [ExchangeSelector] [Settings]
```

### Empty State CTA Component

Replaces the raw text in MainView's balance panel:

```tsx
<div class="mx-4 my-3 p-4 rounded-lg border border-white/10 text-center">
  <p class="text-sm text-text-dim mb-2">
    {exchangeMode() === "dex" ? "No wallet connected" : "No CEX exchange linked"}
  </p>
  <button
    class="px-4 py-1.5 text-xs font-medium rounded bg-accent/20 text-accent hover:bg-accent/30 transition"
    onClick={() => window.open(`${webAppUrl}/account`, '_blank')}
  >
    Connect Account
  </button>
</div>
```

### Files

- `src/popup/components/ExchangeToggle.tsx` — **new**: CEX/DEX segmented pill toggle
- `src/popup/components/HeaderBar.tsx` — add ExchangeToggle to layout
- `src/popup/components/ExchangeSelector.tsx` — filter accounts by current mode
- `src/popup/components/MainView.tsx` — replace raw text with CTA card component
- `src/background.ts` — update `getActiveExchangeId`, `setActiveExchangeId`, add migration
- `src/utils.ts` — add `getExchangeType()` helper

### Dependencies Added

None — uses existing Solid.js primitives and browser.storage API.

---

## Acceptance Criteria

- [x] `[CEX | DEX]` toggle visible in header bar, persists across popup opens
- [x] Switching toggle filters ExchangeSelector to only show accounts of that type
- [x] Each mode remembers its own active account independently
- [x] `getActiveExchangeId()` returns the correct per-mode account
- [x] Existing `activeExchangeId` is migrated on first load
- [x] Empty state shows inline CTA card with mode-aware copy + Connect button
- [x] Connect button opens web app `/account` in new tab
- [x] `bun run build` passes with zero errors

---

## Risks

1. **Storage migration race** — If user opens multiple tabs during migration, the legacy key could be read twice. Mitigation: migration is idempotent (check if per-mode keys already exist before migrating).
2. **Mode drift** — User adds a DEX account while in CEX mode and doesn't know it's available. Mitigation: `ensureActiveExchange()` already auto-selects first account; no special handling needed.

---

## Completion Signal

This spec is complete when:
1. CEX/DEX toggle renders in header and filters accounts by type
2. Per-mode active account IDs persist independently
3. Empty state CTA replaces raw text with actionable card
4. Legacy migration handles existing `activeExchangeId`
5. `bun run build` passes
6. Code committed to master
