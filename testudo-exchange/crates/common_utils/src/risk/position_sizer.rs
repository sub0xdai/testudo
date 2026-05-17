//! Position Sizer
//!
//! Calculates optimal position size based on risk parameters.
//! Implements the "Conservative Wins" policy from the PRD.
//!
//! # Position Size Calculation
//!
//! Given:
//! - Account balance
//! - Entry price
//! - Stop loss price
//! - Risk parameters
//!
//! The position size is the MINIMUM of:
//! 1. Size from account % risk
//! 2. Size from fixed risk amount
//! 3. Maximum position size limit

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use super::config::RiskConfig;

/// Position sizing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSizeResult {
    /// The recommended position size
    pub size: Decimal,

    /// The risk amount in quote currency
    pub risk_amount: Decimal,

    /// Which limit was the binding constraint
    pub limiting_factor: LimitingFactor,

    /// All calculated sizes for transparency
    pub calculations: PositionSizeCalculations,
}

/// Which factor limited the position size
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitingFactor {
    AccountRiskPercent,
    MaxRiskAmount,
    MaxPositionSize,
    InsufficientBalance,
}

/// All the individual calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSizeCalculations {
    pub from_account_percent: Decimal,
    pub from_max_risk_amount: Option<Decimal>,
    pub from_max_position_size: Option<Decimal>,
    pub stop_distance: Decimal,
    pub stop_distance_percent: Decimal,
}

/// Position sizer implementing "Conservative Wins" policy
pub struct PositionSizer {
    config: RiskConfig,
}

impl PositionSizer {
    /// Create a new position sizer with the given risk config
    pub fn new(config: RiskConfig) -> Self {
        Self { config }
    }

    /// Calculate position size for a trade
    ///
    /// # Arguments
    /// * `account_balance` - Total account balance in quote currency
    /// * `entry_price` - Planned entry price
    /// * `stop_loss_price` - Stop loss price
    ///
    /// # Returns
    /// Position size result with the recommended size and calculations
    pub fn calculate_position_size(
        &self,
        account_balance: Decimal,
        entry_price: Decimal,
        stop_loss_price: Decimal,
    ) -> PositionSizeResult {
        // Calculate stop distance
        let stop_distance = (entry_price - stop_loss_price).abs();
        let stop_distance_percent = if entry_price > dec!(0) {
            (stop_distance / entry_price) * dec!(100)
        } else {
            dec!(0)
        };

        // Size from account percent risk
        // risk_amount = account_balance * risk_percent / 100
        // size = risk_amount / stop_distance
        let risk_from_percent = account_balance * self.config.account_risk_percent / dec!(100);
        let size_from_percent = if stop_distance > dec!(0) {
            risk_from_percent / stop_distance
        } else {
            dec!(0)
        };

        // Size from max risk amount
        let size_from_max_risk = self.config.max_risk_amount.map(|max_risk| {
            if stop_distance > dec!(0) {
                max_risk / stop_distance
            } else {
                dec!(0)
            }
        });

        // Size from max position size (direct limit)
        let size_from_max_position = self.config.max_position_size;

        // Find the minimum (Conservative Wins)
        let mut min_size = size_from_percent;
        let mut limiting_factor = LimitingFactor::AccountRiskPercent;

        if let Some(size) = size_from_max_risk {
            if size < min_size {
                min_size = size;
                limiting_factor = LimitingFactor::MaxRiskAmount;
            }
        }

        if let Some(size) = size_from_max_position {
            if size < min_size {
                min_size = size;
                limiting_factor = LimitingFactor::MaxPositionSize;
            }
        }

        // Check if we can afford this position
        let required_margin = min_size * entry_price / Decimal::from(self.config.max_leverage);
        if required_margin > account_balance {
            // Reduce to what we can afford
            min_size = account_balance * Decimal::from(self.config.max_leverage) / entry_price;
            limiting_factor = LimitingFactor::InsufficientBalance;
        }

        // Calculate actual risk amount for the final size
        let risk_amount = min_size * stop_distance;

        PositionSizeResult {
            size: min_size,
            risk_amount,
            limiting_factor,
            calculations: PositionSizeCalculations {
                from_account_percent: size_from_percent,
                from_max_risk_amount: size_from_max_risk,
                from_max_position_size: size_from_max_position,
                stop_distance,
                stop_distance_percent,
            },
        }
    }

    /// Calculate position size using ATR for stop distance
    ///
    /// # Arguments
    /// * `account_balance` - Total account balance in quote currency
    /// * `entry_price` - Planned entry price
    /// * `atr` - Average True Range value
    /// * `atr_multiplier` - Optional multiplier (uses config default if not provided)
    pub fn calculate_position_size_with_atr(
        &self,
        account_balance: Decimal,
        entry_price: Decimal,
        atr: Decimal,
        atr_multiplier: Option<Decimal>,
    ) -> PositionSizeResult {
        let multiplier = atr_multiplier
            .or(self.config.default_stop_atr_multiplier)
            .unwrap_or(dec!(2));

        let stop_distance = atr * multiplier;
        let stop_loss_price = entry_price - stop_distance;

        self.calculate_position_size(account_balance, entry_price, stop_loss_price)
    }

    /// Check if a trade meets the minimum risk/reward ratio
    pub fn meets_risk_reward_ratio(
        &self,
        entry_price: Decimal,
        stop_loss_price: Decimal,
        take_profit_price: Decimal,
    ) -> bool {
        let risk = (entry_price - stop_loss_price).abs();
        let reward = (take_profit_price - entry_price).abs();

        if risk == dec!(0) {
            return false;
        }

        let ratio = reward / risk;

        match self.config.min_risk_reward_ratio {
            Some(min_ratio) => ratio >= min_ratio,
            None => true,
        }
    }

    /// Calculate the take profit price for a given risk/reward ratio
    pub fn calculate_take_profit(
        &self,
        entry_price: Decimal,
        stop_loss_price: Decimal,
        risk_reward_ratio: Decimal,
        is_long: bool,
    ) -> Decimal {
        let risk = (entry_price - stop_loss_price).abs();
        let reward = risk * risk_reward_ratio;

        if is_long {
            entry_price + reward
        } else {
            entry_price - reward
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_position_sizing() {
        let config = RiskConfig::new().with_account_risk_percent(dec!(2));
        let sizer = PositionSizer::new(config);

        // Account: $10,000, Entry: $50,000, Stop: $49,000 (2% stop)
        let result = sizer.calculate_position_size(dec!(10000), dec!(50000), dec!(49000));

        // Risk: 2% of $10,000 = $200
        // Stop distance: $1,000
        // Size: $200 / $1,000 = 0.2 BTC
        assert_eq!(result.size, dec!(0.2));
        assert_eq!(result.risk_amount, dec!(200));
        assert_eq!(result.limiting_factor, LimitingFactor::AccountRiskPercent);
    }

    #[test]
    fn test_max_risk_amount_limits() {
        let config = RiskConfig::new()
            .with_account_risk_percent(dec!(2))
            .with_max_risk_amount(dec!(100)); // Limit to $100 risk

        let sizer = PositionSizer::new(config);

        // Without max risk: 2% of $10,000 = $200 risk = 0.2 BTC
        // With $100 max risk: $100 / $1,000 = 0.1 BTC
        let result = sizer.calculate_position_size(dec!(10000), dec!(50000), dec!(49000));

        assert_eq!(result.size, dec!(0.1));
        assert_eq!(result.limiting_factor, LimitingFactor::MaxRiskAmount);
    }

    #[test]
    fn test_max_position_size_limits() {
        let config = RiskConfig::new()
            .with_account_risk_percent(dec!(2))
            .with_max_position_size(dec!(0.05)); // Max 0.05 BTC

        let sizer = PositionSizer::new(config);

        // Without limit: 0.2 BTC
        // With 0.05 BTC limit: 0.05 BTC
        let result = sizer.calculate_position_size(dec!(10000), dec!(50000), dec!(49000));

        assert_eq!(result.size, dec!(0.05));
        assert_eq!(result.limiting_factor, LimitingFactor::MaxPositionSize);
    }

    #[test]
    fn test_conservative_wins() {
        // Test that the smallest of all limits is used
        let config = RiskConfig::new()
            .with_account_risk_percent(dec!(2)) // Would give 0.2 BTC
            .with_max_risk_amount(dec!(150)) // Would give 0.15 BTC
            .with_max_position_size(dec!(0.1)); // Limit: 0.1 BTC

        let sizer = PositionSizer::new(config);
        let result = sizer.calculate_position_size(dec!(10000), dec!(50000), dec!(49000));

        // Max position size (0.1) is the smallest, so it wins
        assert_eq!(result.size, dec!(0.1));
        assert_eq!(result.limiting_factor, LimitingFactor::MaxPositionSize);
    }

    #[test]
    fn test_insufficient_balance() {
        let config = RiskConfig::new()
            .with_account_risk_percent(dec!(50)) // 50% risk would need more than balance
            .with_max_leverage(1); // 1x leverage for this margin test

        let sizer = PositionSizer::new(config);

        // 50% of $1000 = $500 risk, with $1000 stop = 0.5 BTC
        // At $50,000 per BTC, this needs $25,000 margin - more than $1000 balance
        let result = sizer.calculate_position_size(dec!(1000), dec!(50000), dec!(49000));

        assert_eq!(result.limiting_factor, LimitingFactor::InsufficientBalance);
        // With 1x leverage, max we can buy is $1000 / $50,000 = 0.02 BTC
        assert_eq!(result.size, dec!(0.02));
    }

    #[test]
    fn test_atr_based_sizing() {
        let config = RiskConfig::new()
            .with_account_risk_percent(dec!(2))
            .with_max_risk_amount(dec!(200));

        let sizer = PositionSizer::new(config);

        // ATR of $500, 2x multiplier = $1000 stop distance
        let result = sizer.calculate_position_size_with_atr(
            dec!(10000),   // account
            dec!(50000),   // entry
            dec!(500),     // ATR
            Some(dec!(2)), // 2x ATR multiplier
        );

        // Same as basic test: $200 risk / $1000 stop = 0.2 BTC
        assert_eq!(result.size, dec!(0.2));
        assert_eq!(result.calculations.stop_distance, dec!(1000));
    }

    #[test]
    fn test_risk_reward_ratio() {
        let config = RiskConfig::new().with_min_risk_reward(dec!(2));
        let sizer = PositionSizer::new(config);

        // Entry: 50000, Stop: 49000, Take Profit: 52000
        // Risk: 1000, Reward: 2000, Ratio: 2:1
        assert!(sizer.meets_risk_reward_ratio(dec!(50000), dec!(49000), dec!(52000)));

        // Entry: 50000, Stop: 49000, Take Profit: 50500
        // Risk: 1000, Reward: 500, Ratio: 0.5:1 - FAILS
        assert!(!sizer.meets_risk_reward_ratio(dec!(50000), dec!(49000), dec!(50500)));
    }

    #[test]
    fn test_calculate_take_profit() {
        let config = RiskConfig::new();
        let sizer = PositionSizer::new(config);

        // Long: Entry 50000, Stop 49000, 2:1 RR
        let tp = sizer.calculate_take_profit(dec!(50000), dec!(49000), dec!(2), true);
        assert_eq!(tp, dec!(52000)); // Risk 1000, Reward 2000

        // Short: Entry 50000, Stop 51000, 2:1 RR
        let tp = sizer.calculate_take_profit(dec!(50000), dec!(51000), dec!(2), false);
        assert_eq!(tp, dec!(48000)); // Risk 1000, Reward 2000
    }

    #[test]
    fn test_calculations_transparency() {
        let config = RiskConfig::new()
            .with_account_risk_percent(dec!(2))
            .with_max_risk_amount(dec!(100))
            .with_max_position_size(dec!(0.5));

        let sizer = PositionSizer::new(config);
        let result = sizer.calculate_position_size(dec!(10000), dec!(50000), dec!(49000));

        // Check all calculations are present
        assert_eq!(result.calculations.stop_distance, dec!(1000));
        assert_eq!(result.calculations.stop_distance_percent, dec!(2));
        assert_eq!(result.calculations.from_account_percent, dec!(0.2));
        assert_eq!(result.calculations.from_max_risk_amount, Some(dec!(0.1)));
        assert_eq!(result.calculations.from_max_position_size, Some(dec!(0.5)));
    }
}
