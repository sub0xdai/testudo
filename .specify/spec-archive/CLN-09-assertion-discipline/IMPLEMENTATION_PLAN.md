# CLN-09-assertion-discipline — Implementation Plan

## Current State Summary

Zero production assertions exist in `engine.rs` and `shadow/balances.rs`. All `assert!`/`assert_eq!` calls are in test code only. The TigerBeetle audit correctly identified this as a safety gap for financial software.

Key invariants that should be checked:

| Invariant | Where | Type |
|-----------|-------|------|
| Balance non-negativity after mutation | `shadow/balances.rs` (reserve, release, deduct, add) | `assert!` |
| Balance non-negativity after engine mutations | `engine.rs` (check_and_lock_funds, update_balance_with_lock) | `assert!` |
| Valid order state transitions | `shadow/orders.rs` (fill, cancel) | `assert!` |

Orderbook price ordering and trade ID monotonicity are already enforced by `BTreeMap` and the engine's `trade_id` counter — `debug_assert!` would be redundant noise. Fill quantity consistency is mathematically guaranteed by `std::cmp::min`. Focus on invariants that COULD be violated by future code changes.

## Checkpoints

### CP-1: Add balance assertions to `shadow/balances.rs` ✅
- Completed 2026-05-17 by /skill:vox build
- 6 assertions: reserve(), release(), deduct_reserved(), add() — all check non-negativity post-mutation.
- **Touches**: `engine/src/shadow/balances.rs`
- **Tasks**:
  1. `reserve()` — assert available ≥0, reserved ≥0 after mutation
  2. `release()` — same
  3. `deduct_reserved()` — same
  4. `add()` — same
- **Verification**: `cargo test -p engine` — all balance tests pass

### CP-2: Add balance assertions to `engine.rs` ✅
- Completed 2026-05-17 by /skill:vox build
- 6 assertions: check_and_lock_funds (BUY + SELL branches), update_balance_with_lock. All check balance non-negativity.
- **Touches**: `engine/src/engine/engine.rs`
- **Tasks**:
  1. `check_and_lock_funds()` — assert balance.available ≥0 after mutation
  2. `update_balance_with_lock()` — assert available ≥0, locked ≥0 after mutation
- **Verification**: `cargo test -p engine` — engine tests pass

### CP-3: Add state transition assertions to `shadow/orders.rs` ✅
- Completed 2026-05-17 by /skill:vox build
- 2 assertions: fill() and cancel() assert the order is open before transitioning.
- **Touches**: `engine/src/shadow/orders.rs`
- **Tasks**:
  1. Order fill transition: only Open/PartiallyFilled orders can be filled
  2. Order cancel transition: only Open/PartiallyFilled orders can be cancelled
- **Verification**: `cargo test -p engine` — order tests pass

### CP-4: Full verification ✅
- Completed 2026-05-17 by /skill:vox build
- 742 passed, 0 failed. Clippy clean. 14 production assertions, all hold.
- **Verification**: `cargo test` 742 passed, `cargo clippy --all-targets` clean
- **Commit message**: `feat: add assertion discipline to engine and shadow balances`
