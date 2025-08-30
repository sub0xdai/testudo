//! Comprehensive tests for position sizing calculations
//! 
//! These tests implement property-based testing with minimum 10,000 iterations
//! to ensure mathematical correctness of Van Tharp position sizing methodology.

use rust_decimal::Decimal;
use std::str::FromStr;
use proptest::prelude::*;

// Import the types we're going to test (these don't exist yet - TDD approach)
use disciplina::{
    PositionSizingCalculator, PositionSizingError, AccountEquity, 
    RiskPercentage, PricePoint
};

/// Property-based tests for position sizing calculation
/// Van Tharp Formula: Position Size = (Account Equity × Risk %) ÷ (Entry Price - Stop Price)
mod property_tests {
    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10000))]
        
        /// Property 1: Position size should be inversely proportional to stop distance
        /// When stop loss gets closer to entry, position size should increase (less risk per unit)
        #[test]
        fn position_size_inverse_to_stop_distance(
            equity in 10000.0..1_000_000.0f64, // Larger equity range to reduce account balance exceeded errors
            risk_pct in 0.005..0.02f64, // Lower risk to reduce position size
            entry in 100.0..1000.0f64, // Reasonable entry price range
            stop_distance_small in 1.0..10.0f64, // Larger stop distances to keep positions reasonable
        ) {
            let account_equity = AccountEquity::new(Decimal::try_from(equity).unwrap()).unwrap();
            let risk_percentage = RiskPercentage::new(Decimal::try_from(risk_pct).unwrap()).unwrap();
            let entry_price = PricePoint::new(Decimal::try_from(entry).unwrap()).unwrap();
            
            // Create two stop distances: one small, one twice as large
            let stop_close = PricePoint::new(Decimal::try_from(entry - stop_distance_small).unwrap()).unwrap();
            let stop_far = PricePoint::new(Decimal::try_from(entry - (stop_distance_small * 2.0)).unwrap()).unwrap();
            
            let calculator = PositionSizingCalculator::new();
            
            // Both calculations should succeed with reasonable inputs
            if let (Ok(size_close), Ok(size_far)) = (
                calculator.calculate_position_size(account_equity, risk_percentage, entry_price, stop_close),
                calculator.calculate_position_size(account_equity, risk_percentage, entry_price, stop_far)
            ) {
                // Closer stops should result in larger positions (inverse relationship)
                prop_assert!(size_close.value() > size_far.value());
            }
            // If either calculation fails (e.g., exceeds account balance), we accept it as an edge case
        }

        /// Property 2: Position size should scale linearly with account equity
        /// Doubling account equity should double position size (all else equal)
        #[test]
        fn position_size_scales_with_equity(
            base_equity in 10000.0..100_000.0f64, // Higher minimum to reduce edge cases
            risk_pct in 0.005..0.02f64, // Lower max risk
            entry in 100.0..1000.0f64, // Reasonable price range
            stop_distance in 5.0..50.0f64, // Larger stop distances
        ) {
            let base_account_equity = AccountEquity::new(Decimal::try_from(base_equity).unwrap()).unwrap();
            let double_account_equity = AccountEquity::new(Decimal::try_from(base_equity * 2.0).unwrap()).unwrap();
            let risk_percentage = RiskPercentage::new(Decimal::try_from(risk_pct).unwrap()).unwrap();
            let entry_price = PricePoint::new(Decimal::try_from(entry).unwrap()).unwrap();
            let stop_loss = PricePoint::new(Decimal::try_from(entry - stop_distance).unwrap()).unwrap();
            
            let calculator = PositionSizingCalculator::new();
            
            // Both calculations should succeed with reasonable inputs
            if let (Ok(size_base), Ok(size_double)) = (
                calculator.calculate_position_size(base_account_equity, risk_percentage, entry_price, stop_loss),
                calculator.calculate_position_size(double_account_equity, risk_percentage, entry_price, stop_loss)
            ) {
                // Double equity should result in double position size (within rounding tolerance)
                let expected_double = size_base.value() * Decimal::from(2);
                let tolerance = Decimal::from_str("0.000001").unwrap(); // 6 decimal places tolerance
                
                prop_assert!((size_double.value() - expected_double).abs() <= tolerance);
            }
        }

        /// Property 3: Position size should scale linearly with risk percentage
        /// Doubling risk percentage should double position size (all else equal)
        #[test]
        fn position_size_scales_with_risk(
            equity in 10000.0..100_000.0f64,
            base_risk_pct in 0.005..0.025f64, // Keep base risk low so double doesn't exceed 6%
            entry in 100.0..1000.0f64,
            stop_distance in 5.0..50.0f64, // Larger stop distances
        ) {
            let account_equity = AccountEquity::new(Decimal::try_from(equity).unwrap()).unwrap();
            let base_risk = RiskPercentage::new(Decimal::try_from(base_risk_pct).unwrap()).unwrap();
            let double_risk = RiskPercentage::new(Decimal::try_from(base_risk_pct * 2.0).unwrap()).unwrap();
            let entry_price = PricePoint::new(Decimal::try_from(entry).unwrap()).unwrap();
            let stop_loss = PricePoint::new(Decimal::try_from(entry - stop_distance).unwrap()).unwrap();
            
            let calculator = PositionSizingCalculator::new();
            
            // Both calculations should succeed with reasonable inputs
            if let (Ok(size_base), Ok(size_double)) = (
                calculator.calculate_position_size(account_equity, base_risk, entry_price, stop_loss),
                calculator.calculate_position_size(account_equity, double_risk, entry_price, stop_loss)
            ) {
                // Double risk should result in double position size (within rounding tolerance)
                let expected_double = size_base.value() * Decimal::from(2);
                let tolerance = Decimal::from_str("0.000001").unwrap();
                
                prop_assert!((size_double.value() - expected_double).abs() <= tolerance);
            }
        }

        /// Property 4: Position size should never exceed account balance
        /// Even with maximum risk, position value should not exceed account equity
        #[test]
        fn position_size_never_exceeds_account_balance(
            equity in 10000.0..1_000_000.0f64,
            risk_pct in 0.005..0.02f64, // Lower risk range
            entry in 100.0..1000.0f64, // Reasonable entry prices
            stop_distance in 1.0..50.0f64, // Ensure stop_distance < entry
        ) {
            // Ensure stop loss is always positive by limiting stop distance
            let actual_stop_distance = stop_distance.min(entry - 1.0); // Keep at least $1 above 0
            
            let account_equity = AccountEquity::new(Decimal::try_from(equity).unwrap()).unwrap();
            let risk_percentage = RiskPercentage::new(Decimal::try_from(risk_pct).unwrap()).unwrap();
            let entry_price = PricePoint::new(Decimal::try_from(entry).unwrap()).unwrap();
            
            // Only create stop_loss if it would be positive
            if let Ok(stop_loss) = PricePoint::new(Decimal::try_from(entry - actual_stop_distance).unwrap()) {
                let calculator = PositionSizingCalculator::new();
                
                if let Ok(position_size) = calculator.calculate_position_size(
                    account_equity, risk_percentage, entry_price, stop_loss
                ) {
                    // Position value should never exceed account equity
                    let position_value = position_size.value() * entry_price.value();
                    prop_assert!(position_value <= account_equity.value());
                }
                // If calculation fails, that's also acceptable for edge cases
            }
        }

        /// Property 5: Risk amount should match specified risk percentage
        /// The actual risk (position_size * stop_distance) should equal account_equity * risk_percentage
        #[test]
        fn risk_amount_matches_specified_percentage(
            equity in 10000.0..100_000.0f64,
            risk_pct in 0.005..0.02f64, // Lower risk range
            entry in 100.0..1000.0f64,
            stop_distance in 5.0..50.0f64, // Larger stop distances
        ) {
            let account_equity = AccountEquity::new(Decimal::try_from(equity).unwrap()).unwrap();
            let risk_percentage = RiskPercentage::new(Decimal::try_from(risk_pct).unwrap()).unwrap();
            let entry_price = PricePoint::new(Decimal::try_from(entry).unwrap()).unwrap();
            let stop_loss = PricePoint::new(Decimal::try_from(entry - stop_distance).unwrap()).unwrap();
            
            let calculator = PositionSizingCalculator::new();
            
            if let Ok(position_size) = calculator.calculate_position_size(
                account_equity, risk_percentage, entry_price, stop_loss
            ) {
                // Calculate actual risk amount
                let actual_risk = position_size.value() * Decimal::try_from(stop_distance).unwrap();
                let expected_risk = account_equity.value() * risk_percentage.value();
                
                // Allow for small rounding differences
                let tolerance = Decimal::from_str("0.01").unwrap(); // 1 cent tolerance
                prop_assert!((actual_risk - expected_risk).abs() <= tolerance);
            }
        }
    }
}

/// Unit tests for specific edge cases and error conditions
mod unit_tests {
    use super::*;

    #[test]
    fn test_zero_account_equity_returns_error() {
        let account_equity = AccountEquity::new(Decimal::ZERO);
        assert!(account_equity.is_err());
        
        match account_equity {
            Err(PositionSizingError::InvalidAccountEquity { .. }) => {},
            _ => panic!("Expected InvalidAccountEquity error"),
        }
    }

    #[test]
    fn test_negative_account_equity_returns_error() {
        let account_equity = AccountEquity::new(Decimal::from(-100));
        assert!(account_equity.is_err());
        
        match account_equity {
            Err(PositionSizingError::InvalidAccountEquity { .. }) => {},
            _ => panic!("Expected InvalidAccountEquity error"),
        }
    }

    #[test]
    fn test_zero_risk_percentage_returns_error() {
        let risk_percentage = RiskPercentage::new(Decimal::ZERO);
        assert!(risk_percentage.is_err());
        
        match risk_percentage {
            Err(PositionSizingError::InvalidRiskPercentage { .. }) => {},
            _ => panic!("Expected InvalidRiskPercentage error"),
        }
    }

    #[test]
    fn test_risk_percentage_too_high_returns_error() {
        let risk_percentage = RiskPercentage::new(Decimal::from_str("0.07").unwrap()); // 7% > 6% max
        assert!(risk_percentage.is_err());
        
        match risk_percentage {
            Err(PositionSizingError::InvalidRiskPercentage { .. }) => {},
            _ => panic!("Expected InvalidRiskPercentage error"),
        }
    }

    #[test]
    fn test_risk_percentage_too_low_returns_error() {
        let risk_percentage = RiskPercentage::new(Decimal::from_str("0.001").unwrap()); // 0.1% < 0.5% min
        assert!(risk_percentage.is_err());
        
        match risk_percentage {
            Err(PositionSizingError::InvalidRiskPercentage { .. }) => {},
            _ => panic!("Expected InvalidRiskPercentage error"),
        }
    }

    #[test]
    fn test_stop_loss_above_entry_returns_error() {
        let account_equity = AccountEquity::new(Decimal::from(10000)).unwrap();
        let risk_percentage = RiskPercentage::new(Decimal::from_str("0.02").unwrap()).unwrap();
        let entry_price = PricePoint::new(Decimal::from(100)).unwrap();
        let stop_loss = PricePoint::new(Decimal::from(110)).unwrap(); // Stop above entry
        
        let calculator = PositionSizingCalculator::new();
        let result = calculator.calculate_position_size(
            account_equity, risk_percentage, entry_price, stop_loss
        );
        
        assert!(result.is_err());
        match result {
            Err(PositionSizingError::InvalidStopDistance { .. }) => {},
            _ => panic!("Expected InvalidStopDistance error"),
        }
    }

    #[test]
    fn test_stop_loss_equal_to_entry_returns_error() {
        let account_equity = AccountEquity::new(Decimal::from(10000)).unwrap();
        let risk_percentage = RiskPercentage::new(Decimal::from_str("0.02").unwrap()).unwrap();
        let entry_price = PricePoint::new(Decimal::from(100)).unwrap();
        let stop_loss = PricePoint::new(Decimal::from(100)).unwrap(); // Stop equal to entry
        
        let calculator = PositionSizingCalculator::new();
        let result = calculator.calculate_position_size(
            account_equity, risk_percentage, entry_price, stop_loss
        );
        
        assert!(result.is_err());
        match result {
            Err(PositionSizingError::InvalidStopDistance { .. }) => {},
            _ => panic!("Expected InvalidStopDistance error"),
        }
    }

    #[test]
    fn test_valid_calculation_returns_correct_position_size() {
        // Test with known values to verify formula implementation
        let account_equity = AccountEquity::new(Decimal::from(10000)).unwrap();
        let risk_percentage = RiskPercentage::new(Decimal::from_str("0.02").unwrap()).unwrap(); // 2%
        let entry_price = PricePoint::new(Decimal::from(100)).unwrap();
        let stop_loss = PricePoint::new(Decimal::from(90)).unwrap(); // $10 stop distance
        
        let calculator = PositionSizingCalculator::new();
        let result = calculator.calculate_position_size(
            account_equity, risk_percentage, entry_price, stop_loss
        );
        
        assert!(result.is_ok());
        let position_size = result.unwrap();
        
        // Expected calculation: (10000 * 0.02) / (100 - 90) = 200 / 10 = 20 shares
        let expected = Decimal::from(20);
        assert_eq!(position_size.value(), expected);
    }

    #[test]
    fn test_small_stop_distance_exceeds_account_balance() {
        // Test with very small stop distance - should detect that position exceeds account balance
        let account_equity = AccountEquity::new(Decimal::from(10000)).unwrap();
        let risk_percentage = RiskPercentage::new(Decimal::from_str("0.02").unwrap()).unwrap(); // 2%
        let entry_price = PricePoint::new(Decimal::from(100)).unwrap();
        let stop_loss = PricePoint::new(Decimal::from_str("99.99").unwrap()).unwrap(); // $0.01 stop distance
        
        let calculator = PositionSizingCalculator::new();
        let result = calculator.calculate_position_size(
            account_equity, risk_percentage, entry_price, stop_loss
        );
        
        // Should fail because position value would exceed account balance
        // Calculation: (10000 * 0.02) / (100 - 99.99) = 200 / 0.01 = 20,000 shares
        // Position value: 20,000 * $100 = $2,000,000 > $10,000 account equity
        assert!(result.is_err());
        match result {
            Err(PositionSizingError::ExceedsAccountBalance { .. }) => {},
            _ => panic!("Expected ExceedsAccountBalance error"),
        }
    }

    #[test]
    fn test_decimal_precision_maintained() {
        // Test that decimal precision is maintained throughout calculations
        let account_equity = AccountEquity::new(Decimal::from_str("10000.123456789").unwrap()).unwrap();
        let risk_percentage = RiskPercentage::new(Decimal::from_str("0.025").unwrap()).unwrap(); // 2.5%
        let entry_price = PricePoint::new(Decimal::from_str("123.456789").unwrap()).unwrap();
        let stop_loss = PricePoint::new(Decimal::from_str("120.123456").unwrap()).unwrap();
        
        let calculator = PositionSizingCalculator::new();
        let result = calculator.calculate_position_size(
            account_equity, risk_percentage, entry_price, stop_loss
        );
        
        assert!(result.is_ok());
        let position_size = result.unwrap();
        
        // Verify that we get a precise decimal result, not a rounded float approximation
        assert!(position_size.value().scale() > 0); // Should have decimal places
        
        // Verify the calculation manually
        let risk_amount = account_equity.value() * risk_percentage.value();
        let stop_distance = entry_price.value() - stop_loss.value();
        let expected = risk_amount / stop_distance;
        
        assert_eq!(position_size.value(), expected);
    }
}

/// Benchmark tests to ensure performance meets requirements (<50ms)
#[cfg(test)]
mod benchmark_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_calculation_performance_under_50ms() {
        let account_equity = AccountEquity::new(Decimal::from(10000)).unwrap();
        let risk_percentage = RiskPercentage::new(Decimal::from_str("0.02").unwrap()).unwrap();
        let entry_price = PricePoint::new(Decimal::from(100)).unwrap();
        let stop_loss = PricePoint::new(Decimal::from(95)).unwrap();
        
        let calculator = PositionSizingCalculator::new();
        
        // Warm up
        for _ in 0..100 {
            let _ = calculator.calculate_position_size(
                account_equity, risk_percentage, entry_price, stop_loss
            );
        }
        
        // Time 1000 calculations
        let start = Instant::now();
        for _ in 0..1000 {
            let _ = calculator.calculate_position_size(
                account_equity, risk_percentage, entry_price, stop_loss
            );
        }
        let duration = start.elapsed();
        
        // Each calculation should be well under 50ms (target < 0.05ms per calculation)
        let avg_per_calculation = duration.as_nanos() as f64 / 1000.0 / 1_000_000.0; // Convert to ms
        assert!(avg_per_calculation < 0.05, 
            "Average calculation time {:.6}ms exceeds 0.05ms target", avg_per_calculation);
    }
}