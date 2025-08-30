//! Position Sizing and Risk Calculation Engine
//!
//! This crate implements Van Tharp's position sizing methodology with mathematical precision
//! and formal verification through property-based testing. All calculations use decimal
//! arithmetic to ensure financial accuracy.
//!
//! ## Core Formula
//!
//! ```text
//! Position Size = (Account Equity × Risk %) ÷ (Entry Price - Stop Loss Price)
//! ```
//!
//! ## Usage Example
//!
//! ```rust
//! use disciplina::{PositionSizingCalculator, AccountEquity, RiskPercentage, PricePoint};
//! use rust_decimal::Decimal;
//! use std::str::FromStr;
//!
//! let calculator = PositionSizingCalculator::new();
//! let account_equity = AccountEquity::new(Decimal::from(10000))?;
//! let risk_percentage = RiskPercentage::new(Decimal::from_str("0.02")?)?; // 2%
//! let entry_price = PricePoint::new(Decimal::from(100))?;
//! let stop_loss = PricePoint::new(Decimal::from(95))?;
//!
//! let position_size = calculator.calculate_position_size(
//!     account_equity,
//!     risk_percentage,
//!     entry_price,
//!     stop_loss,
//! )?;
//!
//! println!("Position size: {} shares", position_size.value());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod types;
pub mod errors;
pub mod calculator;

// Re-export main types for convenience
pub use types::{AccountEquity, RiskPercentage, PricePoint, PositionSize};
pub use errors::PositionSizingError;
pub use calculator::PositionSizingCalculator;

/// Result type for all position sizing operations
pub type Result<T> = std::result::Result<T, PositionSizingError>;

#[cfg(test)]
mod integration_tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn test_end_to_end_position_sizing() {
        let calculator = PositionSizingCalculator::new();
        let account_equity = AccountEquity::new(Decimal::from(50000)).unwrap();
        let risk_percentage = RiskPercentage::new(Decimal::from_str("0.015").unwrap()).unwrap(); // 1.5%
        let entry_price = PricePoint::new(Decimal::from_str("250.50").unwrap()).unwrap();
        let stop_loss = PricePoint::new(Decimal::from_str("240.25").unwrap()).unwrap();

        let result = calculator.calculate_position_size(
            account_equity,
            risk_percentage,
            entry_price,
            stop_loss,
        );

        assert!(result.is_ok());
        let position_size = result.unwrap();

        // Manually verify: (50000 * 0.015) / (250.50 - 240.25) = 750 / 10.25 ≈ 73.17
        let expected = Decimal::from_str("73.170731707317073170731707317").unwrap();
        assert_eq!(position_size.value(), expected);
    }
}