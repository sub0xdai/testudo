//! Risk Service
//!
//! Orchestrates risk validation and position sizing for the Decision Loop.
//! This service combines validation checks with position sizing to produce
//! a unified RiskCheckResult.

// @anchor exchange:common_utils:service
// @tags infra

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use super::config::RiskConfig;
use super::sizing::calculate_position_size;
use super::types::{RiskCheckResult, RiskRejection, RiskWarning};

// Re-export from sizing for external use
pub use super::sizing::{MarketData, TradingStats};

/// Account state for risk validation
#[derive(Debug, Clone)]
pub struct AccountState {
    /// Current account balance in quote currency
    pub balance: Decimal,
    /// Number of currently open positions
    pub open_position_count: u32,
    /// Today's realized P&L
    pub daily_pnl: Decimal,
    /// Account balance at start of day
    pub starting_balance: Decimal,
}

impl AccountState {
    /// Calculate current daily drawdown percentage
    pub fn daily_drawdown_percent(&self) -> Decimal {
        if self.starting_balance <= dec!(0) {
            return dec!(0);
        }

        if self.daily_pnl >= dec!(0) {
            return dec!(0); // No drawdown if profitable
        }

        // Drawdown is the negative P&L as a percentage
        (self.daily_pnl.abs() / self.starting_balance) * dec!(100)
    }
}

/// Order request for validation
#[derive(Debug, Clone)]
pub struct OrderRequest {
    /// Trading symbol
    pub symbol: String,
    /// Order side (buy/sell or long/short)
    pub side: OrderSide,
    /// User-specified size (optional, will be calculated if not provided)
    pub user_size: Option<Decimal>,
    /// Entry price
    pub entry_price: Decimal,
    /// Stop loss price (required for most risk configs)
    pub stop_loss_price: Option<Decimal>,
    /// Take profit price
    pub take_profit_price: Option<Decimal>,
    /// Requested leverage
    pub leverage: u8,
}

/// Order side
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Long,
    Short,
}

/// Risk Service for validating orders and calculating position sizes
pub struct RiskService {
    config: RiskConfig,
}

impl RiskService {
    /// Create a new RiskService with the given configuration
    pub fn new(config: RiskConfig) -> Self {
        Self { config }
    }

    /// Validate an order and calculate position size
    ///
    /// This is the main entry point for the Decision Loop. It:
    /// 1. Validates the order against all risk rules
    /// 2. Calculates the optimal position size if validation passes
    /// 3. Returns a unified result with approval/rejection and any warnings
    ///
    /// # Arguments
    /// * `order` - The order request to validate
    /// * `account` - Current account state
    /// * `market_data` - Current market data (for volatility sizing)
    /// * `trading_stats` - Optional trading statistics (for Kelly sizing)
    pub fn validate(
        &self,
        order: &OrderRequest,
        account: &AccountState,
        market_data: Option<&MarketData>,
        trading_stats: Option<&TradingStats>,
    ) -> RiskCheckResult {
        let mut warnings = Vec::new();

        // === BLOCKING CHECKS (return rejection immediately) ===

        // 1. Check stop loss requirement
        if self.config.require_stop_loss && order.stop_loss_price.is_none() {
            return RiskCheckResult::rejected(RiskRejection::StopLossRequired);
        }

        // 2. Check leverage limit
        if order.leverage > self.config.max_leverage {
            return RiskCheckResult::rejected(RiskRejection::LeverageExceeded {
                requested: order.leverage,
                maximum: self.config.max_leverage,
            });
        }

        // 3. Check max open positions
        if let Some(max_positions) = self.config.max_open_positions {
            if account.open_position_count >= max_positions {
                return RiskCheckResult::rejected(RiskRejection::MaxPositionsReached {
                    current: account.open_position_count,
                    maximum: max_positions,
                });
            }
        }

        // 4. Check daily drawdown limit
        if let Some(max_drawdown) = self.config.daily_max_drawdown_percent {
            let current_drawdown = account.daily_drawdown_percent();
            if current_drawdown >= max_drawdown {
                return RiskCheckResult::rejected(RiskRejection::DailyDrawdownExceeded {
                    current_drawdown_percent: current_drawdown,
                    limit_percent: max_drawdown,
                });
            }

            // Warning if approaching limit (>80% of limit)
            if current_drawdown >= max_drawdown * dec!(0.8) {
                warnings.push(RiskWarning::ApproachingDrawdownLimit {
                    current_drawdown_percent: current_drawdown,
                    limit_percent: max_drawdown,
                });
            }
        }

        // 5. Check risk/reward ratio
        if let (Some(stop), Some(tp)) = (order.stop_loss_price, order.take_profit_price) {
            if let Some(min_rr) = self.config.min_risk_reward_ratio {
                let risk = (order.entry_price - stop).abs();
                let reward = (tp - order.entry_price).abs();

                if risk > dec!(0) {
                    let rr_ratio = reward / risk;
                    if rr_ratio < min_rr {
                        return RiskCheckResult::rejected(RiskRejection::InsufficientRiskReward {
                            calculated: rr_ratio,
                            minimum: min_rr,
                        });
                    }
                }
            }
        }

        // === POSITION SIZING ===

        // Build market data for sizing if not provided
        let sizing_market_data = market_data.cloned().unwrap_or(MarketData {
            current_price: order.entry_price,
            entry_price: order.entry_price,
            stop_loss_price: order.stop_loss_price,
            take_profit_price: order.take_profit_price,
            atr: None,
            typical_atr: None,
        });

        let sizing_result = calculate_position_size(
            &self.config,
            account.balance,
            &sizing_market_data,
            trading_stats,
        );

        let calculated_size = sizing_result.size;
        let sizing_method = sizing_result.method;

        // Determine final size (user override or calculated)
        let final_size = if let Some(user_size) = order.user_size {
            // User specified a size - check if it exceeds calculated
            if user_size > calculated_size && calculated_size > dec!(0) {
                warnings.push(RiskWarning::SizeOverrideExceedsCalculated {
                    user_size,
                    calculated_size,
                });
            }
            user_size
        } else {
            calculated_size
        };

        // === SIZE VALIDATION ===

        // 6. Check position size limit
        if let Some(max_size) = self.config.max_position_size {
            if final_size > max_size {
                return RiskCheckResult::rejected(RiskRejection::PositionSizeExceeded {
                    requested: final_size,
                    maximum: max_size,
                });
            }
        }

        // 7. Check risk amount limit
        if let Some(stop) = order.stop_loss_price {
            let stop_distance = (order.entry_price - stop).abs();
            let risk_amount = final_size * stop_distance;

            if let Some(max_risk) = self.config.max_risk_amount {
                if risk_amount > max_risk {
                    return RiskCheckResult::rejected(RiskRejection::RiskAmountExceeded {
                        calculated_risk: risk_amount,
                        maximum: max_risk,
                    });
                }
            }

            // Stop distance warnings
            let stop_percent = if order.entry_price > dec!(0) {
                (stop_distance / order.entry_price) * dec!(100)
            } else {
                dec!(0)
            };

            if stop_percent < dec!(0.5) {
                warnings.push(RiskWarning::TightStopLoss {
                    stop_distance_percent: stop_percent,
                });
            } else if stop_percent > dec!(5) {
                warnings.push(RiskWarning::WideStopLoss {
                    stop_distance_percent: stop_percent,
                });
            }
        }

        // 8. Check balance sufficiency
        let required_margin = final_size * order.entry_price / Decimal::from(order.leverage.max(1));
        if required_margin > account.balance {
            return RiskCheckResult::rejected(RiskRejection::InsufficientBalance {
                required: required_margin,
                available: account.balance,
            });
        }

        // === ADDITIONAL WARNINGS ===

        // Large position warning (>50% of account)
        if account.balance > dec!(0) {
            let position_value = final_size * order.entry_price;
            let position_percent = (position_value / account.balance) * dec!(100);
            if position_percent > dec!(50) {
                warnings.push(RiskWarning::LargePositionSize {
                    position_percent_of_account: position_percent,
                });
            }
        }

        // High volatility warning
        if let Some(md) = market_data {
            if let (Some(atr), Some(typical)) = (md.atr, md.typical_atr) {
                if typical > dec!(0) {
                    let atr_percent = (atr / md.current_price) * dec!(100);
                    let typical_percent = (typical / md.current_price) * dec!(100);

                    if atr > typical * dec!(1.5) {
                        warnings.push(RiskWarning::HighVolatilityDetected {
                            current_atr_percent: atr_percent,
                            typical_atr_percent: typical_percent,
                        });
                    }
                }
            }
        }

        // === APPROVED ===
        RiskCheckResult::approved(final_size, sizing_method).with_warnings(warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk::SizingMethod;

    fn default_account() -> AccountState {
        AccountState {
            balance: dec!(10000),
            open_position_count: 0,
            daily_pnl: dec!(0),
            starting_balance: dec!(10000),
        }
    }

    fn default_order() -> OrderRequest {
        OrderRequest {
            symbol: "BTC_USDC".to_string(),
            side: OrderSide::Long,
            user_size: None, // Let service calculate
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: Some(dec!(52000)),
            leverage: 1,
        }
    }

    // ==================== Sizing Tests ====================

    #[test]
    fn test_sizing_fixed_fractional() {
        // $10k account, 2% risk, entry 50k, stop 49k -> 0.2 BTC
        let config = RiskConfig::new().with_account_risk_percent(dec!(2));
        let service = RiskService::new(config);

        let result = service.validate(&default_order(), &default_account(), None, None);

        assert!(result.approved);
        assert_eq!(result.calculated_size, Some(dec!(0.2)));
        assert_eq!(
            result.sizing_method_used,
            Some(SizingMethod::FixedFractional)
        );
    }

    #[test]
    fn test_sizing_conservative_wins() {
        // Multiple sizing methods, smallest should win
        let config = RiskConfig::new()
            .with_account_risk_percent(dec!(2)) // 0.2 BTC
            .with_max_position_size(dec!(0.15)); // 0.15 BTC (smallest)

        let service = RiskService::new(config);

        let result = service.validate(&default_order(), &default_account(), None, None);

        assert!(result.approved);
        assert_eq!(result.calculated_size, Some(dec!(0.15)));
        assert_eq!(result.sizing_method_used, Some(SizingMethod::MaxRiskCap));
    }

    // ==================== Rejection Tests ====================

    #[test]
    fn test_reject_stop_loss_required() {
        let config = RiskConfig::new().with_require_stop_loss(true);
        let service = RiskService::new(config);

        let mut order = default_order();
        order.stop_loss_price = None;

        let result = service.validate(&order, &default_account(), None, None);

        assert!(!result.approved);
        assert!(matches!(
            result.rejection_reason,
            Some(RiskRejection::StopLossRequired)
        ));
    }

    #[test]
    fn test_reject_insufficient_balance() {
        let config = RiskConfig::new().with_require_stop_loss(false);
        let service = RiskService::new(config);

        let mut account = default_account();
        account.balance = dec!(1000); // Not enough for 0.1 BTC @ $50k

        let mut order = default_order();
        order.stop_loss_price = None;
        order.user_size = Some(dec!(0.1)); // 0.1 * 50000 = $5000 required

        let result = service.validate(&order, &account, None, None);

        assert!(!result.approved);
        assert!(matches!(
            result.rejection_reason,
            Some(RiskRejection::InsufficientBalance { .. })
        ));
    }

    #[test]
    fn test_reject_drawdown_exceeded() {
        let config = RiskConfig::new()
            .with_daily_max_drawdown(dec!(5))
            .with_require_stop_loss(false);
        let service = RiskService::new(config);

        let mut account = default_account();
        account.daily_pnl = dec!(-600); // 6% drawdown

        let result = service.validate(&default_order(), &account, None, None);

        assert!(!result.approved);
        assert!(matches!(
            result.rejection_reason,
            Some(RiskRejection::DailyDrawdownExceeded { .. })
        ));
    }

    #[test]
    fn test_reject_max_positions_reached() {
        let config = RiskConfig::new()
            .with_max_open_positions(3)
            .with_require_stop_loss(false);
        let service = RiskService::new(config);

        let mut account = default_account();
        account.open_position_count = 3;

        let result = service.validate(&default_order(), &account, None, None);

        assert!(!result.approved);
        assert!(matches!(
            result.rejection_reason,
            Some(RiskRejection::MaxPositionsReached {
                current: 3,
                maximum: 3
            })
        ));
    }

    #[test]
    fn test_reject_leverage_exceeded() {
        let config = RiskConfig::new()
            .with_max_leverage(5)
            .with_require_stop_loss(false);
        let service = RiskService::new(config);

        let mut order = default_order();
        order.leverage = 10;

        let result = service.validate(&order, &default_account(), None, None);

        assert!(!result.approved);
        assert!(matches!(
            result.rejection_reason,
            Some(RiskRejection::LeverageExceeded {
                requested: 10,
                maximum: 5
            })
        ));
    }

    #[test]
    fn test_default_config_allows_exchange_leverage() {
        // Default config should not block legitimate exchange leverage (up to 125x).
        // Per-pair limits are enforced by the exchange at execution time.
        let config = RiskConfig::new().with_require_stop_loss(false);
        let service = RiskService::new(config);

        for leverage in [10, 20, 50, 75, 100, 125] {
            let mut order = default_order();
            order.leverage = leverage;
            order.stop_loss_price = None;
            order.user_size = Some(dec!(0.001)); // small size to avoid balance rejection

            let result = service.validate(&order, &default_account(), None, None);

            assert!(
                result.approved,
                "Leverage {}x should be allowed by default config, got rejection: {:?}",
                leverage, result.rejection_reason
            );
        }
    }

    #[test]
    fn test_reject_insufficient_risk_reward() {
        let config = RiskConfig::new().with_min_risk_reward(dec!(2));
        let service = RiskService::new(config);

        let mut order = default_order();
        // Risk: $1000, Reward: $500 -> 0.5:1 ratio
        order.take_profit_price = Some(dec!(50500));

        let result = service.validate(&order, &default_account(), None, None);

        assert!(!result.approved);
        assert!(matches!(
            result.rejection_reason,
            Some(RiskRejection::InsufficientRiskReward { .. })
        ));
    }

    // ==================== Warning Tests ====================

    #[test]
    fn test_approve_with_warnings() {
        let config = RiskConfig::new()
            .with_account_risk_percent(dec!(2))
            .with_daily_max_drawdown(dec!(5));
        let service = RiskService::new(config);

        let mut account = default_account();
        // 4.5% drawdown - approaching limit (80% of 5%)
        account.daily_pnl = dec!(-450);

        let result = service.validate(&default_order(), &account, None, None);

        assert!(result.approved);
        assert!(result.has_warnings());
        assert!(result
            .warnings
            .iter()
            .any(|w| matches!(w, RiskWarning::ApproachingDrawdownLimit { .. })));
    }

    #[test]
    fn test_warning_size_override() {
        let config = RiskConfig::new()
            .with_account_risk_percent(dec!(1)) // 1% risk
            .with_max_leverage(5)
            .with_require_stop_loss(true);
        let service = RiskService::new(config);

        let account = default_account(); // $10,000

        let mut order = default_order();
        // With $10k balance, 1% risk = $100, stop = $1000 -> calculated = 0.1 BTC
        // User wants 0.15 BTC (larger than calculated, but within margin with 5x leverage)
        // Margin needed: 0.15 * 50000 / 5 = $1500 < $10000
        order.user_size = Some(dec!(0.15));
        order.leverage = 5;

        let result = service.validate(&order, &account, None, None);

        assert!(result.approved);
        assert!(result
            .warnings
            .iter()
            .any(|w| matches!(w, RiskWarning::SizeOverrideExceedsCalculated { .. })));
    }

    #[test]
    fn test_warning_tight_stop() {
        let config = RiskConfig::new()
            .with_account_risk_percent(dec!(2))
            .with_max_position_size(dec!(0.1)); // Cap size to avoid insufficient balance
        let service = RiskService::new(config);

        let mut order = default_order();
        // 0.2% stop distance
        order.stop_loss_price = Some(dec!(49900));

        let result = service.validate(&order, &default_account(), None, None);

        assert!(result.approved);
        assert!(result
            .warnings
            .iter()
            .any(|w| matches!(w, RiskWarning::TightStopLoss { .. })));
    }

    #[test]
    fn test_warning_wide_stop() {
        let config = RiskConfig::new()
            .with_account_risk_percent(dec!(2))
            .with_min_risk_reward(dec!(1)); // Lower RR to avoid that rejection
        let service = RiskService::new(config);

        let mut order = default_order();
        // 10% stop distance
        order.stop_loss_price = Some(dec!(45000));
        order.take_profit_price = Some(dec!(60000)); // Ensure good RR

        let result = service.validate(&order, &default_account(), None, None);

        assert!(result.approved);
        assert!(result
            .warnings
            .iter()
            .any(|w| matches!(w, RiskWarning::WideStopLoss { .. })));
    }

    #[test]
    fn test_warning_high_volatility() {
        let config = RiskConfig::new().with_account_risk_percent(dec!(2));
        let service = RiskService::new(config);

        let market_data = MarketData {
            current_price: dec!(50000),
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: Some(dec!(52000)),
            atr: Some(dec!(2000)),         // 4% ATR
            typical_atr: Some(dec!(1000)), // 2% typical - current is 2x
        };

        let result = service.validate(
            &default_order(),
            &default_account(),
            Some(&market_data),
            None,
        );

        assert!(result.approved);
        assert!(result
            .warnings
            .iter()
            .any(|w| matches!(w, RiskWarning::HighVolatilityDetected { .. })));
    }

    // ==================== Account State Tests ====================

    #[test]
    fn test_daily_drawdown_calculation() {
        let account = AccountState {
            balance: dec!(9500),
            open_position_count: 0,
            daily_pnl: dec!(-500),
            starting_balance: dec!(10000),
        };

        // Drawdown = 500 / 10000 * 100 = 5%
        assert_eq!(account.daily_drawdown_percent(), dec!(5));
    }

    #[test]
    fn test_daily_drawdown_positive_pnl() {
        let account = AccountState {
            balance: dec!(10500),
            open_position_count: 0,
            daily_pnl: dec!(500), // Profitable
            starting_balance: dec!(10000),
        };

        // No drawdown when profitable
        assert_eq!(account.daily_drawdown_percent(), dec!(0));
    }
}
