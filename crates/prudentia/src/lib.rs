//! Prudentia - Risk Management and Testudo Protocol Enforcement
//!
//! This crate embodies the Roman virtue of Prudentia (prudence) through comprehensive
//! risk management and systematic capital protection. It serves as the immovable guardian
//! that prevents catastrophic losses through disciplined protocol enforcement.
//!
//! ## Core Mission
//!
//! Prudentia implements the Testudo Protocol - a comprehensive risk management framework
//! designed to protect traders from emotional decision-making and account destruction.
//! Every trade must pass through multiple layers of risk validation before execution.
//!
//! ## Risk Management Components
//!
//! - **Trade Validation**: Individual trade risk assessment using Van Tharp methodology
//! - **Protocol Enforcement**: Immutable risk limits that cannot be overridden
//! - **Portfolio Tracking**: Real-time monitoring of total portfolio risk exposure
//! - **Circuit Breaker**: Automatic trading halt on consecutive losses
//! - **Daily Limits**: Daily loss limits with automatic reset functionality
//!
//! ## Testudo Protocol Limits
//!
//! The protocol enforces these immutable limits to prevent account blowups:
//! - Maximum individual trade risk: 6% of account equity
//! - Maximum portfolio risk: 10% across all open positions
//! - Circuit breaker: 3 consecutive losses halt trading
//! - Minimum reward/risk ratio: 2:1 for profitable expectation
//!
//! ## Roman Military Principle: Prudentia
//!
//! Like the Roman virtue of prudence, this crate considers not just immediate risk
//! but long-term capital preservation. Every decision is mathematically verified
//! and designed to protect against the psychological pitfalls of trading.
//!
//! ## Usage Example
//!
//! ```rust
//! use prudentia::{RiskValidator, TradeProposal, TradeSide};
//! use disciplina::{AccountEquity, RiskPercentage, PricePoint};
//! use rust_decimal_macros::dec;
//!
//! let validator = RiskValidator::new();
//! 
//! let proposal = TradeProposal::new(
//!     "BTC/USDT".to_string(),
//!     TradeSide::Long,
//!     PricePoint::new(dec!(50000))?,
//!     PricePoint::new(dec!(48000))?, // 4% stop loss
//!     Some(PricePoint::new(dec!(54000))?), // 8% take profit (2:1 ratio)
//!     AccountEquity::new(dec!(10000))?,
//!     RiskPercentage::new(dec!(0.02))?, // 2% risk
//! )?;
//!
//! let result = validator.validate_trade(&proposal);
//! 
//! if result.is_approved() {
//!     // Trade passes all risk checks - safe to execute
//!     validator.record_trade_execution(&proposal)?;
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

// Primary modules - Risk Management
pub mod types;
pub mod risk;
pub mod monitoring;

// Secondary modules - Exchange Integration (legacy support)
pub mod exchange;

// Re-export core risk management types and functions
pub use types::{
    TradeProposal, TradeSide, RiskAssessment, ApprovalStatus, 
    ProtocolViolation, ViolationSeverity, ProtocolLimits, RiskProfile
};

pub use risk::{
    RiskEngine, RiskValidator, TestudoProtocol, RiskValidationResult,
    RiskRule, RiskViolation, TradeRiskAssessment,
    AssessmentRiskRule, MaxTradeRiskRule,  // Task 2: New RiskRule trait and implementation
    RiskManagementProtocol, ProtocolAssessmentResult, ProtocolDecision,  // Task 3: Risk Management Protocol
    ProtocolError, RuleAssessmentResult,   // Task 3: Supporting types
    MaxPortfolioRiskRule, OpenPosition, DailyLossLimitRule, ConsecutiveLossLimitRule  // Task 4a, 4b & 4c: Portfolio-level risk management
};

pub use monitoring::{
    PortfolioTracker, PortfolioRiskMetrics, ConsecutiveLossTracker,
    CircuitBreakerState, CircuitBreakerAction, RealTimeRiskMetrics
};

// Legacy exchange integration exports (for backward compatibility)
pub use exchange::{
    ExchangeAdapterTrait, BinanceAdapter, ExchangeConfig,
    CircuitBreaker, ExchangeRateLimiter, FailoverManager, ExchangeFailoverConfig
};

use rust_decimal::Decimal;
use std::time::Duration;
use thiserror::Error;

/// Prudentia risk management errors
#[derive(Debug, Error, Clone)]
pub enum PrudentiaError {
    #[error("Risk validation failed: {reason}")]
    RiskValidationFailure { reason: String },
    
    #[error("Protocol violation: {violation}")]
    ProtocolViolation { violation: String },
    
    #[error("Circuit breaker active: trading is halted due to {reason}")]
    CircuitBreakerActive { reason: String },
    
    #[error("Position sizing calculation failed: {reason}")]
    PositionSizingFailure { reason: String },
    
    #[error("Portfolio risk limit exceeded: current={current}%, limit={limit}%")]
    PortfolioRiskExceeded { current: Decimal, limit: Decimal },
    
    #[error("Daily loss limit exceeded: current=${current}, limit=${limit}")]
    DailyLossLimitExceeded { current: Decimal, limit: Decimal },
    
    #[error("Trade proposal invalid: {reason}")]
    InvalidTradeProposal { reason: String },
    
    #[error("Risk engine configuration error: {reason}")]
    ConfigurationError { reason: String },
    
    #[error("Protocol state error: {reason}")]
    ProtocolStateError { reason: String },
    
    // Legacy exchange integration errors (for backward compatibility)
    #[error("Exchange connection failed: {exchange} - {reason}")]
    ExchangeConnectionFailure { exchange: String, reason: String },
    
    #[error("Exchange rate limit exceeded: {exchange} - retry after {retry_after:?}")]
    ExchangeRateLimitExceeded { exchange: String, retry_after: Duration },
}

/// Result type for all Prudentia operations
pub type Result<T> = std::result::Result<T, PrudentiaError>;

/// Convenient type alias for creating standard risk validators
pub type StandardRiskValidator = RiskValidator;

/// Convenient type alias for creating conservative risk validators
pub type ConservativeRiskValidator = RiskValidator;

/// Convenient type alias for creating aggressive risk validators  
pub type AggressiveRiskValidator = RiskValidator;

/// Risk management utility functions
pub struct RiskManager;

impl RiskManager {
    /// Create a risk validator with default (standard) limits
    pub fn standard() -> RiskValidator {
        RiskValidator::new()
    }
    
    /// Create a risk validator with conservative limits (for new traders)
    pub fn conservative() -> RiskValidator {
        RiskValidator::conservative()
    }
    
    /// Create a risk validator with aggressive limits (for experienced traders)
    pub fn aggressive() -> RiskValidator {
        RiskValidator::aggressive()
    }
    
    /// Create a risk validator with custom protocol limits
    pub fn with_custom_limits(limits: ProtocolLimits) -> RiskValidator {
        RiskValidator::with_limits(limits)
    }
    
    /// Create protocol limits based on trader experience level
    pub fn limits_for_experience_level(level: TraderExperienceLevel) -> ProtocolLimits {
        match level {
            TraderExperienceLevel::Beginner => ProtocolLimits::conservative_limits(),
            TraderExperienceLevel::Intermediate => ProtocolLimits::default_limits(),
            TraderExperienceLevel::Advanced => ProtocolLimits::aggressive_limits(),
        }
    }
}

/// Trader experience levels for determining appropriate risk limits
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraderExperienceLevel {
    /// New to trading - needs maximum protection
    Beginner,
    /// Has some experience - standard protection
    Intermediate,
    /// Experienced trader - can handle higher risk
    Advanced,
}

#[cfg(test)]
mod tests {
    use super::*;
    use disciplina::{AccountEquity, RiskPercentage, PricePoint};
    use rust_decimal_macros::dec;
    
    #[test]
    fn test_risk_manager_factory_methods() {
        let standard_validator = RiskManager::standard();
        let conservative_validator = RiskManager::conservative();
        let aggressive_validator = RiskManager::aggressive();
        
        // Verify different protocol limits are set
        let standard_limits = standard_validator.protocol_limits();
        let conservative_limits = conservative_validator.protocol_limits();
        let aggressive_limits = aggressive_validator.protocol_limits();
        
        assert_eq!(standard_limits.max_individual_trade_risk, dec!(0.06));
        assert_eq!(conservative_limits.max_individual_trade_risk, dec!(0.02));
        assert_eq!(aggressive_limits.max_individual_trade_risk, dec!(0.10));
    }
    
    #[test]
    fn test_trader_experience_level_limits() {
        let beginner_limits = RiskManager::limits_for_experience_level(TraderExperienceLevel::Beginner);
        let intermediate_limits = RiskManager::limits_for_experience_level(TraderExperienceLevel::Intermediate);
        let advanced_limits = RiskManager::limits_for_experience_level(TraderExperienceLevel::Advanced);
        
        // Beginner should have most conservative limits
        assert!(beginner_limits.max_individual_trade_risk < intermediate_limits.max_individual_trade_risk);
        assert!(intermediate_limits.max_individual_trade_risk < advanced_limits.max_individual_trade_risk);
        
        // Beginner should have lower consecutive loss tolerance
        assert!(beginner_limits.max_consecutive_losses <= intermediate_limits.max_consecutive_losses);
        assert!(intermediate_limits.max_consecutive_losses <= advanced_limits.max_consecutive_losses);
    }
    
    #[test]
    fn test_custom_limits_validator() {
        let custom_limits = ProtocolLimits::default();
        let validator = RiskManager::with_custom_limits(custom_limits.clone());
        
        assert_eq!(validator.protocol_limits(), &custom_limits);
    }
    
    #[test]
    fn test_risk_validation_integration() {
        let validator = RiskManager::standard();
        
        // Create a valid trade proposal
        let proposal = TradeProposal::new(
            "BTC/USDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(),
            Some(PricePoint::new(dec!(54000)).unwrap()),
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(),
        ).unwrap();
        
        let result = validator.validate_trade(&proposal);
        assert!(result.is_approved());
        
        let assessment = result.assessment().unwrap();
        assert_eq!(assessment.risk_percentage, dec!(0.02));
        assert!(assessment.position_size.value() > Decimal::ZERO);
    }
    
    #[test]
    fn test_prudentia_error_types() {
        let risk_error = PrudentiaError::RiskValidationFailure {
            reason: "Test error".to_string()
        };
        
        assert!(risk_error.to_string().contains("Risk validation failed"));
        
        let protocol_error = PrudentiaError::ProtocolViolation {
            violation: "Exceeded risk limit".to_string()
        };
        
        assert!(protocol_error.to_string().contains("Protocol violation"));
    }
}