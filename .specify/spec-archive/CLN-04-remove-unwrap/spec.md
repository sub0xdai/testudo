# Specification: Remove `unwrap()` from Production Code Paths

**Spec ID:** CLN-04-remove-unwrap
**Date:** 2026-05-15
**Status:** Draft
**Class:** Refactor / Safety
**Priority:** P1 — `unwrap()` in production code is a crash-on-unexpected; the constitution forbids it
**Depends on:** CLN-01, CLN-02, CLN-03 (typed errors give us proper error types for replacement)
**Series:** CLN-01 through CLN-09 (Phase 1 — Open-Source Readiness Cleanup)

---

## Problem Statement

The constitution explicitly states: *"Result<T,E> everywhere (never unwrap() in prod)"*. Yet the codebase has `unwrap()` calls outside of test code:

**Production `unwrap()` calls (non-test, non-assert):**
1. **`engine.rs:53`** — `let trade_id: i64 = get_latest_trade_id_from_db(pool, market).await.unwrap();` — DB call could fail on connection loss or missing data.
2. **`request_response.rs:129-130`** — `serde_json::to_string(&wrapper).unwrap()` / `from_str(&serialized).unwrap()` — serialization roundtrip in error path could panic on type mismatch.
3. **`shadow/actor.rs:1401`** — `self.engine.order_groups.get_group_mut(group_id).unwrap()` — group might not exist after concurrent modification.

**Test-only `unwrap()` calls (acceptable, but 12+ occurrences):**
- `shadow/actor.rs` lines 1499, 1536, 1565, 1589, 1607, 1610, 1617, 1630, 1633, 1648, 1663, 1667, 1719` — all in `#[cfg(test)]` blocks, fine to keep.
- `shadow/orders.rs:744, 860, 886, 905` — test assertions.
- `shadow/balances.rs:285, 291, 350, 355` — test setup.
- `shadow/transaction.rs:537, 559` — test assertions.
- `shadow/trade_event.rs:95` — test setup.

**Rule:** Production code paths (no `#[cfg(test)]`) must use proper error handling. Test code `unwrap()` is acceptable — it panics early and clearly.

---

## User Stories

- **As an operator**, I want the engine to return errors on failures instead of crashing the process, so that one bad database query doesn't take down the matching engine.
- **As a developer**, I want zero `unwrap()` in production paths, so that code review can flag any new unwrap as a violation.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Replace `engine.rs:53` DB unwrap with proper error handling | High | engine/engine.rs |
| FR-2 | Replace `request_response.rs:129-130` serde unwraps with expect/error | High | pg_queue/request_response.rs |
| FR-3 | Replace `shadow/actor.rs:1401` group unwrap with `ok_or` pattern | High | shadow/actor.rs |
| FR-4 | Add `#![deny(clippy::unwrap_used)]` to engine crate (with test exemptions) | Medium | engine/ |
| FR-5 | `cargo clippy --all-targets` shows zero `unwrap_used` warnings in non-test code | High | All |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Fix `engine.rs:53` — DB unwrap becomes `?` with typed error | Engine init handles DB failure gracefully |
| CP-2 | Fix `request_response.rs:129-130` — serde unwraps become `expect` or `?` | Serialization failures logged, not panicked |
| CP-3 | Fix `actor.rs:1401` — group fetch unwrap becomes `ok_or` | Concurrent group mutation handled |
| CP-4 | Enable `clippy::unwrap_used` lint (test-exempted) | CI catches future unwraps |

### Fix 1: `engine.rs:53` — DB Call

**Current (`engine.rs:50-55`):**
```rust
pub async fn init_engine(pool: &PgPool, market: String) -> Engine {
    let trade_id: i64 = get_latest_trade_id_from_db(pool, market.clone())
        .await
        .unwrap();  // <-- PANICS on DB failure
    // ...
}
```

**After:**
```rust
pub async fn init_engine(pool: &PgPool, market: String) -> Result<Engine, EngineError> {
    let trade_id: i64 = get_latest_trade_id_from_db(pool, market.clone())
        .await
        .map_err(|e| EngineError::InternalError {
            detail: format!("Failed to fetch latest trade_id for {}: {}", market, e),
        })?;
    // ...
    Ok(engine)
}
```

**Note:** This changes the return type of `init_engine` from `Engine` to `Result<Engine, EngineError>`. Callers must be updated.

### Fix 2: `request_response.rs:129-130` — Serde Roundtrip

**Current:**
```rust
let serialized = serde_json::to_string(&wrapper).unwrap();
let deserialized: RequestWrapper = serde_json::from_str(&serialized).unwrap();
```

**After (if this is a debug-only or test path):**
```rust
let serialized = serde_json::to_string(&wrapper)
    .expect("RequestWrapper serialization should never fail");
let deserialized: RequestWrapper = serde_json::from_str(&serialized)
    .expect("RequestWrapper deserialization should never fail");
```

Or if in a real code path:
```rust
let serialized = serde_json::to_string(&wrapper)
    .map_err(|e| PgQueueError::Serialization(e.to_string()))?;
let deserialized: RequestWrapper = serde_json::from_str(&serialized)
    .map_err(|e| PgQueueError::Deserialization(e.to_string()))?;
```

**Note:** Check if this is test code. If in `#[cfg(test)]` block, add `#[allow(clippy::unwrap_used)]` and leave as-is.

### Fix 3: `shadow/actor.rs:1401` — Group Fetch

**Current:**
```rust
let group = self.engine.order_groups.get_group_mut(group_id).unwrap();
```

**After:**
```rust
let group = self.engine.order_groups
    .get_group_mut(group_id)
    .ok_or_else(|| EngineError::OrderNotFound {
        order_id: group_id.to_string(),
        user_id: "group_lookup".to_string(),
    })?;
```

### Clippy Lint Enforcement

Add to `crates/engine/src/lib.rs`:
```rust
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
```

This allows `unwrap()` in tests but forbids it in production code.

### Paved Roads

- `EngineError` from CLN-03 — provides error variants for replacement
- `PgQueueError` in `pg_queue/src/errors.rs` — existing typed error pattern
- Constitution: *"Result<T,E> everywhere (never unwrap() in prod)"*

### Files

- `testudo-exchange/crates/engine/src/engine/engine.rs` — fix line 53
- `testudo-exchange/crates/pg_queue/src/request_response.rs` — fix lines 129-130
- `testudo-exchange/crates/engine/src/shadow/actor.rs` — fix line 1401
- `testudo-exchange/crates/engine/src/lib.rs` — add clippy deny (optional, nice-to-have)

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Zero `unwrap()` calls in non-test production code paths
- [ ] `init_engine` returns `Result<Engine, EngineError>` — all callers updated
- [ ] `shadow/actor.rs:1401` uses `ok_or` pattern
- [ ] `request_response.rs:129-130` uses `expect` (if debug-only) or `?` (if production)
- [ ] `cargo clippy --all-targets` passes (no new warnings)
- [ ] `cargo test` passes

---

## Risks

1. **`init_engine` return type change breaks callers.** The function is called from `main.rs` and possibly `shadow/actor.rs` tests. Mitigation: audit all call sites before changing the signature; all should already be in async contexts that can propagate errors.
2. **`request_response.rs` is test-only.** The file may be entirely test infrastructure. If so, adding `#[allow(clippy::unwrap_used)]` to the test module is the right fix, not rewriting the code.

---

## Completion Signal

This spec is complete when:
1. `grep -rn "\.unwrap()" testudo-exchange/crates/ --include='*.rs' | grep -v '#\[cfg(test)' | grep -v '//' | grep -v 'mod tests'` returns zero matches (or only `expect()` explanations)
2. `cargo clippy --all-targets` passes
3. `cargo test` passes
4. Code committed to master
