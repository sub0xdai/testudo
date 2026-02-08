# Feature: Panic Prevention - Critical Path Error Handling

> Spec ID: 002-panic-prevention
> Created: 2026-01-20
> Status: Complete
> Priority: P0 (Critical Infrastructure)

---

## Overview

The codebase contains `unwrap()` calls on critical paths (database connections, Redis operations, asset parsing) that cause full service panics when infrastructure hiccups occur. This spec replaces these with proper `Result` propagation and graceful error handling to achieve 99.9% uptime resilience.

---

## User Stories

- [x] As a system operator, I want the exchange to handle infrastructure failures gracefully so that temporary Redis/DB outages don't crash the service.
- [x] As a developer, I want clear error types so that I can debug issues without digging through panic traces.

---

## Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | Replace `unwrap()` calls in `engine/engine.rs` with `?` operator | High |
| FR-2 | Create distinct error enum: `EngineError { DbError, RedisError, ParseError, LogicError }` | High |
| FR-3 | Implement Redis connection recovery with exponential backoff | Medium |
| FR-4 | All public engine functions must return `Result<T, EngineError>` | High |

---

## Acceptance Criteria

- [ ] Zero `unwrap()` calls on DB/Redis operations in production code paths
- [ ] `EngineError` enum exists with variants for each failure type
- [ ] Engine gracefully handles Redis disconnect (logs error, retries)
- [ ] `cargo clippy --all-targets` passes
- [ ] `cargo test` passes

---

## Technical Notes

### Files to Modify

- `testudo-exchange/crates/engine/src/engine/engine.rs` - Lines 50, 122, 123 unwraps
- `testudo-exchange/crates/engine/src/engine/mod.rs` - Add EngineError type
- `testudo-exchange/crates/engine/src/shadow/mod.rs` - Review for unwraps
- `testudo-exchange/crates/redis/src/lib.rs` - Add retry logic

### Critical Unwraps to Replace

| File | Line | Current Code | Risk |
|------|------|--------------|------|
| engine.rs | 50 | `get_latest_trade_id_from_db(...).await.unwrap()` | DB connection failure |
| engine.rs | 122 | `Asset::from_str(assets[0]).unwrap()` | Invalid symbol format |
| engine.rs | 123 | `Asset::from_str(assets[1]).unwrap()` | Invalid symbol format |

### Dependencies

- None - internal refactor

### Assumptions

- Existing error handling patterns in the codebase can be extended
- Callers are prepared to handle `Result` returns

---

## Completion Signal

### Implementation Checklist
- [ ] All functional requirements implemented
- [ ] All acceptance criteria verified
- [ ] Code follows project constitution standards
- [ ] No new linting warnings introduced

### Testing Requirements
- [ ] `cd testudo-exchange && cargo clippy --all-targets && cargo test` passes
- [ ] Unit tests for error propagation paths

### Quality Verification
- [ ] Service doesn't panic on simulated DB timeout
- [ ] Errors are logged with context

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

None - scope is well-defined.

---

*Template version: 1.0*
