//! Testudo Protocol limits and constraints
//!
//! This module defines the immutable risk limits that form the core of the
//! Testudo Protocol. These limits protect traders from catastrophic losses
//! and emotional trading decisions.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

/// Core Testudo Protocol limits (IMMUTABLE)
///
/// These limits are designed to prevent account blowups and enforce disciplined
/// risk management. They are based on professional trading standards and
/// psychological research on trader behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProtocolLimits {
    /// Maximum risk per individual trade (default: 6% of account equity)
    /// This prevents any single trade from causing catastrophic damage
    pub max_individual_trade_risk: Decimal,
    
    /// Minimum risk per individual trade (default: 0.5% of account equity)
    /// This ensures trades are meaningful and not overly conservative
    pub min_individual_trade_risk: Decimal,
    
    /// Maximum total portfolio risk across all open positions (default: 10%)
    /// This prevents overexposure from multiple correlated positions
    pub max_total_portfolio_risk: Decimal,
    
    /// Maximum number of consecutive losing trades before circuit breaker (default: 3)
    /// This protects against emotional revenge trading and system failures
    pub max_consecutive_losses: u32,
    
    /// Minimum reward-to-risk ratio for trades (default: 2.0)
    /// This ensures trades have positive expected value over time
    pub min_reward_risk_ratio: Decimal,
    
    /// Maximum number of open positions allowed simultaneously (default: 5)
    /// This prevents over-diversification and unmanageable portfolio complexity
    pub max_open_positions: u32,
    
    /// Maximum daily loss limit as percentage of account (default: 5%)
    /// This provides daily circuit breaker protection
    pub max_daily_loss: Decimal,
    
    /// Maximum drawdown before trading halt (default: 10%)
    /// This prevents deep portfolio drawdowns
    pub max_drawdown: Decimal,
}

impl ProtocolLimits {
    /// Create the standard Testudo Protocol limits
    /// 
    /// These limits are based on Van Tharp's research and professional
    /// risk management standards. They should not be modified without
    /// careful consideration of the psychological and mathematical implications.
    pub const fn default_limits() -> ProtocolLimits {
        ProtocolLimits {
            max_individual_trade_risk: dec!(0.06),    // 6%
            min_individual_trade_risk: dec!(0.005),   // 0.5%
            max_total_portfolio_risk: dec!(0.10),     // 10%
            max_consecutive_losses: 3,
            min_reward_risk_ratio: dec!(2.0),         // 2:1 minimum
            max_open_positions: 5,
            max_daily_loss: dec!(0.05),               // 5%
            max_drawdown: dec!(0.10),                 // 10%
        }
    }
    
    /// Create conservative protocol limits for new traders
    /// 
    /// These limits provide extra protection for inexperienced traders
    /// who are still learning risk management principles.
    pub const fn conservative_limits() -> ProtocolLimits {
        ProtocolLimits {
            max_individual_trade_risk: dec!(0.02),    // 2% (reduced from 6%)
            min_individual_trade_risk: dec!(0.005),   // 0.5%
            max_total_portfolio_risk: dec!(0.05),     // 5% (reduced from 10%)
            max_consecutive_losses: 2,                // Lower tolerance
            min_reward_risk_ratio: dec!(3.0),         // Higher requirement
            max_open_positions: 3,                    // Fewer positions
            max_daily_loss: dec!(0.02),               // 2% (reduced from 5%)
            max_drawdown: dec!(0.05),                 // 5% (reduced from 10%)
        }
    }
    
    /// Create aggressive protocol limits for experienced traders
    /// 
    /// These limits allow for higher risk but still maintain protection
    /// against catastrophic losses. Only recommended for experienced traders
    /// with proven track records.
    pub const fn aggressive_limits() -> ProtocolLimits {
        ProtocolLimits {
            max_individual_trade_risk: dec!(0.10),    // 10% (increased from 6%)
            min_individual_trade_risk: dec!(0.01),    // 1%
            max_total_portfolio_risk: dec!(0.15),     // 15% (increased from 10%)
            max_consecutive_losses: 5,                // Higher tolerance
            min_reward_risk_ratio: dec!(1.5),         // Lower requirement
            max_open_positions: 8,                    // More positions allowed
            max_daily_loss: dec!(0.08),               // 8% (increased from 5%)
            max_drawdown: dec!(0.15),                 // 15% (increased from 10%)
        }
    }
    
    /// Validate that a risk percentage complies with individual trade limits
    pub fn validate_individual_trade_risk(&self, risk_percentage: Decimal) -> Result<(), ProtocolLimitViolation> {
        if risk_percentage > self.max_individual_trade_risk {
            return Err(ProtocolLimitViolation::ExceedsMaxIndividualRisk {
                current: risk_percentage,
                limit: self.max_individual_trade_risk,
            });
        }
        
        if risk_percentage < self.min_individual_trade_risk {
            return Err(ProtocolLimitViolation::BelowMinIndividualRisk {
                current: risk_percentage,
                limit: self.min_individual_trade_risk,
            });
        }
        
        Ok(())
    }
    
    /// Validate that portfolio risk complies with total portfolio limits
    pub fn validate_portfolio_risk(&self, total_risk: Decimal) -> Result<(), ProtocolLimitViolation> {
        if total_risk > self.max_total_portfolio_risk {
            return Err(ProtocolLimitViolation::ExceedsMaxPortfolioRisk {
                current: total_risk,
                limit: self.max_total_portfolio_risk,
            });
        }
        
        Ok(())
    }
    
    /// Validate that consecutive losses don't exceed circuit breaker threshold
    pub fn validate_consecutive_losses(&self, consecutive_losses: u32) -> Result<(), ProtocolLimitViolation> {
        if consecutive_losses >= self.max_consecutive_losses {
            return Err(ProtocolLimitViolation::ExceedsMaxConsecutiveLosses {
                current: consecutive_losses,
                limit: self.max_consecutive_losses,
            });
        }
        
        Ok(())
    }
    
    /// Validate that reward-to-risk ratio meets minimum requirements
    pub fn validate_reward_risk_ratio(&self, ratio: Decimal) -> Result<(), ProtocolLimitViolation> {
        if ratio < self.min_reward_risk_ratio {
            return Err(ProtocolLimitViolation::BelowMinRewardRiskRatio {
                current: ratio,
                limit: self.min_reward_risk_ratio,
            });
        }
        
        Ok(())
    }
}

impl Default for ProtocolLimits {
    fn default() -> Self {
        Self::default_limits()
    }
}

/// Violations of protocol limits
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum ProtocolLimitViolation {
    #[error("Individual trade risk {current} exceeds maximum limit {limit}")]
    ExceedsMaxIndividualRisk { current: Decimal, limit: Decimal },
    
    #[error("Individual trade risk {current} below minimum limit {limit}")]
    BelowMinIndividualRisk { current: Decimal, limit: Decimal },
    
    #[error("Total portfolio risk {current} exceeds maximum limit {limit}")]
    ExceedsMaxPortfolioRisk { current: Decimal, limit: Decimal },
    
    #[error("Consecutive losses {current} exceeds maximum limit {limit}")]
    ExceedsMaxConsecutiveLosses { current: u32, limit: u32 },
    
    #[error("Reward-to-risk ratio {current} below minimum requirement {limit}")]
    BelowMinRewardRiskRatio { current: Decimal, limit: Decimal },
    
    #[error("Number of open positions {current} exceeds maximum limit {limit}")]
    ExceedsMaxOpenPositions { current: u32, limit: u32 },
    
    #[error("Daily loss {current} exceeds maximum limit {limit}")]
    ExceedsMaxDailyLoss { current: Decimal, limit: Decimal },
    
    #[error("Drawdown {current} exceeds maximum limit {limit}")]
    ExceedsMaxDrawdown { current: Decimal, limit: Decimal },
}

/// Trait for types that can be validated against protocol limits
pub trait ProtocolCompliant {
    /// Validate this item against the given protocol limits
    fn validate_against_protocol(&self, limits: &ProtocolLimits) -> Result<(), Vec<ProtocolLimitViolation>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_protocol_limits() {
        let limits = ProtocolLimits::default();
        
        assert_eq!(limits.max_individual_trade_risk, dec!(0.06));
        assert_eq!(limits.min_individual_trade_risk, dec!(0.005));
        assert_eq!(limits.max_total_portfolio_risk, dec!(0.10));
        assert_eq!(limits.max_consecutive_losses, 3);
        assert_eq!(limits.min_reward_risk_ratio, dec!(2.0));
    }
    
    #[test]
    fn test_conservative_limits() {
        let limits = ProtocolLimits::conservative_limits();
        
        assert_eq!(limits.max_individual_trade_risk, dec!(0.02)); // More conservative
        assert_eq!(limits.max_total_portfolio_risk, dec!(0.05));  // More conservative
        assert_eq!(limits.max_consecutive_losses, 2);             // Lower tolerance
    }
    
    #[test]
    fn test_aggressive_limits() {
        let limits = ProtocolLimits::aggressive_limits();
        
        assert_eq!(limits.max_individual_trade_risk, dec!(0.10)); // More aggressive
        assert_eq!(limits.max_total_portfolio_risk, dec!(0.15));  // More aggressive
        assert_eq!(limits.max_consecutive_losses, 5);             // Higher tolerance
    }
    
    #[test]
    fn test_individual_trade_risk_validation() {
        let limits = ProtocolLimits::default();
        
        // Valid risk
        assert!(limits.validate_individual_trade_risk(dec!(0.03)).is_ok());
        
        // Too high risk
        let result = limits.validate_individual_trade_risk(dec!(0.08));
        assert!(result.is_err());
        match result {
            Err(ProtocolLimitViolation::ExceedsMaxIndividualRisk { current, limit }) => {
                assert_eq!(current, dec!(0.08));
                assert_eq!(limit, dec!(0.06));
            }
            _ => panic!("Expected ExceedsMaxIndividualRisk error"),
        }
        
        // Too low risk
        let result = limits.validate_individual_trade_risk(dec!(0.001));
        assert!(result.is_err());
        match result {
            Err(ProtocolLimitViolation::BelowMinIndividualRisk { current, limit }) => {
                assert_eq!(current, dec!(0.001));
                assert_eq!(limit, dec!(0.005));
            }
            _ => panic!("Expected BelowMinIndividualRisk error"),
        }
    }
    
    #[test]
    fn test_portfolio_risk_validation() {
        let limits = ProtocolLimits::default();
        
        // Valid portfolio risk
        assert!(limits.validate_portfolio_risk(dec!(0.08)).is_ok());
        
        // Portfolio risk too high
        let result = limits.validate_portfolio_risk(dec!(0.12));
        assert!(result.is_err());
        match result {
            Err(ProtocolLimitViolation::ExceedsMaxPortfolioRisk { current, limit }) => {
                assert_eq!(current, dec!(0.12));
                assert_eq!(limit, dec!(0.10));
            }
            _ => panic!("Expected ExceedsMaxPortfolioRisk error"),
        }
    }
    
    #[test]
    fn test_consecutive_losses_validation() {
        let limits = ProtocolLimits::default();
        
        // Valid consecutive losses
        assert!(limits.validate_consecutive_losses(2).is_ok());
        
        // Too many consecutive losses
        let result = limits.validate_consecutive_losses(3);
        assert!(result.is_err());
        match result {
            Err(ProtocolLimitViolation::ExceedsMaxConsecutiveLosses { current, limit }) => {
                assert_eq!(current, 3);
                assert_eq!(limit, 3);
            }
            _ => panic!("Expected ExceedsMaxConsecutiveLosses error"),
        }
    }
    
    #[test]
    fn test_reward_risk_ratio_validation() {
        let limits = ProtocolLimits::default();
        
        // Valid reward-risk ratio
        assert!(limits.validate_reward_risk_ratio(dec!(2.5)).is_ok());
        
        // Reward-risk ratio too low
        let result = limits.validate_reward_risk_ratio(dec!(1.5));
        assert!(result.is_err());
        match result {
            Err(ProtocolLimitViolation::BelowMinRewardRiskRatio { current, limit }) => {
                assert_eq!(current, dec!(1.5));
                assert_eq!(limit, dec!(2.0));
            }
            _ => panic!("Expected BelowMinRewardRiskRatio error"),
        }
    }
}