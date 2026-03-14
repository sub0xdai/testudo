# Spec: 011-popup-redesign — Extension Popup Tab Redesign

> Priority: P1 | Depends on: EXT-08, EXT-12 | Status: Complete
> Date: 2026-02-12

---

## Overview

Redesign the browser extension popup from a monolithic single-scroll layout into a tabbed interface with persistent header balance, visual range sliders, and rich position cards. Inspired by trading extension UIs with prominent portfolio values, tab navigation, and card-based position displays.

**Current:** Single-scroll MainView: header -> trade management (number inputs) -> active orders (flat list) -> balance (buried) -> mode toggle -> footer.

**Target:** Persistent header with balance + mode toggle, 3-tab layout (Trade / Positions / Account), visual range sliders, rich PositionCards with accent borders and management badges.

---

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Tab structure | Trade / Positions / Account | Groups related concerns; Trade first since it's configured before placing orders |
| Balance location | Persistent in header (always visible) | User preference; mirrors reference UIs showing portfolio value prominently |
| Slider implementation | Native `<input type="range">` with CSS styling | Simpler than custom drag implementation; still visual, accessible by default |
| Mode toggle location | Compact pills in header | Recovers vertical space from full-width buttons |
| Position card layout | Left accent border + structured rows | Matches reference UI pattern; color-codes LONG/SHORT at a glance |
| Default tab on open | Trade | User configures management settings before placing trades on TradingView |
| Tab state persistence | None (always opens on Trade) | Popup is ephemeral; no value in persisting tab state |

---

## Functional Requirements

| ID | Requirement | Component | Files | Status |
|----|-------------|-----------|-------|--------|
| FR-1 | Extract header into `HeaderBar` component with logo, compact mode toggle, WS status dot, and settings gear | HeaderBar | `popup/components/HeaderBar.tsx` (new) | pending |
| FR-2 | Display portfolio total (available + locked USDT) in header, always visible across all tabs, formatted with commas and 2 decimal places | HeaderBar | `popup/components/HeaderBar.tsx` (new) | pending |
| FR-3 | Create `TabBar` component with 3 tabs (Trade / Positions / Account), active tab indicated by 2px white bottom border, Positions tab shows count badge | TabBar | `popup/components/TabBar.tsx` (new) | pending |
| FR-4 | Refactor `MainView` into tab controller that renders HeaderBar + TabBar + conditional tab content + simplified footer | MainView | `popup/components/MainView.tsx` | pending |
| FR-5 | Replace `<input type="number">` fields in TradeManagement with styled `<input type="range">` sliders with editable value display and min/max labels | TradeManagement | `popup/components/TradeManagement.tsx` | pending |
| FR-6 | Wrap Trailing Stop and Partial TP sections in collapsible toggle card containers -- bordered card with toggle in header, body slides open/closed | TradeManagement | `popup/components/TradeManagement.tsx` | pending |
| FR-7 | Create `PositionCard` component with 3px left accent border (green=LONG, red=SHORT), structured rows for symbol/direction/status, prices, TP targets, management badges, and cancel button | PositionCard | `popup/components/PositionCard.tsx` (new) | pending |
| FR-8 | Refactor `ActiveOrders` to render PositionCards instead of flat row list, with section header showing count + refresh button | ActiveOrders | `popup/components/ActiveOrders.tsx` | pending |
| FR-9 | Account tab displays balance breakdown (Available in green, Locked in orange), connection info (WS status + URL), and account email | Account tab | `popup/components/MainView.tsx` | pending |
| FR-10 | Empty state for Positions tab: centered "NO ACTIVE POSITIONS" with subtext "Trades placed via TradingView will appear here automatically" | ActiveOrders | `popup/components/ActiveOrders.tsx` | pending |
| FR-11 | Simplify footer to email + "PAPER ONLY" badge only (WS status moved to header) | Footer | `popup/components/MainView.tsx` | pending |
| FR-12 | Add CSS for range slider styling, tab classes, position card accents, toggle card transitions, and refresh spin animation | CSS | `popup/popup.css` | pending |
| FR-13 | Update E2E tests to navigate to correct tab before asserting per-tab elements (click tab-positions before checking active-orders, click tab-account before checking balance-section) | E2E Tests | `tests/e2e/popup.spec.ts` | pending |
| FR-14 | Preserve ALL existing `data-testid` attributes; add new testids for tabs, position cards, sliders, and badges | All | all modified files | pending |
| FR-15 | Extension builds successfully for both Chrome and Firefox with zero TypeScript errors | Build | all files | pending |

---

## Component Architecture

### New Components

| Component | File | Description |
|-----------|------|-------------|
| HeaderBar | `src/popup/components/HeaderBar.tsx` | Persistent header: [ws-dot] TESTUDO [PAPER\|LIVE] [gear] + balance row |
| TabBar | `src/popup/components/TabBar.tsx` | 3-tab navigation: Trade / Positions(n) / Account |
| PositionCard | `src/popup/components/PositionCard.tsx` | Rich card per trade group with accent border + management badges |

### Modified Components

| Component | File | Changes |
|-----------|------|---------|
| MainView | `src/popup/components/MainView.tsx` | Refactor into tab controller shell |
| TradeManagement | `src/popup/components/TradeManagement.tsx` | Replace number inputs with range sliders, add toggle cards |
| ActiveOrders | `src/popup/components/ActiveOrders.tsx` | Replace flat list with PositionCard components |
| ModeToggle | `src/popup/components/ModeToggle.tsx` | Add `compact` prop for header variant |
| popup.css | `src/popup/popup.css` | Add slider, tab, accent, transition styles |
| E2E tests | `tests/e2e/popup.spec.ts` | Add tab navigation to per-tab assertions |

### Unchanged Components

| Component | File | Reason |
|-----------|------|--------|
| AuthSection | `src/popup/components/AuthSection.tsx` | No structural changes |
| SettingsView | `src/popup/components/SettingsView.tsx` | No structural changes |
| AuthContext | `src/popup/context/AuthContext.tsx` | No structural changes |
| StatusBar | `src/popup/components/StatusBar.tsx` | Reused as-is (rendered in header + account tab) |
| App.tsx | `src/popup/App.tsx` | View routing unchanged |

---

## Wireframes

### Header (persistent, all tabs)

```
+------------------------------------------------------------------+
| [*] TESTUDO                       [PAPER|live]       [gear]      |
|                    $12,450.00 USDT                               |
+------------------------------------------------------------------+
|    TRADE          POSITIONS (2)          ACCOUNT                  |
|    =====                                                         |
+------------------------------------------------------------------+
```

### Trade Tab (default)

```
|  RISK PER TRADE                                                  |
|  [============================--------]  1.0 %                   |
|   0.1                                               10.0         |
|                                                                  |
|  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  |
|                                                                  |
|  BREAK-EVEN TRIGGER                                              |
|  [==================------------------]  50 %                    |
|   10                                                100          |
|                                                                  |
|  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  |
|                                                                  |
|  TRAILING STOP                                    [ OFF ]        |
|  +------------------------------------------------------+       |
|  |  (collapsed -- toggle ON to reveal distance slider)  |       |
|  +------------------------------------------------------+       |
|                                                                  |
|  PARTIAL TAKE PROFIT                              [ OFF ]        |
|  +------------------------------------------------------+       |
|  |  (collapsed -- toggle ON to reveal close % slider)   |       |
|  +------------------------------------------------------+       |
```

### Positions Tab

```
|  2 ACTIVE POSITIONS                              [refresh]       |
|                                                                  |
|  +------------------------------------------------------+       |
|  |GRN|  BTCUSDT                    LONG    [Active]     |       |
|  |   |  Entry  64,231.45       SL  63,800.00            |       |
|  |   |  TP1  65,000.00 (50%)  [filled]                  |       |
|  |   |                                                   |       |
|  |   |  [BE: triggered]  [Trail: ON @ 25%]              |       |
|  |   |                                       [CANCEL]    |       |
|  +------------------------------------------------------+       |
|                                                                  |
|  +------------------------------------------------------+       |
|  |RED|  ETHUSDT                   SHORT    [Pending]    |       |
|  |   |  Entry  --               SL  3,250.00            |       |
|  |   |                                                   |       |
|  |   |  [BE: armed @ 50%]  [Trail: OFF]                |       |
|  |   |                                       [CANCEL]    |       |
|  +------------------------------------------------------+       |
```

### Positions Tab (empty)

```
|                                                                  |
|                  NO ACTIVE POSITIONS                             |
|                                                                  |
|             Trades placed via TradingView will                   |
|             appear here automatically.                           |
|                                                                  |
```

### Account Tab

```
|  +------------------------------------------------------+       |
|  |  AVAILABLE                               10,200.00   |       |
|  +------------------------------------------------------+       |
|  |  LOCKED                                   2,250.00   |       |
|  +------------------------------------------------------+       |
|                                                                  |
|  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  |
|                                                                  |
|  CONNECTION                                                      |
|  [*] Connected              ws://localhost:4000                  |
|                                                                  |
|  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  |
|                                                                  |
|  ACCOUNT                                                         |
|  trader@testudo.io                                               |
```

---

## CSS Additions

```css
/* Range Slider */
input[type="range"] {
  -webkit-appearance: none;
  width: 100%;
  height: 4px;
  background: var(--color-bg-elevated);
  outline: none;
}
input[type="range"]::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 12px;
  height: 12px;
  background: var(--color-text-primary);
  border: 2px solid var(--color-signal-green);
  cursor: grab;
}
input[type="range"]::-webkit-slider-thumb:active { cursor: grabbing; }

/* Tab Bar */
.tab-active { color: var(--color-text-primary); border-bottom: 2px solid var(--color-text-primary); }
.tab-inactive { color: var(--color-text-dim); border-bottom: 2px solid transparent; }
.tab-inactive:hover { color: var(--color-text-secondary); }

/* Position Card Accents */
.accent-long { border-left: 3px solid var(--color-signal-green); }
.accent-short { border-left: 3px solid var(--color-signal-red); }

/* Toggle Card */
.toggle-card-body { max-height: 0; overflow: hidden; transition: max-height 150ms ease-out; }
.toggle-card-body.expanded { max-height: 120px; }

/* Refresh Spin */
@keyframes refresh-spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
.refresh-spinning { animation: refresh-spin 300ms ease-out; }
```

---

## Data-Testid Attributes

### Preserved (all existing testids unchanged)
`auth-section`, `login-email`, `login-password`, `login-btn`, `paper-mode-btn`, `settings-btn`, `settings-back`, `backend-url`, `ws-url`, `save-status`, `logout-btn`, `trade-management`, `risk-percent`, `break-even-at`, `trailing-toggle`, `trailing-distance`, `partial-tp-toggle`, `partial-tp-close`, `active-orders`, `order-row`, `cancel-order`, `refresh-orders`, `orders-error`, `balance-section`, `balance-available`, `balance-locked`, `mode-toggle`, `mode-paper`, `mode-live`, `status-bar`, `status-dot`, `status-text`, `footer-email`, `footer-paper`

### New
`header-bar`, `header-balance`, `tab-bar`, `tab-trade`, `tab-positions`, `tab-account`, `tab-positions-count`, `position-card`, `position-symbol`, `position-direction`, `position-status`, `position-entry`, `position-sl`, `position-tp`, `position-be-badge`, `position-trail-badge`, `empty-positions`, `risk-slider`, `be-slider`, `trailing-card`, `partial-tp-card`

---

## E2E Test Changes

Tests that check `active-orders` or `balance-section` visibility must navigate to the correct tab first:

```typescript
// active-orders: click Positions tab first
await page.locator('[data-testid="tab-positions"]').click();
await expect(page.locator('[data-testid="active-orders"]')).toBeVisible();

// balance-section: click Account tab first
await page.locator('[data-testid="tab-account"]').click();
await expect(page.locator('[data-testid="balance-section"]')).toBeVisible();
```

The `bypassAuthGate` helper still works (Trade tab is default, `trade-management` visible immediately).

---

## Acceptance Criteria

1. `bun run build` succeeds for both Chrome and Firefox
2. `bun run typecheck` reports zero errors
3. `bun run test` passes all unit tests
4. `bun run test:e2e` passes all E2E tests (updated for tab navigation)
5. 3-tab layout renders correctly (Trade / Positions / Account)
6. Portfolio balance visible in header across all tabs
7. Range sliders functional for all trade management inputs
8. PositionCards display with correct accent colors and management badges
9. All existing `data-testid` attributes preserved
10. Compact mode toggle functional in header

---

## Completion Signal

All components refactored, no monolithic MainView scroll remains. `grep -r "ModeToggle\|StatusBar" src/popup/components/MainView.tsx` returns zero direct imports (both extracted to HeaderBar).

---

## Risks

| Risk | Mitigation |
|------|------------|
| Range slider CSS varies across browsers | Test in Chrome + Firefox; use both webkit and moz prefixes |
| E2E tests break from tab restructure | Update tests before merging; keep all testids stable |
| Popup height may exceed Chrome limits | Chrome allows up to 600px; keep content scrollable within tabs |
| Toggle card animation jank | Use `max-height` transition, avoid `height: auto` animation |
