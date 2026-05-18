# CLN-10-clippy-and-balance-atomicity — Implementation Plan

## Current State Summary

4 clippy warnings active:

| Warning | File | Already fixed in CLN-09? |
|---------|------|--------------------------|
| `unused import: OrderGroup` | `actor.rs` | Introduced by CLN-09 fix — needs removal |
| `useless_conversion` | `cex_client.rs:644` | Pre-existing — `.into()` on already-String |
| `unused variable: placed` | `actor.rs:1835` | Pre-existing — `_placed` fix |
| `manual_contains` | `evaluator.rs:188` | Pre-existing — `.iter().any()` → `.contains()` |

Balance atomicity: CLN-09 already added non-negativity `debug_assert!` in `check_and_lock_funds`. This spec adds the conservation assertion (`total_before == total_after`) and the spec's more comprehensive `debug_assert_eq!` pattern.

## Checkpoints

### CP-1: Fix 4 clippy warnings ✅
- Completed 2026-05-17 by /skill:vox build
- Removed unused OrderGroup import (with allow), _placed fix, msg_text.into(), iter().any()→contains()
- **Touches**: `actor.rs`, `cex_client.rs`, `evaluator.rs`
- **Tasks**:
  1. Remove unused `OrderGroup` import from `actor.rs` (only needed in tests via `super::*`)
  2. `msg_text.into()` → `msg_text` in `cex_client.rs`
  3. `placed` → `_placed` in `actor.rs` test
  4. `.iter().any()` → `.contains()` in `evaluator.rs`
- **Verification**: `cargo clippy --all-targets` shows zero warnings

### CP-2: Add conservation assertion to `check_and_lock_funds` ✅
- Completed 2026-05-17 by /skill:vox build
- Added `debug_assert_eq!(total_before, total_after)` after BUY and SELL mutations
- **Touches**: `engine/src/engine/engine.rs`
- **Tasks**:
  1. Add `debug_assert_eq!(total_before, total_after)` after BUY and SELL branches
  2. Capture `total_before = balance.available + balance.locked` before mutation
- **Verification**: `cargo test -p engine` passes; conservation assertion holds

### CP-3: Full verification ✅
- Completed 2026-05-17 by /skill:vox build
- `cargo clippy --all-targets -- -D warnings`: zero warnings. `cargo test`: 742 passed, 0 failed.
- **Verification**: `cargo clippy --all-targets` zero warnings; `cargo test` 742 passed
