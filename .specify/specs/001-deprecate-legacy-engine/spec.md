# Feature: Deprecate Legacy Engine

> Spec ID: 001-deprecate-legacy-engine
> Created: 2026-01-20
> Status: Complete
> Priority: P0 (Critical Infrastructure)

---

## Overview

The Testudo exchange currently operates two incompatible matching engines: a legacy blocking engine (`engine/engine.rs`) using `std::sync::Mutex` and a modern async Shadow Engine (`shadow/mod.rs`) using `tokio::sync::RwLock`. The legacy engine bypasses the Risk Module entirely, creating solvency risks. This spec deprecates the legacy engine by routing all traffic through the Shadow Engine via the Decision Loop.

---

## User Stories

- [x] As a system operator, I want all orders to pass through risk validation so that the exchange maintains solvency guarantees.
- [x] As a developer, I want a single engine architecture so that I can maintain and reason about the codebase.

---

## Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | All HTTP/WebSocket trade requests must route through the Decision Loop | High |
| FR-2 | Remove or gate the legacy `Engine` struct from accepting direct orders | High |
| FR-3 | The `std::sync::Mutex<UserBalances>` maps in `engine.rs` must be migrated to Shadow Engine's RwLock managers | High |
| FR-4 | The router's order routes must exclusively call Shadow Engine methods | High |

---

## Acceptance Criteria

- [ ] No order can reach the matching engine without passing through `DecisionLoop::execute()`
- [ ] The legacy `Engine::place_order()` method is either removed or returns an error
- [ ] `cargo clippy --all-targets` passes with no new warnings
- [ ] `cargo test` passes (all existing tests)
- [ ] Paper trading flow continues to work end-to-end

---

## Technical Notes

### Files to Modify

- `testudo-exchange/crates/engine/src/engine/engine.rs` - Gate or deprecate `place_order()`
- `testudo-exchange/crates/engine/src/main.rs` - Remove direct Engine order processing
- `testudo-exchange/crates/router/src/routes/order.rs` - Ensure all paths use Decision Loop
- `testudo-exchange/crates/router/src/routes/paper_trade.rs` - Verify Shadow Engine integration

### Dependencies

- Shadow Engine must be operational (currently is)
- Decision Loop must be functional (currently is)

### Assumptions

- Paper trading mode is the primary use case currently
- Live trading will be re-enabled after this unification

---

## Completion Signal

### Implementation Checklist
- [ ] All functional requirements implemented
- [ ] All acceptance criteria verified
- [ ] Code follows project constitution standards
- [ ] No new linting warnings introduced

### Testing Requirements
- [ ] `cd testudo-exchange && cargo clippy --all-targets && cargo test` passes
- [ ] Manual verification: place order via API, confirm it goes through Decision Loop

### Quality Verification
- [ ] No panic on order submission
- [ ] Correct error responses for invalid orders

### Iteration Protocol
If any check fails:
1. Identify the issue from error output
2. Fix the code
3. Commit the fix
4. Re-run verification
5. Repeat until ALL checks pass

### Done Signal
When ALL above criteria are satisfied, output:
```
<promise>DONE</promise>
```

---

## Clarifications Needed

None - requirements are clear from PRD.

---

*Template version: 1.0*
