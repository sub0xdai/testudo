# Disciplina: Core Financial Engine

This crate implements the Van Tharp position sizing methodology with a mandate for mathematical precision and formal verification.

## Immutable Rules (Non-Negotiable)
1.  **Zero Modification of Core Formulas:** Existing, verified position sizing calculations **must not be altered**. New logic must be implemented as a separate, new feature.
2.  **Decimal-Only Arithmetic:** All financial calculations **must** use `Decimal` types to prevent floating-point precision errors. `f64`/`f32` are forbidden for monetary values.
3.  **Property-Based Testing is Mandatory:** Every financial formula **must** be proven correct with property-based tests covering at least 10,000 iterations.
4.  **Strict Input Validation:** All inputs must be rigorously validated against protocol limits and logical constraints before any calculation is performed.

### Core API Signature
This is the fundamental interface for position sizing.
`
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
`

---

## Critical Patterns & Commands 📜

### Property-Based Testing
Tests must prove mathematical properties, not just check single values.
`
proptest! {
    #[test]
    fn van_tharp_mathematical_properties(
        // ... inputs using ranges for f64 converted to Decimal
    ) {
        // Test properties like:
        // 1. Position size scales linearly with account equity.
        // 2. Position size decreases as stop-loss moves closer to entry.
        // 3. Risk amount is precisely maintained.
    }
}
`

### Error Handling
Errors must be explicit, typed, and actionable.
`
#[derive(Debug, thiserror::Error)]
pub enum RiskCalculationError {
    #[error("Invalid inputs: {reason}")]
    InvalidInputs { reason: String },

    #[error("Mathematical overflow in position calculation")]
    MathematicalOverflow,

    #[error("Position size would exceed account balance")]
    ExceedsAccountBalance,
}
`

## Key Commands

### Primary Test Command (TDD Guard Enabled)
Use this command for all development. It enforces the Red-Green-Refactor cycle.
```
cargo nextest run | tdd-guard-rust --passthrough
```

### Additional Commands
- **Run all crate tests**: `cargo test --package disciplina`
- **Run property tests**: `cargo test --package disciplina property -- --ignored`
- **Run benchmarks**: `cargo bench --package disciplina position_sizing`

---

### Performance & Precision Targets
- **Calculation Latency**: <50ms
- **Numeric Precision**: Decimal (28 digits)
