//! Risk validation coordinator
//!
//! This module provides a high-level interface for coordinating
//! risk validation between the RiskEngine and TestudoProtocol.

use crate::types::{TradeProposal, RiskAssessment, ProtocolLimits};
use crate::risk::{RiskEngine, TestudoProtocol};
use std::sync::{Arc, Mutex};
use tracing::{info, error};

/// High-level risk validator that coordinates between RiskEngine and TestudoProtocol
///
/// This validator provides a unified interface for risk assessment that combines
/// individual trade validation (RiskEngine) with portfolio-level protocol enforcement
/// (TestudoProtocol).
#[derive(Debug)]
pub struct RiskValidator {
    /// Risk engine for individual trade assessment
    risk_engine: RiskEngine,
    /// Testudo Protocol for portfolio-level enforcement
    protocol: Arc<Mutex<TestudoProtocol>>,
}

impl RiskValidator {
    /// Create a new risk validator with default configuration
    pub fn new() -> Self {
        let risk_engine = RiskEngine::new();
        let protocol = Arc::new(Mutex::new(TestudoProtocol::new()));
        
        Self {
            risk_engine,
            protocol,
        }
    }
    
    /// Create a new risk validator with custom protocol limits
    pub fn with_limits(limits: ProtocolLimits) -> Self {
        let risk_engine = RiskEngine::with_limits(limits.clone());
        let protocol = Arc::new(Mutex::new(TestudoProtocol::with_limits(limits)));
        
        Self {
            risk_engine,
            protocol,
        }
    }
    
    /// Create a conservative risk validator for new traders
    pub fn conservative() -> Self {
        let risk_engine = RiskEngine::conservative();
        let protocol = Arc::new(Mutex::new(TestudoProtocol::conservative()));
        
        Self {
            risk_engine,
            protocol,
        }
    }
    
    /// Create an aggressive risk validator for experienced traders
    pub fn aggressive() -> Self {
        let risk_engine = RiskEngine::aggressive();
        let protocol = Arc::new(Mutex::new(TestudoProtocol::aggressive()));
        
        Self {
            risk_engine,
            protocol,
        }
    }
    
    /// Perform comprehensive risk validation on a trade proposal
    ///
    /// This method combines individual trade risk assessment from the RiskEngine
    /// with portfolio-level protocol validation from the TestudoProtocol.
    pub fn validate_trade(&self, proposal: &TradeProposal) -> RiskValidationResult {
        info!("Starting comprehensive risk validation for proposal {}", proposal.id);
        
        // Step 1: Check if trading is currently allowed (circuit breaker, etc.)
        let trading_allowed = {
            match self.protocol.lock() {
                Ok(mut protocol) => protocol.is_trading_allowed(),
                Err(e) => {
                    error!("Failed to acquire protocol lock: {}", e);
                    return RiskValidationResult::SystemError {
                        message: "Failed to access protocol state".to_string(),
                    };
                }
            }
        };
        
        if !trading_allowed {
            return RiskValidationResult::TradingHalted {
                reason: "Circuit breaker active - trading is temporarily halted".to_string(),
            };
        }
        
        // Step 2: Perform individual trade risk assessment
        let mut assessment = self.risk_engine.assess_trade(proposal);
        
        // Step 3: Check protocol-level constraints
        let protocol_validation = {
            match self.protocol.lock() {
                Ok(mut protocol) => protocol.validate_trade(proposal),
                Err(e) => {
                    error!("Failed to acquire protocol lock for validation: {}", e);
                    return RiskValidationResult::SystemError {
                        message: "Failed to validate against protocol".to_string(),
                    };
                }
            }
        };
        
        // Step 4: Merge protocol violations into assessment
        if let Err(protocol_violations) = protocol_validation {
            for violation in protocol_violations {
                assessment.add_violation(violation);
            }
        }
        
        // Step 5: Determine final result
        let result = if assessment.is_blocked() {
            RiskValidationResult::Blocked { assessment }
        } else if assessment.is_approved() {
            RiskValidationResult::Approved { assessment }
        } else if assessment.requires_modification() {
            RiskValidationResult::RequiresModification { assessment }
        } else {
            // This shouldn't happen, but handle gracefully
            RiskValidationResult::Approved { assessment }
        };
        
        info!(
            "Risk validation completed for proposal {}: {:?}",
            proposal.id,
            std::mem::discriminant(&result)
        );
        
        result
    }
    
    /// Record successful trade execution
    ///
    /// This method should be called when a trade that passed validation
    /// is actually executed on the exchange.
    pub fn record_trade_execution(&self, proposal: &TradeProposal) -> Result<(), String> {
        match self.protocol.lock() {
            Ok(mut protocol) => {
                protocol.record_trade_execution(proposal);
                info!("Recorded trade execution for proposal {}", proposal.id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to record trade execution: {}", e);
                Err("Failed to update protocol state".to_string())
            }
        }
    }
    
    /// Record trade outcome (win or loss)
    ///
    /// This method should be called when a trade is closed to update
    /// consecutive loss tracking and portfolio risk calculations.
    pub fn record_trade_outcome(
        &self,
        symbol: &str,
        trade_risk: rust_decimal::Decimal,
        was_loss: bool,
        loss_amount: Option<rust_decimal::Decimal>,
    ) -> Result<(), String> {
        match self.protocol.lock() {
            Ok(mut protocol) => {
                protocol.record_trade_outcome(symbol, trade_risk, was_loss, loss_amount);
                info!(
                    "Recorded trade outcome for {}: loss={}, amount={:?}",
                    symbol, was_loss, loss_amount
                );
                Ok(())
            }
            Err(e) => {
                error!("Failed to record trade outcome: {}", e);
                Err("Failed to update protocol state".to_string())
            }
        }
    }
    
    /// Get current protocol status
    pub fn get_protocol_status(&self) -> Result<crate::risk::protocol::ProtocolStatus, String> {
        match self.protocol.lock() {
            Ok(protocol) => Ok(protocol.get_status()),
            Err(e) => {
                error!("Failed to get protocol status: {}", e);
                Err("Failed to access protocol state".to_string())
            }
        }
    }
    
    /// Manually reset the circuit breaker (admin function)
    pub fn reset_circuit_breaker(&self) -> Result<(), String> {
        match self.protocol.lock() {
            Ok(mut protocol) => {
                protocol.reset_circuit_breaker();
                info!("Circuit breaker manually reset");
                Ok(())
            }
            Err(e) => {
                error!("Failed to reset circuit breaker: {}", e);
                Err("Failed to access protocol state".to_string())
            }
        }
    }
    
    /// Get remaining risk budget
    pub fn remaining_risk_budget(&self) -> Result<rust_decimal::Decimal, String> {
        match self.protocol.lock() {
            Ok(protocol) => Ok(protocol.remaining_risk_budget()),
            Err(e) => {
                error!("Failed to get risk budget: {}", e);
                Err("Failed to access protocol state".to_string())
            }
        }
    }
    
    /// Get remaining daily loss budget
    pub fn remaining_daily_budget(&self, account_equity: rust_decimal::Decimal) -> Result<rust_decimal::Decimal, String> {
        match self.protocol.lock() {
            Ok(protocol) => Ok(protocol.remaining_daily_budget(account_equity)),
            Err(e) => {
                error!("Failed to get daily budget: {}", e);
                Err("Failed to access protocol state".to_string())
            }
        }
    }
    
    /// Get protocol limits
    pub fn protocol_limits(&self) -> &ProtocolLimits {
        self.risk_engine.protocol_limits()
    }
    
    /// Get risk engine information
    pub fn risk_engine_info(&self) -> (usize, Vec<(String, u8, String)>) {
        (self.risk_engine.rule_count(), self.risk_engine.rule_info())
    }
}

impl Default for RiskValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of comprehensive risk validation
#[derive(Debug)]
pub enum RiskValidationResult {
    /// Trade is approved and can be executed
    Approved { assessment: RiskAssessment },
    
    /// Trade requires modification before execution
    RequiresModification { assessment: RiskAssessment },
    
    /// Trade is blocked and cannot be executed
    Blocked { assessment: RiskAssessment },
    
    /// Trading is temporarily halted (circuit breaker, system maintenance, etc.)
    TradingHalted { reason: String },
    
    /// System error occurred during validation
    SystemError { message: String },
}

impl RiskValidationResult {
    /// Check if the trade is approved for execution
    pub fn is_approved(&self) -> bool {
        matches!(self, RiskValidationResult::Approved { .. })
    }
    
    /// Check if the trade requires modification
    pub fn requires_modification(&self) -> bool {
        matches!(self, RiskValidationResult::RequiresModification { .. })
    }
    
    /// Check if the trade is blocked
    pub fn is_blocked(&self) -> bool {
        matches!(
            self,
            RiskValidationResult::Blocked { .. } | RiskValidationResult::TradingHalted { .. }
        )
    }
    
    /// Check if there was a system error
    pub fn is_error(&self) -> bool {
        matches!(self, RiskValidationResult::SystemError { .. })
    }
    
    /// Get the risk assessment if available
    pub fn assessment(&self) -> Option<&RiskAssessment> {
        match self {
            RiskValidationResult::Approved { assessment } => Some(assessment),
            RiskValidationResult::RequiresModification { assessment } => Some(assessment),
            RiskValidationResult::Blocked { assessment } => Some(assessment),
            _ => None,
        }
    }
    
    /// Get a user-friendly status message
    pub fn status_message(&self) -> String {
        match self {
            RiskValidationResult::Approved { assessment } => {
                format!(
                    "✅ Trade approved: Position size {} with risk ${:.2} ({:.1}%)",
                    assessment.position_size.value(),
                    assessment.risk_amount,
                    assessment.risk_percentage * rust_decimal::Decimal::from(100)
                )
            }
            RiskValidationResult::RequiresModification { assessment } => {
                format!(
                    "⚠️ Trade requires modification: {} violations found",
                    assessment.violations.len()
                )
            }
            RiskValidationResult::Blocked { assessment } => {
                format!(
                    "🚫 Trade blocked: {} violations prevent execution",
                    assessment.violations.len()
                )
            }
            RiskValidationResult::TradingHalted { reason } => {
                format!("⏸️ Trading halted: {}", reason)
            }
            RiskValidationResult::SystemError { message } => {
                format!("💥 System error: {}", message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TradeSide;
    use disciplina::{AccountEquity, RiskPercentage, PricePoint};
    use rust_decimal_macros::dec;
    
    fn create_test_proposal(risk_pct: rust_decimal::Decimal) -> TradeProposal {
        TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(),
            Some(PricePoint::new(dec!(54000)).unwrap()),
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(risk_pct).unwrap(),
        ).unwrap()
    }
    
    #[test]
    fn test_validator_creation() {
        let validator = RiskValidator::new();
        let (rule_count, _) = validator.risk_engine_info();
        assert!(rule_count > 0);
        
        let status = validator.get_protocol_status().unwrap();
        assert_eq!(status.total_portfolio_risk, dec!(0));
        assert!(!status.circuit_breaker_active);
    }
    
    #[test]
    fn test_valid_trade_approval() {
        let validator = RiskValidator::new();
        let proposal = create_test_proposal(dec!(0.02)); // 2% risk
        
        let result = validator.validate_trade(&proposal);
        
        assert!(result.is_approved());
        
        let assessment = result.assessment().unwrap();
        assert!(assessment.is_approved());
        assert!(assessment.violations.is_empty());
    }
    
    #[test]
    fn test_excessive_risk_blocking() {
        let validator = RiskValidator::new();
        let proposal = create_test_proposal(dec!(0.08)); // 8% risk (exceeds 6% limit)
        
        let result = validator.validate_trade(&proposal);
        
        assert!(result.is_blocked());
        
        let assessment = result.assessment().unwrap();
        assert!(!assessment.is_approved());
        assert!(!assessment.violations.is_empty());
    }
    
    #[test]
    fn test_trade_execution_recording() {
        let validator = RiskValidator::new();
        let proposal = create_test_proposal(dec!(0.03)); // 3% risk
        
        // Validate and execute trade
        let result = validator.validate_trade(&proposal);
        assert!(result.is_approved());
        
        let execution_result = validator.record_trade_execution(&proposal);
        assert!(execution_result.is_ok());
        
        // Check that portfolio risk is updated
        let status = validator.get_protocol_status().unwrap();
        assert_eq!(status.total_portfolio_risk, dec!(0.03));
        assert_eq!(status.open_positions, 1);
    }
    
    #[test]
    fn test_trade_outcome_recording() {
        let validator = RiskValidator::new();
        let proposal = create_test_proposal(dec!(0.02));
        
        // Execute trade
        validator.record_trade_execution(&proposal).unwrap();
        
        // Record a loss
        let outcome_result = validator.record_trade_outcome("BTCUSDT", dec!(0.02), true, Some(dec!(200)));
        assert!(outcome_result.is_ok());
        
        // Check status
        let status = validator.get_protocol_status().unwrap();
        assert_eq!(status.consecutive_losses, 1);
        assert_eq!(status.daily_loss, dec!(200));
        assert_eq!(status.total_portfolio_risk, dec!(0)); // Should be removed after outcome
        assert_eq!(status.open_positions, 0);
    }
    
    #[test]
    fn test_circuit_breaker_activation() {
        let validator = RiskValidator::new();
        let proposal = create_test_proposal(dec!(0.02));
        
        // Record 3 consecutive losses to trigger circuit breaker
        validator.record_trade_outcome("BTCUSDT", dec!(0.02), true, Some(dec!(200))).unwrap();
        validator.record_trade_outcome("ETHUSDT", dec!(0.02), true, Some(dec!(200))).unwrap();
        validator.record_trade_outcome("ADAUSDT", dec!(0.02), true, Some(dec!(200))).unwrap();
        
        // Circuit breaker should now be active
        let status = validator.get_protocol_status().unwrap();
        assert!(status.circuit_breaker_active);
        
        // New trade should be blocked
        let result = validator.validate_trade(&proposal);
        assert!(!result.is_approved());
        
        // Reset circuit breaker
        let reset_result = validator.reset_circuit_breaker();
        assert!(reset_result.is_ok());
        
        // Trading should now be allowed again
        let result = validator.validate_trade(&proposal);
        assert!(result.is_approved());
    }
    
    #[test]
    fn test_budget_calculations() {
        let validator = RiskValidator::new();
        let proposal = create_test_proposal(dec!(0.04)); // 4% risk
        
        // Initial budgets
        let risk_budget = validator.remaining_risk_budget().unwrap();
        let daily_budget = validator.remaining_daily_budget(dec!(10000)).unwrap();
        
        assert_eq!(risk_budget, dec!(0.10)); // 10% max portfolio risk
        assert_eq!(daily_budget, dec!(500)); // 5% of 10000 = 500
        
        // Execute trade
        validator.record_trade_execution(&proposal).unwrap();
        
        // Check updated budgets
        let risk_budget = validator.remaining_risk_budget().unwrap();
        assert_eq!(risk_budget, dec!(0.06)); // 10% - 4% = 6%
        
        // Record a loss to update daily budget
        validator.record_trade_outcome("BTCUSDT", dec!(0.04), true, Some(dec!(300))).unwrap();
        
        let daily_budget = validator.remaining_daily_budget(dec!(10000)).unwrap();
        assert_eq!(daily_budget, dec!(200)); // 500 - 300 = 200
    }
    
    #[test]
    fn test_validation_result_methods() {
        let validator = RiskValidator::new();
        let approved_proposal = create_test_proposal(dec!(0.02));
        let blocked_proposal = create_test_proposal(dec!(0.08));
        
        let approved_result = validator.validate_trade(&approved_proposal);
        assert!(approved_result.is_approved());
        assert!(!approved_result.is_blocked());
        assert!(!approved_result.is_error());
        assert!(approved_result.assessment().is_some());
        
        let blocked_result = validator.validate_trade(&blocked_proposal);
        assert!(!blocked_result.is_approved());
        assert!(blocked_result.is_blocked());
        assert!(!blocked_result.is_error());
        assert!(blocked_result.assessment().is_some());
        
        // Test status messages
        let approved_msg = approved_result.status_message();
        assert!(approved_msg.contains("approved"));
        
        let blocked_msg = blocked_result.status_message();
        assert!(blocked_msg.contains("blocked"));
    }
}