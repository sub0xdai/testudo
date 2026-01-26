# Implementation Plan

> Last updated: 2026-01-26
> Current spec: 006-execution-latency
> Phase: BUILD

---

## Active Spec: 006-execution-latency

### Tasks

| ID | Task | Status | Notes |
|----|------|--------|-------|
| T1 | Create `TradeIntent` struct in `execution_types.rs` | pending | Fields: symbol, entry, stop, account_equity, risk_pct |
| T2 | Create `SignedOrder` struct in `execution_types.rs` | pending | Output of ExecutionService |
| T3 | Implement auto-sizing formula in `risk/calculator.rs` | pending | `Size = (Equity * 0.02) / abs(Entry - Stop)` |
| T4 | Create `MockCexGateway` trait | pending | For testing without real funds |
| T5 | Implement `ExecutionService::process_order()` | pending | Accept TradeIntent, return SignedOrder |
| T6 | Add latency guard (reject if >50ms) | pending | FR-3 requirement |
| T7 | Create latency benchmark test | pending | `test_end_to_end_execution_latency` |
| T8 | Verify P99 latency <10ms | pending | Run 100 iterations, assert avg <10,000μs |

### Discoveries

<!-- Ralph will add discoveries here as implementation progresses -->

### Blockers

<!-- Document any blockers encountered -->

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| 001-deprecate-legacy-engine | 2026-01-20 |
| 002-panic-prevention | 2026-01-20 |
| 003-risk-enforcement | 2026-01-20 |
| 004-read-compute-write | 2026-01-20 |
| 005-atomic-cascades | 2026-01-21 |

---

## Next Up

- 007-open-positions-layer (if not done)
- Additional specs as defined

---

*This file is persistent state. Ralph updates it each iteration.*
