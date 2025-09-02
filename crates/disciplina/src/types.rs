//! Core types for position sizing calculations
//!
//! This module defines type-safe wrappers around financial values to prevent
//! common errors like negative prices or invalid risk percentages.

use crate::errors::PositionSizingError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Minimum allowable risk percentage (0.5%)
pub const MIN_RISK_PERCENTAGE: &str = "0.005";

/// Maximum allowable risk percentage (6% - Testudo Protocol limit)
pub const MAX_RISK_PERCENTAGE: &str = "0.06";

/// Represents account equity with validation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AccountEquity(Decimal);

impl AccountEquity {
    /// Creates a new AccountEquity instance
    /// 
    /// # Arguments
    /// * `value` - Account equity amount in dollars/base currency
    /// 
    /// # Returns
    /// * `Ok(AccountEquity)` if value is positive
    /// * `Err(PositionSizingError)` if value is zero or negative
    /// 
    /// # Examples
    /// ```
    /// use disciplina::AccountEquity;
    /// use rust_decimal::Decimal;
    /// 
    /// let equity = AccountEquity::new(Decimal::from(10000))?;
    /// assert_eq!(equity.value(), Decimal::from(10000));
    /// # Ok::<(), disciplina::PositionSizingError>(())
    /// ```
    pub fn new(value: Decimal) -> Result<Self, PositionSizingError> {
        if value <= Decimal::ZERO {
            return Err(PositionSizingError::invalid_account_equity(value));
        }
        Ok(Self(value))
    }

    /// Returns the underlying account equity value
    pub fn value(self) -> Decimal {
        self.0
    }

    /// Creates AccountEquity from a string representation
    /// 
    /// # Examples
    /// ```
    /// use disciplina::AccountEquity;
    /// 
    /// let equity = AccountEquity::from_str("25000.50")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_str(s: &str) -> Result<Self, PositionSizingError> {
        let decimal = Decimal::from_str(s)
            .map_err(|_| PositionSizingError::calculation_failed(
                format!("Failed to parse account equity from string: {}", s)
            ))?;
        Self::new(decimal)
    }
}

impl fmt::Display for AccountEquity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}

/// Represents risk percentage with validation (0.5% to 6%)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RiskPercentage(Decimal);

impl RiskPercentage {
    /// Creates a new RiskPercentage instance
    /// 
    /// # Arguments
    /// * `value` - Risk percentage as decimal (e.g., 0.02 for 2%)
    /// 
    /// # Returns
    /// * `Ok(RiskPercentage)` if value is between 0.5% and 6%
    /// * `Err(PositionSizingError)` if value is outside valid range
    /// 
    /// # Examples
    /// ```
    /// use disciplina::RiskPercentage;
    /// use rust_decimal::Decimal;
    /// use std::str::FromStr;
    /// 
    /// let risk = RiskPercentage::new(Decimal::from_str("0.02")?)?; // 2%
    /// assert_eq!(risk.as_percentage(), Decimal::from(2));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(value: Decimal) -> Result<Self, PositionSizingError> {
        let min_risk = Decimal::from_str(MIN_RISK_PERCENTAGE).unwrap();
        let max_risk = Decimal::from_str(MAX_RISK_PERCENTAGE).unwrap();
        
        if value < min_risk || value > max_risk {
            return Err(PositionSizingError::invalid_risk_percentage(value));
        }
        Ok(Self(value))
    }

    /// Returns the underlying risk percentage value as decimal
    pub fn value(self) -> Decimal {
        self.0
    }

    /// Returns the risk percentage as a percentage (e.g., 0.02 -> 2.0)
    pub fn as_percentage(self) -> Decimal {
        self.0 * Decimal::from(100)
    }

    /// Creates RiskPercentage from percentage value (e.g., 2.5 for 2.5%)
    /// 
    /// # Examples
    /// ```
    /// use disciplina::RiskPercentage;
    /// use rust_decimal::Decimal;
    /// use std::str::FromStr;
    /// 
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let risk = RiskPercentage::from_percentage(Decimal::from_str("2.5")?)?; // 2.5%
    /// assert_eq!(risk.value(), Decimal::from_str("0.025")?);
    /// # Ok(())
    /// # }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_percentage(percentage: Decimal) -> Result<Self, PositionSizingError> {
        let decimal_value = percentage / Decimal::from(100);
        Self::new(decimal_value)
    }

    /// Creates RiskPercentage from string representation
    pub fn from_str(s: &str) -> Result<Self, PositionSizingError> {
        let decimal = Decimal::from_str(s)
            .map_err(|_| PositionSizingError::calculation_failed(
                format!("Failed to parse risk percentage from string: {}", s)
            ))?;
        Self::new(decimal)
    }
}

impl fmt::Display for RiskPercentage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}%", self.as_percentage())
    }
}

/// Represents a price point with validation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PricePoint(Decimal);

impl PricePoint {
    /// Creates a new PricePoint instance
    /// 
    /// # Arguments
    /// * `value` - Price value in dollars/base currency
    /// 
    /// # Returns
    /// * `Ok(PricePoint)` if value is positive
    /// * `Err(PositionSizingError)` if value is zero or negative
    /// 
    /// # Examples
    /// ```
    /// use disciplina::PricePoint;
    /// use rust_decimal::Decimal;
    /// use std::str::FromStr;
    /// 
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let price = PricePoint::new(Decimal::from_str("123.45")?)?;
    /// assert_eq!(price.value(), Decimal::from_str("123.45")?);
    /// # Ok(())
    /// # }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(value: Decimal) -> Result<Self, PositionSizingError> {
        if value <= Decimal::ZERO {
            return Err(PositionSizingError::invalid_price_point(value));
        }
        Ok(Self(value))
    }

    /// Returns the underlying price value
    pub fn value(self) -> Decimal {
        self.0
    }

    /// Creates PricePoint from string representation
    pub fn from_str(s: &str) -> Result<Self, PositionSizingError> {
        let decimal = Decimal::from_str(s)
            .map_err(|_| PositionSizingError::calculation_failed(
                format!("Failed to parse price point from string: {}", s)
            ))?;
        Self::new(decimal)
    }
}

impl fmt::Display for PricePoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}

/// Represents a calculated position size
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PositionSize(Decimal);

impl PositionSize {
    /// Creates a new PositionSize instance (typically used internally)
    /// 
    /// # Arguments
    /// * `value` - Position size in shares/units
    /// 
    /// # Returns
    /// * `Ok(PositionSize)` if value is positive
    /// * `Err(PositionSizingError)` if value is zero or negative
    pub fn new(value: Decimal) -> Result<Self, PositionSizingError> {
        if value <= Decimal::ZERO {
            return Err(PositionSizingError::InvalidPositionSizeResult { value });
        }
        Ok(Self(value))
    }

    /// Returns the underlying position size value
    pub fn value(self) -> Decimal {
        self.0
    }

    /// Returns the position size rounded to a specific number of decimal places
    /// 
    /// # Arguments
    /// * `decimal_places` - Number of decimal places to round to
    /// 
    /// # Examples
    /// ```
    /// use disciplina::PositionSize;
    /// use rust_decimal::Decimal;
    /// use std::str::FromStr;
    /// 
    /// let size = PositionSize::new(Decimal::from_str("123.456789")?)?;
    /// assert_eq!(size.rounded(2).to_string(), "123.46");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn rounded(self, decimal_places: u32) -> Decimal {
        self.0.round_dp(decimal_places)
    }

    /// Calculates the total value of this position at a given price
    /// 
    /// # Arguments
    /// * `price` - Price per share/unit
    /// 
    /// # Examples
    /// ```
    /// use disciplina::{PositionSize, PricePoint};
    /// use rust_decimal::Decimal;
    /// use std::str::FromStr;
    /// 
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let size = PositionSize::new(Decimal::from(100))?;
    /// let price = PricePoint::new(Decimal::from_str("50.25")?)?;
    /// let total_value = size.total_value(price);
    /// assert_eq!(total_value, Decimal::from_str("5025.00")?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn total_value(self, price: PricePoint) -> Decimal {
        self.0 * price.value()
    }
}

impl fmt::Display for PositionSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} shares", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_equity_validation() {
        // Valid account equity
        let valid = AccountEquity::new(Decimal::from(10000));
        assert!(valid.is_ok());
        assert_eq!(valid.unwrap().value(), Decimal::from(10000));

        // Zero account equity should fail
        let zero = AccountEquity::new(Decimal::ZERO);
        assert!(zero.is_err());
        match zero {
            Err(PositionSizingError::InvalidAccountEquity { value }) => {
                assert_eq!(value, Decimal::ZERO);
            }
            _ => panic!("Expected InvalidAccountEquity error"),
        }

        // Negative account equity should fail
        let negative = AccountEquity::new(Decimal::from(-1000));
        assert!(negative.is_err());
    }

    #[test]
    fn test_risk_percentage_validation() {
        // Valid risk percentages
        let valid_min = RiskPercentage::new(Decimal::from_str("0.005").unwrap());
        assert!(valid_min.is_ok());

        let valid_mid = RiskPercentage::new(Decimal::from_str("0.02").unwrap());
        assert!(valid_mid.is_ok());

        let valid_max = RiskPercentage::new(Decimal::from_str("0.06").unwrap());
        assert!(valid_max.is_ok());

        // Risk percentage too low
        let too_low = RiskPercentage::new(Decimal::from_str("0.001").unwrap());
        assert!(too_low.is_err());

        // Risk percentage too high
        let too_high = RiskPercentage::new(Decimal::from_str("0.1").unwrap());
        assert!(too_high.is_err());
    }

    #[test]
    fn test_risk_percentage_conversion() {
        let risk = RiskPercentage::new(Decimal::from_str("0.025").unwrap()).unwrap();
        assert_eq!(risk.as_percentage(), Decimal::from_str("2.5").unwrap());

        let from_percentage = RiskPercentage::from_percentage(Decimal::from_str("3.0").unwrap()).unwrap();
        assert_eq!(from_percentage.value(), Decimal::from_str("0.03").unwrap());
    }

    #[test]
    fn test_price_point_validation() {
        // Valid price point
        let valid = PricePoint::new(Decimal::from_str("123.45").unwrap());
        assert!(valid.is_ok());
        assert_eq!(valid.unwrap().value(), Decimal::from_str("123.45").unwrap());

        // Zero price should fail
        let zero = PricePoint::new(Decimal::ZERO);
        assert!(zero.is_err());

        // Negative price should fail
        let negative = PricePoint::new(Decimal::from(-50));
        assert!(negative.is_err());
    }

    #[test]
    fn test_position_size_calculations() {
        let size = PositionSize::new(Decimal::from_str("123.456789").unwrap()).unwrap();
        
        // Test rounding
        assert_eq!(size.rounded(2), Decimal::from_str("123.46").unwrap());
        assert_eq!(size.rounded(0), Decimal::from(123));

        // Test total value calculation
        let price = PricePoint::new(Decimal::from_str("10.50").unwrap()).unwrap();
        // Calculate expected: 123.456789 * 10.50 = 1296.29628450
        let expected_total = size.value() * price.value();
        assert_eq!(size.total_value(price), expected_total);
    }

    #[test]
    fn test_display_formatting() {
        let equity = AccountEquity::new(Decimal::from(10000)).unwrap();
        assert_eq!(equity.to_string(), "$10000");

        let risk = RiskPercentage::new(Decimal::from_str("0.025").unwrap()).unwrap();
        assert_eq!(risk.to_string(), "2.50%");

        let price = PricePoint::new(Decimal::from_str("123.45").unwrap()).unwrap();
        assert_eq!(price.to_string(), "$123.45");

        let size = PositionSize::new(Decimal::from(150)).unwrap();
        assert_eq!(size.to_string(), "150 shares");
    }

    #[test]
    fn test_from_str_parsing() {
        let equity = AccountEquity::from_str("25000.75").unwrap();
        assert_eq!(equity.value(), Decimal::from_str("25000.75").unwrap());

        let risk = RiskPercentage::from_str("0.018").unwrap();
        assert_eq!(risk.value(), Decimal::from_str("0.018").unwrap());

        let price = PricePoint::from_str("87.32").unwrap();
        assert_eq!(price.value(), Decimal::from_str("87.32").unwrap());

        // Test invalid string parsing
        let invalid_equity = AccountEquity::from_str("not_a_number");
        assert!(invalid_equity.is_err());
    }

    #[test]
    fn test_ordering_and_comparison() {
        let equity1 = AccountEquity::new(Decimal::from(1000)).unwrap();
        let equity2 = AccountEquity::new(Decimal::from(2000)).unwrap();
        assert!(equity1 < equity2);

        let risk1 = RiskPercentage::new(Decimal::from_str("0.01").unwrap()).unwrap();
        let risk2 = RiskPercentage::new(Decimal::from_str("0.02").unwrap()).unwrap();
        assert!(risk1 < risk2);

        let price1 = PricePoint::new(Decimal::from(100)).unwrap();
        let price2 = PricePoint::new(Decimal::from(200)).unwrap();
        assert!(price1 < price2);
    }

    #[test]
    fn test_serialization() {
        use serde_json;

        let equity = AccountEquity::new(Decimal::from(10000)).unwrap();
        let json = serde_json::to_string(&equity).unwrap();
        let deserialized: AccountEquity = serde_json::from_str(&json).unwrap();
        assert_eq!(equity, deserialized);

        let risk = RiskPercentage::new(Decimal::from_str("0.02").unwrap()).unwrap();
        let json = serde_json::to_string(&risk).unwrap();
        let deserialized: RiskPercentage = serde_json::from_str(&json).unwrap();
        assert_eq!(risk, deserialized);
    }
}