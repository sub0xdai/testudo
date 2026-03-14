# Quality Checklist — 017-reliability-hardening

**Spec ID:** 017-reliability-hardening
**Date:** 2026-03-10

---

## Pre-Implementation

- [ ] Read `main.rs` and confirm broadcast channel creation at lines 315-319
- [ ] Read `fill_detector.rs` and confirm `run()` signature takes `broadcast::Receiver`
- [ ] Read `ws_subscription_manager.rs` and confirm sender type is `broadcast::Sender`
- [ ] Read `ws_stream.rs` and locate all 5 `unwrap()` call sites
- [ ] Read `routes/trade.rs` and locate all `println!()` calls
- [ ] Read `ws-stream/src/main.rs` and confirm no `set_nodelay` call exists
- [ ] Run `cargo test` baseline — all tests pass before changes

## FR-1/FR-2: broadcast → mpsc for Fill Events

- [ ] Replace `broadcast::channel(256)` with `mpsc::channel(1024)` in `main.rs`
- [ ] Update `FillDetectorService::run()` to accept `mpsc::Receiver`
- [ ] Remove `Lagged` error handling arm (not applicable to mpsc)
- [ ] Update `WsSubscriptionManager` sender type to `mpsc::Sender`
- [ ] Verify backpressure: sender blocks or logs on full (never silent drop)
- [ ] `cargo test` — all existing tests still pass

## FR-3: Post-Rehydration Reconciliation

- [ ] Add reconciliation log after rehydration in `rehydration.rs`
- [ ] Log counts: shadow group count vs exchange open order count
- [ ] Use `tracing::warn!` with structured fields on mismatch
- [ ] Verify no functional behavior change (log only, no corrective action)

## FR-4: Eliminate unwrap() Panics

- [ ] Replace unwrap at ws_stream.rs line 59 with match + tracing::error
- [ ] Replace unwrap at ws_stream.rs line 121 with match + tracing::error
- [ ] Replace unwrap at ws_stream.rs line 156 with match + tracing::error
- [ ] Replace unwrap at ws_stream.rs line 218 with match + tracing::error
- [ ] Replace unwrap at ws_stream.rs line 290 with match + tracing::error
- [ ] Correct control flow: `continue` in loops, `return` in standalone fns
- [ ] `cargo test` — all existing tests still pass

## FR-5: Replace println! with tracing

- [ ] Replace println at trade.rs line 31 with tracing::debug
- [ ] Replace println at trade.rs line 36 with tracing::debug
- [ ] Replace println at trade.rs line 47 with tracing::debug
- [ ] Replace println at trade.rs line 57 with tracing::debug
- [ ] Replace println at trade.rs line 84 with tracing::debug

## FR-6: Enable TCP_NODELAY

- [ ] Add `stream.set_nodelay(true)` in ws-stream accept loop
- [ ] Handle `set_nodelay` error with `tracing::warn`

## FR-7/FR-8: New Tests

- [ ] Test: send 1024+ events through mpsc channel, assert all received
- [ ] Test: non-serializable WsResponse does not panic

## Post-Implementation

- [ ] `cargo clippy --all-targets` passes with no warnings
- [ ] `cargo test` — all existing + new tests pass
- [ ] No new `unwrap()` calls in production code
- [ ] No `println!()` calls remain in production code
- [ ] Grep for `broadcast::channel` — only non-fill channels remain
