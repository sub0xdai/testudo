# Specification: Split `shadow/actor.rs` (2,029 Lines) into Focused Modules

**Spec ID:** CLN-05-split-shadow-actor
**Date:** 2026-05-15
**Status:** Draft
**Class:** Refactor / Code Quality
**Priority:** P1 — 2,029-line monolith is the single largest code quality risk flagged by the TigerBeetle audit
**Depends on:** CLN-01, CLN-02, CLN-03, CLN-04
**Series:** CLN-01 through CLN-09 (Phase 1 — Open-Source Readiness Cleanup)

---

## Problem Statement

`testudo-exchange/crates/engine/src/shadow/actor.rs` is **2,029 lines** — the largest file in the entire codebase by a factor of 4 (next largest is `engine.rs` at 489 lines). The TigerBeetle comparison audit flagged this as the single highest-impact code quality issue:

> *"`shadow/actor.rs` is 2029 lines — a single file monolith. This is a critical architectural risk."*

The file contains:
- `EngineActor` struct and `spawn()` logic
- `EngineHandle` with all public methods (`place_order`, `cancel_order`, `init_user`, etc.)
- The main `actor_loop()` with event dispatch (fills, cancels, price updates)
- Order group lifecycle management
- Fill detection and processing
- Price update handling
- Rehydration logic
- ~500+ lines of tests (acceptable — tests stay in `actor.rs` or `tests/`)

TigerBeetle's standard is a 70-line function limit and modular decomposition. While a strict 70-line limit may be excessive here, breaking into modules of 150–300 lines each dramatically improves readability, testability, and reviewability.

---

## User Stories

- **As a developer**, I want to find the fill-detection logic without scrolling through 2,000 lines, so that I can debug issues in under 5 minutes.
- **As a reviewer**, I want each module to have a single responsibility, so that I can reason about correctness in isolation.
- **As a new contributor**, I want the shadow engine's architecture to be visible from the module structure, so that I can understand the system without reading a monolithic file.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Extract `EngineActor` struct + spawn + actor loop into `actor.rs` (≤300 lines) | High | shadow/ |
| FR-2 | Extract `EngineHandle` API into `handle.rs` (≤200 lines) | High | shadow/ |
| FR-3 | Extract fill detection and processing into `fill_processor.rs` (≤200 lines) | High | shadow/ |
| FR-4 | Extract order group lifecycle into `order_groups.rs` (≤200 lines) | Medium | shadow/ |
| FR-5 | Extract price update handling into `price_updates.rs` (≤100 lines) | Medium | shadow/ |
| FR-6 | Extract rehydration logic into `rehydration.rs` (≤100 lines) | Medium | shadow/ |
| FR-7 | Tests remain in `actor.rs` or move to `tests/actor_tests.rs` — split test modules alongside code | Medium | shadow/ |
| FR-8 | All existing tests pass with zero behavioral changes | High | All |
| FR-9 | `shadow/mod.rs` re-exports the public API (EngineActor, EngineHandle) | High | shadow/ |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Extract `EngineHandle` into `handle.rs` — zero logic changes | Compiles, handle tests pass |
| CP-2 | Extract fill processing into `fill_processor.rs` | Compiles, fill tests pass |
| CP-3 | Extract order groups + price updates + rehydration | Compiles, all shadow tests pass |
| CP-4 | Slim `actor.rs` to ≤300 lines (actor loop + event dispatch core) | Full `cargo test` green, clippy clean |
| CP-5 | Verify public API unchanged via `shadow/mod.rs` re-exports | All external callers compile without changes |

### Proposed Module Structure

**Before:**
```
crates/engine/src/shadow/
├── mod.rs           (re-exports)
├── actor.rs         (2,029 lines — monster)
├── balances.rs      (already clean, 300 lines)
├── decision_loop.rs
├── orders.rs
├── trade_event.rs
└── transaction.rs
```

**After:**
```
crates/engine/src/shadow/
├── mod.rs              (re-exports: EngineActor, EngineHandle, ShadowEngine)
├── actor.rs            (~300 lines — EngineActor struct + spawn + actor_loop dispatch)
├── handle.rs           (~200 lines — EngineHandle public API)
├── fill_processor.rs   (~200 lines — fill detection, processing, event emission)
├── order_groups.rs     (~200 lines — group lifecycle, break-even, trailing stop)
├── price_updates.rs    (~100 lines — price update handling)
├── rehydration.rs      (~100 lines — startup state rehydration)
├── balances.rs         (existing, unchanged)
├── decision_loop.rs    (existing, unchanged)
├── orders.rs           (existing, unchanged)
├── trade_event.rs      (existing, unchanged)
├── transaction.rs      (existing, unchanged)
└── tests/
    ├── actor_tests.rs  (tests extracted from actor.rs)
    └── ... (existing test modules)
```

### Extraction Strategy

For each extraction, move code as blocks — do not refactor or change logic:

1. **Identify a contiguous block** of code in `actor.rs` that forms a logical unit.
2. **Move it** to the new file verbatim.
3. **Adjust imports** — add `use super::*` or specific imports as needed.
4. **Move associated tests** to the corresponding test module.
5. **Verify with `cargo check`** before moving to the next block.

### Key Structs (before extraction)

```rust
// actor.rs: EngineHandle (public API for all callers)
pub struct EngineHandle {
    sender: mpsc::Sender<EngineCommand>,
    // ...
}
impl EngineHandle {
    pub async fn place_order(...) -> Result<ShadowOrder, String>
    pub async fn cancel_order(...) -> Result<ShadowOrder, String>
    pub async fn init_user(...) -> Result<(), String>
    pub async fn get_active_symbols(...) -> Vec<String>
    pub async fn get_user_orders(...) -> Vec<ShadowOrder>
}

// actor.rs: EngineActor (main event loop)
pub struct EngineActor {
    engine: ShadowEngine,
    // ...
}
impl EngineActor {
    pub fn spawn(...) -> EngineHandle
    async fn actor_loop(&mut self) { /* main dispatch loop */ }
}

// actor.rs: fill processing
async fn process_fill(&mut self, fill: FillEvent) { ... }
async fn process_price_update(&mut self, update: PriceUpdate) { ... }

// actor.rs: order group management
fn check_break_even(&mut self, ...) { ... }
fn check_trailing_stop(&mut self, ...) { ... }
fn manage_order_group_lifecycle(&mut self, ...) { ... }

// actor.rs: rehydration
async fn rehydrate_state(&mut self) { ... }
```

### Paved Roads

- `shadow/balances.rs` — example of a clean, focused shadow module (300 lines, single responsibility)
- `shadow/orders.rs` — clean module for order management
- Rust module convention: `pub use` in `mod.rs` for public API surface

### Files

- `testudo-exchange/crates/engine/src/shadow/actor.rs` — slim to ~300 lines
- `testudo-exchange/crates/engine/src/shadow/handle.rs` — **NEW**, EngineHandle API
- `testudo-exchange/crates/engine/src/shadow/fill_processor.rs` — **NEW**, fill detection
- `testudo-exchange/crates/engine/src/shadow/order_groups.rs` — **NEW**, group lifecycle
- `testudo-exchange/crates/engine/src/shadow/price_updates.rs` — **NEW**, price updates
- `testudo-exchange/crates/engine/src/shadow/rehydration.rs` — **NEW**, rehydration
- `testudo-exchange/crates/engine/src/shadow/mod.rs` — update re-exports

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `actor.rs` is ≤300 lines (excluding tests)
- [ ] `handle.rs` exists with EngineHandle and all public API methods
- [ ] `fill_processor.rs` exists with fill detection/processing logic
- [ ] `order_groups.rs` exists with order group lifecycle
- [ ] `price_updates.rs` exists with price update handling
- [ ] `rehydration.rs` exists with rehydration logic
- [ ] `shadow/mod.rs` re-exports unchanged public API
- [ ] All callers of `EngineActor`, `EngineHandle` compile without changes
- [ ] `cargo clippy --all-targets && cargo test` passes
- [ ] No behavioral changes — this is a pure code movement refactor

---

## Risks

1. **Circular imports.** Extracted modules may need types from each other. Mitigation: keep shared types (EngineActor, EngineHandle, etc.) in `actor.rs` and `handle.rs`; extracted modules take references or use `super::` imports.
2. **Test breakage from import changes.** Tests reference types from `actor.rs`. Mitigation: keep tests in `actor.rs` or move to `tests/` subdirectory with explicit `use crate::shadow::*`.
3. **Git blame loss.** Moving large blocks makes `git blame` harder. Mitigation: commit each extraction as a separate commit with clear message "refactor: extract fill_processor from actor.rs (no logic changes)".

---

## Completion Signal

This spec is complete when:
1. All 5 new modules exist with extracted code
2. `actor.rs` ≤300 lines
3. Full test suite passes
4. No callers need import path changes
5. Code committed to master
