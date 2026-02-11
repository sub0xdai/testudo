# EXT-12: Popup Balance Display + Sizing Rework

> Priority: P1 | Depends on: EXT-11 | Status: Complete
> Created: 2026-02-11

## Overview

**Current:** The popup panel is 400px wide with 11-13px font sizes. Trade management, active orders, mode toggle, and status bar all display correctly but the panel feels cramped. Account balance is only shown on the Alt+X confirmation modal (added in the balance feature commit) — traders have no way to see their available USDT or locked margin from the popup itself.

**Target:** Add a balance section to the popup's MainView showing available and locked USDT. Increase popup width to 460px and bump font sizes across all components by ~2px for improved readability. No structural changes — same 3-view architecture from EXT-11.

## User Stories

- [ ] As a trader, I want to see my available USDT balance in the popup so I can check my account without opening the modal.
- [ ] As a trader, I want to see how much margin is currently locked so I know my remaining buying power.
- [ ] As a trader, I want larger text in the popup so I can read values quickly at a glance.

## Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | **Balance Section**: MainView shows an "Account" section between Active Orders and Mode Toggle displaying available USDT and locked USDT. Values are fetched via `GET_BALANCES` message to background service (already implemented). | High |
| FR-2 | **Balance Fetch on Mount**: Balance is fetched when MainView mounts and refreshes when a `WS_ORDER_UPDATE` message is received (same trigger as Active Orders refresh). | High |
| FR-3 | **Balance Loading/Error States**: Show "..." while loading. If fetch fails, show "unavailable" in dim text. Non-blocking — the rest of the popup renders normally. | Medium |
| FR-4 | **Popup Width**: Increase from 400px to 460px in `App.tsx`. | High |
| FR-5 | **Font Size Increase**: Bump base font sizes across all popup components by ~2px. See Technical Notes for exact values. | High |
| FR-6 | **No New Dependencies**: Uses existing `GET_BALANCES` message handler and `BalanceResponse` type. No new npm packages. | High |

## Technical Notes

### Files to Modify

| File | Change |
|------|--------|
| `src/popup/App.tsx` | Change `w-[400px]` to `w-[460px]` |
| `src/popup/components/MainView.tsx` | Add balance section between ActiveOrders and ModeToggle; import and use `BalanceResponse` type |
| `src/popup/components/TradeManagement.tsx` | Bump label font from `text-[11px]` to `text-[13px]`, field labels from `text-xs` (12px) to `text-sm` (14px), input font from 13px to 15px, toggle buttons from `text-[10px]` to `text-xs` (12px) |
| `src/popup/components/ActiveOrders.tsx` | Bump header from `text-[11px]` to `text-[13px]`, symbol from `text-xs` to `text-sm`, side/status from `text-[10px]` to `text-xs`, entry/SL from `text-[10px]` to `text-[11px]` |
| `src/popup/components/StatusBar.tsx` | Bump text from `text-[11px]` to `text-[13px]`, dot from `w-2 h-2` to `w-2.5 h-2.5` |
| `src/popup/popup.css` | Bump base input font-size from 13px to 15px |

### MainView Balance Section Layout

```
┌───────────────────────────────────┐
│ TESTUDO                        ⚙  │
│───────────────────────────────────│
│ TRADE MANAGEMENT                  │
│ Risk %                         1  │
│ Break-even %                  50  │
│ Trailing Stop               OFF   │
│ Partial TP                  OFF   │
│───────────────────────────────────│
│ ACTIVE ORDERS (1)              ↻  │
│ SOL_USDT  LONG ■ Active          │
│───────────────────────────────────│
│ ACCOUNT                          │  ← NEW
│ Available    5,000.00 USDT       │  ← NEW
│ Locked          48.62 USDT       │  ← NEW
│───────────────────────────────────│
│         [ PAPER ]                 │
│───────────────────────────────────│
│ ■ Disconnected        PAPER ONLY │
└───────────────────────────────────┘
```

### Balance Section Component (inline in MainView)

The balance section follows the same pattern as ActiveOrders — fetch on mount, refresh on WS updates:

```tsx
// Inside MainView, add state:
const [balance, setBalance] = createSignal<BalanceResponse[] | null>(null);
const [balanceLoading, setBalanceLoading] = createSignal(true);

async function fetchBalance() {
  try {
    const resp = await browser.runtime.sendMessage({ type: "GET_BALANCES" });
    if (resp?.success && resp.data) setBalance(resp.data);
  } catch { /* non-blocking */ }
  setBalanceLoading(false);
}

// Derive USDT values:
const usdt = () => balance()?.find((b) => b.asset === "USDT");
const available = () => usdt() ? parseFloat(usdt()!.available) : null;
const locked = () => usdt() ? parseFloat(usdt()!.locked) : null;
```

Call `fetchBalance()` on mount and on WS_ORDER_UPDATE (add a message listener matching ActiveOrders pattern).

### Font Size Map (before → after)

| Context | Before | After |
|---------|--------|-------|
| Section headers (TRADE MANAGEMENT, ACTIVE ORDERS) | 11px | 13px |
| Field labels (Risk %, Break-even %) | 12px (text-xs) | 14px (text-sm) |
| Input values | 13px (CSS base) | 15px |
| Toggle buttons (ON/OFF) | 10px | 12px (text-xs) |
| Order symbol | 12px (text-xs) | 14px (text-sm) |
| Order metadata (side, status, entry, SL) | 10px | 12px (text-xs) |
| Status bar text | 11px | 13px |
| Footer email / PAPER ONLY | 11px | 13px |
| Balance values | — | 14px (text-sm), monospace |
| Balance labels | — | 13px, text-text-secondary |

### Styling

Balance section uses existing design tokens:
- Section label: `text-signal-orange uppercase tracking-widest font-bold` (matches TRADE MANAGEMENT, ACTIVE ORDERS)
- Available value: `text-signal-green font-mono` (green = positive/available)
- Locked value: `text-signal-orange font-mono` (orange = warning/reserved)
- "unavailable" fallback: `text-text-dim italic`

### Dependencies

- `BalanceResponse` type from `../../types` (already exists)
- `GET_BALANCES` message handler in `background.ts` (already exists)
- No new npm packages

---

## Acceptance Criteria

- [ ] Popup shows available USDT and locked USDT in an "Account" section
- [ ] Balance refreshes when orders are placed/updated (WS_ORDER_UPDATE)
- [ ] Balance section shows "..." during loading, "unavailable" on error
- [ ] Popup width is 460px
- [ ] All font sizes match the "after" column in Font Size Map
- [ ] Section headers are 13px, field labels 14px, inputs 15px
- [ ] `bun run build` succeeds for Chrome and Firefox
- [ ] Existing functionality unchanged (trade management, active orders, mode toggle, settings)
- [ ] `background.ts` is unmodified (zero changes)

---

## Completion Signal

### Implementation Checklist
- [ ] `App.tsx` width updated to 460px
- [ ] `popup.css` input base font bumped to 15px
- [ ] `TradeManagement.tsx` font sizes bumped per map
- [ ] `ActiveOrders.tsx` font sizes bumped per map
- [ ] `StatusBar.tsx` font sizes bumped per map
- [ ] `MainView.tsx` balance section added with fetch-on-mount
- [ ] `MainView.tsx` balance refreshes on WS_ORDER_UPDATE
- [ ] `MainView.tsx` footer email/paper-only text bumped to 13px

### Testing Requirements
- [ ] `bun run build` exits 0
- [ ] Manual: popup shows balance section with USDT values
- [ ] Manual: place a trade, balance updates in popup
- [ ] Manual: text is visibly larger and more readable
- [ ] Manual: popup width increased, no horizontal overflow

### Done Signal
When ALL above criteria are satisfied, output:
```
<promise>DONE</promise>
```

---

*Template version: 1.0*
