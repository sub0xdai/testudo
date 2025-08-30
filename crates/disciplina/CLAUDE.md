# Disciplina: Core Financial Engine Context

## 🏛️ Mission: Van Tharp Position Sizing with Mathematical Precision

**Disciplina** embodies the Roman virtue of discipline in systematic position sizing calculations. This crate implements Van Tharp's position sizing methodology with **99.9% mathematical accuracy** and formal verification.

---

## 🎯 Core Principles (IMMUTABLE)

### Sacred Rules
1. **NEVER modify existing position sizing calculations** - only add new implementations
2. **Every financial formula requires property-based testing** with 10,000+ iterations
3. **Zero tolerance for floating-point precision errors** - use Decimal types exclusively
4. **Formal verification mindset** - prove correctness through mathematical properties

### Van Tharp Formula Implementation
```rust
// Position Size = (Account Equity × Risk %) ÷ (Entry - Stop Loss)
impl PositionSizeCalculator for VanTharpCalculator {
    fn calculate_position_size(
        &self,
        account_equity: Decimal,
        risk_percentage: Decimal,
        entry_price: Decimal,
        stop_loss: Decimal,
    ) -> Result<PositionSize, RiskCalculationError>;
}
```

---

## 🔧 Development Patterns

### Property-Based Testing (MANDATORY)
```rust
proptest! {
    #[test]
    fn van_tharp_mathematical_properties(
        account_equity in 1000.0..1000000.0f64,
        risk_pct in 0.005..0.06f64,
        entry_price in 1.0..100.0f64,
        stop_distance in 0.01..10.0f64,
    ) {
        let equity = Decimal::from_f64(account_equity).unwrap();
        let risk = Decimal::from_f64(risk_pct).unwrap();
        let entry = Decimal::from_f64(entry_price).unwrap();
        let stop = entry - Decimal::from_f64(stop_distance).unwrap();
        
        let position = calculator.calculate_position_size(equity, risk, entry, stop)?;
        
        // Property 1: Position size increases with account equity
        // Property 2: Position size decreases as stop gets closer to entry
        // Property 3: Position size never exceeds account balance
        // Property 4: Risk percentage is exactly maintained
    }
}
```

### Error Handling Protocol
```rust
#[derive(Debug, thiserror::Error)]
pub enum RiskCalculationError {
    #[error("Invalid inputs: {reason}")]
    InvalidInputs { reason: String },
    
    #[error("Mathematical overflow in position calculation")]
    MathematicalOverflow,
    
    #[error("Position size would exceed account balance")]
    ExceedsAccountBalance,
    
    #[error("Risk percentage violates Testudo Protocol limits")]
    ProtocolViolation,
}
```

---

## 📊 Performance Requirements

### Latency Targets (NON-NEGOTIABLE)
- Position calculation: **<50ms** for Van Tharp formula
- Property verification: **<100ms** for validation suite
- Memory usage: **<1MB** for calculator instance

### Benchmarking Commands
```bash
# Run position sizing benchmarks
cargo bench --package disciplina position_sizing

# Memory profiling
cargo test --package disciplina -- --nocapture --test-threads=1
```

---

## 🚨 Critical Validation Rules

### Input Validation (STRICT)
```rust
fn validate_inputs(
    account_equity: Decimal,
    risk_percentage: Decimal,
    entry_price: Decimal,
    stop_loss: Decimal,
) -> Result<(), RiskCalculationError> {
    // Account equity must be positive
    ensure!(account_equity > Decimal::ZERO, "Account equity must be positive");
    
    // Risk percentage must be within Testudo Protocol limits (0.5% - 6%)
    ensure!(risk_percentage >= Decimal::from_str("0.005")?, "Risk too low");
    ensure!(risk_percentage <= Decimal::from_str("0.06")?, "Risk exceeds protocol");
    
    // Entry and stop must create valid risk/reward scenario
    ensure!(entry_price != stop_loss, "Entry cannot equal stop loss");
    
    Ok(())
}
```

### Calculation Verification
Every calculation result must be verified through:
1. **Reverse calculation**: Given position size, derive original risk percentage
2. **Range checking**: Position size within reasonable bounds
3. **Precision validation**: No floating-point artifacts
4. **Protocol compliance**: Testudo risk limits enforced

---

## 🧪 Testing Requirements

### Test Categories (ALL MANDATORY)
```bash
# Unit tests for individual functions
cargo test disciplina::tests::unit --test-threads=1

# Property-based tests (minimum 10,000 iterations)
cargo test disciplina::tests::property --ignored --test-threads=1

# Integration tests with realistic market scenarios
cargo test disciplina::tests::integration --test-threads=1

# Performance regression tests
cargo test disciplina::tests::performance --release --test-threads=1
```

### Test Data Requirements
- **Realistic account sizes**: $1,000 to $1,000,000
- **Various risk percentages**: 0.5% to 6% (Testudo Protocol limits)
- **Market price ranges**: $0.01 to $100,000 (crypto + traditional assets)
- **Edge cases**: Minimum position sizes, maximum equity scenarios

---

## 📝 Implementation Checklist

### Before Adding New Features
- [ ] Read existing Van Tharp implementation
- [ ] Understand mathematical properties being maintained
- [ ] Write property-based tests FIRST
- [ ] Implement with Decimal precision
- [ ] Verify through reverse calculations
- [ ] Benchmark against performance targets
- [ ] Document mathematical reasoning

### Code Review Requirements
- [ ] All calculations use Decimal types (never f64 for money)
- [ ] Property-based tests cover mathematical edge cases
- [ ] Error handling provides actionable feedback
- [ ] Performance benchmarks meet <50ms target
- [ ] No modification to existing Van Tharp implementations
- [ ] Formal verification reasoning documented

---

## 🔄 Maintenance Protocol

### Monthly Audits
- [ ] Property-based test effectiveness review
- [ ] Performance regression analysis
- [ ] Mathematical accuracy verification
- [ ] Protocol compliance validation

### Quarterly Reviews
- [ ] Van Tharp methodology alignment check
- [ ] New testing scenarios based on user data
- [ ] Performance optimization opportunities
- [ ] Integration with other Testudo components

---

## 🏛️ The Disciplina Way

*"Disciplina in calculation, precision in execution, perfection in protection of capital."*

Every line of code in this crate serves the sacred duty of protecting trader capital through mathematically verified position sizing. Approach each calculation with the discipline of a Roman engineer building an aqueduct—built to last millennia, tested under all conditions, never compromised by expedience.

---

**Crate Version**: 0.1.0  
**Mathematical Precision**: Decimal (28-digit precision)  
**Test Coverage**: 100% with property-based verification  
**Performance Target**: <50ms Van Tharp calculations