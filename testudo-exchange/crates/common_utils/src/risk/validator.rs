//! Risk Validator
//!
//! Validates orders against risk limits before execution.
//! Implements pre-trade risk checks from the PRD.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use super::config::RiskConfig;

/// Result of risk validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskValidationResult {
    pub is_valid: bool,
    pub violations: Vec<RiskViolation>,
    pub warnings: Vec<RiskWarning>,
}

impl RiskValidationResult {
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            violations: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn with_violation(violation: RiskViolation) -> Self {
        Self {
            is_valid: false,
            violations: vec![violation],
            warnings: Vec::new(),
        }
    }

    pub fn add_violation(&mut self, violation: RiskViolation) {
        self.is_valid = false;
        self.violations.push(violation);
    }

    pub fn add_warning(&mut self, warning: RiskWarning) {
        self.warnings.push(warning);
    }
}

/// Types of risk violations that block trades
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "details")]
pub enum RiskViolation {
    /// Position size exceeds maximum allowed
    PositionSizeExceeded {
        requested: Decimal,
        maximum: Decimal,
    },

    /// Risk amount exceeds maximum allowed
    RiskAmountExceeded {
        calculated_risk: Decimal,
        maximum: Decimal,
    },

    /// Stop loss is required but not provided
    StopLossRequired,

    /// Risk/reward ratio is below minimum
    InsufficientRiskReward {
        calculated: Decimal,
        minimum: Decimal,
    },

    /// Daily drawdown limit reached
    DailyDrawdownLimitReached {
        current_drawdown: Decimal,
        limit: Decimal,
    },

    /// Maximum open positions reached
    MaxOpenPositionsReached { current: u32, maximum: u32 },

    /// Leverage exceeds maximum allowed
    LeverageExceeded { requested: u8, maximum: u8 },

    /// Insufficient balance for the trade
    InsufficientBalance {
        required: Decimal,
        available: Decimal,
    },
}

/// Warnings that don't block trades but inform the user
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "details")]
pub enum RiskWarning {
    /// Risk is higher than usual
    HighRisk {
        risk_percent: Decimal,
        typical: Decimal,
    },

    /// Stop is very tight
    TightStop { stop_percent: Decimal },

    /// Stop is very wide
    WideStop { stop_percent: Decimal },

    /// Position is large relative to account
    LargePosition { position_percent: Decimal },
}

/// Order details for validation
#[derive(Debug, Clone)]
pub struct OrderForValidation {
    pub symbol: String,
    pub side: OrderSide,
    pub size: Decimal,
    pub entry_price: Decimal,
    pub stop_loss_price: Option<Decimal>,
    pub take_profit_price: Option<Decimal>,
    pub leverage: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Account state for validation
#[derive(Debug, Clone)]
pub struct AccountState {
    pub balance: Decimal,
    pub open_position_count: u32,
    pub daily_pnl: Decimal,
    pub starting_balance: Decimal,
}

impl AccountState {
    /// Calculate current daily drawdown percentage
    pub fn daily_drawdown_percent(&self) -> Decimal {
        if self.starting_balance == dec!(0) {
            return dec!(0);
        }
        let drawdown = self.starting_balance - (self.starting_balance + self.daily_pnl);
        (drawdown / self.starting_balance) * dec!(100)
    }
}

/// Risk validator that checks orders against risk rules
pub struct RiskValidator {
    config: RiskConfig,
}

impl RiskValidator {
    pub fn new(config: RiskConfig) -> Self {
        Self { config }
    }

    /// Validate an order against all risk rules
    pub fn validate(
        &self,
        order: &OrderForValidation,
        account: &AccountState,
    ) -> RiskValidationResult {
        let mut result = RiskValidationResult::valid();

        // Check stop loss requirement
        if self.config.require_stop_loss && order.stop_loss_price.is_none() {
            result.add_violation(RiskViolation::StopLossRequired);
        }

        // Check position size limit
        if let Some(max_size) = self.config.max_position_size {
            if order.size > max_size {
                result.add_violation(RiskViolation::PositionSizeExceeded {
                    requested: order.size,
                    maximum: max_size,
                });
            }
        }

        // Check leverage
        if order.leverage > self.config.max_leverage {
            result.add_violation(RiskViolation::LeverageExceeded {
                requested: order.leverage,
                maximum: self.config.max_leverage,
            });
        }

        // Check open positions limit
        if let Some(max_positions) = self.config.max_open_positions {
            if account.open_position_count >= max_positions {
                result.add_violation(RiskViolation::MaxOpenPositionsReached {
                    current: account.open_position_count,
                    maximum: max_positions,
                });
            }
        }

        // Check daily drawdown
        if let Some(max_drawdown) = self.config.daily_max_drawdown_percent {
            let current_drawdown = account.daily_drawdown_percent();
            if current_drawdown >= max_drawdown {
                result.add_violation(RiskViolation::DailyDrawdownLimitReached {
                    current_drawdown,
                    limit: max_drawdown,
                });
            }
        }

        // Check risk amount
        if let Some(stop_price) = order.stop_loss_price {
            let stop_distance = (order.entry_price - stop_price).abs();
            let risk_amount = order.size * stop_distance;

            if let Some(max_risk) = self.config.max_risk_amount {
                if risk_amount > max_risk {
                    result.add_violation(RiskViolation::RiskAmountExceeded {
                        calculated_risk: risk_amount,
                        maximum: max_risk,
                    });
                }
            }

            // Check risk/reward ratio
            if let (Some(tp), Some(min_rr)) =
                (order.take_profit_price, self.config.min_risk_reward_ratio)
            {
                let reward = (tp - order.entry_price).abs();
                let ratio = if stop_distance > dec!(0) {
                    reward / stop_distance
                } else {
                    dec!(0)
                };

                if ratio < min_rr {
                    result.add_violation(RiskViolation::InsufficientRiskReward {
                        calculated: ratio,
                        minimum: min_rr,
                    });
                }
            }

            // Warnings
            let risk_percent = if account.balance > dec!(0) {
                (risk_amount / account.balance) * dec!(100)
            } else {
                dec!(0)
            };

            // Warn if risk is significantly higher than config
            if risk_percent > self.config.account_risk_percent * dec!(1.5) {
                result.add_warning(RiskWarning::HighRisk {
                    risk_percent,
                    typical: self.config.account_risk_percent,
                });
            }

            // Warn about tight stops (< 0.5%)
            let stop_percent = (stop_distance / order.entry_price) * dec!(100);
            if stop_percent < dec!(0.5) {
                result.add_warning(RiskWarning::TightStop { stop_percent });
            }

            // Warn about wide stops (> 5%)
            if stop_percent > dec!(5) {
                result.add_warning(RiskWarning::WideStop { stop_percent });
            }
        }

        // Check balance
        let required_margin = order.size * order.entry_price / Decimal::from(order.leverage);
        if required_margin > account.balance {
            result.add_violation(RiskViolation::InsufficientBalance {
                required: required_margin,
                available: account.balance,
            });
        }

        // Warn about large positions (> 50% of account)
        let position_value = order.size * order.entry_price;
        if account.balance > dec!(0) {
            let position_percent = (position_value / account.balance) * dec!(100);
            if position_percent > dec!(50) {
                result.add_warning(RiskWarning::LargePosition { position_percent });
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_account() -> AccountState {
        AccountState {
            balance: dec!(10000),
            open_position_count: 0,
            daily_pnl: dec!(0),
            starting_balance: dec!(10000),
        }
    }

    fn default_order() -> OrderForValidation {
        OrderForValidation {
            symbol: "BTC_USDC".to_string(),
            side: OrderSide::Buy,
            size: dec!(0.1),
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: Some(dec!(52000)),
            leverage: 1,
        }
    }

    #[test]
    fn test_valid_order() {
        let config = RiskConfig::default();
        let validator = RiskValidator::new(config);

        let result = validator.validate(&default_order(), &default_account());
        assert!(result.is_valid);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn test_stop_loss_required() {
        let config = RiskConfig::new().with_require_stop_loss(true);
        let validator = RiskValidator::new(config);

        let mut order = default_order();
        order.stop_loss_price = None;

        let result = validator.validate(&order, &default_account());
        assert!(!result.is_valid);
        assert!(matches!(
            result.violations.first(),
            Some(RiskViolation::StopLossRequired)
        ));
    }

    #[test]
    fn test_position_size_exceeded() {
        let config = RiskConfig::new()
            .with_max_position_size(dec!(0.05))
            .with_require_stop_loss(false);
        let validator = RiskValidator::new(config);

        let order = default_order(); // size = 0.1, max = 0.05

        let result = validator.validate(&order, &default_account());
        assert!(!result.is_valid);
        assert!(matches!(
            result.violations.first(),
            Some(RiskViolation::PositionSizeExceeded { .. })
        ));
    }

    #[test]
    fn test_leverage_exceeded() {
        let config = RiskConfig::new()
            .with_max_leverage(5)
            .with_require_stop_loss(false);
        let validator = RiskValidator::new(config);

        let mut order = default_order();
        order.leverage = 10;

        let result = validator.validate(&order, &default_account());
        assert!(!result.is_valid);
        assert!(matches!(
            result.violations.first(),
            Some(RiskViolation::LeverageExceeded { .. })
        ));
    }

    #[test]
    fn test_max_open_positions() {
        let config = RiskConfig::new()
            .with_max_open_positions(3)
            .with_require_stop_loss(false);
        let validator = RiskValidator::new(config);

        let mut account = default_account();
        account.open_position_count = 3;

        let result = validator.validate(&default_order(), &account);
        assert!(!result.is_valid);
        assert!(matches!(
            result.violations.first(),
            Some(RiskViolation::MaxOpenPositionsReached { .. })
        ));
    }

    #[test]
    fn test_daily_drawdown_limit() {
        let config = RiskConfig::new()
            .with_daily_max_drawdown(dec!(5))
            .with_require_stop_loss(false);
        let validator = RiskValidator::new(config);

        let mut account = default_account();
        account.daily_pnl = dec!(-600); // 6% drawdown

        let result = validator.validate(&default_order(), &account);
        assert!(!result.is_valid);
        assert!(matches!(
            result.violations.first(),
            Some(RiskViolation::DailyDrawdownLimitReached { .. })
        ));
    }

    #[test]
    fn test_insufficient_risk_reward() {
        let config = RiskConfig::new().with_min_risk_reward(dec!(2));
        let validator = RiskValidator::new(config);

        let mut order = default_order();
        order.take_profit_price = Some(dec!(50500)); // Only 0.5:1 RR

        let result = validator.validate(&order, &default_account());
        assert!(!result.is_valid);
        assert!(matches!(
            result.violations.first(),
            Some(RiskViolation::InsufficientRiskReward { .. })
        ));
    }

    #[test]
    fn test_insufficient_balance() {
        let config = RiskConfig::new().with_require_stop_loss(false);
        let validator = RiskValidator::new(config);

        let mut account = default_account();
        account.balance = dec!(1000); // Not enough for 0.1 BTC @ $50k

        let result = validator.validate(&default_order(), &account);
        assert!(!result.is_valid);
        assert!(matches!(
            result.violations.first(),
            Some(RiskViolation::InsufficientBalance { .. })
        ));
    }

    #[test]
    fn test_warning_high_risk() {
        let config = RiskConfig::new().with_account_risk_percent(dec!(1));
        let validator = RiskValidator::new(config);

        let mut order = default_order();
        // 0.1 BTC * $1000 stop = $100 risk on $10k account = 1%
        // To trigger warning (> 1.5x config), we need > 1.5% risk
        // 0.2 BTC * $1000 = $200 risk on $10k = 2% > 1.5%
        order.size = dec!(0.2);

        let result = validator.validate(&order, &default_account());
        assert!(result
            .warnings
            .iter()
            .any(|w| matches!(w, RiskWarning::HighRisk { .. })));
    }

    #[test]
    fn test_warning_tight_stop() {
        let config = RiskConfig::new();
        let validator = RiskValidator::new(config);

        let mut order = default_order();
        // 0.2% stop
        order.stop_loss_price = Some(dec!(49900));

        let result = validator.validate(&order, &default_account());
        assert!(result
            .warnings
            .iter()
            .any(|w| matches!(w, RiskWarning::TightStop { .. })));
    }

    #[test]
    fn test_warning_wide_stop() {
        let config = RiskConfig::new();
        let validator = RiskValidator::new(config);

        let mut order = default_order();
        // 10% stop
        order.stop_loss_price = Some(dec!(45000));
        order.take_profit_price = Some(dec!(65000)); // Ensure good RR

        let result = validator.validate(&order, &default_account());
        assert!(result
            .warnings
            .iter()
            .any(|w| matches!(w, RiskWarning::WideStop { .. })));
    }
}
