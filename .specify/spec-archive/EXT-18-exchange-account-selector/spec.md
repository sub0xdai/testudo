# EXT-18: Exchange Account Selector in Popup

| Field    | Value                                    |
|----------|------------------------------------------|
| Status   | In Progress                              |
| Date     | 2026-02-28                               |
| Depends  | EXT-15, EXT-16, EXT-17                   |
| Phase    | Extension — Trading UX                   |

## 1. Overview

### Current State
- Exchange account selection only available deep in SettingsView via ExchangeManager component
- No quick way to switch exchanges without leaving the main trading view
- Header bar shows: [StatusBar] ... [ModeToggle] [Settings]
- Active exchange stored in browser.storage.local as `activeExchangeId`
- Background handlers `LIST_EXCHANGE_ACCOUNTS`, `GET_ACTIVE_EXCHANGE`, `SET_ACTIVE_EXCHANGE` already exist

### Target State
- Compact exchange selector dropdown in header bar for quick account switching
- Visible only when authenticated (not paper-only) and exchange accounts exist
- Selecting an exchange automatically updates `activeExchangeId` in storage
- MainView already listens to `activeExchangeId` changes and refreshes balance (EXT-17)

## 2. Functional Requirements

### FR-1: ExchangeSelector Component
- FR-1.1: New component `ExchangeSelector.tsx` renders as compact dropdown
- FR-1.2: On mount, fetches exchange accounts via `LIST_EXCHANGE_ACCOUNTS` and active ID via `GET_ACTIVE_EXCHANGE`
- FR-1.3: Displays active exchange as a pill/badge showing capitalized exchange name
- FR-1.4: Clicking opens a dropdown listing all accounts with active indicator
- FR-1.5: Selecting an account sends `SET_ACTIVE_EXCHANGE` message, updates storage
- FR-1.6: Click-outside closes dropdown
- FR-1.7: Shows nothing if no accounts exist (graceful empty state)

### FR-2: HeaderBar Integration
- FR-2.1: Import and render ExchangeSelector between StatusBar and ModeToggle
- FR-2.2: Only show when user is authenticated (not paper-only)

## 3. Acceptance Criteria

1. Exchange selector visible in header when authenticated with 1+ exchange accounts
2. Clicking selector opens dropdown with all connected accounts
3. Selecting different account updates `activeExchangeId` in storage
4. Balance panel refreshes after exchange switch (existing EXT-17 behavior)
5. Selector hidden when no exchange accounts exist
6. Selector hidden for paper-only users
7. Extension builds without TypeScript errors

## 4. Completion Signal

```
feat: EXT-18 exchange account selector in popup header
```
