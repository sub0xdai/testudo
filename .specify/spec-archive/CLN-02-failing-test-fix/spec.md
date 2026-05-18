# Specification: Fix 2 Failing Tests

**Spec ID:** CLN-02-failing-test-fix
**Date:** 2026-05-15
**Status:** Draft
**Class:** Testing / Fix
**Priority:** P0 — failing tests block CI, signal broken invariants in production code
**Depends on:** CLN-01 (dependency hardening — tests must pass after crate updates)
**Series:** CLN-01 through CLN-09 (Phase 1 — Open-Source Readiness Cleanup)

---

## Problem Statement

`cargo test` in `testudo-exchange` reports **2 failing tests** out of 740 passing. These have been failing since at least the TigerBeetle comparison audit (2026-05-06). Failing tests are:

1. **`routes::auth::tests::test_me_returns_user_info`** — panics with `assertion left == right failed: left: Null, right: "361d96ce-e1d9-4d64-accf-9d1e8749e784"`. The test expects a specific UUID in the `/me` response but the response field is `Null`.

2. **`services::integration_tests::tests::test_reconciliation_pending_entry_gone`** — panics with `assertion left == right failed: left: 0, right: 1`. A reconciliation assertion expects 1 pending entry but finds 0.

The constitution mandates: *"Never delete a failing test to make the pipeline pass. Fix the implementation."* These tests encode real expectations. The job is to understand what changed in the implementation that broke them and fix the implementation.

---

## User Stories

- **As a developer**, I want `cargo test` to pass with zero failures, so that CI gates are meaningful and production invariants are verifiable.
- **As a contributor**, I want failing tests to reflect actual bugs, not bit-rotted assertions, so that I can trust the test suite.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `test_me_returns_user_info` passes — `/me` endpoint returns the correct user UUID | High | router/auth |
| FR-2 | `test_reconciliation_pending_entry_gone` passes — reconciliation correctly identifies pending entries after sync | High | router/integration_tests |
| FR-3 | Root cause documented in test comments explaining why the fix was needed | Medium | Both |
| FR-4 | No existing tests are disabled or deleted | High | All |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Diagnose both failures — read test code and endpoint/service code | Root cause identified for both tests |
| CP-2 | Fix `test_me_returns_user_info` — adjust implementation | Test passes, `/me` still works correctly |
| CP-3 | Fix `test_reconciliation_pending_entry_gone` — adjust implementation | Test passes, reconciliation still correct |
| CP-4 | Full `cargo test` run — 740+ passing, 0 failing | Green test suite |

### Test 1: `test_me_returns_user_info`

**Location:** `testudo-exchange/crates/router/src/routes/auth.rs:695`

**Failure:** `assertion left == right failed: left: Null, right: "361d96ce-e1d9-4d64-accf-9d1e8749e784"`

**Diagnosis approach:**

```bash
# Read the test to understand expected behavior
# Line ~695 in auth.rs
```

The test creates a user, calls `/me`, and expects the response to contain a matching user ID. The `Null` on the left suggests either:
- The test user isn't being persisted correctly before the `/me` call
- The `/me` handler no longer returns `user_id` in the expected field
- A recent schema change dropped or renamed the `user_id` field in the `/me` response struct

**Likely fix:** The `/me` response type changed (e.g., `user_id` was renamed, moved to a nested field, or the user creation in the test doesn't match the new schema). Read the current `MeResponse` struct and align the test assertion.

### Test 2: `test_reconciliation_pending_entry_gone`

**Location:** `testudo-exchange/crates/router/src/services/integration_tests.rs:812`

**Failure:** `assertion left == right failed: left: 0, right: 1`

This test verifies that after a sync operation, a previously pending reconciliation entry is gone (processed). The assertion expects 1 entry to have been processed, but finds 0.

**Diagnosis approach:**

- Read lines 780–830 of `integration_tests.rs` to understand the test flow
- Check the reconciliation service logic — did the event processing pipeline change?
- Check if the pending entry is being filtered or skipped in a new code path
- Verify the test setup creates the entry with the correct status/state

**Likely fix:** A change in the reconciliation event processing (possibly related to `JNL-SYNC-01` pull-based journal changes or recent pending-group reconciliation grace period) altered how pending entries are counted or processed. Align the test expectations with the current reconciliation behavior, or fix the reconciliation logic if the test expectation is correct.

### Paved Roads

- Existing test patterns in `auth.rs` tests and `integration_tests.rs` — follow the setup/assert patterns
- `MeResponse` struct — likely in `crates/router/src/models/` or `crates/router/src/routes/auth.rs`
- Reconciliation logic — `crates/router/src/services/sync_service.rs` or `integration_tests.rs`
- Test helper utilities — user creation, exchange account setup in `integration_tests.rs`

### Files

- `testudo-exchange/crates/router/src/routes/auth.rs` — read test (line ~695), possibly fix `/me` handler
- `testudo-exchange/crates/router/src/services/integration_tests.rs` — read test (line ~812), fix reconciliation logic or test
- `testudo-exchange/crates/router/src/models/` — check `MeResponse` and related types

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `test_me_returns_user_info` passes
- [ ] `test_reconciliation_pending_entry_gone` passes
- [ ] `cargo test` shows 742 passing, 0 failing, 22 ignored
- [ ] Both test files include comments explaining the fix
- [ ] No test code was deleted or `#[ignore]`'d to achieve green
- [ ] `cargo clippy --all-targets` passes

---

## Risks

1. **Fix for test 1 breaks the real `/me` endpoint for live users.** Mitigation: verify `/me` returns correct data with a manual curl or integration environment test.
2. **Test 2 was asserting a real bug.** If the assertion "expect 1, got 0" reveals that reconciliation genuinely drops entries, the fix is in the reconciliation engine, not the test. Do not weaken the assertion — fix the engine.

---

## Completion Signal

This spec is complete when:
1. Both failing tests pass
2. `cargo test` output shows zero failures
3. Root cause documented in each test
4. Code committed to master
