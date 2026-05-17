# CLN-03-typed-engine-errors — Implementation Plan

## Current State Summary

The classic matching engine (`engine/src/engine/engine.rs`) returns `Result<_, &str>` and `Result<_, ()>` everywhere. `error.rs` is fully commented out. The shadow engine (`shadow/actor.rs`) already has a typed `EngineError` enum (actor-level: `ActorShutdown`, `Internal(String)`), but the classic engine's errors are still strings.

The spec's FR-5 says "update callers in shadow/actor.rs" — but `actor.rs` does NOT call the classic `Engine`. The shadow engine is a separate codebase. Actual callers of the classic engine are `order.rs` (queue handler) and engine-internal tests.

### Error site inventory

| File | Function | Current Return | String Errors |
|------|----------|---------------|---------------|
| `engine.rs` | `create_order_pg` | `Result<String, &str>` | "Legacy engine deprecated" |
| `engine.rs` | `get_open_order` | `Result<&Order, ()>` | orderbook not found |
| `engine.rs` | `cancel_order` | `Result<String, &str>` | orderbook not found, cancel failed |
| `engine.rs` | `cancel_all_orders` | `Result<String, &str>` | orderbook not found |
| `engine.rs` | `check_and_lock_funds` | `Result<(), &str>` | user not found, mutex lock failed, no balance, insufficient funds/quantity |
| `engine.rs` | `update_user_balance` | `Result<(), &str>` | delegates to update_balance_with_lock |
| `engine.rs` | `update_balance_with_lock` | `Result<(), &str>` | user not found, mutex lock failed, no balance |
| `types/engine.rs` | `Asset::parse` | `Result<Asset, &'static str>` | invalid asset string |
| `order.rs` | `handle_order_pg` | n/a (handles via println) | matches on `Err(())` and `Err(err)` |

## Checkpoints

### CP-1: Define `CoreEngineError` enum + migrate `engine.rs` ✅
- Completed 2026-05-17 by /skill:vox build
- Defined `CoreEngineError` in `engine/src/engine/error.rs` with 9 variants covering all error sites.
- Migrated `engine.rs`, `orderbook.rs`, `types/engine.rs` to typed errors.
- **Touches**: `engine/src/engine/error.rs` (rewrite), `engine/src/engine/engine.rs` (all `&str` → `CoreEngineError`, `()` → `CoreEngineError`)
- **Tasks**:
  1. Define `CoreEngineError` enum in `error.rs` with variants: `OrderbookNotFound`, `UserNotFound`, `OrderNotFound`, `InsufficientFunds`, `InsufficientQuantity`, `BalanceNotFound`, `MutexLockFailed`, `Deprecated`, `AssetParseError`
  2. Implement `Display` and `std::error::Error`
  3. Replace all `Result<_, &str>` and `Result<_, ()>` in `engine.rs` with `Result<_, CoreEngineError>`
  4. Update `Asset::parse` to return `Result<Asset, CoreEngineError>` (or its own error mapped)
- **Verification**: `cargo check -p engine` compiles; `cargo test -p engine` passes

### CP-2: Migrate `order.rs` callers ✅
- Completed 2026-05-17 by /skill:vox build
- Updated `Err(())` → `Err(_)` pattern. `Err(err)` already works via Display.
- **Touches**: `engine/src/order.rs`
- **Tasks**:
  1. Replace `Err(())` match with `Err(CoreEngineError::...)` pattern
  2. Replace `Err(err)` string matching with typed error matching
  3. Wire error Display output for pg_notify responses
- **Verification**: `cargo check -p engine` passes; engine tests pass

### CP-3: Add `#[deprecated]` handling and final cleanup ✅
- Completed 2026-05-17 by /skill:vox build
- `create_order_pg` returns `CoreEngineError::Deprecated`. Removed `clippy::result_unit_err` allow. Exported `CoreEngineError` from lib.rs.
- Zero `Result<_, &str>` or `Result<_, ()>` remain in engine public API.
- **Touches**: `engine/src/engine/engine.rs` (create_order_pg), `engine/src/lib.rs`
- **Tasks**:
  1. Confirm `create_order_pg` is actually deprecated and no callers remain
  2. Remove dead `create_order_pg` code path or keep with `Deprecated` error variant
  3. Ensure all `Result<_, &str>` signatures are eliminated from engine crate public API
- **Verification**: `rg 'Result.*&str' crates/engine/src/` returns zero hits (excluding non-error uses)

### CP-4: Full CI verification ✅
- Completed 2026-05-17 by /skill:vox build
- 742 passed, 0 failed, 22 ignored. Clippy passes.
- **Touches**: All
- **Tasks**:
  1. `cargo clippy --all-targets` passes
  2. `cargo test` passes (742 passed, 0 failed, 22 ignored)
- **Verification**: Both commands exit 0

## Risks

1. **`Asset::parse` returns `&'static str`** — Converting this to `CoreEngineError` requires adding the error type to `types/engine.rs` or mapping at call sites. Prefer mapping at call sites with `.map_err(|_| CoreEngineError::AssetParseError { ... })`.

2. **`order.rs` `eprintln!` for missing pubsub_id** — These aren't actual error returns, just diagnostic prints with early returns. They remain as-is (not part of the typed error surface).

3. **Naming conflict with `shadow::actor::EngineError`** — `CoreEngineError` avoids the conflict. The classic engine's `error.rs` gets the new type; `actor.rs` is untouched.
