# Specification: Native WebSocket Fill Subscription

**Spec ID:** HL-04-ws-fills
**Date:** 2026-03-16
**Status:** Draft
**Class:** Feature / Exchange Integration
**Priority:** P1 — required for live fill detection
**Depends on:** HL-02 (auth for signing WS subscription)
**Series:** HL-01 through HL-06 (native Hyperliquid integration)

---

## Problem Statement

Fill detection currently flows through the sidecar WebSocket (`/ws/orders`). Hyperliquid accounts need a native Rust WebSocket path using the SDK's `WsProvider` that translates HL order events into the existing `OrderUpdateEvent` type consumed by `FillDetectorService`.

---

## User Stories

- **As a trader**, I want real-time fill notifications for Hyperliquid orders, so that position management reacts immediately.
- **As a developer**, I want Hyperliquid fill events to feed into the same `FillDetectorService` as sidecar events, so that no downstream changes are needed.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `HyperliquidFillSubscriber` connects via `WsProvider::connect(network)` | High | Router |
| FR-2 | Subscribe to `subscribe_order_updates(addr)` for order status changes | High | Router |
| FR-3 | Translate HL `OrderUpdate` events → `OrderUpdateEvent` (existing type from `cex_client.rs`) | High | Router |
| FR-4 | Feed translated events into existing `mpsc::Sender<OrderUpdateEvent>` channel | High | Router |
| FR-5 | Auto-reconnect with exponential backoff (mirrors `WsSubscriptionManager` pattern) | High | Router |
| FR-6 | Graceful shutdown via `CancellationToken` or `watch` channel | Medium | Router |

---

## Technical Implementation

### Translation Mapping

```
HL OrderUpdate status "filled"   → OrderUpdateEvent { status: "closed", filled: qty, ... }
HL OrderUpdate status "canceled" → OrderUpdateEvent { status: "canceled", ... }
HL OrderUpdate status "open"     → OrderUpdateEvent { status: "open", remaining: qty, ... }
```

### SDK WebSocket Types

```rust
// From hyperliquid-sdk-rs
Message::OrderUpdates(OrderUpdates { data: Vec<OrderUpdate> })

OrderUpdate {
    order: BasicOrder { coin, side, limit_px, sz, oid, timestamp, orig_sz, cloid },
    status: String,           // "open", "filled", "canceled", etc.
    status_timestamp: u64,
}
```

### Target Type (existing, from `cex_client.rs`)

```rust
pub struct OrderUpdateEvent {
    pub id: String,           // oid.to_string()
    pub symbol: String,       // AssetUniverse::from_hl_coin(coin)
    pub status: String,       // translated status
    pub side: String,         // "buy" or "sell" (lowercase)
    pub price: Option<f64>,   // limit_px parsed
    pub amount: Option<f64>,  // orig_sz parsed
    pub filled: Option<f64>,  // orig_sz - sz for filled
    pub remaining: Option<f64>, // sz parsed
    pub average: Option<f64>, // limit_px for filled orders
    pub timestamp: Option<i64>, // status_timestamp as i64
}
```

### Files

- `crates/router/src/services/hyperliquid/ws_fills.rs`
- Update `crates/router/src/services/hyperliquid/mod.rs`

### Reuse

- `OrderUpdateEvent` type from `cex_client.rs` — consumed by `FillDetectorService` unchanged
- Reconnection pattern from `WsSubscriptionManager` (exponential backoff, `wait_or_cancel`)

---

## Acceptance Criteria

- [ ] `HyperliquidFillSubscriber` connects to HL WebSocket
- [ ] Order status changes translated to `OrderUpdateEvent`
- [ ] Events forwarded to `mpsc::Sender<OrderUpdateEvent>`
- [ ] Auto-reconnect with exponential backoff on disconnect
- [ ] Graceful shutdown on cancellation signal
- [ ] Unit tests verify correct `OrderUpdateEvent` translation for filled/canceled/open states
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **WebSocket disconnects** — Hyperliquid periodically disconnects WebSocket clients. Mitigation: exponential backoff reconnection.
2. **Event ordering** — events may arrive out of order. Mitigation: FillDetectorService already handles this.

---

## Completion Signal

This spec is complete when:
1. Fill subscriber connects and translates events
2. Events feed into existing fill detection pipeline
3. Reconnection is robust
4. All unit tests pass
5. Code committed to master
