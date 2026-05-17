# Specification: Fix Clippy Warnings + Harden `check_and_lock_funds` Atomicity

**Spec ID:** CLN-10-clippy-and-balance-atomicity
**Date:** 2026-05-15
**Status:** Draft
**Class:** Safety / Code Quality
**Priority:** P1 — clippy warnings are regressions-in-waiting; non-atomic balance mutations are the last Safety gap from the TigerBeetle audit
**Depends on:** CLN-01, CLN-03 (typed errors give us proper error types for the atomicity fix)
**Series:** CLN-01 through CLN-10 (Phase 1 — Open-Source Readiness Cleanup)

---

## Problem Statement

Two loose ends from the TigerBeetle audit remain unaddressed by CLN-01 through CLN-09:

### 1. Clippy Warnings (3 instances)

The DevSecOps audit (`DEVSECOPS_AUDIT.md`, 2026-04-03) found 3 active `cargo clippy` warnings:

| File | Line | Rule | Issue |
|------|------|------|-------|
| `crates/router/src/services/cex_client.rs` | 644 | `clippy::useless_conversion` | `.send(WsMessage::Text(msg_text.into()))` — `msg_text` is already `String`, `.into()` is a no-op |
| `crates/engine/src/shadow/actor.rs` | 1835 | `unused_variables` | `let placed = handle.place_order(...)` — `placed` is never read, should be `_placed` |
| `crates/router/src/services/trade_manager/evaluator.rs` | 188 | `clippy::manual_contains` | `!actions.iter().any(\|a\| *a == ...)` — should be `!actions.contains(&...)` |

These are trivial fixes but they signal sloppiness. The constitution requires: *"All code must pass linting and tests before commit."* Zero clippy warnings should be the baseline, enforced in CI.

### 2. `check_and_lock_funds` Lacks Atomicity Guarantees

The TigerBeetle audit flagged:

> *"`check_and_lock_funds` mutates balances directly across multiple hashmap lookups without atomicity guarantees."*

The function performs multiple `HashMap` lookups to read, check, and mutate balances. Between the "check" and "lock" operations, no atomicity mechanism prevents concurrent modifications. While the `Arc<Mutex<Engine>>` provides whole-engine serialization (only one thread accesses the engine at a time), the code structure doesn't make this invariant explicit. A future refactor that splits the Mutex could silently introduce a race condition.

The fix is defensive: add `debug_assert!` invariant checks after the mutation to verify balance consistency, making the implicit serialization contract visible and verifiable at runtime.

---

## User Stories

- **As a developer**, I want `cargo clippy` to return zero warnings, so that CI gates are meaningful and every warning signals a real problem.
- **As an auditor**, I want `check_and_lock_funds` to verify its own invariants after mutating balances, so that a future concurrency refactor can't silently break atomicity.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Fix `clippy::useless_conversion` in `cex_client.rs:644` | High | router |
| FR-2 | Fix `unused_variables` in `actor.rs:1835` | High | engine/shadow |
| FR-3 | Fix `clippy::manual_contains` in `evaluator.rs:188` | High | router/trade_manager |
| FR-4 | Add `debug_assert!` post-mutation balance invariants in `check_and_lock_funds` | High | engine/engine.rs |
| FR-5 | Add `#![deny(clippy::all)]` to the workspace or per-crate lib.rs files (with `#[allow]` for intentional exceptions) | Medium | All crates |
| FR-6 | `cargo clippy --all-targets` exits with code 0 and zero warnings | High | All |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Fix 3 clippy warnings | `cargo clippy` shows zero warnings |
| CP-2 | Add balance atomicity assertions to `check_and_lock_funds` | Assertions verified in existing tests |
| CP-3 | Add `#[deny(clippy::all)]` with necessary `#[allow]` exceptions | `cargo clippy --all-targets` fails on new warnings |
| CP-4 | Full `cargo clippy --all-targets && cargo test` | Green on both |

### Fix 1: `cex_client.rs:644` — Useless Conversion

**Current:**
```rust
.send(WsMessage::Text(msg_text.into()))
```

**After:**
```rust
.send(WsMessage::Text(msg_text))
```

### Fix 2: `actor.rs:1835` — Unused Variable

**Current:**
```rust
let placed = handle.place_order(user_id, order).await.unwrap();
```

**After:**
```rust
let _placed = handle.place_order(user_id, order).await.unwrap();
```

### Fix 3: `evaluator.rs:188` — Manual Contains

**Current:**
```rust
assert!(!actions.iter().any(|a| *a == ManagementAction::MoveStopToEntry));
```

**After:**
```rust
assert!(!actions.contains(&ManagementAction::MoveStopToEntry));
```

### Fix 4: `check_and_lock_funds` Atomicity Assertions

The function `check_and_lock_funds` in `engine.rs` performs:

```
1. Lookup user balances (HashMap get)
2. Check free balance >= required amount
3. Debit free balance (free -= amount)
4. Credit reserved balance (reserved += amount)
```

Between steps 2 and 3, no atomicity mechanism exists. But `Arc<Mutex<Engine>>` serializes all access — so the atomicity guarantee is *implicit* (whole-engine lock), not *explicit* (balance-level invariant check).

**After — add post-mutation invariants:**

```rust
pub fn check_and_lock_funds(
    &mut self,
    user_id: &str,
    asset: &str,
    amount: Decimal,
) -> Result<(), EngineError> {
    let balances = self.balances
        .entry(user_id.to_string())
        .or_default();

    let balance = balances
        .entry(asset.to_string())
        .or_insert_with(|| UserBalance::default());

    // Pre-condition: capture state for post-condition check
    let free_before = balance.free;
    let reserved_before = balance.reserved;
    let total_before = free_before + reserved_before;

    if balance.free < amount {
        return Err(EngineError::InsufficientFunds {
            user_id: user_id.to_string(),
            asset: asset.to_string(),
            required: amount,
            available: balance.free,
        });
    }

    balance.free -= amount;
    balance.reserved += amount;

    // Post-condition: verify invariants
    debug_assert!(
        balance.free >= Decimal::ZERO,
        "Balance invariant: free balance must never be negative. user={user_id}, asset={asset}, free={free}",
        user_id = user_id, asset = asset, free = balance.free
    );
    debug_assert!(
        balance.reserved >= Decimal::ZERO,
        "Balance invariant: reserved balance must never be negative. user={user_id}, asset={asset}, reserved={reserved}",
        user_id = user_id, asset = asset, reserved = balance.reserved
    );
    debug_assert_eq!(
        free_before + reserved_before,
        balance.free + balance.reserved,
        "Balance conservation invariant: total must be unchanged by reserve operation. user={user_id}, asset={asset}, before={before}, after={after}",
        user_id = user_id,
        asset = asset,
        before = total_before,
        after = balance.free + balance.reserved,
    );

    Ok(())
}
```

These assertions are `debug_assert!` (stripped in `--release`) because:
- The `Arc<Mutex<Engine>>` already guarantees atomicity in production
- The assertions document the implicit contract for developers
- They catch bugs in tests and debug builds without runtime overhead in prod

### Fix 5: Deny Clippy Warnings in CI

Add to each crate's `lib.rs` (or `main.rs` for binaries):

```rust
// crates/engine/src/lib.rs
#![deny(clippy::all)]
#![deny(clippy::pedantic)]
// Allow pedantic rules that don't apply
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
```

For crates that can't pass `clippy::pedantic` immediately, start with:
```rust
#![deny(clippy::all)]
```

And add denies incrementally in future specs.

**CI enforcement** — already present in `.github/workflows/ci.yml`:
```yaml
- name: Clippy
  run: cd testudo-exchange && cargo clippy --all-targets -- -D warnings
```

This is already set to `-D warnings` — meaning clippy warnings are already errors in CI. The 3 existing warnings must be in test code or disabled paths. Fix them to restore the gate.

### Paved Roads

- `cargo clippy --all-targets -- -D warnings` — already the CI gate, just needs to be clean
- `debug_assert!` used extensively in Rust standard library for invariant checking
- Constitution: *"All code must pass linting and tests before commit"*

### Files

- `testudo-exchange/crates/router/src/services/cex_client.rs` — fix line 644
- `testudo-exchange/crates/engine/src/shadow/actor.rs` — fix line 1835
- `testudo-exchange/crates/router/src/services/trade_manager/evaluator.rs` — fix line 188
- `testudo-exchange/crates/engine/src/engine/engine.rs` — add atomicity assertions to `check_and_lock_funds`
- `testudo-exchange/crates/engine/src/lib.rs` — add `#![deny(clippy::all)]`
- `testudo-exchange/crates/router/src/lib.rs` — add `#![deny(clippy::all)]`
- `testudo-exchange/crates/ws-stream/src/main.rs` — add `#![deny(clippy::all)]`
- `testudo-exchange/crates/db-processor/src/main.rs` — add `#![deny(clippy::all)]`

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `cex_client.rs:644` — `.into()` removed from `WsMessage::Text(msg_text.into())`
- [ ] `actor.rs:1835` — `placed` → `_placed`
- [ ] `evaluator.rs:188` — `iter().any()` → `contains()`
- [ ] `check_and_lock_funds` has `debug_assert!` for free ≥ 0, reserved ≥ 0, and conservation of total
- [ ] `cargo clippy --all-targets -- -D warnings` exits 0 with zero warnings
- [ ] `cargo test` passes — no assertion failures in debug mode
- [ ] CI pipeline (`ci.yml`) passes on PR

---

## Risks

1. **`check_and_lock_funds` may not have the exact shape assumed.** The function may be named differently or structured differently than described. Mitigation: read the actual function before writing assertions; adapt the checks to the real code shape.
2. **`#![deny(clippy::all)]` may surface pre-existing warnings we missed.** The 3 known warnings are fixed here, but there may be more in test code or conditional compilation. Mitigation: run `cargo clippy --all-targets -- -D warnings` and fix any additional warnings before adding the per-crate deny.
3. **`debug_assert!` in test-only builds may fire on existing bugs.** If the conservation assertion fails, it means balances are already not conserved in some edge case. Mitigation: fix the bug — this is the diagnostic value of the assertion.

---

## Completion Signal

This spec is complete when:
1. All 3 clippy warnings are fixed
2. `check_and_lock_funds` has post-mutation invariant checks
3. `cargo clippy --all-targets -- -D warnings` exits with code 0
4. `cargo test` passes in debug mode
5. Code committed to master
