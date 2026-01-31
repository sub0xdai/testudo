# Implementation Plan

> Last updated: 2026-01-30
> Current spec: None (all specs complete)
> Phase: READY FOR NEW SPEC

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

| Spec | Completion Date | Notes |
|------|-----------------|-------|
| 001-deprecate-legacy-engine | 2026-01-20 | Shadow Engine routing |
| 002-panic-prevention | 2026-01-20 | Result propagation |
| 003-risk-enforcement | 2026-01-20 | risk_validated field |
| 004-read-compute-write | 2026-01-20 | Lock-minimizing pattern |
| 005-atomic-cascades | 2026-01-21 | TransactionContext |
| 006-execution-latency | 2026-01-26 | 3μs avg, 70k orders/sec |
| 007-redis-to-postgres | 2026-01-31 | Unified data layer, pg_queue crate |

---

## Next Up

**All Phase 1-2 specs complete.** Candidate areas for Phase 3:

- Live Exchange Integration (production Binance Futures)
- Analytics Dashboard (P&L tracking, win rate, drawdown)
- Multi-Strategy Support (strategy registry in Decision Loop)
- Mobile Optimization (responsive position tool, touch gestures)

---

*This file is persistent state. Ralph updates it each iteration.*
