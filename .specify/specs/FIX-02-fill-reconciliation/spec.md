# Specification: WebSocket Fill Price Reconciliation

**Spec ID:** FIX-02-fill-reconciliation
**Date:** 2026-03-16
**Status:** Complete
**Class:** Feature / Financial Correctness
**Priority:** P0 — wrong fill prices reported to position tracker
**Depends on:** FIX-01 (Decimal migration must complete first)
**Series:** FIX-01 through FIX-07 (Hyperliquid audit remediation)
**Audit Refs:** Critical #2, High #9

---

## Problem Statement

The WebSocket fill subscriber uses `order.limit_px` (the limit price of the order) as the average fill price. For limit orders at rest, this is correct — the fill price equals the limit price. But for market orders (IOC at extreme slippage price), stop-market orders, and any order experiencing slippage, `limit_px` diverges significantly from the actual execution price.

The Hyperliquid SDK's `BasicOrder` struct (in WS `OrderUpdate` messages) **does not contain an `avg_px` field** — this was confirmed against the SDK documentation. Only the REST `FilledOrder` type has `avg_px`. This is an architectural gap that requires a REST reconciliation query after fill detection.

Additionally, fills that occur during WebSocket reconnection are permanently lost. There is no watermark or reconciliation mechanism to detect the gap.

---

## User Stories

- **As a trader**, I want accurate fill prices in my position tracker, so that my P&L calculations are correct.
- **As a trader**, I want fills during connectivity issues to be recovered, so that no trades are silently lost.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | After WS reports `status: "filled"`, query REST for the true `avg_px` from `FilledOrder` | High | Router (ws_fills.rs) |
| FR-2 | Enrich `OrderUpdateEvent.average` with the REST-derived avg_px before forwarding to fill detector | High | Router (ws_fills.rs) |
| FR-3 | Track last known `status_timestamp` as a watermark | High | Router (ws_fills.rs) |
| FR-4 | After successful WS reconnect, query recent order updates since watermark | High | Router (ws_fills.rs) |
| FR-5 | Deduplicate events seen via both WS and reconciliation query | Medium | Router (ws_fills.rs) |

---

## Technical Implementation

### Fill Price Enrichment

After the WS reports a fill, query `InfoProvider::user_state()` or `InfoProvider::frontend_open_orders()` to find the order's actual average fill price. The WS event triggers the query; the enriched event is forwarded downstream.

```rust
// In HyperliquidFillSubscriber — after translate() produces a "closed" event:
if event.status == "closed" {
    // Query REST for actual fill price
    match self.info.frontend_open_orders(self.user_address).await {
        Ok(orders) => {
            // Find matching order by OID and extract avg_px if available
            // Note: filled orders may not appear in open_orders — use user_state
        }
        Err(e) => {
            tracing::warn!("Fill price reconciliation failed: {e}, using limit_px");
        }
    }
}
```

**Alternative approach**: Subscribe to `user_fills` or `user_events` WS channel if the SDK exposes it. The fills endpoint typically includes execution price. Check SDK for `Subscription::UserFills` or similar.

### Reconnect Reconciliation

```rust
// After successful reconnect in run():
if let Some(last_ts) = self.last_event_timestamp {
    // Query REST for any fills since last_ts
    // This catches fills that occurred during the reconnect gap
    self.reconcile_since(last_ts).await;
}
```

### Deduplication

Use a bounded `HashSet<u64>` (keyed by OID) of recently-seen events to prevent duplicates when both WS and reconciliation report the same fill.

### Files

- `crates/router/src/services/hyperliquid/ws_fills.rs` — main changes
- `crates/router/src/services/hyperliquid/exchange_api.rs` — may need to expose InfoProvider query methods

### Dependencies

- `HyperliquidFillSubscriber` needs access to `InfoProvider` (currently it only has `network`, `user_address`, and `order_update_sender`)

---

## Acceptance Criteria

- [x] Filled orders have `average` populated from REST `avg_px`, not WS `limit_px`
- [x] If REST query fails, `limit_px` is used as fallback with a warning log
- [x] `last_event_timestamp` watermark tracked across reconnects
- [x] After reconnect, fills since watermark are queried and forwarded
- [x] Duplicate events are filtered (same OID not sent twice)
- [x] Unit tests verify fill price enrichment for limit vs market orders
- [x] Integration test (ignored, testnet-gated) verifies REST reconciliation
- [x] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **REST rate limiting** — Querying REST on every fill adds latency and API calls. Mitigation: batch fills within a 100ms window; only query REST for fills where price matters (market/stop orders).
2. **Stale data** — REST query may not yet reflect the fill if there's replication lag. Mitigation: retry once after 500ms if order not found.
3. **InfoProvider shared state** — `InfoProvider` is currently not in `HyperliquidFillSubscriber`. Must be injected at construction time.

---

## Completion Signal

This spec is complete when:
1. Fill prices are accurate for market and stop orders
2. Reconnect gaps are reconciled
3. No duplicate events reach the fill detector
4. All tests pass
5. Code committed to master
