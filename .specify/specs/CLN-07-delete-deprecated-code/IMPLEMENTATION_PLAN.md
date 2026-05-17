# CLN-07-delete-deprecated-code — Implementation Plan

## Current State Summary

Three items assessed:

| Item | Status | Action |
|------|--------|--------|
| `Engine::create_order_pg()` | `#[deprecated]`, always returns `Err`. Still called from `order.rs` queue handler. | Delete function + update caller |
| `check_fills()` | **Not deprecated.** Active production function in `shadow/orders.rs` for fill checking. | Keep — TigerBeetle audit was incorrect |
| `#[deprecated]` scan | Only one hit: `create_order_pg` | Remove the only one |

The `check_fills` function in `shadow/orders.rs` is a legitimate shadow engine fill-checker — it has no `#[deprecated]` attribute and is actively tested. The TigerBeetle audit misidentified it. This simplifies the spec to a single item.

## Checkpoints

### CP-1: Delete `create_order_pg()` and update caller ✅
- Completed 2026-05-17 by /skill:vox build
- Deleted function from engine.rs, removed `Deprecated` error variant, updated order.rs handler to return direct JSON error. `cargo fix` cleaned unused imports.
- **Touches**: `engine/src/engine/engine.rs` (remove function), `engine/src/order.rs` (remove call)
- **Tasks**:
  1. Delete `create_order_pg` function from `engine.rs` (~12 lines)
  2. Replace `OrderRequests::CreateOrder` match arm in `order.rs` with direct JSON error response (no engine call needed)
  3. Remove unused import `PgQueueManager` if this was the only user
- **Verification**: `cargo check --workspace` compiles; `cargo test` passes
- **Commit message**: `refactor: delete deprecated create_order_pg`

### CP-2: Verify zero dead_code + full test suite ✅
- Completed 2026-05-17 by /skill:vox build
- `rg create_order_pg` returns only comment references. 742 passed, 0 failed. Clippy clean.
- **Touches**: None (verification only)
- **Tasks**:
  1. `cargo clippy --all-targets` — no dead_code warnings on engine crate
  2. `cargo test` — 742 passed, 0 failed
  3. `rg create_order_pg` returns zero matches
- **Verification**: All three pass

## Risks

- None. `create_order_pg` always returns `Err` — deleting it cannot break anything that was working.
