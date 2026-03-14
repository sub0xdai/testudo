# EXT-16: Live Trading Readiness

| Field    | Value                                    |
|----------|------------------------------------------|
| Status   | Draft                                    |
| Date     | 2026-02-24                               |
| Depends  | 012-ccxt-multi-exchange, EXT-15           |
| Phase    | Extension — Live Trading Hardening        |

## 1. Overview

### Current State
- CCXT sidecar health checked only on backend startup (non-fatal warning if unreachable)
- No runtime health monitoring — if sidecar dies mid-session, trades fail silently
- Backend selects first exchange account via `accounts.first()` — no user control over which exchange executes
- Trade management events (break-even trigger, trailing stop amend, partial TP fill) execute in backend daemon but are invisible to the user
- Extension already shows WS connected/disconnected dot and has 6 `OrderEventType` values

### Target State
- Periodic sidecar heartbeat with state surfaced in extension (sidecar status dot or merged with WS indicator)
- User selects active exchange in settings or modal; backend routes trades to that account
- Trade management actions broadcast via WebSocket as typed events, displayed as toasts in extension

## 2. User Stories

1. As a trader, I want to know if the CCXT sidecar is down so I don't submit live trades that will fail silently
2. As a trader with multiple exchange accounts, I want to choose which exchange my trades execute on
3. As a trader, I want to see when my break-even triggers, trailing stop moves, or partial TP fills so I know my position is being managed

## 3. Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Health check interval | 30 seconds | Frequent enough to catch failures; low overhead (single GET /health) |
| Health state propagation | Backend → WS broadcast to extension | Reuses existing WS channel; no new polling endpoint needed |
| Sidecar status display | Merge into existing StatusBar | Avoids UI clutter; red dot already means "something is wrong" |
| Exchange selection storage | `browser.storage.local` as `activeExchangeId` | Same pattern as executionMode; persists across popup open/close |
| Exchange ID in trade payload | Add `exchange_account_id` field | Backend can look up specific account instead of `.first()` |
| Management event format | Reuse `order.*` WS stream with new event subtypes | Consistent with existing event system; no new subscription channel |

## 4. Functional Requirements

### FR-1: Sidecar Health Monitoring (Backend)
- FR-1.1: Spawn background task in `main.rs` that calls `ccxt_client.health_check()` every 30 seconds
- FR-1.2: Track sidecar state as `SidecarHealth` enum: `Healthy`, `Degraded`, `Unreachable`
- FR-1.3: On state change, publish `sidecar.health` event to WS stream (via `pg_queue` or direct broadcast)
- FR-1.4: Log state transitions at `warn` level (healthy→unreachable) or `info` (unreachable→healthy)
- FR-1.5: Expose `GET /api/v1/health/sidecar` endpoint returning current sidecar state (for extension polling fallback)

### FR-2: Sidecar Status in Extension
- FR-2.1: Add `SIDECAR_STATUS` message type in background.ts → `GET /api/v1/health/sidecar`
- FR-2.2: Listen for `sidecar.health` events on existing WS connection
- FR-2.3: Update StatusBar: when in LIVE mode, show compound status (WS + sidecar). If sidecar unreachable, override dot to orange with "Sidecar Down" text
- FR-2.4: When sidecar is unreachable and user is in LIVE mode, show warning banner below header: "Live trading unavailable — exchange connection lost"

### FR-3: Exchange Account Selection
- FR-3.1: Add `activeExchangeId` signal to extension, persisted in `browser.storage.local`
- FR-3.2: In SettingsView (inside ExchangeManager), add "Active" toggle/radio on each account card — clicking sets that account as active
- FR-3.3: Store selected `exchange_account_id` in background.ts; include in trade execution payload
- FR-3.4: Add optional `exchange_account_id: string` field to `TradePayload` interface in `types.ts`
- FR-3.5: In `background.ts` `executeTrade()`, attach `exchange_account_id` from stored active exchange to the request body
- FR-3.6: In backend `CcxtExchangeApi::load_credentials()` (`exchange_api.rs`), if `exchange_account_id` is present in request, use that specific account instead of `.first()`
- FR-3.7: Fallback: if no `exchange_account_id` in payload, retain current `.first()` behavior for backwards compatibility
- FR-3.8: In confirmation modal, display the active exchange name as a badge (e.g., "WOO X" or "Binance")

### FR-4: Trade Management Event Visibility
- FR-4.1: In backend `TradeManagerService`, when a management action executes (break-even move, trailing stop amend, partial TP fill), publish event to `order.{userId}` WS stream
- FR-4.2: Event payload format: `{ event: "order.break_even" | "order.trailing_moved" | "order.partial_tp", symbol: string, detail: string }`
- FR-4.3: Add new event types to extension `OrderEventType`: `"order.break_even"`, `"order.trailing_moved"`, `"order.partial_tp"`
- FR-4.4: Add corresponding entries in `ORDER_EVENT_STYLES` with appropriate colors (break_even: blue/info, trailing_moved: blue/info, partial_tp: green/success)
- FR-4.5: Extension popup already handles `WS_ORDER_UPDATE` — ensure new event types render as toasts with the correct styling

## 5. File Context

### New Files
| File | Purpose |
|------|---------|
| — | No new files needed; all changes to existing files |

### Modified Files
| File | Changes |
|------|---------|
| `testudo-exchange/crates/router/src/main.rs` | FR-1.1: Spawn health check background task |
| `testudo-exchange/crates/router/src/services/exchange_api.rs` | FR-3.6, FR-3.7: Accept optional exchange_account_id in load_credentials |
| `testudo-exchange/crates/router/src/routes/health.rs` (or inline in main) | FR-1.5: GET /api/v1/health/sidecar endpoint |
| `testudo-extension/src/types.ts` | FR-3.4: Add exchange_account_id to TradePayload; FR-4.3: Add new OrderEventType values |
| `testudo-extension/src/background.ts` | FR-2.1: SIDECAR_STATUS handler; FR-2.2: WS sidecar event listener; FR-3.3, FR-3.5: Attach exchange_account_id to trade |
| `testudo-extension/src/popup/components/StatusBar.tsx` | FR-2.3: Compound WS + sidecar status display |
| `testudo-extension/src/popup/components/ExchangeManager.tsx` | FR-3.2: Active toggle on account cards |
| `testudo-extension/src/popup/components/SettingsView.tsx` | Minor: pass active exchange state if needed |
| `testudo-extension/src/modal.tsx` or `src/components/TradeForm.tsx` | FR-3.8: Display active exchange badge |

### Unchanged (Backend Complete)
| File | Status |
|------|--------|
| `testudo-ccxt/src/server.js` | Already has GET /health endpoint |
| `testudo-exchange/crates/ws-stream/` | Existing broadcast mechanism sufficient |

## 6. Acceptance Criteria

1. With sidecar stopped: StatusBar shows orange dot + "Sidecar Down" within 30 seconds; warning banner visible in LIVE mode
2. With sidecar running: StatusBar shows blue dot + "Connected" as before
3. `GET /api/v1/health/sidecar` returns `{ "status": "healthy" }` or `{ "status": "unreachable" }`
4. User with 2 exchange accounts can set one as active; trades route to selected account
5. Trades without `exchange_account_id` still work (backwards compat — uses first account)
6. Active exchange name displayed in confirmation modal header
7. When break-even triggers on a live position, extension shows info toast: "Break-even triggered on BTCUSDT"
8. When trailing stop amends, extension shows info toast with new stop price
9. When partial TP fills, extension shows success toast with fill details
10. Extension builds with zero TypeScript errors
11. Backend passes `cargo clippy --all-targets` and `cargo test`

## 7. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Health check adds latency to sidecar | Low | GET /health is ~1ms; 30s interval = negligible load |
| Stale exchange selection after account deletion | Medium | On account delete, clear `activeExchangeId` if it matches deleted account |
| Trade manager doesn't currently publish WS events | Medium | May require refactoring TradeManagerService to accept WS broadcast handle |
| Race condition: sidecar reports healthy but dies before trade | Low | Trade-level error handling already returns 502; user sees error toast |

## 8. Implementation Order

1. **FR-3** (Exchange selection) — highest user impact, unblocks live multi-exchange trading
2. **FR-1 + FR-2** (Sidecar health) — safety net for live trading
3. **FR-4** (Management visibility) — polish, can be done last

## 9. Completion Signal

```
feat: EXT-16 live trading readiness — exchange selection, sidecar health, management events
```
