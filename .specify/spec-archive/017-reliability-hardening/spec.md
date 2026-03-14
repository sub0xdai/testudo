# Specification: Reliability Hardening

**Spec ID:** 017-reliability-hardening
**Date:** 2026-03-10
**Status:** Complete
**Class:** Hardening
**Origin:** HFT gap analysis — filtered for retail-grade reliability wins

---

## Overview

Four targeted fixes to eliminate silent data loss, panic risks, debug noise, and unnecessary network latency in the Rust backend. These were surfaced by a mechanical sympathy audit but filtered through the lens of what actually matters for a CCXT-routed retail trading overlay.

**Current state:**
- Fill events use a lossy `tokio::broadcast(256)` — if the FillDetector lags, fills are silently discarded, causing shadow engine state to diverge from exchange reality.
- 5 `unwrap()` calls on `serde_json::to_string()` in `ws_stream.rs` can panic and crash the WebSocket publisher thread.
- 4 `println!()` calls in `routes/trade.rs` bypass structured logging.
- WebSocket TCP connections use Nagle's algorithm (no `TCP_NODELAY`), adding ~40ms latency to fill event delivery.

**Target state:**
- Fill events are never silently lost. Backpressure is explicit.
- WebSocket serialization failures are logged and skipped, never panicked.
- All stdout debug output replaced with `tracing` macros.
- WebSocket connections transmit frames immediately (Nagle disabled).

---

## Constraint: Zero Regression

Every FR must preserve existing behavior. The changes are strictly:
- Channel semantics (lossy → lossless) — same data, stronger delivery guarantee
- Panic → graceful error — same happy path, safer sad path
- `println!` → `tracing::debug!` — same information, proper sink
- Socket option toggle — same connection, lower latency

No new features. No API changes. No schema changes.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Replace `tokio::broadcast(256)` for `OrderUpdateEvent` with `tokio::mpsc` bounded channel. FillDetector is the sole consumer — broadcast semantics are unnecessary. Use `mpsc::channel(1024)` so the producer (`WsSubscriptionManager`) blocks on backpressure instead of dropping events. | Critical | Router / IPC |
| FR-2 | In `FillDetectorService::run()`, remove the `Lagged` error arm (no longer applicable with mpsc). Keep the `Closed` arm for shutdown. | Critical | Router / Fill Detector |
| FR-3 | Add a reconciliation log on startup: after rehydration, compare shadow engine order group count against exchange open orders count. Log any mismatch as `tracing::warn!`. This provides a safety net if fills were missed during downtime. | High | Router / Rehydration |
| FR-4 | Replace all 5 `serde_json::to_string().unwrap()` calls in `ws_stream.rs` with `match` + `tracing::error!` on failure, then `continue` or `return` (skip the bad message, don't crash). | Critical | Engine / WS Stream |
| FR-5 | Replace all 4 `println!()` calls in `routes/trade.rs` with `tracing::debug!()` using structured fields. | Medium | Router / Routes |
| FR-6 | In `ws-stream/src/main.rs`, call `stream.set_nodelay(true)` on each accepted `TcpStream` before passing it to `accept_connection()`. | Medium | WS Stream / Network |
| FR-7 | Add test: send 1024+ events through the fill channel, verify none are lost. | Critical | Test |
| FR-8 | Add test: `WsResponse` serialization with an intentionally non-serializable payload does not panic. | High | Test |

---

## Technical Implementation

### 1) broadcast → mpsc for Fill Events (FR-1, FR-2)

**File:** `crates/router/src/main.rs` (lines 315-319)

Current:
```rust
let (order_update_tx, order_update_rx) =
    tokio::sync::broadcast::channel::<services::OrderUpdateEvent>(256);
```

Replace with:
```rust
let (order_update_tx, order_update_rx) =
    tokio::sync::mpsc::channel::<services::OrderUpdateEvent>(1024);
```

**File:** `crates/router/src/services/fill_detector.rs` (lines 67-87)

Change `run()` signature from `broadcast::Receiver` to `mpsc::Receiver`:
```rust
pub async fn run(&self, mut rx: mpsc::Receiver<OrderUpdateEvent>) {
    tracing::info!("FillDetectorService started");

    while let Some(event) = rx.recv().await {
        self.handle_order_update(event).await;
    }

    tracing::info!("FillDetector channel closed, shutting down");
}
```

**File:** `crates/router/src/services/ws_subscription_manager.rs`

Update sender type from `broadcast::Sender` to `mpsc::Sender`. Replace `.send()` with `.send().await` (or `.try_send()` with explicit error handling and metric increment — never silent drop).

**Key behavioral change:** The producer (WsSubscriptionManager) will now apply backpressure when the channel is full instead of silently dropping events. If fill processing is slow, the WS subscription task will pause reading from the sidecar WebSocket until the channel drains. This is correct — it's better to slow down ingestion than lose fills.

### 2) Startup Reconciliation (FR-3)

**File:** `crates/router/src/services/rehydration.rs`

After rehydration completes, add:
```rust
let shadow_group_count = engine.order_groups.read().await.active_count();
let exchange_orders = ccxt_client.fetch_open_orders(&symbol).await?;
if shadow_group_count != exchange_orders.len() {
    tracing::warn!(
        shadow = shadow_group_count,
        exchange = exchange_orders.len(),
        "Post-rehydration mismatch: shadow groups vs exchange orders"
    );
}
```

This is a diagnostic log, not a corrective action. It surfaces divergence for manual review.

### 3) Eliminate unwrap() Panics (FR-4)

**File:** `crates/engine/src/engine/ws_stream.rs` (lines 59, 121, 156, 218, 290)

Replace each instance of:
```rust
let ws_response_string = serde_json::to_string(&ws_response).unwrap();
```

With:
```rust
let ws_response_string = match serde_json::to_string(&ws_response) {
    Ok(s) => s,
    Err(e) => {
        tracing::error!(error = %e, stream = %stream, "Failed to serialize WsResponse, skipping");
        continue; // or return, depending on enclosing control flow
    }
};
```

Note: 3 of the 5 call sites are inside loops (`continue`), 2 are in standalone functions (`return`). Adjust control flow accordingly.

### 4) Replace println! with tracing (FR-5)

**File:** `crates/router/src/routes/trade.rs` (lines 31, 36, 47, 57, 84)

| Line | Current | Replacement |
|------|---------|-------------|
| 31 | `println!("Get Trades: {}", symbol)` | `tracing::debug!(symbol = %symbol, "get_trades")` |
| 36 | `println!("Cache HIT for {} ({:?})", ...)` | `tracing::debug!(cache_key = %cache_key, elapsed = ?starttime.elapsed(), "cache_hit")` |
| 47 | `println!("Timeout: {:?}", ...)` | `tracing::debug!(elapsed = ?starttime.elapsed(), "db_connection_failed")` |
| 57 | `println!("Timeout: {:?}", ...)` | `tracing::debug!(elapsed = ?starttime.elapsed(), "query_failed")` |
| 84 | `println!("Cache MISS for {} ({:?})", ...)` | `tracing::debug!(cache_key = %cache_key, elapsed = ?starttime.elapsed(), "cache_miss")` |

### 5) Enable TCP_NODELAY (FR-6)

**File:** `crates/ws-stream/src/main.rs` (lines 53-61)

```rust
while let Ok((stream, _)) = listener.accept().await {
    if let Err(e) = stream.set_nodelay(true) {
        tracing::warn!(error = %e, "Failed to set TCP_NODELAY");
    }
    tokio::spawn(accept_connection(stream, ...));
}
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `crates/router/src/main.rs` | FR-1: broadcast → mpsc channel creation |
| `crates/router/src/services/fill_detector.rs` | FR-2: mpsc::Receiver, remove Lagged arm |
| `crates/router/src/services/ws_subscription_manager.rs` | FR-1: mpsc::Sender, backpressure-aware send |
| `crates/router/src/services/rehydration.rs` | FR-3: post-rehydration reconciliation log |
| `crates/engine/src/engine/ws_stream.rs` | FR-4: unwrap → match + tracing::error |
| `crates/router/src/routes/trade.rs` | FR-5: println → tracing::debug |
| `crates/ws-stream/src/main.rs` | FR-6: set_nodelay(true) |

---

## Verification

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

- [ ] `broadcast::channel` replaced with `mpsc::channel` for fill events
- [ ] FillDetectorService compiles with mpsc::Receiver
- [ ] WsSubscriptionManager sends with backpressure (no silent drops)
- [ ] Post-rehydration reconciliation log fires on count mismatch
- [ ] All 5 `unwrap()` calls in ws_stream.rs replaced with match
- [ ] All 4 `println!()` calls in trade.rs replaced with tracing
- [ ] `set_nodelay(true)` called on accepted WebSocket streams
- [ ] All existing tests pass (zero regression)
- [ ] New test: 1024+ fill events delivered without loss
- [ ] New test: bad serialization payload logged, not panicked

---

## Completion Signal

All verification checkboxes green. `cargo clippy --all-targets && cargo test` passes. No new warnings.
