# Feature: Execution Tool Low-Latency Verification

> Spec ID: 006-execution-latency
> Created: 2026-01-26
> Status: Ready
> Priority: P0 (Performance Critical)

---

## Overview

The Execution Tool is a "Draw-to-Trade" feature where a user draws a reward/risk tool (Entry, Stop Loss, Take Profit) on the frontend chart. This visual action must instantly translate into a normalized order payload, calculate position size based on a static 2% risk model, and dispatch the order to the backend.

**The Goal:** Minimize "Internal Tick-to-Trade" latency. The time from the user confirming the draw to the backend dispatching the CEX API request must be **< 15ms** (excluding external network RTT).

---

## User Stories

- [ ] As a scalper, I want position sizing to be calculated automatically based on my Stop Loss distance so that I never risk more than 2% of my equity, even during high volatility.
- [ ] As a high-frequency trader, I want my order processed by the internal engine in under 5ms so that my execution price is as close as possible to the chart price.
- [ ] As a user, I want visual feedback (e.g., a "Fill" marker) on the chart instantly after execution.

---

## Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | **Auto-Sizing Logic**: `Size = (Account_Equity * 0.02) / abs(Entry_Price - Stop_Price)`. Result must be rounded to valid lot size steps. | High |
| FR-2 | **Execution Service**: The `ExecutionService` struct must accept a `TradeIntent` payload and return a `SignedOrder` ready for CEX dispatch. | High |
| FR-3 | **Latency Guard**: The execution pipeline must fail/reject if internal processing timestamp delta exceeds 50ms. | Medium |
| FR-4 | **Mock Gateway**: Implement a `MockCexGateway` trait to simulate exchange latency for testing without real funds. | High |

---

## Acceptance Criteria

- [ ] **Position Sizing Precision**: Calculations must use `rust_decimal` or similar to avoid floating point errors.
- [ ] **Performance Target**: P99 internal latency for `submit_order` function < 10ms.
- [ ] **Throughput**: System handles 100 sequential orders/sec without blocking the async runtime.
- [ ] `cargo clippy --all-targets` passes with no warnings.
- [ ] `cargo test` passes, including the new latency benchmark.

---

## Technical Notes

### Files to Modify

- `testudo-exchange/crates/router/src/services/execution_service.rs` (Logic)
- `testudo-exchange/crates/engine/src/risk/calculator.rs` (Math)
- `testudo-exchange/crates/router/tests/latency_bench.rs` (New Benchmark)

### Latency Targets

| Operation | Target | Current |
|-----------|--------|---------|
| Risk Calc & Sizing | < 1ms | |
| Signing & Payload Gen | < 3ms | |
| **Total Internal Pipeline** | **< 10ms** | |

### Dependencies

- `tokio::time::Instant` for monotonic timing.
- `criterion` (optional) or `std::time` for micro-benchmarking inside tests.
- `rust_decimal` for financial math.

### Assumptions

- User Account Equity is cached in memory (fetching from DB is too slow for this path).
- We are mocking the actual HTTP/WebSocket call to the CEX; we measure time *until* the packet is ready to leave the interface.

---

## Verification Test

The agent must create this test to prove performance.

```rust
// tests/execution_latency_bench.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_end_to_end_execution_latency() {
        // 1. Setup
        let service = ExecutionService::new(MockCexGateway::new());
        let intent = TradeIntent {
            symbol: "BTC/USDT".to_string(),
            entry: dec!(50000),
            stop: dec!(49000), // 2% distance implies sizing logic
            account_equity: dec!(10000),
            risk_pct: dec!(0.02),
        };

        // 2. Warmup (Run once to prime caches/instruction cache)
        let _ = service.process_order(intent.clone()).await;

        // 3. Benchmark Loop (100 iterations)
        let mut total_duration = 0;
        let iterations = 100;

        for _ in 0..iterations {
            let start = Instant::now();

            // ACT: The critical path
            let result = service.process_order(intent.clone()).await;

            let elapsed = start.elapsed().as_micros();
            total_duration += elapsed;

            assert!(result.is_ok());
        }

        // 4. Assertions
        let avg_latency = total_duration / iterations;
        println!("Average Internal Latency: {} microseconds", avg_latency);

        // Fail if average processing > 10ms (10,000 micros)
        assert!(avg_latency < 10_000, "Latency too high! Average: {}us", avg_latency);
    }
}
```

---

## Completion Signal

### Implementation Checklist
- [ ] `ExecutionService` implements the `Size` calculation correctly.
- [ ] `MockCexGateway` exists to trap the outgoing request.
- [ ] Latency benchmark test exists in `tests/`.
- [ ] All functional requirements implemented.

### Testing Requirements
- [ ] `cd testudo-exchange && cargo clippy --all-targets && cargo test` passes.
- [ ] Verify that the benchmark output shows `< 10,000` microseconds.

### Iteration Protocol
If any check fails:
1. Identify the issue (Logic error vs Performance bottleneck).
2. If logic: Fix bug.
3. If performance: Refactor for fewer allocations (clone less, use references).
4. Re-run verification.

### Done Signal
When ALL above criteria are satisfied, output:
```
<promise>DONE</promise>
```

---

*Template version: 1.0*
