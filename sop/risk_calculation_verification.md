# Risk Calculation Verification SOP
## Standard Operating Procedure for Van Tharp Position Sizing

**Document ID**: SOP-001  
**Version**: 1.0  
**Effective Date**: 2025-08-30  
**Review Cycle**: Quarterly  
**Owner**: Risk Engineering Team

---

## 🎯 Purpose

This SOP ensures mathematical accuracy and formal verification of all position sizing calculations in the Testudo Trading Platform, following Van Tharp methodology and Testudo Protocol enforcement.

## 🏛️ Roman Military Principle
**Disciplina** - Mathematical precision without deviation. Every calculation must be verified through multiple independent methods.

---

## 📋 Scope

This procedure applies to:
- Van Tharp position sizing calculations
- Risk percentage validations
- Account equity assessments
- Position size boundary conditions
- Emergency risk limit overrides

---

## 🔧 Procedure Steps

### Phase 1: Pre-Calculation Validation

#### Step 1.1: Input Parameter Verification
```rust
// Required validation checks before calculation
fn validate_calculation_inputs(
    account_equity: Decimal,
    risk_percentage: Decimal, 
    entry_price: Decimal,
    stop_loss: Decimal,
) -> Result<(), ValidationError> {
    
    // Account equity must be positive
    ensure!(account_equity > Decimal::ZERO, "Account equity must be positive");
    
    // Risk percentage must be between 0.5% and 6%
    ensure!(
        risk_percentage >= Decimal::from_str("0.005").unwrap() &&
        risk_percentage <= Decimal::from_str("0.06").unwrap(),
        "Risk percentage must be between 0.5% and 6%"
    );
    
    // Entry price must be positive
    ensure!(entry_price > Decimal::ZERO, "Entry price must be positive");
    
    // Stop loss must create meaningful risk
    let price_diff = (entry_price - stop_loss).abs();
    let min_diff = entry_price * Decimal::from_str("0.001").unwrap(); // 0.1% minimum
    ensure!(price_diff >= min_diff, "Stop loss too close to entry price");
    
    Ok(())
}
```

#### Step 1.2: Market Condition Checks
- [ ] Verify exchange connectivity
- [ ] Confirm current market price within 1% of entry price
- [ ] Check for extreme volatility conditions
- [ ] Validate trading hours for the instrument

### Phase 2: Van Tharp Calculation

#### Step 2.1: Core Formula Implementation
```rust
/// Van Tharp Position Sizing Formula
/// Position Size = (Account Risk $) / (Entry Price - Stop Loss Price)
fn calculate_van_tharp_position_size(
    account_equity: Decimal,
    risk_percentage: Decimal,
    entry_price: Decimal,
    stop_loss: Decimal,
) -> Result<Decimal, CalculationError> {
    
    // Calculate dollar risk
    let dollar_risk = account_equity * risk_percentage;
    
    // Calculate price risk per unit
    let price_risk = (entry_price - stop_loss).abs();
    
    // Calculate position size
    let position_size = dollar_risk / price_risk;
    
    // Apply precision rounding (8 decimal places for crypto)
    let rounded_position = position_size.round_dp(8);
    
    Ok(rounded_position)
}
```

#### Step 2.2: Calculation Cross-Validation
Each calculation must be verified through **three independent methods**:

1. **Primary Calculation**: Van Tharp formula implementation
2. **Secondary Verification**: Alternative calculation approach
3. **Property-Based Testing**: Automated verification of mathematical properties

```rust
#[cfg(test)]
mod verification_tests {
    use super::*;
    use proptest::prelude::*;
    
    // Property: Position size should decrease as stop loss gets closer to entry
    proptest! {
        #[test]
        fn position_size_inversely_related_to_risk_distance(
            account_equity in 1000.0..100000.0,
            risk_pct in 0.005..0.06,
            entry_price in 1.0..100.0,
            stop_distance in 0.01..10.0
        ) {
            let stop_close = entry_price - stop_distance;
            let stop_far = entry_price - (stop_distance * 2.0);
            
            let size_close = calculate_position_size(account_equity, risk_pct, entry_price, stop_close)?;
            let size_far = calculate_position_size(account_equity, risk_pct, entry_price, stop_far)?;
            
            assert!(size_close < size_far, "Closer stops should result in smaller positions");
        }
    }
}
```

### Phase 3: Testudo Protocol Validation

#### Step 3.1: Risk Limit Enforcement
```rust
fn enforce_testudo_protocol(
    calculated_position: Decimal,
    user_account: &UserAccount,
    current_positions: &[Position],
) -> Result<Decimal, ProtocolViolation> {
    
    // Check individual trade risk limit (max 6%)
    let trade_risk = calculate_trade_risk(calculated_position, user_account);
    ensure!(trade_risk <= user_account.max_risk_per_trade, 
            ProtocolViolation::ExceedsTradeRiskLimit);
    
    // Check total portfolio risk (max 10%)
    let total_risk = calculate_total_portfolio_risk(current_positions) + trade_risk;
    ensure!(total_risk <= Decimal::from_str("0.10").unwrap(),
            ProtocolViolation::ExceedsPortfolioRiskLimit);
    
    // Check daily loss limits
    let daily_pnl = calculate_daily_pnl(user_account);
    if daily_pnl <= user_account.daily_loss_limit {
        return Err(ProtocolViolation::DailyLossLimitReached);
    }
    
    Ok(calculated_position)
}
```

#### Step 3.2: Circuit Breaker Checks
- [ ] Verify no consecutive loss limit reached (default: 3 trades)
- [ ] Check overall drawdown limits (default: 10% max)
- [ ] Confirm account is not in trading suspension
- [ ] Validate position count limits (default: 5 simultaneous positions)

### Phase 4: Final Verification & Logging

#### Step 4.1: Mathematical Verification
Execute comprehensive verification suite:

```bash
# Run property-based tests (minimum 10,000 iterations)
cargo test --package risk_engine -- --test-threads=1 --nocapture

# Run formal verification checks
cargo test verification_suite -- --ignored

# Benchmark calculation performance (<50ms requirement)
cargo bench position_sizing_benchmarks
```

#### Step 4.2: Audit Trail Creation
```rust
#[derive(Debug, Serialize)]
pub struct PositionSizingAuditLog {
    pub timestamp: DateTime<Utc>,
    pub user_id: String,
    pub account_equity: Decimal,
    pub risk_percentage: Decimal,
    pub entry_price: Decimal,
    pub stop_loss: Decimal,
    pub calculated_position: Decimal,
    pub validation_checks: Vec<ValidationResult>,
    pub protocol_enforcements: Vec<ProtocolCheck>,
    pub calculation_time_ms: u64,
    pub verification_hash: String, // SHA-256 of all inputs and outputs
}
```

---

## 🚨 Error Handling

### Calculation Failures
If any calculation step fails:

1. **Log Error**: Record detailed error information
2. **Alert Admin**: Notify system administrators immediately  
3. **Block Trade**: Do not allow trade execution
4. **User Notification**: Inform user of calculation failure (not technical details)
5. **Fallback**: Offer manual position size entry with additional confirmations

### Emergency Procedures
In case of system-wide calculation failures:

1. **Trading Halt**: Immediately suspend all new position creation
2. **Investigation**: Deploy emergency response team
3. **Rollback**: Use last known good calculation engine
4. **Communication**: Notify all active users via WebSocket alert

---

## 📊 Quality Assurance Metrics

### Performance Benchmarks
- **Calculation Speed**: <50ms per calculation (99th percentile)
- **Memory Usage**: <1MB per calculation
- **Accuracy**: Zero tolerance for calculation errors
- **Availability**: 99.99% calculation service uptime

### Verification Requirements
- **Test Coverage**: 100% for all calculation paths
- **Property Tests**: Minimum 10,000 iterations per property
- **Formal Verification**: Mathematical proofs for core formulas
- **Independent Review**: Peer review required for all calculation changes

---

## 📝 Documentation Requirements

### Code Documentation
```rust
/// Calculates Van Tharp position size with formal verification
/// 
/// # Formula
/// Position Size = (Account Equity × Risk %) ÷ (Entry Price - Stop Price)
/// 
/// # Verification
/// This calculation is formally verified through:
/// 1. Property-based testing ensuring mathematical invariants
/// 2. Cross-validation with alternative calculation methods  
/// 3. Boundary condition testing for edge cases
/// 
/// # Security
/// All inputs are validated and sanitized before calculation
/// Results are logged with cryptographic hash for audit trail
/// 
/// # Errors
/// Returns CalculationError if:
/// - Inputs fail validation
/// - Calculation results in infinite/NaN values
/// - Result exceeds platform limits
#[formally_verified]
pub fn calculate_position_size(/*...*/) -> Result<Decimal, CalculationError>
```

### Change Management
All modifications to calculation logic require:
- [ ] Formal proof of mathematical correctness
- [ ] Independent peer review by senior engineer
- [ ] Comprehensive test suite expansion
- [ ] Backward compatibility verification
- [ ] Staged rollout with monitoring

---

## 🔄 Review & Maintenance

### Monthly Reviews
- Calculation accuracy metrics analysis
- Performance benchmark review  
- Error rate trending
- User feedback incorporation

### Quarterly Reviews
- SOP effectiveness assessment
- Calculation algorithm optimization opportunities
- Risk limit appropriateness evaluation
- Emergency procedure testing

### Annual Reviews
- Complete SOP revision
- Mathematical model validation
- Competitive analysis of position sizing approaches
- Formal verification toolchain upgrades

---

## 📚 References

- Van Tharp, "Trade Your Way to Financial Freedom"
- Testudo Protocol Specification v1.0
- Risk Management Best Practices (Prop Trading Firms)
- ISO 9001:2015 Quality Management Standards
- Formal Verification Guidelines for Financial Software

---

**Approval Signatures**:
- Risk Engineering Lead: _________________ Date: _________
- Platform Architect: __________________ Date: _________
- Compliance Officer: __________________ Date: _________

**Next Review Date**: 2025-11-30