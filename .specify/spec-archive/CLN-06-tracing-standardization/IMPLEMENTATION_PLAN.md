# CLN-06-tracing-standardization — Implementation Plan

## Current State Summary

19 `eprintln!` calls remain in production code (down from the spec's 22 — 3 were removed during CLN-03 typed error work). Distribution:

| Crate | Count | Context |
|-------|-------|---------|
| engine/engine.rs | 3 | Orderbook not found, deprecated path |
| engine/ws_stream.rs | 1 | Orderbook not found |
| engine/order.rs | 6 | Missing pubsub_id (can't respond) |
| engine/user.rs | 1 | Missing pubsub_id |
| ws-stream/ | 6 | TCP_NODELAY, listen/unlisten, invalid subscription, notification errors |
| sqlx_postgres/api_keys.rs | 1 | Test-only DB not available — keep |
| router/main.rs | 2 | Config validation, migration failure |

**Key finding**: `ws-stream/src/main.rs` does NOT initialize a tracing subscriber. Replacing `eprintln!` with `tracing::warn!`/`error!` there will produce silent output. Need to add subscriber init.

## Checkpoints

### CP-1: Replace `eprintln!` in `engine/` ✅
- Completed 2026-05-17 by /skill:vox build
- Replaced 11 instances in engine.rs, order.rs, user.rs, ws_stream.rs. Added `clippy::print_stderr` deny.
- **Touches**: 4 files, 11 instances
- **Tasks**:
  1. `engine.rs`: orderbook not found → `tracing::warn!`, deprecated → `tracing::error!`
  2. `order.rs`: missing pubsub_id → `tracing::warn!`
  3. `user.rs`: missing pubsub_id → `tracing::warn!`
  4. `ws_stream.rs`: orderbook not found → `tracing::warn!`
  5. Add `#![cfg_attr(not(test), deny(clippy::print_stderr))]` to engine lib.rs
- **Verification**: `cargo check --workspace`, engine tests pass

### CP-2: Replace `eprintln!` in `ws-stream/` + add tracing subscriber ✅
- Completed 2026-05-17 by /skill:vox build
- Added `tracing`/`tracing-subscriber` deps, `fmt::init()` at startup. Replaced 6 instances.
- **Touches**: `ws-stream/src/main.rs`, `ws-stream/src/pg_ws_manager.rs`
- **Tasks**:
  1. Add `tracing_subscriber::fmt::init()` at start of ws-stream `main()`
  2. Replace 6 `eprintln!` calls with `tracing::warn!`/`tracing::error!`
  3. Add `tracing`, `tracing-subscriber` to ws-stream Cargo.toml if missing
- **Verification**: `cargo check -p ws-stream`

### CP-3: Replace `eprintln!` in `router/src/main.rs` + clippy lint ✅
- Completed 2026-05-17 by /skill:vox build
- Replaced 2 instances (config validation, migration failure). 742 passed, 0 failed, clippy clean.
- **Touches**: `router/src/main.rs`
- **Tasks**:
  1. Config validation failure → `tracing::error!`
  2. Migration failure → `tracing::error!`
- **Verification**: `cargo check --workspace`, full test suite, clippy clean

## Risks

1. **ws-stream missing tracing subscriber** — Adding `tracing_subscriber::fmt::init()` is trivial and consistent with router's pattern. No risk.
2. **api_keys.rs test-only eprintln** — Kept as-is. Clippy's `not(test)` exemption handles it.
