# Prudentia: Risk Management & Protocol Enforcement

This crate is the guardian of the Testudo platform, enforcing the core risk protocol without exception. It serves as the final authority on trade validation, preventing capital destruction through systematic, non-negotiable risk controls.

## The Testudo Protocol (Immutable)
These limits are the foundational law of the platform and **cannot be overridden**.
`
pub const TESTUDO_PROTOCOL: ProtocolLimits = ProtocolLimits {
    max_individual_trade_risk: Decimal::from_str("0.06").unwrap(),  // 6% max per trade
    max_total_portfolio_risk: Decimal::from_str("0.10").unwrap(),   // 10% max portfolio
    max_consecutive_losses: 3,                                      // Circuit breaker trigger
    min_individual_trade_risk: Decimal::from_str("0.005").unwrap(), // 0.5% minimum
};
`

---

## Core Risk Components
The protocol is enforced by three primary risk assessment systems.

### 1. Individual Trade Assessment
Each trade is assessed for risk amount, reward/risk ratio, and its potential impact on the portfolio's drawdown.
`
pub struct TradeRiskAssessment {
    risk_amount: Decimal,       // Dollar amount at risk
    risk_percentage: Decimal,   // Percentage of account equity
    reward_risk_ratio: Decimal, // Minimum 2:1 expected
}
`
### 2. Portfolio Risk Aggregation
The total risk exposure is monitored in real-time, accounting for the sum of all open positions and their correlations.
`
pub struct PortfolioRiskMetrics {
    total_risk_exposure: Decimal, // Sum of all open position risks
    correlation_risk: Decimal,      // Risk from correlated positions
    risk_utilization: Decimal,      // Percentage of max portfolio risk used
}
`
### 3. Circuit Breakers
Trading is automatically halted if the `max_consecutive_losses` limit is breached.
`
impl ConsecutiveLossTracker {
    pub fn record_trade_outcome(&mut self, outcome: TradeOutcome) -> CircuitBreakerAction {
        // ... logic to halt trading after 3 consecutive losses
    }
}
`
---

## Safety & Alerting Systems

### Real-Time Monitoring & Alerts
The system must constantly monitor risk and fire alerts if thresholds are approached or breached. The `RiskAlert` enum defines the critical events.
`
pub enum RiskAlert {
    IndividualTradeRiskExceeded { trade_id: TradeId, risk_pct: Decimal },
    PortfolioRiskApproachingLimit { current: Decimal, limit: Decimal },
    ConsecutiveLossWarning { count: u32, max: u32 },
    DailyLossApproachingLimit { current: Decimal, limit: Decimal },
}
`
### Audit & Compliance
Every risk validation decision **must** be logged to an immutable audit trail with a cryptographic hash for integrity.
`
pub struct RiskAuditEvent {
    timestamp: SystemTime,
    decision: RiskDecision,
    reasoning: String,
    hash: String, // Cryptographic integrity
}
`
---

## Testing Mandate: Formal Verification
The correctness of the risk engine must be proven with property-based testing. Tests must simulate thousands of scenarios to ensure protocol limits are never violated under any circumstances.
`
proptest! {
    #[test]
    fn protocol_limits_are_never_exceeded(
        // Use randomized inputs for account equity, trade sizes, and outcomes...
    ) {
        // Property: No combination of valid individual trades,
        // even with losses, can ever exceed the max_total_portfolio_risk.
        // The test must rigorously verify this boundary condition.
    }
}
```

---

## Key Commands

### Primary Test Command (TDD Guard Enabled)
Use this command for all development. It enforces the Red-Green-Refactor cycle.
```
cargo nextest run | tdd-guard-rust --passthrough
```

### Additional Commands
- **Run all crate tests**: `cargo test --package prudentia`
- **Run property tests**: `cargo test --package prudentia property -- --ignored`
- **Run risk validation**: `cargo test --package prudentia risk_validation -- --release`
`
