# EXT-04: REST Trade Execution (Paper)

> Priority: P0 | Depends on: EXT-03 | Status: COMPLETE

## Overview
**Current:** Modal confirms trade details but `executeTrade()` is a stub that logs to console.
**Target:** On `Enter`, the extension sends the trade setup to `POST /api/v1/trades` on the configured backend. Backend calculates position size via existing risk engine, creates order group in Shadow Engine. Extension displays success/error toast.

## Functional Requirements

| ID | Requirement | Status |
|----|-------------|--------|
| FR-1 | REST dispatch -- on confirmation, POST /api/v1/trades with scraped payload via background worker | DONE |
| FR-2 | Payload mapping -- map TradeSetup to /trades body: { symbol, side, entry_price, stop_loss_price, take_profit_price, quantity } | DONE |
| FR-3 | Symbol normalization -- convert TradingView symbols (BTCUSDT) to backend format (BTC_USDT) | DONE |
| FR-4 | Response handling -- "Order Sent" toast on 200, "Error: [message]" on failure | DONE |
| FR-5 | Backend URL -- read from chrome.storage (configured in popup) | DONE |
| FR-6 | User identity -- send X-User-Id header (hardcoded default for paper trading) | DONE |

## Architecture

Content scripts cannot make cross-origin fetch() calls due to Manifest V3 restrictions.
The flow is:
1. `content.ts` calls `executeTrade(setup)` on Enter
2. `content.ts` sends message `{ type: "EXECUTE_TRADE", payload }` to background worker
3. `background.ts` receives message, reads backendUrl from storage, POSTs to `/api/v1/trades`
4. `background.ts` returns response to content script
5. `content.ts` shows success/error toast

## Backend API Contract (existing, no changes)

**POST /api/v1/trades**
```json
{
  "symbol": "BTC_USDC",
  "side": "buy",
  "quantity": "0.001",
  "entry_price": "95000.50",
  "stop_loss_price": "94000.00",
  "take_profit_price": "97000.00"
}
```
Headers: `X-User-Id: <uuid>`, `Content-Type: application/json`

## Key Files

| File | Purpose |
|------|---------|
| `testudo-extension/src/content.ts` | Wire executeTrade() to message background |
| `testudo-extension/src/background.ts` | Add EXECUTE_TRADE message handler with fetch() |
| `testudo-extension/manifest.json` | Add host_permissions for backend URL |

## Acceptance Criteria
- Alt+X -> Enter on a Long Position tool sends POST to backend
- Backend returns 200 with order group details
- Extension shows "Order Sent" toast notification
- Failed requests (backend down, invalid symbol) show error toast
- Content script communicates via background worker (not direct fetch)

## Verification
```bash
cd testudo-extension && bun run typecheck && bun run build
# Manual: load in Chrome, open TradingView, Alt+X, Enter
# Check: curl http://localhost:8080/api/v1/trades -H "X-User-Id: test-user"
```

