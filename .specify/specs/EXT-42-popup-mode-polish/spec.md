# Specification: Extension Popup Mode Polish

**Spec ID:** EXT-42-popup-mode-polish
**Date:** 2026-03-28
**Status:** Draft
**Class:** UX Polish
**Priority:** P2 — Functional but confusing mode behavior.
**Depends on:** DEPLOY-03-extension-prod-urls (complete)

---

## Problem Statement

The extension popup's CEX/DEX mode toggle has incorrect behavior:

1. **CEX mode shows Hyperliquid balance** — When no CEX exchange is linked but Hyperliquid is, selecting CEX still shows the Hyperliquid balance and badge. Should show empty state: `--` balance with "No CEX exchange linked" and a "Connect Account" button.

2. **DEX mode empty state** — When DEX mode is selected but no Hyperliquid account exists, the empty state should show "No DEX exchange linked" with "Connect Account" button pointing to `desk.testudo.vip/account`.

3. **Connect Account button** — The "Connect Account" button in both empty states should link to the desk account page (`DESK_URL/account`), not show internal extension settings.

---

## Implementation

### T1: Filter active exchange by mode

In the popup's MainView or balance logic, filter the active exchange based on the selected mode:
- CEX mode: only show exchanges where `type === 'cex'` (Binance, WOO, Bybit, OKX)
- DEX mode: only show exchanges where `type === 'dex'` (Hyperliquid)

If no exchange matches the current mode, show the empty state.

### T2: Empty state per mode

**CEX empty state:**
```
BALANCE  --

No CEX exchange linked

[ Connect Account ]  → opens desk.testudo.vip/account
```

**DEX empty state:**
```
BALANCE  --

No DEX exchange linked

[ Connect Account ]  → opens desk.testudo.vip/account
```

### T3: Balance display filtering

The balance panel should only display the balance from an exchange matching the current mode. When switching modes:
- If the active exchange doesn't match the mode, clear the displayed balance
- Fetch balance from the first exchange matching the mode, if available

### T4: Exchange badge

The exchange name badge (e.g., "HYPERLIQUID") next to BALANCE should only show for exchanges matching the current mode. Show nothing (or `--`) when no matching exchange exists.

### T5: Verify

- `bun run build` and `bun run build:prod` pass
- CEX mode with only Hyperliquid linked → shows empty state
- DEX mode with only CEX linked → shows empty state
- Switching modes updates balance display correctly
- "Connect Account" opens desk account page

---

## Key Files

- `testudo-extension/src/popup/components/MainView.tsx` — Balance panel, mode switching
- `testudo-extension/src/popup/components/ExchangeToggle.tsx` — CEX/DEX toggle
- `testudo-extension/src/popup/components/ExchangeSelector.tsx` — Exchange dropdown
- `testudo-extension/src/background/api.ts` — Exchange account listing

---

## Acceptance Criteria

- [ ] CEX mode only shows CEX exchanges
- [ ] DEX mode only shows DEX exchanges
- [ ] Empty state with `--` balance when no exchange matches mode
- [ ] "Connect Account" links to desk account page
- [ ] Mode switching correctly updates balance display
- [ ] No Hyperliquid badge/balance when CEX is selected (and vice versa)
