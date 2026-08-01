# CLN-04-remove-unwrap — Implementation Plan

## Current State Summary

Three production `unwrap()` calls identified, all outside `#[cfg(test)]` blocks:

1. **`engine.rs:54`** — `init_engine` calls `get_latest_trade_id_from_db(...).unwrap()`. Only called from engine's dev binary `main.rs`, but the public API should still handle errors.
2. **`request_response.rs:129-130`** — `serde_json::to_string(...).unwrap()` / `from_str(...).unwrap()` — both inside a `#[test]` function. Covered by clippy's test exemption.
3. **`actor.rs:1401`** — `self.engine.order_groups.get_group_mut(group_id).unwrap()` in `UpdateGroupStopLoss` handler. Production code path — the group lookup is already checked by a preceding `if let Some(group) = ...` block, making the `unwrap()` logically redundant but prone to TOCTOU.

The remaining 42+ unwraps in actor.rs are all in `#[cfg(test)]` test functions — acceptable per spec.

## Checkpoints

### CP-1: Fix `engine.rs:54` — DB unwrap in `init_engine` ✅
- Completed 2026-05-17 by /skill:vox build
- Added `Internal { detail }` to `CoreEngineError`. Changed `init_engine` → `Result<(), CoreEngineError>`. Propagated DB error via `map_err`.
- **Touches**: `engine/src/engine/engine.rs`, `engine/src/engine/error.rs`, `engine/src/main.rs`
- **Tasks**:
  1. Add `Internal { detail: String }` variant to `CoreEngineError`
  2. Change `init_engine` to return `Result<(), CoreEngineError>`, propagate DB error
  3. Update caller in `engine/src/main.rs` to handle the `Result`
- **Verification**: `cargo check -p engine` compiles; engine binary builds

### CP-2: Fix `actor.rs:1401` — group fetch unwrap ✅
- Completed 2026-05-17 by /skill:vox build
- Replaced `.unwrap()` with explicit `match` → `Some(g)` / `None` → reply with `EngineError::Internal`.
- Also fixed `order_group.rs:535` — replaced `is_some() && unwrap()` pattern with `is_some_and()`.
- **Touches**: `engine/src/shadow/actor.rs`
- **Tasks**:
  1. Replace `.unwrap()` with `ok_or(EngineError::Internal(...))` or restructure to reuse the existing `if let Some(group)` check
  2. Remove the double-get_group + get_group_mut pattern
- **Verification**: `cargo check -p engine` compiles; actor tests pass

### CP-3: Enable clippy lint + final verification ✅
- Completed 2026-05-17 by /skill:vox build
- Added `#![cfg_attr(not(test), deny(clippy::unwrap_used))]`. Zero violations. 742 passed, 0 failed.
- **Touches**: `engine/src/lib.rs`
- **Tasks**:
  1. Add `#![cfg_attr(not(test), deny(clippy::unwrap_used))]` to engine crate
  2. Verify clippy produces zero `unwrap_used` warnings in non-test code
  3. Full test suite passes
- **Verification**: `cargo clippy --all-targets` has no `unwrap_used` outside test code; `cargo test` 742 passed

## Risks

1. **`init_engine` return type change** — Only called from engine's own dev binary `main.rs`. Router doesn't use `init_engine`. Minimal blast radius.
2. **`request_response.rs` unwraps** — Already in a `#[test]` function; clippy's `not(test)` exemption handles this automatically. No code change needed.
3. **`actor.rs:1401` TOCTOU** — The current code pattern checks `if let Some(group) = get_group(group_id) {... let group = get_group_mut(...).unwrap()}`. The second lookup could fail if another task removed the group between the two calls. The `if let` already guards, but the `get_group_mut().unwrap()` pattern is fragile. The cleanest fix is to restructure to use the mutable reference from the guard itself, or use `ok_or`.
