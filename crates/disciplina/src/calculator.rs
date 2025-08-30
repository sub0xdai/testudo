//! Position sizing calculator implementing Van Tharp methodology
//!
//! This module contains the core calculator that implements the Van Tharp position
//! sizing formula with mathematical precision using decimal arithmetic.

use crate::errors::PositionSizingError;
use crate::types::{AccountEquity, RiskPercentage, PricePoint, PositionSize};
use rust_decimal::Decimal;
use tracing::{debug, instrument, warn};

/// Core position sizing calculator implementing Van Tharp methodology
///
/// The calculator uses the formula:
/// ```text
/// Position Size = (Account Equity × Risk %) ÷ (Entry Price - Stop Loss Price)
/// ```
/// 
/// All calculations use decimal arithmetic to prevent floating-point precision errors
/// that could result in incorrect position sizes.
/// 
/// # Examples
/// 
/// ```
/// use disciplina::{PositionSizingCalculator, AccountEquity, RiskPercentage, PricePoint};
/// use rust_decimal::Decimal;
/// use std::str::FromStr;
/// 
/// let calculator = PositionSizingCalculator::new();
/// let account_equity = AccountEquity::new(Decimal::from(10000))?;
/// let risk_percentage = RiskPercentage::new(Decimal::from_str("0.02")?)?; // 2%
/// let entry_price = PricePoint::new(Decimal::from(100))?;
/// let stop_loss = PricePoint::new(Decimal::from(95))?;
/// 
/// let position_size = calculator.calculate_position_size(
///     account_equity,
///     risk_percentage,
///     entry_price,
///     stop_loss,
/// )?;
/// 
/// // Expected: (10000 * 0.02) / (100 - 95) = 200 / 5 = 40 shares
/// assert_eq!(position_size.value(), Decimal::from(40));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct PositionSizingCalculator {
    /// Optional precision override for calculations (defaults to 28 decimal places)
    precision: Option<u32>,
}

impl PositionSizingCalculator {
    /// Creates a new position sizing calculator with default settings
    /// 
    /// # Returns
    /// A new `PositionSizingCalculator` instance ready for calculations
    /// 
    /// # Examples
    /// ```
    /// use disciplina::PositionSizingCalculator;
    /// 
    /// let calculator = PositionSizingCalculator::new();
    /// ```
    pub fn new() -> Self {
        Self {
            precision: None,
        }
    }

    /// Creates a new calculator with custom decimal precision
    /// 
    /// # Arguments
    /// * `precision` - Number of decimal places to maintain in calculations
    /// 
    /// # Examples
    /// ```
    /// use disciplina::PositionSizingCalculator;
    /// 
    /// let calculator = PositionSizingCalculator::with_precision(6);
    /// ```
    pub fn with_precision(precision: u32) -> Self {
        Self {
            precision: Some(precision),
        }
    }

    /// Calculates position size using Van Tharp methodology
    /// 
    /// This is the core method that implements the Van Tharp position sizing formula:
    /// Position Size = (Account Equity × Risk %) ÷ (Entry Price - Stop Loss Price)
    /// 
    /// # Arguments
    /// * `account_equity` - Total account balance available for trading
    /// * `risk_percentage` - Risk per trade as decimal (e.g., 0.02 for 2%)
    /// * `entry_price` - Planned entry price for the position
    /// * `stop_loss` - Stop loss price (must be below entry for long positions)
    /// 
    /// # Returns
    /// * `Ok(PositionSize)` - Calculated position size in shares/units
    /// * `Err(PositionSizingError)` - If inputs are invalid or calculation fails
    /// 
    /// # Errors
    /// This function returns an error if:
    /// - Stop loss is greater than or equal to entry price
    /// - Calculation results in overflow
    /// - Position size would be zero or negative
    /// - Position value would exceed account balance
    /// 
    /// # Examples
    /// ```
    /// use disciplina::{PositionSizingCalculator, AccountEquity, RiskPercentage, PricePoint};
    /// use rust_decimal::Decimal;
    /// use std::str::FromStr;
    /// 
    /// let calculator = PositionSizingCalculator::new();
    /// let result = calculator.calculate_position_size(
    ///     AccountEquity::new(Decimal::from(50000))?,
    ///     RiskPercentage::new(Decimal::from_str("0.015")?)?, // 1.5%
    ///     PricePoint::new(Decimal::from(200))?,
    ///     PricePoint::new(Decimal::from(180))?, // $20 stop distance
    /// )?;
    /// 
    /// // Expected: (50000 * 0.015) / (200 - 180) = 750 / 20 = 37.5 shares
    /// assert_eq!(result.value(), Decimal::from_str("37.5")?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[instrument(level = "debug", skip(self), 
        fields(
            account_equity = %account_equity.value(),
            risk_percentage = %risk_percentage.value(),
            entry_price = %entry_price.value(),
            stop_loss = %stop_loss.value()
        ))]
    pub fn calculate_position_size(
        &self,
        account_equity: AccountEquity,
        risk_percentage: RiskPercentage,
        entry_price: PricePoint,
        stop_loss: PricePoint,
    ) -> Result<PositionSize, PositionSizingError> {
        // Validate that stop loss is below entry price (for long positions)
        if stop_loss.value() >= entry_price.value() {
            warn!(
                entry_price = %entry_price.value(),
                stop_loss = %stop_loss.value(),
                "Stop loss must be below entry price for long positions"
            );
            return Err(PositionSizingError::invalid_stop_distance(
                entry_price.value(),
                stop_loss.value(),
            ));
        }

        // Calculate stop distance (risk per share)
        let stop_distance = entry_price.value() - stop_loss.value();

        // Handle edge case where stop distance is zero (should be caught above, but double-check)
        if stop_distance == Decimal::ZERO {
            return Err(PositionSizingError::division_by_zero(
                entry_price.value(),
                stop_loss.value(),
            ));
        }

        // Calculate risk amount (total dollar amount at risk)
        let risk_amount = match account_equity.value().checked_mul(risk_percentage.value()) {
            Some(amount) => amount,
            None => {
                warn!("Multiplication overflow calculating risk amount");
                return Err(PositionSizingError::CalculationOverflow);
            }
        };

        // Calculate position size using Van Tharp formula
        let position_size_decimal = match risk_amount.checked_div(stop_distance) {
            Some(size) => size,
            None => {
                warn!("Division error calculating position size");
                return Err(PositionSizingError::CalculationOverflow);
            }
        };

        // Apply precision if specified
        let final_position_size = if let Some(precision) = self.precision {
            position_size_decimal.round_dp(precision)
        } else {
            position_size_decimal
        };

        debug!(
            risk_amount = %risk_amount,
            stop_distance = %stop_distance,
            calculated_position_size = %final_position_size,
            "Position size calculation completed"
        );

        // Create PositionSize instance (this will validate that size is positive)
        let position_size = PositionSize::new(final_position_size)?;

        // Additional validation: ensure position value doesn't exceed account balance
        let position_value = position_size.total_value(entry_price);
        if position_value > account_equity.value() {
            warn!(
                position_size = %position_size.value(),
                position_value = %position_value,
                account_balance = %account_equity.value(),
                "Position value would exceed account balance"
            );
            return Err(PositionSizingError::exceeds_account_balance(
                position_value,
                account_equity.value(),
            ));
        }

        Ok(position_size)
    }

    /// Validates a complete trading setup before calculation
    /// 
    /// This method performs comprehensive validation of all inputs to ensure
    /// they form a valid trading scenario.
    /// 
    /// # Arguments
    /// * `account_equity` - Account balance
    /// * `risk_percentage` - Risk per trade
    /// * `entry_price` - Entry price
    /// * `stop_loss` - Stop loss price
    /// 
    /// # Returns
    /// * `Ok(())` - If all inputs are valid
    /// * `Err(PositionSizingError)` - If any input is invalid
    /// 
    /// # Examples
    /// ```
    /// use disciplina::{PositionSizingCalculator, AccountEquity, RiskPercentage, PricePoint};
    /// use rust_decimal::Decimal;
    /// use std::str::FromStr;
    /// 
    /// let calculator = PositionSizingCalculator::new();
    /// let validation_result = calculator.validate_trading_setup(
    ///     AccountEquity::new(Decimal::from(10000))?,
    ///     RiskPercentage::new(Decimal::from_str("0.02")?)?,
    ///     PricePoint::new(Decimal::from(100))?,
    ///     PricePoint::new(Decimal::from(95))?,
    /// );
    /// 
    /// assert!(validation_result.is_ok());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn validate_trading_setup(
        &self,
        _account_equity: AccountEquity,
        _risk_percentage: RiskPercentage,
        entry_price: PricePoint,
        stop_loss: PricePoint,
    ) -> Result<(), PositionSizingError> {
        // Type constructors already validate individual components,
        // but we need to validate the relationship between entry and stop

        if stop_loss.value() >= entry_price.value() {
            return Err(PositionSizingError::invalid_stop_distance(
                entry_price.value(),
                stop_loss.value(),
            ));
        }

        // Validate that the stop distance is not unreasonably small
        let stop_distance = entry_price.value() - stop_loss.value();
        let min_stop_distance = entry_price.value() * Decimal::new(1, 5); // 0.001% of entry price
        
        if stop_distance < min_stop_distance {
            warn!(
                stop_distance = %stop_distance,
                min_stop_distance = %min_stop_distance,
                "Stop distance is extremely small, may result in unrealistic position size"
            );
            // Don't error, but warn about potentially unrealistic scenarios
        }

        Ok(())
    }

    /// Calculates the dollar amount at risk for a given setup
    /// 
    /// # Arguments
    /// * `account_equity` - Account balance
    /// * `risk_percentage` - Risk per trade
    /// 
    /// # Returns
    /// Dollar amount that will be at risk
    /// 
    /// # Examples
    /// ```
    /// use disciplina::{PositionSizingCalculator, AccountEquity, RiskPercentage};
    /// use rust_decimal::Decimal;
    /// use std::str::FromStr;
    /// 
    /// let calculator = PositionSizingCalculator::new();
    /// let risk_amount = calculator.calculate_risk_amount(
    ///     AccountEquity::new(Decimal::from(10000))?,
    ///     RiskPercentage::new(Decimal::from_str("0.02")?)?, // 2%
    /// );
    /// 
    /// assert_eq!(risk_amount, Decimal::from(200)); // 10000 * 0.02 = 200
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn calculate_risk_amount(
        &self,
        account_equity: AccountEquity,
        risk_percentage: RiskPercentage,
    ) -> Decimal {
        account_equity.value() * risk_percentage.value()
    }

    /// Calculates stop distance (risk per share) for a trade setup
    /// 
    /// # Arguments
    /// * `entry_price` - Entry price
    /// * `stop_loss` - Stop loss price
    /// 
    /// # Returns
    /// * `Ok(Decimal)` - Stop distance in price units
    /// * `Err(PositionSizingError)` - If stop loss is not below entry
    /// 
    /// # Examples
    /// ```
    /// use disciplina::{PositionSizingCalculator, PricePoint};
    /// use rust_decimal::Decimal;
    /// 
    /// let calculator = PositionSizingCalculator::new();
    /// let stop_distance = calculator.calculate_stop_distance(
    ///     PricePoint::new(Decimal::from(100))?,
    ///     PricePoint::new(Decimal::from(95))?,
    /// )?;
    /// 
    /// assert_eq!(stop_distance, Decimal::from(5));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn calculate_stop_distance(
        &self,
        entry_price: PricePoint,
        stop_loss: PricePoint,
    ) -> Result<Decimal, PositionSizingError> {
        if stop_loss.value() >= entry_price.value() {
            return Err(PositionSizingError::invalid_stop_distance(
                entry_price.value(),
                stop_loss.value(),
            ));
        }

        Ok(entry_price.value() - stop_loss.value())
    }
}

impl Default for PositionSizingCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_calculator_creation() {
        let calc1 = PositionSizingCalculator::new();
        assert!(calc1.precision.is_none());

        let calc2 = PositionSizingCalculator::with_precision(4);
        assert_eq!(calc2.precision, Some(4));

        let calc3 = PositionSizingCalculator::default();
        assert!(calc3.precision.is_none());
    }

    #[test]
    fn test_basic_position_calculation() {
        let calculator = PositionSizingCalculator::new();
        
        let result = calculator.calculate_position_size(
            AccountEquity::new(Decimal::from(10000)).unwrap(),
            RiskPercentage::new(Decimal::from_str("0.02").unwrap()).unwrap(), // 2%
            PricePoint::new(Decimal::from(100)).unwrap(),
            PricePoint::new(Decimal::from(95)).unwrap(),
        );

        assert!(result.is_ok());
        let position_size = result.unwrap();
        
        // Expected: (10000 * 0.02) / (100 - 95) = 200 / 5 = 40
        assert_eq!(position_size.value(), Decimal::from(40));
    }

    #[test]
    fn test_precision_rounding() {
        let calculator = PositionSizingCalculator::with_precision(2);
        
        let result = calculator.calculate_position_size(
            AccountEquity::new(Decimal::from(10000)).unwrap(),
            RiskPercentage::new(Decimal::from_str("0.023").unwrap()).unwrap(), // 2.3%
            PricePoint::new(Decimal::from_str("100.33").unwrap()).unwrap(),
            PricePoint::new(Decimal::from_str("97.17").unwrap()).unwrap(), // 3.16 stop distance
        );

        assert!(result.is_ok());
        let position_size = result.unwrap();
        
        // Expected calculation: (10000 * 0.023) / 3.16 = 230 / 3.16 ≈ 72.78481013
        // With precision 2: 72.78
        assert_eq!(position_size.value(), Decimal::from_str("72.78").unwrap());
    }

    #[test]
    fn test_validation_methods() {
        let calculator = PositionSizingCalculator::new();
        
        // Test risk amount calculation
        let risk_amount = calculator.calculate_risk_amount(
            AccountEquity::new(Decimal::from(25000)).unwrap(),
            RiskPercentage::new(Decimal::from_str("0.015").unwrap()).unwrap(), // 1.5%
        );
        assert_eq!(risk_amount, Decimal::from_str("375").unwrap()); // 25000 * 0.015

        // Test stop distance calculation
        let stop_distance = calculator.calculate_stop_distance(
            PricePoint::new(Decimal::from(150)).unwrap(),
            PricePoint::new(Decimal::from(142)).unwrap(),
        ).unwrap();
        assert_eq!(stop_distance, Decimal::from(8));

        // Test trading setup validation
        let validation = calculator.validate_trading_setup(
            AccountEquity::new(Decimal::from(10000)).unwrap(),
            RiskPercentage::new(Decimal::from_str("0.02").unwrap()).unwrap(),
            PricePoint::new(Decimal::from(100)).unwrap(),
            PricePoint::new(Decimal::from(95)).unwrap(),
        );
        assert!(validation.is_ok());
    }

    #[test]
    fn test_error_cases() {
        let calculator = PositionSizingCalculator::new();
        
        // Stop loss above entry price
        let result = calculator.calculate_position_size(
            AccountEquity::new(Decimal::from(10000)).unwrap(),
            RiskPercentage::new(Decimal::from_str("0.02").unwrap()).unwrap(),
            PricePoint::new(Decimal::from(95)).unwrap(),
            PricePoint::new(Decimal::from(100)).unwrap(), // Stop above entry
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            PositionSizingError::InvalidStopDistance { .. } => {},
            _ => panic!("Expected InvalidStopDistance error"),
        }

        // Stop loss equal to entry price
        let result = calculator.calculate_position_size(
            AccountEquity::new(Decimal::from(10000)).unwrap(),
            RiskPercentage::new(Decimal::from_str("0.02").unwrap()).unwrap(),
            PricePoint::new(Decimal::from(100)).unwrap(),
            PricePoint::new(Decimal::from(100)).unwrap(), // Stop equal to entry
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_decimal_precision_maintained() {
        let calculator = PositionSizingCalculator::new();
        
        let result = calculator.calculate_position_size(
            AccountEquity::new(Decimal::from_str("12345.6789").unwrap()).unwrap(),
            RiskPercentage::new(Decimal::from_str("0.0234").unwrap()).unwrap(),
            PricePoint::new(Decimal::from_str("987.654321").unwrap()).unwrap(),
            PricePoint::new(Decimal::from_str("876.543210").unwrap()).unwrap(),
        );

        assert!(result.is_ok());
        let position_size = result.unwrap();
        
        // Verify the calculation maintains full decimal precision
        let expected_risk = Decimal::from_str("12345.6789").unwrap() * Decimal::from_str("0.0234").unwrap();
        let expected_stop_distance = Decimal::from_str("987.654321").unwrap() - Decimal::from_str("876.543210").unwrap();
        let expected_position = expected_risk / expected_stop_distance;
        
        assert_eq!(position_size.value(), expected_position);
        assert!(position_size.value().scale() > 0); // Should have decimal places
    }

    #[test]
    fn test_extreme_values() {
        let calculator = PositionSizingCalculator::new();
        
        // Test case that should exceed account balance and be rejected
        let result = calculator.calculate_position_size(
            AccountEquity::new(Decimal::from(10000)).unwrap(),
            RiskPercentage::new(Decimal::from_str("0.01").unwrap()).unwrap(), // 1%
            PricePoint::new(Decimal::from(100)).unwrap(),
            PricePoint::new(Decimal::from_str("99.999").unwrap()).unwrap(), // 0.001 stop distance
        );

        // This should fail because position value would exceed account balance
        assert!(result.is_err());
        match result.unwrap_err() {
            PositionSizingError::ExceedsAccountBalance { .. } => {},
            _ => panic!("Expected ExceedsAccountBalance error"),
        }
    }
}