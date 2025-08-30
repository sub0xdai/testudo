//! Error types for position sizing calculations
//!
//! This module defines comprehensive error handling for all position sizing operations.
//! Each error variant provides clear context about what went wrong and how to fix it.

use rust_decimal::Decimal;
use thiserror::Error;

/// Comprehensive error types for position sizing calculations
#[derive(Debug, Error, Clone, PartialEq)]
pub enum PositionSizingError {
    /// Account equity is invalid (zero, negative, or missing)
    #[error("Invalid account equity: {value}. Account equity must be positive (> 0)")]
    InvalidAccountEquity { value: Decimal },

    /// Risk percentage is outside acceptable bounds
    #[error("Invalid risk percentage: {value}. Risk must be between 0.5% (0.005) and 6% (0.06)")]
    InvalidRiskPercentage { value: Decimal },

    /// Price point is invalid (zero, negative, or missing)
    #[error("Invalid price point: {value}. Price must be positive (> 0)")]
    InvalidPricePoint { value: Decimal },

    /// Stop loss distance creates invalid risk calculation
    #[error("Invalid stop distance: entry_price={entry}, stop_loss={stop}. Stop loss must be below entry price for long positions")]
    InvalidStopDistance { entry: Decimal, stop: Decimal },

    /// Calculation would result in arithmetic overflow
    #[error("Calculation overflow: position size calculation exceeded maximum decimal precision")]
    CalculationOverflow,

    /// Position size calculation resulted in zero or negative size
    #[error("Invalid position size result: {value}. Position size must be positive")]
    InvalidPositionSizeResult { value: Decimal },

    /// Division by zero in stop distance calculation
    #[error("Division by zero: entry price ({entry}) equals stop loss ({stop})")]
    DivisionByZero { entry: Decimal, stop: Decimal },

    /// Position would exceed reasonable limits (prevents absurd position sizes)
    #[error("Position size {position_size} would exceed account balance {account_balance}")]
    ExceedsAccountBalance {
        position_size: Decimal,
        account_balance: Decimal,
    },

    /// Generic calculation error for edge cases
    #[error("Calculation failed: {reason}")]
    CalculationFailed { reason: String },
}

impl PositionSizingError {
    /// Creates an InvalidAccountEquity error
    pub fn invalid_account_equity(value: Decimal) -> Self {
        Self::InvalidAccountEquity { value }
    }

    /// Creates an InvalidRiskPercentage error
    pub fn invalid_risk_percentage(value: Decimal) -> Self {
        Self::InvalidRiskPercentage { value }
    }

    /// Creates an InvalidPricePoint error
    pub fn invalid_price_point(value: Decimal) -> Self {
        Self::InvalidPricePoint { value }
    }

    /// Creates an InvalidStopDistance error
    pub fn invalid_stop_distance(entry: Decimal, stop: Decimal) -> Self {
        Self::InvalidStopDistance { entry, stop }
    }

    /// Creates a DivisionByZero error
    pub fn division_by_zero(entry: Decimal, stop: Decimal) -> Self {
        Self::DivisionByZero { entry, stop }
    }

    /// Creates an ExceedsAccountBalance error
    pub fn exceeds_account_balance(position_size: Decimal, account_balance: Decimal) -> Self {
        Self::ExceedsAccountBalance {
            position_size,
            account_balance,
        }
    }

    /// Creates a CalculationFailed error with custom reason
    pub fn calculation_failed(reason: impl Into<String>) -> Self {
        Self::CalculationFailed {
            reason: reason.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_error_display_formatting() {
        let error = PositionSizingError::invalid_account_equity(Decimal::from(-100));
        assert_eq!(
            error.to_string(),
            "Invalid account equity: -100. Account equity must be positive (> 0)"
        );

        let error = PositionSizingError::invalid_risk_percentage(Decimal::from_str("0.1").unwrap());
        assert_eq!(
            error.to_string(),
            "Invalid risk percentage: 0.1. Risk must be between 0.5% (0.005) and 6% (0.06)"
        );

        let error = PositionSizingError::invalid_stop_distance(
            Decimal::from(100),
            Decimal::from(110),
        );
        assert_eq!(
            error.to_string(),
            "Invalid stop distance: entry_price=100, stop_loss=110. Stop loss must be below entry price for long positions"
        );
    }

    #[test]
    fn test_error_equality() {
        let error1 = PositionSizingError::invalid_account_equity(Decimal::ZERO);
        let error2 = PositionSizingError::invalid_account_equity(Decimal::ZERO);
        let error3 = PositionSizingError::invalid_account_equity(Decimal::from(100));

        assert_eq!(error1, error2);
        assert_ne!(error1, error3);
    }

    #[test]
    fn test_error_constructors() {
        let value = Decimal::from_str("0.001").unwrap();
        let error = PositionSizingError::invalid_risk_percentage(value);
        
        match error {
            PositionSizingError::InvalidRiskPercentage { value: v } => {
                assert_eq!(v, value);
            }
            _ => panic!("Expected InvalidRiskPercentage variant"),
        }
    }
}