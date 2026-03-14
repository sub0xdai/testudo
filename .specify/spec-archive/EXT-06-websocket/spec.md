# EXT-06: WebSocket Connection & Real-Time Status

> Priority: P1 | Depends on: EXT-05 | Status: COMPLETE

## Overview
**Current:** Extension communicates with backend via REST only. Status indicator in popup is static ("Disconnected"). Trade confirmations are fire-and-forget (no real-time feedback after REST response).
**Target:** Background worker maintains a WebSocket connection to `ws-stream` server (port 4000). Subscribes to `order.{user_id}` for real-time order updates. Popup status indicator reflects live connection state. Order events are forwarded to content script as toast notifications.

## Functional Requirements

| ID | Requirement | Status |
|----|-------------|--------|
| FR-1 | WS connection -- background worker connects to ws-stream server using configurable WS URL | DONE |
| FR-2 | Auto-reconnect -- exponential backoff (1s -> 30s max) on disconnect or error | DONE |
| FR-3 | Order subscription -- on connect, send SUBSCRIBE for `order.{user_id}` channel | DONE |
| FR-4 | Connection state -- track connecting/connected/disconnected, broadcast to popup via message | DONE |
| FR-5 | Popup status indicator -- status dot and text update in real-time (green=connected, pulse=connecting, red=disconnected) | DONE |
| FR-6 | Order notifications -- forward order stream messages to content script as toast notifications | DONE |

## Architecture

### WebSocket Protocol (existing ws-stream server)
- **URL:** `ws://localhost:4000` (dev), configurable via popup
- **Subscribe:** `{ "method": "SUBSCRIBE", "params": ["order.{user_id}"], "id": 1 }`
- **Unsubscribe:** `{ "method": "UNSUBSCRIBE", "params": ["order.{user_id}"], "id": 1 }`
- **Server messages:** `{ "stream": "order.{user_id}", "data": { ... } }`

### Background Worker (connection lifecycle)
- Connect on startup if backendUrl is configured
- Reconnect on settings change (new URL)
- Subscribe to `order.{user_id}` using PAPER_USER_ID or JWT-decoded user ID
- Broadcast connection state changes to popup via runtime messages
- Forward order events to active TradingView tabs via tab messaging

### Popup (status display)
- Query WS_STATUS on open
- Listen for WS_STATE_CHANGED broadcasts
- Update status dot color and text

## Key Files

| File | Change |
|------|--------|
| `testudo-extension/src/background.ts` | Add WS client, reconnect logic, state management, message forwarding |
| `testudo-extension/src/popup/popup.ts` | Query and display WS connection state |
| `testudo-extension/src/popup/popup.html` | Add wsUrl input field |
| `testudo-extension/src/content.ts` | Handle WS_ORDER_UPDATE messages as toasts |

## Verification
```bash
cd testudo-extension && bun run build
# Manual: load in Chrome, open popup, verify status indicator
# Manual: start ws-stream server, verify "Connected" status
```
