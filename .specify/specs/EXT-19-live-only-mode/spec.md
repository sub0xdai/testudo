# EXT-19: Remove Paper Trading — Live Only Mode

| Field    | Value                                    |
|----------|------------------------------------------|
| Status   | Draft                                    |
| Date     | 2026-02-28                               |
| Depends  | EXT-17, EXT-18                           |
| Phase    | Extension — Architecture Simplification  |

## 1. Overview

### Current State
- Extension supports dual execution modes: "paper" (shadow engine) and "live" (CCXT sidecar)
- Paper mode uses hardcoded `PAPER_USER_ID` and `X-User-Id` header for unauthenticated trading
- `ModeToggle` component lets users switch between PAPER and LIVE
- "Continue Without Account" button bypasses authentication entirely
- Backend routes paper orders through ShadowEngine, live orders through CcxtExchangeApi
- `GET /api/v1/paper/balances` provides fake 10,000 USDT starting balance
- Dual auth in trade management: JWT for live, X-User-Id fallback for paper
- ~450 lines of extension code and ~800 lines of backend code dedicated to paper trading

### Target State
- All trading is live through CCXT sidecar with real exchange credentials
- Authentication is mandatory — no anonymous/paper fallback
- No mode toggle — the concept of execution mode is removed
- Balance always fetched from the active exchange account via EXT-17 endpoint
- ShadowEngine and paper balance endpoint preserved in backend (not deleted) but unreachable from extension
- Simpler extension codebase: fewer signals, fewer code paths, fewer bugs

### What We Are NOT Removing (Backend Preservation)
The ShadowEngine, paper_balance.rs routes, and shadow adapters remain in the backend codebase. They are tested, stable code that may be useful for future backtesting or CI integration tests. This spec only removes the **extension and frontend paths** that route to paper trading. The backend routes simply become unreachable from the extension UI.

## 2. User Stories

1. As a trader, I want the extension to always show my real exchange balance so I trade with accurate information
2. As a trader, I want a simpler UI without mode toggles so I can focus on trading
3. As a new user, I must create an account and connect an exchange before I can use the extension

## 3. Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Remove ModeToggle entirely | Yes | No modes to toggle — always live |
| Remove "Continue Without Account" | Yes | Authentication required; no anonymous paper fallback |
| Keep ShadowEngine in backend | Yes | Useful for tests and future backtesting; removal is separate concern |
| Remove paper balance endpoint calls | Extension only | Backend route stays, extension just doesn't call it |
| Remove `executionMode` from Settings | Yes | Single mode doesn't need a setting |
| Remove `PAPER_USER_ID` constant | Yes | No unauthenticated trading |
| Require active exchange for balance | Yes | EXT-17 GET_SMART_BALANCE becomes GET_LIVE_BALANCE only |
| Show "connect exchange" prompt when no accounts | Yes | Clear onboarding instead of paper fallback |

## 4. Functional Requirements

### FR-1: Remove Paper Mode from Extension Types and Constants
- FR-1.1: Remove `executionMode` field from `Settings` interface in `types.ts`
- FR-1.2: Remove `PAPER_USER_ID` constant from `utils.ts`
- FR-1.3: Remove `"paper" | "live"` source from `SmartBalanceResponse` — balance is always from exchange
- FR-1.4: Remove `DEFAULT_SETTINGS.executionMode`

### FR-2: Remove Paper Auth Flow
- FR-2.1: Remove `paperOnly` signal and `continueWithoutAccount()` method from `AuthContext.tsx`
- FR-2.2: Remove `paperOnly` from `AuthState` interface
- FR-2.3: Remove "CONTINUE WITHOUT ACCOUNT" button from `AuthSection.tsx`
- FR-2.4: Update `App.tsx` — require authentication (no `paperOnly` bypass)
- FR-2.5: Clean up `paperOnly` storage key references

### FR-3: Remove Mode Toggle
- FR-3.1: Delete `ModeToggle.tsx` component entirely
- FR-3.2: Remove ModeToggle import and rendering from `HeaderBar.tsx`
- FR-3.3: Remove `executionMode` signal and storage listeners from `HeaderBar.tsx`
- FR-3.4: Remove `executionMode` signal from `StatusBar.tsx` — sidecar status always relevant
- FR-3.5: Remove execution mode storage listener from `MainView.tsx`

### FR-4: Simplify Balance Fetching
- FR-4.1: Remove `getBalances()` function from `background.ts` (paper balance fetch)
- FR-4.2: Remove `getSmartBalance()` routing function — replace with direct `getLiveBalance()`
- FR-4.3: Rename `GET_SMART_BALANCE` message to `GET_BALANCE` (always live)
- FR-4.4: Remove `GET_BALANCES` message handler (paper endpoint)
- FR-4.5: Update `MainView.tsx` to use `GET_BALANCE` and remove paper/live badge logic
- FR-4.6: When no active exchange exists, show "Connect an exchange" prompt instead of `$--`

### FR-5: Simplify Trade Execution
- FR-5.1: Remove `X-User-Id` and `PAPER_USER_ID` fallbacks from `executeTrade()` in `background.ts`
- FR-5.2: Remove `X-Execution-Mode` header — all trades are live
- FR-5.3: Require JWT authentication for all trade operations — return error if not authenticated
- FR-5.4: Remove `X-User-Id` fallback from `listTrades()` and `cancelTrade()`

### FR-6: Simplify WebSocket and Sidecar
- FR-6.1: `StatusBar.tsx` — remove execution mode gating; sidecar status always shown
- FR-6.2: `HeaderBar.tsx` — sidecar warning banner always shown when sidecar unreachable (no mode check)
- FR-6.3: `getUserId()` in `background.ts` — remove `PAPER_USER_ID` fallback; return null/error if not authenticated

### FR-7: Update QuickTrade
- FR-7.1: Remove `isLiveMode` check from `QuickTrade.tsx` — always live
- FR-7.2: Remove execution mode fetch from QuickTrade mount

### FR-8: Update ExchangeSelector
- FR-8.1: `ExchangeSelector.tsx` — always visible when authenticated (remove paperOnly gate from HeaderBar)
- FR-8.2: Show "No exchange" state with link to settings when no accounts

### FR-9: Clean Up Storage Keys
- FR-9.1: Remove `executionMode` from `browser.storage.local`
- FR-9.2: Remove `paperOnly` from `browser.storage.local`
- FR-9.3: On extension startup, clean up legacy storage keys

### FR-10: Update Tests
- FR-10.1: Remove paper-mode test cases from `background.test.ts`
- FR-10.2: Remove paper-mode test cases from `utils.test.ts`
- FR-10.3: Update remaining tests to assume live-only mode

## 5. File Context

### Deleted Files
| File | Reason |
|------|--------|
| `testudo-extension/src/popup/components/ModeToggle.tsx` | No execution modes to toggle |

### Modified Files (Extension)
| File | Changes |
|------|---------|
| `src/types.ts` | FR-1: Remove executionMode from Settings, simplify SmartBalanceResponse |
| `src/utils.ts` | FR-1: Remove PAPER_USER_ID, executionMode from defaults |
| `src/popup/context/AuthContext.tsx` | FR-2: Remove paperOnly, continueWithoutAccount |
| `src/popup/components/AuthSection.tsx` | FR-2: Remove "Continue Without Account" button |
| `src/popup/App.tsx` | FR-2: Require auth, no paperOnly bypass |
| `src/popup/components/HeaderBar.tsx` | FR-3, FR-6: Remove ModeToggle, simplify sidecar logic |
| `src/popup/components/StatusBar.tsx` | FR-3, FR-6: Remove executionMode, always show sidecar |
| `src/popup/components/MainView.tsx` | FR-4, FR-5: Simplify balance, remove paper badge |
| `src/popup/components/QuickTrade.tsx` | FR-7: Remove isLiveMode check |
| `src/popup/components/ExchangeSelector.tsx` | FR-8: Always visible when authenticated |
| `src/background.ts` | FR-4, FR-5, FR-6, FR-9: Major simplification |
| `src/background.test.ts` | FR-10: Remove paper test cases |
| `src/utils.test.ts` | FR-10: Remove paper test cases |

### Unchanged (Backend Preserved)
| File | Status |
|------|--------|
| `testudo-exchange/crates/router/src/routes/paper_balance.rs` | Kept for tests/backtesting |
| `testudo-exchange/crates/engine/src/shadow/` | Kept for tests/backtesting |
| `testudo-exchange/crates/router/src/adapters/shadow_adapter.rs` | Kept |
| `testudo-exchange/crates/router/src/routes/trade_management.rs` | Kept (dual auth harmless) |

## 6. Acceptance Criteria

1. Extension requires authentication — no way to use without logging in
2. No "Continue Without Account" button on auth screen
3. No ModeToggle component rendered anywhere
4. Balance always fetched from active exchange account (GET_BALANCE → getLiveBalance)
5. "Connect an exchange" prompt shown when no active exchange (instead of $--)
6. All trade operations require JWT — fail gracefully if not authenticated
7. StatusBar always shows sidecar status (no mode gating)
8. Sidecar warning banner shown when unreachable (no mode check)
9. ExchangeSelector always visible in header when authenticated
10. No references to `PAPER_USER_ID` in extension code
11. No references to `executionMode` in extension Settings type
12. Extension builds without TypeScript errors
13. Backend passes `cargo clippy --all-targets` and `cargo test` (unchanged)
14. Extension tests pass with paper cases removed

## 7. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| New users can't try before creating account | Low | Clear onboarding flow directs to registration + exchange setup |
| Backend dual auth becomes dead code | Low | Harmless; X-User-Id fallback just never triggers from extension |
| Existing users with paperOnly storage key | Low | FR-9.3: Startup cleanup removes legacy keys |
| Trade execution fails without exchange | Medium | FR-5.3: Clear error message "Connect an exchange to trade" |

## 8. Implementation Order

1. **FR-1 + FR-9** (Types, constants, storage cleanup) — foundation changes
2. **FR-2** (Auth flow) — remove paper bypass
3. **FR-3** (ModeToggle removal) — delete component, update header
4. **FR-4** (Balance simplification) — direct live balance only
5. **FR-5** (Trade execution) — remove paper headers
6. **FR-6 + FR-7 + FR-8** (StatusBar, QuickTrade, ExchangeSelector) — UI cleanup
7. **FR-10** (Tests) — update test suite

## 9. Completion Signal

```
feat: EXT-19 remove paper trading — live only mode
```
