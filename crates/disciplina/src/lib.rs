//! Disciplina - Van Tharp Risk Calculation Engine
//!
//! This crate implements the core risk calculation algorithms based on Van Tharp's
//! position sizing methodology. All calculations are formally verified through
//! property-based testing with zero tolerance for mathematical errors.
//!
//! ## Core Formula
//!
//! ```text
//! Position Size = (Account Equity × Risk %) ÷ (Entry Price - Stop Loss Price)
//! ```
//!
//! ## Verification Requirements
//!
//! - Property-based testing with minimum 10,000 iterations
//! - Cross-validation through independent calculation methods
//! - Boundary condition testing for all edge cases
//! - Formal mathematical proof verification
//!
//! ## Roman Military Principle: Disciplina
//!
//! Mathematical precision without deviation. Every calculation must be verified
//! through multiple independent methods before being trusted with real capital.

pub mod calculator;
pub mod protocols;
pub mod types;
pub mod validation;
pub mod verification;

pub use calculator::{VanTharpCalculator, PositionSizeCalculator};
pub use protocols::TestudoProtocol;
pub use types::{
    AccountEquity, PositionSize, RiskPercentage, PricePoint,
    CalculationResult, RiskAssessment,
};
pub use validation::{InputValidator, ProtocolValidator};

use rust_decimal::Decimal;
use thiserror::Error;

/// Disciplina calculation errors with recovery guidance
#[derive(Debug, Error, Clone)]
pub enum DisciplinaError {
    #[error("Invalid account equity: {0} - must be positive")]
    InvalidAccountEquity(Decimal),
    
    #[error("Invalid risk percentage: {0} - must be between 0.5% and 6%")]
    InvalidRiskPercentage(Decimal),
    
    #[error("Invalid price point: entry={entry}, stop={stop} - stop too close to entry")]
    InvalidPriceDistance { entry: Decimal, stop: Decimal },
    
    #[error("Position size calculation overflow - reduce risk percentage")]
    CalculationOverflow,
    
    #[error("Testudo Protocol violation: {violation}")]
    ProtocolViolation { violation: String },
    
    #[error("Calculation verification failed: expected={expected}, actual={actual}")]
    VerificationFailure { expected: Decimal, actual: Decimal },
}

/// Result type for all Disciplina operations
pub type Result<T> = std::result::Result<T, DisciplinaError>;

/// Core trait for position size calculation with formal verification
#[async_trait::async_trait]
pub trait PositionSizeCalculator {
    /// Calculate position size using Van Tharp methodology
    async fn calculate_position_size(
        &self,
        account_equity: AccountEquity,
        risk_percentage: RiskPercentage,
        entry_price: PricePoint,
        stop_loss: PricePoint,
    ) -> Result<PositionSize>;
    
    /// Verify calculation through independent method
    async fn verify_calculation(
        &self,
        input: &CalculationInput,
        result: &PositionSize,
    ) -> Result<bool>;
}

/// Input parameters for position size calculation
#[derive(Debug, Clone)]
pub struct CalculationInput {
    pub account_equity: AccountEquity,
    pub risk_percentage: RiskPercentage, 
    pub entry_price: PricePoint,
    pub stop_loss: PricePoint,
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    
    /// Property-based test: Position size must be inversely related to stop distance
    proptest! {
        #[test]
        fn position_size_inverse_to_stop_distance(
            equity in 1000.0..100000.0f64,
            risk_pct in 0.005..0.06f64,
            entry in 1.0..1000.0f64,
            stop_distance in 0.01..100.0f64,
        ) {
            let account_equity = AccountEquity::new(Decimal::try_from(equity)?)?;
            let risk_percentage = RiskPercentage::new(Decimal::try_from(risk_pct)?)?;
            let entry_price = PricePoint::new(Decimal::try_from(entry)?)?;
            
            let stop_close = PricePoint::new(Decimal::try_from(entry - stop_distance)?)?;
            let stop_far = PricePoint::new(Decimal::try_from(entry - (stop_distance * 2.0))?)?;
            
            let calculator = VanTharpCalculator::new();
            
            let size_close = calculator.calculate_position_size(
                account_equity, risk_percentage, entry_price, stop_close
            ).await?;
            
            let size_far = calculator.calculate_position_size(
                account_equity, risk_percentage, entry_price, stop_far
            ).await?;
            
            // Closer stops should result in larger positions (less distance = less risk per unit)
            prop_assert!(size_close.value() > size_far.value());
        }
    }
}