# Feature: Risk Enforcement Verification

> Spec ID: 003-risk-enforcement
> Created: 2026-01-20
> Status: Ready
> Priority: P0 (Critical Infrastructure)

---

## Overview

Verify and enforce that 100% of order flow passes through the Decision Loop's Risk Module validation. This ensures the "Conservative Wins" logic is applied to every trade, preventing unvalidated orders from bypassing risk checks.

---

## User Stories

- [x] As a trader, I want my risk settings (max position size, risk %) to be enforced on every order so that I can't accidentally over-leverage.
- [x] As a system operator, I want audit logs proving every order was risk-validated so that I can demonstrate compliance.

---

## Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | Add `risk_validated: bool` field to `ShadowOrder` struct | High |
| FR-2 | Decision Loop must set `risk_validated = true` on approved orders | High |
| FR-3 | Shadow Engine must reject orders where `risk_validated != true` | High |
| FR-4 | Add logging for risk validation results (approved/rejected with reason) | Medium |

---

## Acceptance Criteria

- [ ] `ShadowOrder` struct has `risk_validated: bool` field
- [ ] Orders submitted without Decision Loop validation are rejected
- [ ] Risk validation events are logged with order details
- [ ] `cargo clippy --all-targets` passes
- [ ] `cargo test` passes

---

## Technical Notes

### Files to Modify

- `testudo-exchange/crates/engine/src/shadow/orders.rs` - Add field to `ShadowOrder`
- `testudo-exchange/crates/router/src/decision_loop.rs` - Set validation flag
- `testudo-exchange/crates/engine/src/shadow/mod.rs` - Check flag before processing
- `testudo-exchange/crates/router/src/routes/paper_trade.rs` - Ensure flag is set

### Validation Flow

```
Order Request
     │
     ▼
DecisionLoop::execute()
     │
     ├── risk_validated = true (if approved)
     │
     ▼
ShadowEngine::add_order()
     │
     ├── Check risk_validated == true
     │   └── Reject if false
     │
     ▼
Order Added to Book
```

### Dependencies

- Depends on 001-deprecate-legacy-engine being complete

### Assumptions

- All order entry points go through router routes
- No backdoor methods to add orders exist

---

## Completion Signal

### Implementation Checklist
- [ ] All functional requirements implemented
- [ ] All acceptance criteria verified
- [ ] Code follows project constitution standards
- [ ] No new linting warnings introduced

### Testing Requirements
- [ ] `cd testudo-exchange && cargo clippy --all-targets && cargo test` passes
- [ ] Unit test: order without risk_validated flag is rejected
- [ ] Unit test: order with risk_validated flag is accepted

### Quality Verification
- [ ] Logs show risk validation events
- [ ] No orders bypass risk checks

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

None.

---

*Template version: 1.0*
