//! Risk Configuration
//!
//! User-defined risk parameters that control position sizing
//! and trade validation.

// @anchor exchange:common_utils:config
// @tags infra

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Risk configuration for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    /// User ID this config belongs to
    pub user_id: Option<Uuid>,

    /// Maximum risk per trade as percentage of account (e.g., 2.0 = 2%)
    pub account_risk_percent: Decimal,

    /// Maximum dollar amount to risk per trade
    pub max_risk_amount: Option<Decimal>,

    /// Maximum position size in base currency
    pub max_position_size: Option<Decimal>,

    /// Maximum leverage allowed (1 = no leverage)
    pub max_leverage: u8,

    /// Daily maximum drawdown percentage before blocking new trades
    pub daily_max_drawdown_percent: Option<Decimal>,

    /// Maximum number of concurrent open positions
    pub max_open_positions: Option<u32>,

    /// Require stop-loss on all trades
    pub require_stop_loss: bool,

    /// Default stop-loss as ATR multiplier (e.g., 2.0 = 2x ATR)
    pub default_stop_atr_multiplier: Option<Decimal>,

    /// Minimum risk/reward ratio required
    pub min_risk_reward_ratio: Option<Decimal>,

    /// QNT-01a: When true, the trade-management handler routes through the
    /// Calibrated Kelly path (per-setup edge → effective_risk_percent override)
    /// instead of using `account_risk_percent` as-is. Populated per-trade from
    /// the user_settings JSONB blob; preset constructors leave it false so that
    /// fixed-mode behavior is byte-for-byte identical to pre-spec.
    #[serde(default)]
    pub dynamic_risk_enabled: bool,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            user_id: None,
            account_risk_percent: dec!(2), // 2% per trade default
            max_risk_amount: None,
            max_position_size: None,
            max_leverage: 125, // Exchange max — per-pair limits enforced at execution
            daily_max_drawdown_percent: Some(dec!(5)), // 5% daily max drawdown
            max_open_positions: Some(5),
            require_stop_loss: true,
            default_stop_atr_multiplier: Some(dec!(2)),
            min_risk_reward_ratio: Some(dec!(1.5)),
            dynamic_risk_enabled: false,
        }
    }
}

impl RiskConfig {
    /// Create a new RiskConfig with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a conservative risk config
    pub fn conservative() -> Self {
        Self {
            account_risk_percent: dec!(1),
            max_risk_amount: Some(dec!(50)),
            max_leverage: 1,
            daily_max_drawdown_percent: Some(dec!(3)),
            max_open_positions: Some(3),
            require_stop_loss: true,
            default_stop_atr_multiplier: Some(dec!(2.5)),
            min_risk_reward_ratio: Some(dec!(2)),
            ..Default::default()
        }
    }

    /// Create an aggressive risk config
    pub fn aggressive() -> Self {
        Self {
            account_risk_percent: dec!(5),
            max_risk_amount: None,
            max_leverage: 10,
            daily_max_drawdown_percent: Some(dec!(10)),
            max_open_positions: Some(10),
            require_stop_loss: false,
            default_stop_atr_multiplier: Some(dec!(1.5)),
            min_risk_reward_ratio: Some(dec!(1)),
            ..Default::default()
        }
    }

    /// Builder methods
    pub fn with_user_id(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_account_risk_percent(mut self, percent: Decimal) -> Self {
        self.account_risk_percent = percent;
        self
    }

    pub fn with_max_risk_amount(mut self, amount: Decimal) -> Self {
        self.max_risk_amount = Some(amount);
        self
    }

    pub fn with_max_position_size(mut self, size: Decimal) -> Self {
        self.max_position_size = Some(size);
        self
    }

    pub fn with_max_leverage(mut self, leverage: u8) -> Self {
        self.max_leverage = leverage;
        self
    }

    pub fn with_daily_max_drawdown(mut self, percent: Decimal) -> Self {
        self.daily_max_drawdown_percent = Some(percent);
        self
    }

    pub fn with_max_open_positions(mut self, count: u32) -> Self {
        self.max_open_positions = Some(count);
        self
    }

    pub fn with_require_stop_loss(mut self, required: bool) -> Self {
        self.require_stop_loss = required;
        self
    }

    pub fn with_min_risk_reward(mut self, ratio: Decimal) -> Self {
        self.min_risk_reward_ratio = Some(ratio);
        self
    }

    /// QNT-01a: enable the Calibrated Kelly sizing path on a per-trade override.
    pub fn with_dynamic_risk(mut self, enabled: bool) -> Self {
        self.dynamic_risk_enabled = enabled;
        self
    }

    /// Validate the configuration
    /// Returns true if this config matches the default settings.
    /// Used by onboarding status to detect whether user has customized risk.
    /// Excludes user_id from comparison (it's assigned at load time, not a setting).
    pub fn is_default(&self) -> bool {
        let default = Self::default();
        self.account_risk_percent == default.account_risk_percent
            && self.max_risk_amount == default.max_risk_amount
            && self.max_position_size == default.max_position_size
            && self.max_leverage == default.max_leverage
            && self.daily_max_drawdown_percent == default.daily_max_drawdown_percent
            && self.max_open_positions == default.max_open_positions
            && self.require_stop_loss == default.require_stop_loss
            && self.default_stop_atr_multiplier == default.default_stop_atr_multiplier
            && self.min_risk_reward_ratio == default.min_risk_reward_ratio
            && self.dynamic_risk_enabled == default.dynamic_risk_enabled
    }

    pub fn validate(&self) -> Result<(), RiskConfigError> {
        if self.account_risk_percent <= dec!(0) {
            return Err(RiskConfigError::InvalidRiskPercent(
                "Account risk percent must be positive".to_string(),
            ));
        }

        if self.account_risk_percent > dec!(100) {
            return Err(RiskConfigError::InvalidRiskPercent(
                "Account risk percent cannot exceed 100%".to_string(),
            ));
        }

        if let Some(max_risk) = self.max_risk_amount {
            if max_risk <= dec!(0) {
                return Err(RiskConfigError::InvalidRiskAmount(
                    "Max risk amount must be positive".to_string(),
                ));
            }
        }

        if let Some(max_size) = self.max_position_size {
            if max_size <= dec!(0) {
                return Err(RiskConfigError::InvalidPositionSize(
                    "Max position size must be positive".to_string(),
                ));
            }
        }

        if self.max_leverage == 0 {
            return Err(RiskConfigError::InvalidLeverage(
                "Leverage must be at least 1".to_string(),
            ));
        }

        if self.max_leverage > 125 {
            return Err(RiskConfigError::InvalidLeverage(
                "Leverage cannot exceed 125x".to_string(),
            ));
        }

        Ok(())
    }
}

/// Errors in risk configuration
#[derive(Debug, thiserror::Error)]
pub enum RiskConfigError {
    #[error("Invalid risk percent: {0}")]
    InvalidRiskPercent(String),

    #[error("Invalid risk amount: {0}")]
    InvalidRiskAmount(String),

    #[error("Invalid position size: {0}")]
    InvalidPositionSize(String),

    #[error("Invalid leverage: {0}")]
    InvalidLeverage(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RiskConfig::default();
        assert_eq!(config.account_risk_percent, dec!(2));
        assert_eq!(config.max_leverage, 125);
        assert!(config.require_stop_loss);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_conservative_config() {
        let config = RiskConfig::conservative();
        assert_eq!(config.account_risk_percent, dec!(1));
        assert_eq!(config.max_risk_amount, Some(dec!(50)));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_aggressive_config() {
        let config = RiskConfig::aggressive();
        assert_eq!(config.account_risk_percent, dec!(5));
        assert_eq!(config.max_leverage, 10);
        assert!(!config.require_stop_loss);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_builder_pattern() {
        let user_id = Uuid::new_v4();
        let config = RiskConfig::new()
            .with_user_id(user_id)
            .with_account_risk_percent(dec!(3))
            .with_max_risk_amount(dec!(200))
            .with_max_position_size(dec!(0.5));

        assert_eq!(config.user_id, Some(user_id));
        assert_eq!(config.account_risk_percent, dec!(3));
        assert_eq!(config.max_risk_amount, Some(dec!(200)));
        assert_eq!(config.max_position_size, Some(dec!(0.5)));
    }

    #[test]
    fn test_validation_invalid_risk_percent() {
        let config = RiskConfig::new().with_account_risk_percent(dec!(0));
        assert!(config.validate().is_err());

        let config = RiskConfig::new().with_account_risk_percent(dec!(-1));
        assert!(config.validate().is_err());

        let config = RiskConfig::new().with_account_risk_percent(dec!(101));
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_invalid_max_risk() {
        let config = RiskConfig::new().with_max_risk_amount(dec!(0));
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_leverage_bounds() {
        let config = RiskConfig::new().with_max_leverage(0);
        assert!(config.validate().is_err());

        let config = RiskConfig::new().with_max_leverage(126);
        assert!(config.validate().is_err());

        let config = RiskConfig::new().with_max_leverage(125);
        assert!(config.validate().is_ok());

        let config = RiskConfig::new().with_max_leverage(1);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_is_default_returns_true_for_default() {
        let config = RiskConfig::default();
        assert!(config.is_default());
    }

    #[test]
    fn test_is_default_returns_true_with_user_id_set() {
        let config = RiskConfig {
            user_id: Some(Uuid::new_v4()),
            ..Default::default()
        };
        assert!(config.is_default());
    }

    #[test]
    fn test_is_default_returns_false_when_customized() {
        let config = RiskConfig {
            account_risk_percent: dec!(3),
            ..Default::default()
        };
        assert!(!config.is_default());
    }

    #[test]
    fn test_is_default_returns_false_when_leverage_changed() {
        let config = RiskConfig {
            max_leverage: 10,
            ..Default::default()
        };
        assert!(!config.is_default());
    }

    #[test]
    fn test_is_default_returns_false_when_stop_loss_disabled() {
        let config = RiskConfig {
            require_stop_loss: false,
            ..Default::default()
        };
        assert!(!config.is_default());
    }
}
