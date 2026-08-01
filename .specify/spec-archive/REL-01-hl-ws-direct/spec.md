# Specification: Bypass SDK WebSocket — Direct tokio-tungstenite Connection for HL Fills

**Spec ID:** REL-01-hl-ws-direct
**Date:** 2026-03-30
**Status:** Draft
**Class:** Infrastructure / Reliability
**Priority:** P0 — The SDK's `RawWsProvider` silently drops connections without recovery. Fill detection has been broken since Mar 27, causing missed stop-loss events and orphaned exchange positions with real money at risk.
**Depends on:** None (first in series)
**Series:** REL-01 (Reliability hardening)

---

## Problem Statement

The Hyperliquid fill subscriber (`ws_fills.rs`) uses the SDK's `RawWsProvider` for WebSocket connectivity. This provider has a **fatal design flaw**: `start_reading()` at line 351 of the SDK calls `self.ws.take()`, consuming the socket handle. After this call, `ping()` always fails with "Not connected" because `self.ws` is `None`. The `ManagedWsProvider` has the same bug — its `keepalive_loop` immediately triggers reconnection after `start_reading()` because it can't ping a taken socket.

The result: zero application-level keepalive messages are sent. Hyperliquid's load balancer / server-side idle timeout kills the connection silently. The TCP socket goes half-open. `read_frame().await` blocks forever with no timeout. The message channel never closes. Our `msg_rx.recv()` never returns `None`. We never reconnect. **All fill events are permanently lost.**

Production evidence: 10+ router restarts on 2026-03-30 show "HL fill subscriber: connected and subscribed" followed by zero order update events across 12+ hours. The subscriber connected successfully every time but never received a single message. A trader's stop-loss fired on the exchange, consuming margin, but the system never detected it — the position shows as "active" in the extension despite being closed on Hyperliquid.

The fix: bypass both SDK WS providers entirely. Connect directly with `tokio-tungstenite` (already a workspace dependency at v0.24.0), own the read/write split, send application-level pings (`{"method":"ping"}`) every 30 seconds, and wrap all reads in `tokio::time::timeout` to detect silent death.

---

## User Stories

- **As a trader**, I want the system to maintain a healthy, verified WebSocket connection to Hyperliquid, so that my stop-loss and take-profit fills are detected within seconds — not missed for hours.
- **As the system operator**, I want the WS connection to self-diagnose and self-heal via keepalive pings and read timeouts, so that I don't need to manually restart services after silent connection drops.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Connect directly to `wss://api.hyperliquid.xyz/ws` via `tokio-tungstenite` with TLS (native-tls feature). No SDK WS provider used. | High | ws_fills |
| FR-2 | Send subscription JSON manually: `{"method":"subscribe","subscription":{"type":"orderUpdates","user":"0x..."}}` as a Text frame after connect. | High | ws_fills |
| FR-3 | Single `tokio::select!` control loop handling: (a) frame reads with 60s timeout, (b) application-level ping every 30s, (c) REST poll every 30s, (d) stop signal. | High | ws_fills |
| FR-4 | Application-level ping: send `{"method":"ping"}` as Text frame every 30 seconds. HL responds with `{"channel":"pong"}`. This resets the read timeout. | High | ws_fills |
| FR-5 | Read timeout: if no frame (data, pong, ping, anything) arrives within 60 seconds, consider the connection dead, drop the socket, and reconnect with exponential backoff. | High | ws_fills |
| FR-6 | On ping write failure, read error, stream end, or read timeout: break the control loop, drop the socket cleanly, and enter the reconnection backoff. | High | ws_fills |
| FR-7 | Parse incoming text frames for `orderUpdates` channel using local JSON types (`HlWsMessage`, `HlOrderUpdate`, `HlBasicOrder`). No dependency on SDK's `Message` / `OrderUpdate` types for WS. | High | ws_fills |
| FR-8 | Preserve all existing behavior: translate(), reconcile_since(), enrich_fill_price(), record_oid(), build_event_from_fills(), startup 24h reconciliation, REST poll. | High | ws_fills |
| FR-9 | `HyperliquidFillSubscriber::new()` signature unchanged — no caller changes required in `ws_subscription_manager.rs`. | Medium | ws_fills |
| FR-10 | Add `native-tls` feature to `tokio-tungstenite` workspace dependency for WSS support. | Medium | Cargo.toml |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Rewrite `ws_fills.rs`: replace `RawWsProvider` with direct tokio-tungstenite connection, own JSON types, control loop with ping/timeout | `cargo clippy --all-targets && cargo test` passes, all 20+ unit tests pass |
| CP-2 | Deploy and verify via production logs: pong responses received, read timeout never fires during active connection, fills detected within seconds | Log verification on server |

### Architecture: Before vs After

| Aspect | Before (SDK) | After (Direct) |
|--------|-------------|----------------|
| Connection | `RawWsProvider::connect()` → hyper HTTP upgrade → fastwebsockets | `tokio_tungstenite::connect_async()` → native TLS → tungstenite |
| Read loop | SDK's spawned task: `while let Ok(frame) = ws.read_frame().await` (no timeout, ignores pings) | Our `tokio::select!` with `tokio::time::timeout(60s, read.next())` |
| Ping | `ws.ping()` — broken after `start_reading()` consumes socket | Direct `write.send(Text(r#"{"method":"ping"}"#))` every 30s |
| Message types | SDK's `Message`, `OrderUpdate`, `BasicOrder`, `Subscription` | Our own `HlWsMessage`, `HlOrderUpdate`, `HlBasicOrder` (serde) |
| Read/Write split | Impossible — SDK spawns read task, takes ownership | `ws_stream.split()` → `(write, read)` — both accessible in select loop |

### Control Loop

```rust
loop {
    tokio::select! {
        _ = stop_rx.changed() => { /* graceful close */ return Ok(()); }

        result = tokio::time::timeout(60s, read.next()) => {
            match result {
                Ok(Some(Ok(frame))) => self.handle_frame(frame).await,
                Ok(Some(Err(e)))    => return Err(read_error),
                Ok(None)            => return Err(stream_ended),
                Err(_)              => return Err(read_timeout),
            }
        }

        _ = ping_interval.tick() => {
            write.send(Text(r#"{"method":"ping"}"#))?;  // fails → reconnect
        }

        _ = poll_interval.tick() => {
            self.reconcile_since(now - 5min).await;     // REST fallback
        }
    }
}
```

### Local JSON Types

```rust
#[derive(Deserialize)]
struct HlWsMessage {
    channel: Option<String>,
    data: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HlOrderUpdate {
    order: HlBasicOrder,
    status: String,
    status_timestamp: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HlBasicOrder {
    coin: String, side: String, limit_px: String,
    sz: String, oid: u64, orig_sz: String, cloid: Option<String>,
}
```

### Paved Roads

- **`tokio-tungstenite` v0.24.0** (workspace dep, `Cargo.toml:28`): Already used by the router crate. Add `native-tls` feature for WSS.
- **`futures_util::SinkExt/StreamExt`** (workspace dep via `futures-util = "0.3.30"`): Used for `.split()`, `.send()`, `.next()` on the WS stream.
- **`reconnect_delay()` / `wait_or_cancel()`** (`utils/reconnect.rs`): Existing exponential backoff helpers used by the current subscriber.
- **`InfoProvider`** (`hyperliquid_sdk_rs`): Still used for REST fill queries (`user_fills_by_time`). Only the WS layer is bypassed.
- **`AssetUniverse::from_hl_coin()`** (`hyperliquid/universe.rs`): Symbol normalization (BTC → BTC_USDT).

### Files

- `crates/router/src/services/hyperliquid/ws_fills.rs` — Complete rewrite: remove SDK WS imports, add tokio-tungstenite direct connection with keepalive and read timeout
- `Cargo.toml` (workspace root) — Add `native-tls` feature to `tokio-tungstenite`

### Dependencies Added

- `tokio-tungstenite` feature `native-tls` — enables TLS for WSS connections (the crate itself is already a dependency)

---

## Acceptance Criteria

- [ ] No imports from `hyperliquid_sdk_rs::RawWsProvider` or SDK WS types (`Message`, `OrderUpdate`, `Subscription`) in `ws_fills.rs`.
- [ ] Connection established directly via `tokio_tungstenite::connect_async("wss://api.hyperliquid.xyz/ws")`.
- [ ] Application-level ping (`{"method":"ping"}`) sent every 30 seconds as Text frame.
- [ ] Read timeout of 60 seconds wraps all frame reads — triggers reconnect on silent death.
- [ ] Ping write failure, read error, stream end, or read timeout all trigger clean reconnect with exponential backoff.
- [ ] All 20+ existing unit tests pass with local `HlOrderUpdate` type instead of SDK's `OrderUpdate`.
- [ ] `HyperliquidFillSubscriber::new()` signature unchanged — `ws_subscription_manager.rs` requires zero changes.
- [ ] Startup 24h REST reconciliation, periodic 30s REST poll, OID dedup, fill price enrichment all preserved.
- [ ] Production logs show pong responses after deployment (confirms keepalive is working).
- [ ] `cargo clippy --all-targets && cargo test` passes.

---

## Risks

1. **HL subscription JSON format** — The subscription payload format (`{"method":"subscribe","subscription":{"type":"orderUpdates","user":"0x..."}}`) is derived from the SDK source, not official HL docs. If HL changes this format, the subscription silently fails. Mitigation: The subscription confirmation message (`{"channel":"subscriptionResponse",...}`) is logged. If missing after connect, the periodic REST poll catches fills within 30s as a fallback.

2. **Address formatting** — Hyperliquid may expect checksummed vs lowercased Ethereum addresses. The `format!("{:#x}", address)` produces lowercase with `0x` prefix. Mitigation: This matches the format the SDK uses internally. If HL rejects it, REST reconciliation still works as a fallback. Can be verified in production logs immediately after deployment.

3. **native-tls system dependency** — `native-tls` feature requires OpenSSL on Linux. Mitigation: The production server already has OpenSSL (the existing `hyper-rustls` in the SDK dependency chain uses it). If build fails, switch to `rustls-tls-native-roots` feature instead.

---

## Completion Signal

This spec is complete when:
1. `ws_fills.rs` uses tokio-tungstenite directly with 30s pings and 60s read timeout
2. No SDK WS provider imported or used
3. All unit tests pass with local JSON types
4. Production logs confirm pong responses and order update delivery
5. `cargo clippy --all-targets && cargo test` passes
6. Code committed to master
