# Specification: Delete Deprecated Code (`create_order_pg`, `check_fills`)

**Spec ID:** CLN-07-delete-deprecated-code
**Date:** 2026-05-15
**Status:** Draft
**Class:** Refactor / Code Quality
**Priority:** P1 — dead deprecated code confuses readers, bloats binary, and signals poor hygiene
**Depends on:** CLN-01 (dependency hardening must be resolved first so cargo check is clean)
**Series:** CLN-01 through CLN-09 (Phase 1 — Open-Source Readiness Cleanup)

---

## Problem Statement

The codebase carries deprecated code that should have been removed once callers were migrated:

1. **`Engine::create_order_pg()`** (`engine.rs:98-119`) — explicitly `#[deprecated]` since 0.2.0 with the message *"Use Shadow Engine via Decision Loop for risk-validated order execution"*. The function body explicitly returns an error to force callers to use the new path. If it's been returning `Err(...)` and there are zero legitimate callers, it's dead weight.

2. **`check_fills()`** referenced in the TigerBeetle audit as *"deprecated check_fills function — suggests code churn without cleanup"*. Need to locate and verify.

3. **Old spec references.** The archived spec `009-redis-removal/PRD.md` references the deprecated `create_order` function. The `.specify/spec-archive/` entries are historical — they stay.

Other possibly deprecated items to scan:
- Any `#[deprecated]` attribute on non-test functions
- Functions with doc comments or comments saying "TODO: remove"
- Dead code flagged by `cargo clippy` (though clippy may not catch all since `create_order_pg` is `pub`)

The constitution states: *"Delete dead code. No backwards-compatibility hacks unless explicitly required."*

---

## User Stories

- **As a developer**, I want zero dead code in the engine, so that I can trust that every function I see is actually called and tested.
- **As a reviewer**, I want `#[deprecated]` annotations to have a removal plan, not be permanent fixtures.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Delete `Engine::create_order_pg()` and any test coverage that only exercises it | High | engine/engine.rs |
| FR-2 | Identify and delete `check_fills()` if deprecated and caller-free | High | TBD |
| FR-3 | Scan for all `#[deprecated]` annotations — assess each for removal | Medium | All |
| FR-4 | `cargo clippy --all-targets` passes with no dead_code warnings | High | All |
| FR-5 | `cargo test` passes — removed code's tests are deleted, not broken | High | All |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Delete `create_order_pg()` — remove function + its test coverage | Engine compiles, no dead_code warnings |
| CP-2 | Locate and assess `check_fills()` — delete if deprecated | Engine compiles |
| CP-3 | Full scan for `#[deprecated]` and dead code | All deprecated items assessed (kept or removed) |
| CP-4 | `cargo clippy --all-targets && cargo test` green | Zero regressions |

### Item 1: `Engine::create_order_pg()`

**Location:** `testudo-exchange/crates/engine/src/engine/engine.rs:98-119`

```rust
#[deprecated(
    since = "0.2.0",
    note = "Use Shadow Engine via Decision Loop for risk-validated order execution"
)]
pub async fn create_order_pg(
    &mut self,
    input_order: CreateOrder,
    _pg_queue: &Arc<PgQueueManager>,
) -> Result<String, &str> {
    eprintln!(
        "[DEPRECATED] Legacy Engine::create_order_pg() called for user {}. Orders must use Decision Loop.",
        input_order.user_id
    );
    Err("Legacy engine deprecated. Use Decision Loop for order execution.")
}
```

**Action:** Delete the function and any tests that call it. Verify with:
```bash
rg "create_order_pg" testudo-exchange/crates/
```

If any non-test caller exists (unlikely, since the function returns `Err`), update the caller to use the Decision Loop path first.

### Item 2: Locate `check_fills()`

**Action:**
```bash
rg "check_fills" testudo-exchange/crates/
rg "#\[deprecated\]" testudo-exchange/crates/ --include='*.rs'
```

If `check_fills` has the `#[deprecated]` attribute and no callers (or test-only callers), delete it. If it's used in production paths, remove the deprecation and add a comment explaining why it's still needed.

### Item 3: Full Deprecated Scan

Run and triage:
```bash
rg "#\[deprecated" testudo-exchange/crates/ --include='*.rs' -A2
```

For each hit, determine:
- Is it called in any non-deprecated code? (`rg "function_name\(" testudo-exchange/crates/ --include='*.rs' | grep -v deprecated`)
- If dead: delete.
- If still needed: create a spec to migrate callers, remove `#[deprecated]`, add TODO comment.

### Paved Roads

- Constitution principle: *"Delete dead code. No backwards-compatibility hacks."*
- The `create_order_pg` function already returns an error — it's self-documenting dead code
- Standard Rust practice: delete, don't comment-out

### Files

- `testudo-exchange/crates/engine/src/engine/engine.rs` — delete `create_order_pg` (lines 98-119)
- `testudo-exchange/crates/engine/src/engine/engine.rs` — possibly delete more deprecated items
- Any test files that reference deleted functions — remove those tests

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `create_order_pg()` is deleted from `engine.rs`
- [ ] All test code testing `create_order_pg()` is deleted (not broken)
- [ ] Full scan for `#[deprecated]` completed — all instances assessed
- [ ] Any remaining `#[deprecated]` annotations have a documented removal plan (spec reference)
- [ ] `cargo clippy --all-targets` shows zero `dead_code` warnings on engine crate
- [ ] `cargo test` passes (no broken tests from deleted functions)
- [ ] `rg "create_order_pg" testudo-exchange/crates/` returns zero matches

---

## Risks

1. **`create_order_pg` is called by integration tests that also test other things.** Mitigation: read each test before deleting — if the test exercises other functions alongside `create_order_pg`, remove only the deprecated call, not the whole test.
2. **`check_fills` isn't actually deprecated.** The TigerBeetle audit may have misidentified it. Mitigation: verify before deleting — if it's actively used, drop this item from the spec.

---

## Completion Signal

This spec is complete when:
1. `create_order_pg()` is fully removed
2. All identified deprecated code is either deleted or has a removal plan
3. `cargo clippy --all-targets && cargo test` passes
4. Code committed to master
