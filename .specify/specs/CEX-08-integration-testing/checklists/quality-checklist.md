# Quality Checklist — CEX-08 Integration Testing

**Spec ID:** CEX-08-integration-testing
**Date:** 2026-03-15

## Build Verification

- [x] `cd testudo-cex && bun install` succeeds
- [x] `bun run build` succeeds — 504 modules, 48ms
- [x] `bun run start` launches server on port 3100
- [x] `GET /health` returns `{"ok": true}`
- [x] `cd testudo-exchange && cargo clippy --all-targets` clean (2 warnings, 0 errors)
- [x] `cargo test` all pass

## Automated Integration Tests (tests/integration.test.ts)

- [x] Health endpoint returns {ok: true} via real HTTP
- [x] Balance, position, orders, cancel, leverage endpoints work end-to-end
- [x] Bracket order placement returns entry + SL + TP order IDs
- [x] WebSocket subscribes and acknowledges
- [x] Fill events forwarded as order_update with status=closed
- [x] Store-diff cancellation detection (status=canceled)
- [x] OCO verified: SL triggers → TP cancelled (2 events: closed + canceled)
- [x] Reconciler detects orphaned SL/TP orders
- [x] Reconciler preserves entry orders
- [x] Reconciler skips symbols with active positions
- [x] Full pipeline: reconciler → cancel → WS event
- [x] Full lifecycle: bracket place → entry fill → SL fill → TP cancel → no orphans
- [x] No orphaned orders after lifecycle (reconciler verification)
- [x] All 117 tests pass, 0 failures

## Testnet Integration (tests/testnet-integration.test.ts)

- [x] Test script created with WOO_TESTNET_KEY/SECRET env var gating
- [x] Tests gracefully skip when credentials not provided (8 skip)
- [ ] Bracket order placed on WOO X testnet (entry + SL + TP)
- [ ] Entry fill event received by fill_detector
- [ ] SL fill event received by fill_detector (algo stream)
- [ ] OCO verified: SL triggers → TP cancelled
- [ ] No orphaned orders after lifecycle

## Reconciler

- [x] Orphaned orders detected when WebSocket event missed
- [x] Reconciler cancels orphaned TP
- [x] Synthetic cancellation event reaches WS client

## Production Validation

- [ ] Live WOO X bracket order with minimal position
- [ ] All events arrive correctly
- [ ] Clean state after trade closure
