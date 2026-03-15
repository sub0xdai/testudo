# Quality Checklist — CEX-08 Integration Testing

**Spec ID:** CEX-08-integration-testing
**Date:** 2026-03-15

## Build Verification

- [ ] `cd testudo-cex && bun install` succeeds
- [ ] `bun run build` succeeds
- [ ] `bun run start` launches server on port 3100
- [ ] `GET /health` returns `{"ok": true}`
- [ ] `cd testudo-exchange && cargo clippy --all-targets` clean
- [ ] `cargo test` all pass

## Testnet Integration

- [ ] Bracket order placed on WOO X testnet (entry + SL + TP)
- [ ] All three order IDs returned
- [ ] Entry fill event received by fill_detector
- [ ] SL fill event received by fill_detector (algo stream)
- [ ] OCO verified: SL triggers -> TP cancelled
- [ ] No orphaned orders after lifecycle

## Reconciler

- [ ] Orphaned orders detected when WebSocket event missed
- [ ] Reconciler cancels orphaned TP
- [ ] Synthetic cancellation event reaches Rust backend

## Production Validation

- [ ] Live WOO X bracket order with minimal position
- [ ] All events arrive correctly
- [ ] Clean state after trade closure
