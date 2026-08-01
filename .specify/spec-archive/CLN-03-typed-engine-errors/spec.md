# Specification: Replace String Errors with Typed Enums in Engine

**Spec ID:** CLN-03-typed-engine-errors
**Date:** 2026-05-15
**Status:** Draft
**Class:** Refactor / Safety
**Priority:** P1 — string errors lose pattern matching; a financial engine must enumerate failure modes
**Depends on:** CLN-01, CLN-02
**Series:** CLN-01 through CLN-09 (Phase 1 — Open-Source Readiness Cleanup)

---

## Problem Statement

The core matching engine (`testudo-exchange/crates/engine`) uses string errors pervasively. Every public function in `engine.rs` and `order.rs` returns `Result<_, &str>` or `Result<_, ()>`. This was called out in the TigerBeetle comparison audit as a critical gap:

> *"Error handling uses string errors everywhere in the engine instead of typed errors — losing pattern matching."*

The `error.rs` file that was started for typed errors is **entirely commented out** — a planned custom error type that was abandoned.

Concrete problems:
- Callers cannot distinguish between "orderbook not found" vs "funds locked" vs "user doesn't exist"
- Error recovery depends on string matching fragile `eprintln!` messages
- The shadow engine (`shadow/actor.rs`) calls engine methods and can't react differently to different failure modes
- Adding new error variants requires grep'ing for all string literals

The constitution requires: *"Explicit error handling"* and *"Result<T,E> everywhere"*. The engine technically uses `Result`, but with `&str` — which is not explicit error handling in practice.

---

## User Stories

- **As a caller of the engine API**, I want typed error variants so that I can match on specific failure modes and take appropriate recovery action.
- **As a developer reading the code**, I want `EngineError` variants to document all known failure modes of the matching engine.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Define `EngineError` enum with variants for all existing string error paths | High | engine/engine.rs |
| FR-2 | Replace `Result<_, &str>` with `Result<_, EngineError>` across all public engine functions | High | engine/engine.rs |
| FR-3 | Replace `Result<_, ()>` with `Result<_, EngineError>` in `order.rs` (get_open_order, cancel_order, etc.) | High | engine/order.rs |
| FR-4 | Replace `Result<_, &str>` in `error.rs` skeletons with the real enum | High | engine/error.rs |
| FR-5 | Update all callers in `shadow/actor.rs` to match on `EngineError` variants | High | shadow/actor.rs |
| FR-6 | Implement `Display` and `std::error::Error` for `EngineError` | Medium | engine/error.rs |
| FR-7 | All existing engine tests continue to pass with typed errors | High | engine/ |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Define `EngineError` enum in `error.rs`, wire into `engine.rs` | `cargo check --workspace` passes |
| CP-2 | Migrate `engine.rs` and `order.rs` — all `&str` → `EngineError` | Engine compiles and unit tests pass |
| CP-3 | Update `shadow/actor.rs` callers to match on variants | Shadow engine tests pass |
| CP-4 | Full `cargo clippy --all-targets && cargo test` green | No regressions |

### `EngineError` Enum Design

```rust
// crates/engine/src/engine/error.rs

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum EngineError {
    /// No orderbook registered for the given market ticker
    OrderbookNotFound {
        market: String,
    },

    /// User does not exist in the engine's user registry
    UserNotFound {
        user_id: String,
    },

    /// Order ID not found in user's open orders or orderbook
    OrderNotFound {
        order_id: String,
        user_id: String,
    },

    /// Insufficient balance to reserve the required amount
    InsufficientFunds {
        user_id: String,
        asset: String,
        required: rust_decimal::Decimal,
        available: rust_decimal::Decimal,
    },

    /// Attempted operation on a closed/cancelled order
    OrderAlreadyClosed {
        order_id: String,
    },

    /// Invalid order parameters (price <= 0, quantity <= 0, etc.)
    InvalidOrderParameters {
        reason: String,
    },

    /// A deprecated code path was invoked (should not happen in production)
    DeprecatedPath {
        function: String,
    },

    /// An internal invariant was violated
    InternalError {
        detail: String,
    },
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::OrderbookNotFound { market } => {
                write!(f, "No orderbook found for market: {}", market)
            }
            EngineError::UserNotFound { user_id } => {
                write!(f, "User not found: {}", user_id)
            }
            EngineError::OrderNotFound { order_id, user_id } => {
                write!(f, "Order {} not found for user {}", order_id, user_id)
            }
            EngineError::InsufficientFunds { user_id, asset, required, available } => {
                write!(f, "Insufficient {} balance for user {}: required {}, available {}",
                    asset, user_id, required, available)
            }
            EngineError::OrderAlreadyClosed { order_id } => {
                write!(f, "Order {} is already closed", order_id)
            }
            EngineError::InvalidOrderParameters { reason } => {
                write!(f, "Invalid order parameters: {}", reason)
            }
            EngineError::DeprecatedPath { function } => {
                write!(f, "Deprecated code path invoked: {}", function)
            }
            EngineError::InternalError { detail } => {
                write!(f, "Internal engine error: {}", detail)
            }
        }
    }
}

impl std::error::Error for EngineError {}
```

### Migration Pattern

**Before (`engine.rs`):**
```rust
pub fn get_open_order(&mut self, open_order: GetOpenOrder) -> Result<&Order, ()> {
    let orderbook = match self.orderbooks.iter_mut()
        .find(|ob| ob.ticker() == open_order.market) {
        Some(ob) => ob,
        None => {
            eprintln!("No matching orderbook found for market: {}", open_order.market);
            return Err(());
        }
    };
    // ...
}
```

**After:**
```rust
pub fn get_open_order(&mut self, open_order: GetOpenOrder) -> Result<&Order, EngineError> {
    let orderbook = self.orderbooks.iter_mut()
        .find(|ob| ob.ticker() == open_order.market)
        .ok_or_else(|| EngineError::OrderbookNotFound {
            market: open_order.market.clone(),
        })?;
    // ...
}
```

### String Error Mapping

Current `eprintln!` messages → `EngineError` variants (see `engine.rs` and `order.rs`):

| Current string | EngineError variant |
|----------------|-------------------|
| `"No matching orderbook found for market: {market}"` | `OrderbookNotFound { market }` |
| `"CreateOrder missing pubsub_id"` | `InvalidOrderParameters { reason: "missing pubsub_id" }` or `InternalError` |
| `"Legacy engine deprecated"` | `DeprecatedPath { function: "create_order_pg" }` |

### Paved Roads

- `sqlx_postgres/src/repositories/errors.rs` — existing typed error enum pattern to follow
- `shadow/actor.rs` — primary consumer, uses `String` error messages currently
- Constitution: *"Result<T,E> everywhere (never unwrap() in prod)"*

### Files

- `testudo-exchange/crates/engine/src/engine/error.rs` — new `EngineError` enum (replaces commented-out skeleton)
- `testudo-exchange/crates/engine/src/engine/engine.rs` — `&str` / `()` → `EngineError`
- `testudo-exchange/crates/engine/src/order.rs` — `Result<_, &str>` → `Result<_, EngineError>`
- `testudo-exchange/crates/engine/src/user.rs` — `Result<_, &str>` → `Result<_, EngineError>`
- `testudo-exchange/crates/engine/src/shadow/actor.rs` — update callers to match typed errors

### Dependencies Added

None — `std::error::Error` and `std::fmt::Display` are stdlib.

---

## Acceptance Criteria

- [ ] `EngineError` enum defined with variants for all current error paths
- [ ] Zero `Result<_, &str>` return types in `engine.rs`, `order.rs`, `user.rs`
- [ ] Zero `Result<_, ()>` return types in public engine API
- [ ] `shadow/actor.rs` matches on `EngineError` variants, not strings
- [ ] `EngineError` implements `Display` and `std::error::Error`
- [ ] `cargo clippy --all-targets && cargo test` passes
- [ ] No new `#[allow(dead_code)]` on error variants (all variants are used or explicitly marked)

---

## Risks

1. **Shadow actor makes assumptions about error strings.** `actor.rs` may have `if err.contains("orderbook")` patterns. Mitigation: audit all string-matching on engine errors in `actor.rs` before starting.
2. **Large surface area.** 200+ lines of error paths across engine, order, user modules. Mitigation: follow the CP waterfall — engine.rs first, then order.rs, then callers.

---

## Completion Signal

This spec is complete when:
1. `EngineError` enum exists and all engine functions return it
2. All callers handle typed errors
3. `cargo clippy --all-targets && cargo test` passes
4. Code committed to master
