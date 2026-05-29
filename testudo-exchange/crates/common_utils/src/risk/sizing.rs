//! Position Sizing Methods
//!
//! Implements various position sizing algorithms for the Decision Loop.
//! The core principle is "Conservative Wins": always use the smallest
//! position size among all calculated methods.

// @anchor exchange:common_utils:sizing
// @tags infra

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use super::config::RiskConfig;
use super::types::SizingMethod;

/// Market data required for position sizing
#[derive(Debug, Clone)]
pub struct MarketData {
    /// Current price of the asset
    pub current_price: Decimal,
    /// Entry price for the trade
    pub entry_price: Decimal,
    /// Stop loss price
    pub stop_loss_price: Option<Decimal>,
    /// Take profit price
    pub take_profit_price: Option<Decimal>,
    /// Average True Range (14-period typical)
    pub atr: Option<Decimal>,
    /// Typical/baseline ATR for volatility comparison
    pub typical_atr: Option<Decimal>,
}

/// Trading statistics for Kelly Criterion
#[derive(Debug, Clone)]
pub struct TradingStats {
    /// Win rate (0.0 to 1.0)
    pub win_rate: Decimal,
    /// Average winning trade return
    pub avg_win: Decimal,
    /// Average losing trade return (positive number)
    pub avg_loss: Decimal,
}

/// Result of position sizing calculation
#[derive(Debug, Clone)]
pub struct SizingResult {
    /// The calculated position size
    pub size: Decimal,
    /// Which method determined the size
    pub method: SizingMethod,
    /// All calculated sizes for transparency
    pub all_sizes: AllSizingCalculations,
}

/// All individual sizing calculations
#[derive(Debug, Clone)]
pub struct AllSizingCalculations {
    pub fixed_fractional: Option<Decimal>,
    pub kelly_criterion: Option<Decimal>,
    pub volatility_adjusted: Option<Decimal>,
    pub max_position_cap: Option<Decimal>,
}

/// Calculate position size using Fixed Fractional method
///
/// Formula: size = (balance * risk_percent / 100) / stop_distance
///
/// # Arguments
/// * `balance` - Account balance in quote currency
/// * `risk_percent` - Percentage of account to risk (e.g., 2.0 for 2%)
/// * `entry_price` - Entry price
/// * `stop_price` - Stop loss price
///
/// # Returns
/// Position size in base currency
///
/// # Example
/// ```
/// use rust_decimal_macros::dec;
/// use common_utils::risk::sizing::fixed_fractional;
///
/// // $10k account, 2% risk, entry $50k, stop $49k
/// let size = fixed_fractional(dec!(10000), dec!(2), dec!(50000), dec!(49000));
/// assert_eq!(size, dec!(0.2)); // 0.2 BTC
/// ```
pub fn fixed_fractional(
    balance: Decimal,
    risk_percent: Decimal,
    entry_price: Decimal,
    stop_price: Decimal,
) -> Decimal {
    let stop_distance = (entry_price - stop_price).abs();

    if stop_distance <= dec!(0) {
        return dec!(0);
    }

    let risk_amount = balance * risk_percent / dec!(100);
    risk_amount / stop_distance
}

/// Calculate optimal position size using Kelly Criterion
///
/// Formula: kelly = win_rate - (1 - win_rate) / (avg_win / avg_loss)
///
/// The Kelly Criterion determines the optimal fraction of capital to bet
/// to maximize long-term growth rate.
///
/// # Arguments
/// * `win_rate` - Probability of winning (0.0 to 1.0)
/// * `avg_win` - Average return on winning trades
/// * `avg_loss` - Average loss on losing trades (positive number)
///
/// # Returns
/// Optimal fraction of capital to risk (0.0 to 1.0)
/// Returns 0 if expected value is negative
///
/// # Example
/// ```
/// use rust_decimal_macros::dec;
/// use common_utils::risk::sizing::kelly_criterion;
///
/// // 55% win rate, average win 1.5x, average loss 1x
/// let kelly = kelly_criterion(dec!(0.55), dec!(1.5), dec!(1.0));
/// // kelly = 0.55 - (0.45 / 1.5) = 0.55 - 0.3 = 0.25 (25%)
/// ```
pub fn kelly_criterion(win_rate: Decimal, avg_win: Decimal, avg_loss: Decimal) -> Decimal {
    if avg_loss <= dec!(0) || avg_win <= dec!(0) {
        return dec!(0);
    }

    // Clamp win rate to valid range
    let win_rate = win_rate.max(dec!(0)).min(dec!(1));
    let loss_rate = dec!(1) - win_rate;

    // Kelly = W - (L / R) where W = win rate, L = loss rate, R = win/loss ratio
    let win_loss_ratio = avg_win / avg_loss;
    let kelly = win_rate - (loss_rate / win_loss_ratio);

    // Never return negative (would mean don't trade)
    kelly.max(dec!(0))
}

/// Adjust position size based on current volatility vs target volatility
///
/// Formula: adjusted_size = base_size * (target_vol / current_vol)
///
/// This scales position size inversely with volatility:
/// - Higher volatility = smaller position
/// - Lower volatility = larger position
///
/// # Arguments
/// * `base_size` - Base position size (e.g., from fixed fractional)
/// * `target_vol` - Target volatility (e.g., typical ATR)
/// * `current_atr` - Current ATR (Average True Range)
///
/// # Returns
/// Volatility-adjusted position size
///
/// # Example
/// ```
/// use rust_decimal_macros::dec;
/// use common_utils::risk::sizing::volatility_adjusted;
///
/// // Base size 0.2 BTC, target vol $500, current ATR $1000 (high vol)
/// let size = volatility_adjusted(dec!(0.2), dec!(500), dec!(1000));
/// assert_eq!(size, dec!(0.1)); // Halved due to 2x volatility
/// ```
pub fn volatility_adjusted(
    base_size: Decimal,
    target_vol: Decimal,
    current_atr: Decimal,
) -> Decimal {
    if current_atr <= dec!(0) || target_vol <= dec!(0) {
        return base_size;
    }

    let adjustment_ratio = target_vol / current_atr;

    // Apply adjustment but cap at 2x to prevent excessive sizing in low vol
    let capped_ratio = adjustment_ratio.min(dec!(2));

    base_size * capped_ratio
}

/// Calculate position size using "Conservative Wins" principle
///
/// Calculates size using all available methods and returns the smallest.
///
/// # Arguments
/// * `config` - User's risk configuration
/// * `balance` - Account balance
/// * `market_data` - Current market data
/// * `trading_stats` - Optional trading statistics for Kelly
///
/// # Returns
/// SizingResult with the smallest calculated size and method used
pub fn calculate_position_size(
    config: &RiskConfig,
    balance: Decimal,
    market_data: &MarketData,
    trading_stats: Option<&TradingStats>,
) -> SizingResult {
    let mut all_sizes = AllSizingCalculations {
        fixed_fractional: None,
        kelly_criterion: None,
        volatility_adjusted: None,
        max_position_cap: config.max_position_size,
    };

    let mut min_size = Decimal::MAX;
    let mut winning_method = SizingMethod::FixedFractional;

    // 1. Fixed Fractional (requires stop loss)
    if let Some(stop_price) = market_data.stop_loss_price {
        let ff_size = fixed_fractional(
            balance,
            config.account_risk_percent,
            market_data.entry_price,
            stop_price,
        );
        all_sizes.fixed_fractional = Some(ff_size);

        if ff_size > dec!(0) && ff_size < min_size {
            min_size = ff_size;
            winning_method = SizingMethod::FixedFractional;
        }
    }

    // 2. Kelly Criterion (requires trading stats)
    if let Some(stats) = trading_stats {
        let kelly_fraction = kelly_criterion(stats.win_rate, stats.avg_win, stats.avg_loss);

        // Convert Kelly fraction to position size
        // Kelly gives fraction of capital, we need to convert to position units
        if kelly_fraction > dec!(0) && market_data.entry_price > dec!(0) {
            // Apply half-Kelly for safety (common practice)
            let half_kelly = kelly_fraction / dec!(2);
            let kelly_capital = balance * half_kelly;
            let kelly_size = kelly_capital / market_data.entry_price;

            all_sizes.kelly_criterion = Some(kelly_size);

            if kelly_size > dec!(0) && kelly_size < min_size {
                min_size = kelly_size;
                winning_method = SizingMethod::KellyCriterion;
            }
        }
    }

    // 3. Volatility Adjusted (requires ATR data)
    if let (Some(atr), Some(typical_atr)) = (market_data.atr, market_data.typical_atr) {
        // Start with fixed fractional size as base
        if let Some(base_size) = all_sizes.fixed_fractional {
            let vol_size = volatility_adjusted(base_size, typical_atr, atr);
            all_sizes.volatility_adjusted = Some(vol_size);

            if vol_size > dec!(0) && vol_size < min_size {
                min_size = vol_size;
                winning_method = SizingMethod::VolatilityAdjusted;
            }
        }
    }

    // 4. Max Position Cap (direct limit)
    if let Some(max_size) = config.max_position_size {
        if max_size < min_size {
            min_size = max_size;
            winning_method = SizingMethod::MaxRiskCap;
        }
    }

    // Handle case where no valid size was calculated
    if min_size == Decimal::MAX {
        min_size = dec!(0);
    }

    SizingResult {
        size: min_size,
        method: winning_method,
        all_sizes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Fixed Fractional Tests ====================

    #[test]
    fn test_fixed_fractional_basic() {
        // $10k account, 2% risk, entry $50k, stop $49k
        // Risk: 2% of $10,000 = $200
        // Stop distance: $1,000
        // Size: $200 / $1,000 = 0.2 BTC
        let size = fixed_fractional(dec!(10000), dec!(2), dec!(50000), dec!(49000));
        assert_eq!(size, dec!(0.2));
    }

    #[test]
    fn test_fixed_fractional_different_risk() {
        // $10k account, 1% risk, entry $50k, stop $49k
        // Risk: 1% of $10,000 = $100
        // Size: $100 / $1,000 = 0.1 BTC
        let size = fixed_fractional(dec!(10000), dec!(1), dec!(50000), dec!(49000));
        assert_eq!(size, dec!(0.1));
    }

    #[test]
    fn test_fixed_fractional_wider_stop() {
        // $10k account, 2% risk, entry $50k, stop $48k (4% stop)
        // Risk: $200
        // Stop distance: $2,000
        // Size: $200 / $2,000 = 0.1 BTC
        let size = fixed_fractional(dec!(10000), dec!(2), dec!(50000), dec!(48000));
        assert_eq!(size, dec!(0.1));
    }

    #[test]
    fn test_fixed_fractional_zero_stop_distance() {
        // Entry equals stop - should return 0
        let size = fixed_fractional(dec!(10000), dec!(2), dec!(50000), dec!(50000));
        assert_eq!(size, dec!(0));
    }

    #[test]
    fn test_fixed_fractional_short_position() {
        // Short position: stop above entry
        // $10k account, 2% risk, entry $50k, stop $51k
        let size = fixed_fractional(dec!(10000), dec!(2), dec!(50000), dec!(51000));
        assert_eq!(size, dec!(0.2));
    }

    // ==================== Kelly Criterion Tests ====================

    #[test]
    fn test_kelly_criterion_positive_edge() {
        // 55% win rate, 1.5:1 avg win/loss
        // Kelly = 0.55 - (0.45 / 1.5) = 0.55 - 0.30 = 0.25
        let kelly = kelly_criterion(dec!(0.55), dec!(1.5), dec!(1.0));
        assert_eq!(kelly, dec!(0.25));
    }

    #[test]
    fn test_kelly_criterion_break_even() {
        // 50% win rate, 1:1 avg win/loss
        // Kelly = 0.5 - (0.5 / 1.0) = 0
        let kelly = kelly_criterion(dec!(0.5), dec!(1.0), dec!(1.0));
        assert_eq!(kelly, dec!(0));
    }

    #[test]
    fn test_kelly_criterion_negative_edge() {
        // 40% win rate, 1:1 avg win/loss -> negative expectancy
        // Kelly = 0.4 - (0.6 / 1.0) = -0.2 -> clamped to 0
        let kelly = kelly_criterion(dec!(0.4), dec!(1.0), dec!(1.0));
        assert_eq!(kelly, dec!(0));
    }

    #[test]
    fn test_kelly_criterion_high_win_rate() {
        // 70% win rate, 2:1 avg win/loss
        // Kelly = 0.7 - (0.3 / 2.0) = 0.7 - 0.15 = 0.55
        let kelly = kelly_criterion(dec!(0.7), dec!(2.0), dec!(1.0));
        assert_eq!(kelly, dec!(0.55));
    }

    #[test]
    fn test_kelly_criterion_invalid_inputs() {
        // Zero avg loss
        assert_eq!(kelly_criterion(dec!(0.5), dec!(1.0), dec!(0)), dec!(0));

        // Zero avg win
        assert_eq!(kelly_criterion(dec!(0.5), dec!(0), dec!(1.0)), dec!(0));

        // Negative values treated as zero
        assert_eq!(kelly_criterion(dec!(0.5), dec!(-1.0), dec!(1.0)), dec!(0));
    }

    // ==================== Volatility Adjusted Tests ====================

    #[test]
    fn test_volatility_adjusted_high_vol() {
        // Base size 0.2 BTC, target vol $500, current ATR $1000 (2x volatility)
        // Adjustment: 0.2 * (500/1000) = 0.1 BTC
        let size = volatility_adjusted(dec!(0.2), dec!(500), dec!(1000));
        assert_eq!(size, dec!(0.1));
    }

    #[test]
    fn test_volatility_adjusted_low_vol() {
        // Base size 0.2 BTC, target vol $1000, current ATR $500 (0.5x volatility)
        // Ratio would be 2.0, but capped at 2.0
        // Adjustment: 0.2 * 2.0 = 0.4 BTC
        let size = volatility_adjusted(dec!(0.2), dec!(1000), dec!(500));
        assert_eq!(size, dec!(0.4));
    }

    #[test]
    fn test_volatility_adjusted_very_low_vol() {
        // Base size 0.2 BTC, target vol $1000, current ATR $100 (10x lower)
        // Ratio would be 10.0, but capped at 2.0
        let size = volatility_adjusted(dec!(0.2), dec!(1000), dec!(100));
        assert_eq!(size, dec!(0.4)); // Capped at 2x
    }

    #[test]
    fn test_volatility_adjusted_normal_vol() {
        // Base size 0.2 BTC, target vol $500, current ATR $500 (same)
        let size = volatility_adjusted(dec!(0.2), dec!(500), dec!(500));
        assert_eq!(size, dec!(0.2));
    }

    #[test]
    fn test_volatility_adjusted_zero_atr() {
        // Zero ATR returns base size
        let size = volatility_adjusted(dec!(0.2), dec!(500), dec!(0));
        assert_eq!(size, dec!(0.2));
    }

    // ==================== Conservative Wins Tests ====================

    #[test]
    fn test_calculate_position_size_conservative_wins() {
        let config = RiskConfig::new()
            .with_account_risk_percent(dec!(2))
            .with_max_position_size(dec!(0.15)); // This is the smallest

        let market_data = MarketData {
            current_price: dec!(50000),
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: Some(dec!(52000)),
            atr: None,
            typical_atr: None,
        };

        let result = calculate_position_size(&config, dec!(10000), &market_data, None);

        // Fixed fractional would give 0.2 BTC
        // Max position cap is 0.15 BTC (smallest)
        assert_eq!(result.size, dec!(0.15));
        assert_eq!(result.method, SizingMethod::MaxRiskCap);
    }

    #[test]
    fn test_calculate_position_size_with_kelly() {
        let config = RiskConfig::new().with_account_risk_percent(dec!(2));

        let market_data = MarketData {
            current_price: dec!(50000),
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: Some(dec!(52000)),
            atr: None,
            typical_atr: None,
        };

        let stats = TradingStats {
            win_rate: dec!(0.55),
            avg_win: dec!(1.5),
            avg_loss: dec!(1.0),
        };

        let result = calculate_position_size(&config, dec!(10000), &market_data, Some(&stats));

        // Fixed fractional: 0.2 BTC
        // Kelly: 25% * 0.5 (half-kelly) = 12.5% of $10k = $1250 / $50k = 0.025 BTC
        // Kelly should win (smallest)
        assert!(result.all_sizes.fixed_fractional.is_some());
        assert!(result.all_sizes.kelly_criterion.is_some());
        assert_eq!(result.size, dec!(0.025));
        assert_eq!(result.method, SizingMethod::KellyCriterion);
    }

    #[test]
    fn test_calculate_position_size_with_volatility() {
        let config = RiskConfig::new().with_account_risk_percent(dec!(2));

        let market_data = MarketData {
            current_price: dec!(50000),
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: Some(dec!(52000)),
            atr: Some(dec!(2000)), // 2x typical volatility
            typical_atr: Some(dec!(1000)),
        };

        let result = calculate_position_size(&config, dec!(10000), &market_data, None);

        // Fixed fractional: 0.2 BTC
        // Volatility adjusted: 0.2 * (1000/2000) = 0.1 BTC (high vol = smaller size)
        assert_eq!(result.size, dec!(0.1));
        assert_eq!(result.method, SizingMethod::VolatilityAdjusted);
    }

    #[test]
    fn test_calculate_position_size_no_stop_loss() {
        let config = RiskConfig::new()
            .with_account_risk_percent(dec!(2))
            .with_max_position_size(dec!(0.5));

        let market_data = MarketData {
            current_price: dec!(50000),
            entry_price: dec!(50000),
            stop_loss_price: None, // No stop loss
            take_profit_price: None,
            atr: None,
            typical_atr: None,
        };

        let result = calculate_position_size(&config, dec!(10000), &market_data, None);

        // Only max position cap applies
        assert_eq!(result.size, dec!(0.5));
        assert_eq!(result.method, SizingMethod::MaxRiskCap);
        assert!(result.all_sizes.fixed_fractional.is_none());
    }

    #[test]
    fn test_all_sizes_transparency() {
        let config = RiskConfig::new()
            .with_account_risk_percent(dec!(2))
            .with_max_position_size(dec!(1.0));

        let market_data = MarketData {
            current_price: dec!(50000),
            entry_price: dec!(50000),
            stop_loss_price: Some(dec!(49000)),
            take_profit_price: Some(dec!(52000)),
            atr: Some(dec!(1000)),
            typical_atr: Some(dec!(1000)),
        };

        let stats = TradingStats {
            win_rate: dec!(0.6),
            avg_win: dec!(2.0),
            avg_loss: dec!(1.0),
        };

        let result = calculate_position_size(&config, dec!(10000), &market_data, Some(&stats));

        // All methods should have calculated values
        assert!(result.all_sizes.fixed_fractional.is_some());
        assert!(result.all_sizes.kelly_criterion.is_some());
        assert!(result.all_sizes.volatility_adjusted.is_some());
        assert!(result.all_sizes.max_position_cap.is_some());
    }
}
