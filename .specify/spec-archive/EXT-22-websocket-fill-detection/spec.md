# EXT-22: WebSocket Fill Detection

| Field    | Value                                              |
|----------|----------------------------------------------------|
| Status   | Draft                                              |
| Date     | 2026-03-01                                         |
| Depends  | EXT-21, 012-ccxt-multi-exchange                    |
| Phase    | Backend — Real-Time Order Lifecycle                |

## 1. Overview

### Current State

OCO exchange cancellation (just implemented) piggybacks on the shadow engine's `process_price_update()` to detect SL/TP fills. The shadow engine infers fills by checking whether Binance ticker prices cross order trigger levels every 2 seconds. This works but has known gaps:

- **Race conditions**: SL fills on exchange, price bounces back above SL before next poll — shadow engine never sees the cross, TP stays open.
- **Double execution**: Both SL and TP fill within the same 2s window. Shadow engine fires cancels for both siblings, but they've already executed. User ends up in an unintended opposite position.
- **Partial fills**: Exchange fills 60% of TP. Shadow engine doesn't know — it either treats the order as fully filled or not filled. No partial state.
- **Stale UI**: Extension shows "Active" for trades the exchange already closed.

### Target State

The CCXT sidecar subscribes to exchange user data streams via `watchOrders()`. When an order status changes (open → closed, open → canceled, partial fill), the sidecar pushes the event to the Rust backend. The backend uses real fill events — not price inference — to trigger OCO cancellation, update OrderGroup state, and notify the extension.

### Design Principle

This spec replaces the **signal source**, not the logic. The OCO cancellation logic in `process_price_update()` Phase 3 and `PriceFeedService` remains intact as a fallback. WebSocket fill detection adds a faster, authoritative path that fires first. If the WebSocket path handles it, the shadow engine's price-based detection becomes a no-op (order already cancelled).

## 2. User Stories

- **US-1**: As a trader, when my SL fills on the exchange, the sibling TP is cancelled within milliseconds — not up to 2 seconds later.
- **US-2**: As a trader, when my TP partially fills, I see the correct filled/remaining quantities in the extension.
- **US-3**: As a trader, I see real-time order status updates (filled, cancelled) without waiting for the next price poll.

## 3. Functional Requirements

### FR-1: CCXT Sidecar WebSocket Endpoint

**File:** `testudo-ccxt/src/server.js`, `testudo-ccxt/src/handlers.js`

Add a WebSocket endpoint to the sidecar that:

1. Accepts a connection with exchange credentials and symbol list
2. Calls CCXT `exchange.watchOrders(symbol)` in a loop for each subscribed symbol
3. Pushes order update events to the connected client as JSON:

```json
{
  "event": "order_update",
  "data": {
    "id": "12345",
    "symbol": "BTC/USDT:USDT",
    "status": "closed",
    "side": "buy",
    "price": 50000.0,
    "amount": 0.1,
    "filled": 0.1,
    "remaining": 0.0,
    "average": 49998.5,
    "timestamp": 1709280000000
  }
}
```

4. Handles reconnection if the exchange WebSocket drops (CCXT handles this internally, but surface connection state).
5. Supports multiple symbols per connection (one user data stream covers all symbols on most exchanges).

**Note:** CCXT's `watchOrders()` is exchange-agnostic — works on WOO, Binance, Bybit, OKX. The sidecar doesn't need exchange-specific code.

### FR-2: Rust Backend WebSocket Client

**File:** `crates/router/src/services/ccxt_client.rs`

Add a method to `CcxtClient` that:

1. Opens a WebSocket connection to the sidecar's new WS endpoint
2. Sends credentials and symbol subscriptions
3. Receives order update events
4. Exposes events via a `tokio::sync::broadcast` channel

```rust
impl CcxtClient {
    pub async fn subscribe_orders(
        &self,
        credentials: SidecarCredentials,
        symbols: Vec<String>,
    ) -> Result<broadcast::Receiver<OrderUpdateEvent>, CcxtClientError>;
}
```

### FR-3: Fill Detection Service

**New file:** `crates/router/src/services/fill_detector.rs`

A new service that:

1. Subscribes to the `CcxtClient` order update broadcast
2. On `status: "closed"` (filled) events, looks up the exchange order ID in the shadow engine's OrderGroups
3. If the filled order is an SL → cancel TP on exchange (via `ExchangeApi::cancel_order`)
4. If the filled order is a TP → cancel SL on exchange
5. Updates the OrderGroup status accordingly
6. Broadcasts the fill event to the extension via the existing pg_queue NOTIFY channel

**Lookup path:** The fill detector needs to map `exchange_order_id` → `OrderGroup`. Add a reverse index to `OrderGroupManager`:

```rust
// New index in OrderGroupManager
groups_by_exchange_order: HashMap<String, Uuid>,
```

Register all three exchange IDs (entry, SL, TP) when they're stored in `create_trade`.

### FR-4: OrderGroup Exchange ID Index

**File:** `crates/engine/src/shadow/order_group.rs`

Add `groups_by_exchange_order: HashMap<String, Uuid>` to `OrderGroupManager`.

Add methods:

```rust
pub fn register_exchange_order(&mut self, exchange_order_id: String, group_id: Uuid);
pub fn get_by_exchange_order(&self, exchange_order_id: &str) -> Option<&OrderGroup>;
pub fn get_by_exchange_order_mut(&mut self, exchange_order_id: &str) -> Option<&mut OrderGroup>;
```

### FR-5: Extension Real-Time Order Updates

**File:** `testudo-extension/src/background.ts`

The extension already handles `WS_ORDER_UPDATE` messages. Update the handler to process fill events:

1. When an order update arrives with `status: "closed"`, update the local trade state
2. Remove the trade from the active list or mark it as filled
3. Update the popup UI if open

### FR-6: Graceful Degradation

If the sidecar WebSocket connection drops or `watchOrders` is unsupported by the exchange:

1. Log a warning
2. Fall back to the existing shadow engine price-based OCO (already implemented)
3. Retry WebSocket connection with exponential backoff (5s, 10s, 20s, 60s max)

The shadow engine OCO path is the safety net — it's slower but always works.

## 4. Files to Modify

| File | Change | Component |
|------|--------|-----------|
| `testudo-ccxt/src/server.js` | FR-1: Add WebSocket endpoint | Sidecar |
| `testudo-ccxt/src/handlers.js` | FR-1: watchOrders loop + event push | Sidecar |
| `crates/router/src/services/ccxt_client.rs` | FR-2: WebSocket client for sidecar | Backend |
| `crates/router/src/services/fill_detector.rs` | FR-3: New fill detection service | Backend |
| `crates/router/src/services/mod.rs` | FR-3: Register new module | Backend |
| `crates/engine/src/shadow/order_group.rs` | FR-4: Exchange order ID index | Engine |
| `crates/router/src/routes/trade_management.rs` | FR-4: Register exchange IDs in index | Backend |
| `crates/router/src/main.rs` | FR-3: Spawn fill detector service | Backend |
| `testudo-extension/src/background.ts` | FR-5: Handle fill events in UI | Extension |

## 5. Acceptance Criteria

- [ ] Sidecar exposes WebSocket endpoint that streams order updates via CCXT `watchOrders()`
- [ ] Rust backend connects to sidecar WebSocket and receives order events
- [ ] When SL fills on exchange, TP is cancelled within <500ms (not 2s)
- [ ] When TP fills on exchange, SL is cancelled within <500ms
- [ ] Partial fills update OrderGroup state correctly
- [ ] Extension shows real-time order status changes
- [ ] If sidecar WebSocket drops, shadow engine OCO continues to function as fallback
- [ ] No double cancellation — if WebSocket path cancels first, shadow engine price path is a no-op
- [ ] All existing tests pass (`cargo test`, `vitest run`)
- [ ] New tests: fill detector unit tests with mock order events

## 6. Architecture

```
Exchange (WOO/Binance/...)
    │ WebSocket user data stream
    ▼
CCXT Sidecar (Node.js)
    │ watchOrders() → JSON events
    │ WebSocket
    ▼
CcxtClient (Rust)
    │ broadcast::channel<OrderUpdateEvent>
    ▼
FillDetectorService (Rust)
    │ Looks up exchange_order_id → OrderGroup
    │ Fires ExchangeApi::cancel_order() for sibling
    │ Updates OrderGroup status
    │ pg_notify → Extension WebSocket
    ▼
Extension (background.ts)
    │ Updates popup UI
    ▼
User sees real-time fill status
```

## 7. Idempotency

The cancel path must be idempotent:

- `ExchangeApiError::OrderNotFound` is a graceful no-op (sibling already filled/cancelled)
- OrderGroup status is checked before acting — if already `StoppedOut` or `TookProfit`, skip
- The shadow engine's price-based OCO and the WebSocket fill detector may both try to cancel the same order. The second one gets `OrderNotFound` and logs it as debug. No conflict.

## 8. Out of Scope

- **Market data WebSocket**: This spec covers user order streams only, not price ticks. Price data continues via Binance REST polling.
- **Multi-user multiplexing**: Each exchange account gets its own WebSocket connection. Connection pooling across users is a future optimization.
- **ManagedPosition Pending→Filled transition**: The trade manager's position lifecycle is a separate concern — this spec focuses on OCO cancellation accuracy.

## 9. Verification

1. `cd testudo-exchange && cargo test` — all tests pass
2. `cd testudo-ccxt && npm test` — sidecar tests pass
3. `cd testudo-extension && npx vitest run` — extension tests pass
4. Manual: Place trade with SL+TP on WOO → wait for SL to trigger → verify TP cancelled within <1s on WOO dashboard
5. Manual: Kill sidecar WebSocket → verify shadow engine OCO still works as fallback

## 10. Completion Signal

All acceptance criteria checked. Live trade placed, SL triggered on exchange, TP cancelled via WebSocket fill detection (not price poll). Logs show "FillDetector: SL filled, cancelling TP" with <500ms latency from exchange fill to cancel request.
