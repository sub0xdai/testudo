# EXT-17: Live Exchange Balance Display

| Field    | Value                                    |
|----------|------------------------------------------|
| Status   | In Progress                              |
| Date     | 2026-02-28                               |
| Depends  | 012-ccxt-multi-exchange, EXT-15, EXT-16  |
| Phase    | Extension — Live Trading UX              |

## 1. Overview

### Current State
- Extension MainView balance panel shows paper trading USDT balance from `/api/v1/paper/balances`
- No backend endpoint exists to fetch live exchange account balances
- CCXT client can fetch balances internally (used during credential validation and connection testing) but doesn't expose this to the frontend
- When in live mode with an active exchange, users have no visibility into their actual exchange balance

### Target State
- New backend endpoint `GET /api/v1/exchanges/accounts/{id}/balance` returns real exchange balance via CCXT sidecar
- Extension detects execution mode (paper vs live) and active exchange, fetches appropriate balance
- MainView balance panel shows live exchange balance when in live mode with active exchange selected
- Balance refreshes on trade events and periodically

## 2. User Stories

1. As a live trader, I want to see my real exchange balance so I know my available capital
2. As a trader switching between paper and live modes, I want the balance display to automatically reflect the correct source
3. As a trader, I want my balance to refresh after trades execute so I see up-to-date figures

## 3. Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Balance endpoint scope | Return all non-zero assets | Traders may hold multiple assets; USDT-only would hide portfolio |
| Balance source in live mode | Direct CCXT sidecar fetch per request | Real-time accuracy; no stale cache |
| Fallback on sidecar failure | Show last known balance with staleness indicator | Better than blank; user knows data may be stale |
| Extension balance routing | Background handler decides paper vs live based on mode + active exchange | Centralizes logic; popup stays simple |
| Balance type parameter | Default to "future" for futures accounts | Matches existing CCXT client pattern; spot support via query param |

## 4. Functional Requirements

### FR-1: Backend Balance Endpoint
- FR-1.1: Add `GET /api/v1/exchanges/accounts/{id}/balance` route (JWT protected)
- FR-1.2: Load and decrypt credentials for the specified account (ownership-verified)
- FR-1.3: Call `ccxt_client.fetch_balance()` with decrypted credentials
- FR-1.4: Return `ExchangeBalanceResponse` with all non-zero asset balances
- FR-1.5: Support optional `?type=future|spot` query parameter (default: "future")
- FR-1.6: Handle errors: 401 (auth), 404 (account not found), 502 (sidecar unavailable), 429 (rate limited)

### FR-2: Extension Background Handler
- FR-2.1: Add `GET_LIVE_BALANCE` message handler in background.ts
- FR-2.2: Handler fetches active exchange ID, calls `/api/v1/exchanges/accounts/{id}/balance`
- FR-2.3: Returns same `BalanceResponse[]` format as paper balances for UI compatibility
- FR-2.4: If no active exchange set, returns error with descriptive message

### FR-3: Smart Balance Routing
- FR-3.1: Add `GET_BALANCE` unified handler that routes to paper or live based on execution mode
- FR-3.2: Paper mode → existing `GET_BALANCES` flow (GET /api/v1/paper/balances)
- FR-3.3: Live mode + active exchange → `GET_LIVE_BALANCE` flow
- FR-3.4: Live mode + no active exchange → return paper balance with warning flag

### FR-4: MainView Balance Updates
- FR-4.1: `fetchBalance()` in MainView uses the unified `GET_BALANCE` handler
- FR-4.2: Balance panel shows exchange name badge when displaying live balance (e.g., "BINANCE" or "WOO")
- FR-4.3: Balance refreshes on `WS_ORDER_UPDATE` events (existing behavior preserved)
- FR-4.4: Show loading state during balance fetch

## 5. File Context

### Modified Files
| File | Changes |
|------|---------|
| `testudo-exchange/crates/router/src/routes/exchanges.rs` | FR-1: Add get_balance handler |
| `testudo-exchange/crates/router/src/main.rs` | FR-1: Register balance route |
| `testudo-exchange/crates/router/src/types/exchanges.rs` | FR-1: Add ExchangeBalanceResponse type |
| `testudo-extension/src/background.ts` | FR-2, FR-3: Add GET_LIVE_BALANCE and GET_BALANCE handlers |
| `testudo-extension/src/types.ts` | FR-2: Add LiveBalanceResponse interface |
| `testudo-extension/src/popup/components/MainView.tsx` | FR-4: Smart balance routing + exchange badge |

### Unchanged
| File | Status |
|------|--------|
| `testudo-exchange/crates/router/src/services/ccxt_client.rs` | fetch_balance() already exists |
| `testudo-exchange/crates/router/src/repositories/exchange_account.rs` | load_credentials() already exists |

## 6. Acceptance Criteria

1. `GET /api/v1/exchanges/accounts/{id}/balance` returns balance data with valid credentials
2. Endpoint returns 404 for non-existent or non-owned accounts
3. Endpoint returns 502 when CCXT sidecar is unavailable
4. Extension in paper mode shows paper balance (existing behavior unchanged)
5. Extension in live mode with active exchange shows live exchange balance
6. Exchange name badge visible in balance panel during live mode
7. Balance refreshes on WebSocket trade events
8. Extension builds without TypeScript errors
9. Backend passes `cargo clippy --all-targets` and `cargo test`

## 7. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| CCXT sidecar latency on balance fetch | Medium | Show loading state; don't block UI |
| Rate limiting from exchange APIs | Medium | Cache balance for 5s in background; 429 error handling |
| Credential decryption failure | Low | Existing error handling in repository layer |

## 8. Completion Signal

```
feat: EXT-17 live exchange balance — backend endpoint and extension display
```
