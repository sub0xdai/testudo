# Formatio: OODA Loop Trading Engine

This crate implements the OODA loop (Observe → Orient → Decide → Act) for systematic trade execution. It orchestrates data flow and decision-making with a focus on performance, reliability, and safety.

## Core Architecture: The OODA Loop
The engine is built around four core traits that define the stages of a trade cycle.

#### 1. Observe: Market Data Ingestion
`
pub trait MarketObserver {
    async fn observe_market_data(&self) -> Result<MarketSnapshot, ObservationError>;
}
`

#### 2. Orient: Situation Assessment
`
pub trait SituationAssessment {
    fn calculate_optimal_position_size(&self, setup: TradeSetup) -> PositionRecommendation;
}
`

#### 3. Decide: Protocol Enforcement
`
pub trait DecisionEngine {
    fn validate_against_protocol(&self, trade: &ProposedTrade) -> ProtocolValidation;
    fn generate_execution_plan(&self, trade: FilteredTrade) -> ExecutionPlan;
}
`

#### 4. Act: Order Execution
`
pub trait OrderExecutor {
    async fn execute_trade(&self, plan: ExecutionPlan) -> Result<TradeResult, ExecutionError>;
}
`

---

## Key Implementation Patterns
The core architecture is realized through specific, high-performance components.

### Decision Engine & Protocol Rules
The engine integrates components from other crates to enforce the Testudo Protocol.
`
pub struct TestudoDecisionEngine {
    risk_calculator: Arc<VanTharpCalculator>, // From Disciplina crate
    protocol_enforcer: ProtocolEnforcer,     // From Prudentia crate
    circuit_breakers: CircuitBreakerManager,
}

// Protocol Rules (Non-Negotiable)
// 1. Individual trade risk ≤ 6% of account equity.
// 2. Total portfolio risk ≤ 10% of account equity.
// 3. Circuit breaker on 3 consecutive losses.
// 4. Daily loss limit enforcement.
`

### Circuit Breakers
Automated trading halts are a critical safety feature.
`
pub enum CircuitBreakerTrigger {
    ConsecutiveLosses(u32),
    DailyLossLimit(Decimal),
    PortfolioRiskExceeded(Decimal),
    SystemLatencyExceeded(Duration),
    ExchangeConnectivityLoss,
}
`
---

## Performance & Reliability Contracts

### Latency Targets (Critical)
- **Market Observation**: <100ms (WebSocket to cache)
- **Situation Assessment**: <50ms
- **Decision Making**: <25ms
- **Total Execution**: <200ms (UI to exchange confirmation)

### Reliability & Monitoring
- **Throughput**: Must process 1000+ market updates per second.
- **Alerting**: Alerts must trigger if any stage latency exceeds 2x its target, or if the execution failure rate is >1%.

---

## Testing & Commands

### Integration Testing (Mandatory)
Tests must cover the full OODA cycle to ensure component cohesion.
`
#[tokio::test]
async fn complete_ooda_cycle_integration_test() {
    // 1. Observe: Inject simulated market data.
    // 2. Orient: Verify the situation assessment is correct.
    // 3. Decide: Confirm the decision complies with all protocol rules.
    // 4. Act: Execute against a mock exchange and verify the result.
}
`
## Key Commands

### Primary Test Command (TDD Guard Enabled)
Use this command for all development. It enforces the Red-Green-Refactor cycle.
```
cargo nextest run | tdd-guard-rust --passthrough
```

### Additional Commands
- **Run benchmarks**: `cargo bench --package formatio ooda_cycle_latency`
- **Run stress tests**: `cargo test --package formatio stress_tests --ignored --release`
