# CLN-02-failing-test-fix — Implementation Plan

## Current State Summary

`cargo test` shows 2 failures out of 740 tests. Root cause analysis complete:

1. **`test_me_returns_user_info`**: The `/me` handler wraps response in `"user": {"id": ..., "wallet_address": ...}` but the test reads `body["user_id"]` and `body["wallet_address"]` (flat keys, now `Null`). The handler was restructured to nest under `"user"` and the test wasn't updated.

2. **`test_reconciliation_pending_entry_gone`**: A 60-second grace period was added to the `OrderGroupStatus::Pending` reconciliation handler to prevent race conditions. The test creates a group with `created_at = now()` (default), which falls within the grace period, causing `determine_reconcile_actions` to skip it and return 0 actions. The test should backdate the group's `created_at` to bypass the grace period, matching the pattern used by other reconciliation tests.

Both are test-assertion drift — the production code is correct, the tests just need alignment.

## Checkpoints

### CP-1: Fix `test_me_returns_user_info` ✅
- Completed 2026-05-17 by /skill:vox build
- Updated `body["user_id"]` → `body["user"]["id"]` and `body["wallet_address"]` → `body["user"]["wallet_address"]` to match nested `/me` response format.
- **Touches**: `testudo-exchange/crates/router/src/routes/auth.rs` (test only, ~line 695)
- **Tasks**:
  1. Change `body["user_id"]` → `body["user"]["id"]`
  2. Change `body["wallet_address"]` → `body["user"]["wallet_address"]`
  3. Add comment explaining the nested response format
- **Verification**: `cargo test -p router --bin router -- routes::auth::tests::test_me_returns_user_info` passes
- **Commit message**: `fix: align test_me_returns_user_info with nested /me response format`

### CP-2: Fix `test_reconciliation_pending_entry_gone` ✅
- Completed 2026-05-17 by /skill:vox build
- Added `group.created_at` backdating by 120s to bypass the 60s grace period added to pending reconciliation handler.
- **Touches**: `testudo-exchange/crates/router/src/services/integration_tests.rs` (test only, ~line 812)
- **Tasks**:
  1. Add `group.created_at = chrono::Utc::now() - chrono::Duration::seconds(120);` before `determine_reconcile_actions` call
  2. Add comment explaining the 60s grace period and why backdating is needed
- **Verification**: `cargo test -p router --bin router -- services::integration_tests::tests::test_reconciliation_pending_entry_gone` passes
- **Commit message**: `fix: backdate pending group in reconciliation test to bypass 60s grace period`

### CP-3: Full `cargo test` verification ✅
- Completed 2026-05-17 by /skill:vox build
- 742 passed, 0 failed, 22 ignored. Clippy passes.
- **Touches**: None (verification only)
- **Tasks**:
  1. `cargo test` — 742 passing, 0 failing, 22 ignored
  2. `cargo clippy --all-targets` — passes
- **Verification**: Both commands exit 0
- **Commit message**: `test: verify zero failures after CLN-02 fixes`

## Risks

- **None.** Both fixes are test-only changes that don't touch production code. The `/me` handler response format is confirmed correct (nested under `user`). The grace period is a deliberate production safeguard. These are classic assertion-drift fixes.
