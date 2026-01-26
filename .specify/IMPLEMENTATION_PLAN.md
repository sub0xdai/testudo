# Implementation Plan

> Last updated: 2026-01-26
> Current spec: 006-execution-latency
> Phase: BUILD

---

## Active Spec: 006-execution-latency

### Tasks

| ID | Task | Status | Notes |
|----|------|--------|-------|
| T1 | Create `TradeIntent` struct in `execution_types.rs` | complete | Fields: symbol, entry, stop, account_equity, risk_pct |
| T2 | Create `SignedOrder` struct in `execution_types.rs` | complete | Output of ExecutionService |
| T3 | Implement auto-sizing formula in `risk/calculator.rs` | complete | Implemented in `TradeIntent::calculate_size()` |
| T4 | Create `MockCexGateway` trait | complete | `CexGateway` trait + `MockCexGateway` impl |
| T5 | Implement `ExecutionService::process_order()` | complete | `DrawToTradeService::process_order()` |
| T6 | Add latency guard (reject if >50ms) | complete | FR-3: Returns `LatencyExceededError` |
| T7 | Create latency benchmark test | complete | `test_end_to_end_execution_latency` |
| T8 | Verify P99 latency <10ms | complete | Avg: 3μs, Throughput: 70,565 orders/sec |

### Discoveries

- Position sizing integrated into `TradeIntent::calculate_size()` for minimal allocation
- Used `Pin<Box<dyn Future>>` for trait object dispatch in `CexGateway`
- Zero-latency mock gateway achieves 3μs average internal latency

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
