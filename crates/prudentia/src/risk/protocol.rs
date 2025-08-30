//! Testudo Protocol implementation and enforcement
//!
//! This module provides the core implementation of the Testudo Protocol,
//! which serves as the immutable guardian of capital protection through
//! systematic risk management.

use crate::types::{ProtocolLimits, ProtocolViolation, TradeProposal};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::time::{SystemTime, Duration};
use tracing::{info, warn};

/// The Testudo Protocol enforcer
///
/// This struct maintains the state and enforces the immutable rules
/// that protect traders from catastrophic losses. It tracks portfolio
/// risk exposure, consecutive losses, and daily limits.
#[derive(Debug, Clone)]
pub struct TestudoProtocol {
    /// Immutable protocol limits
    limits: ProtocolLimits,
    /// Current portfolio risk exposure by asset/symbol
    portfolio_exposure: HashMap<String, Decimal>,
    /// Total portfolio risk percentage
    total_portfolio_risk: Decimal,
    /// Consecutive loss tracking
    consecutive_losses: u32,
    /// Last loss timestamp for consecutive loss tracking
    last_loss_time: Option<SystemTime>,
    /// Daily loss tracking
    daily_loss: Decimal,
    /// Last daily reset timestamp
    last_daily_reset: SystemTime,
    /// Number of open positions
    open_positions: u32,
    /// Circuit breaker state
    circuit_breaker_active: bool,
    /// Timestamp when circuit breaker was activated
    circuit_breaker_activated_at: Option<SystemTime>,
}

impl TestudoProtocol {
    /// Create a new Testudo Protocol enforcer with default limits
    pub fn new() -> Self {
        Self::with_limits(ProtocolLimits::default())
    }
    
    /// Create a new Testudo Protocol enforcer with custom limits
    pub fn with_limits(limits: ProtocolLimits) -> Self {
        Self {
            limits,
            portfolio_exposure: HashMap::new(),
            total_portfolio_risk: Decimal::ZERO,
            consecutive_losses: 0,
            last_loss_time: None,
            daily_loss: Decimal::ZERO,
            last_daily_reset: SystemTime::now(),
            open_positions: 0,
            circuit_breaker_active: false,
            circuit_breaker_activated_at: None,
        }
    }
    
    /// Conservative protocol for new traders
    pub fn conservative() -> Self {
        Self::with_limits(ProtocolLimits::conservative_limits())
    }
    
    /// Aggressive protocol for experienced traders
    pub fn aggressive() -> Self {
        Self::with_limits(ProtocolLimits::aggressive_limits())
    }
    
    /// Validate a trade proposal against all protocol limits
    pub fn validate_trade(&mut self, proposal: &TradeProposal) -> Result<(), Vec<ProtocolViolation>> {
        let mut violations = Vec::new();
        
        // Reset daily tracking if needed
        self.reset_daily_tracking_if_needed();
        
        // Check if circuit breaker should be reset
        self.check_circuit_breaker_reset();
        
        // 1. Check circuit breaker status
        if self.circuit_breaker_active {
            violations.push(ProtocolViolation::ExceedsMaxConsecutiveLosses {
                current: self.consecutive_losses,
                limit: self.limits.max_consecutive_losses,
            });
        }
        
        // 2. Validate individual trade risk
        if let Err(violation) = self.limits.validate_individual_trade_risk(proposal.risk_percentage.value()) {
            violations.push(violation);
        }
        
        // 3. Calculate potential new portfolio risk
        let trade_risk = proposal.risk_percentage.value();
        let potential_portfolio_risk = self.total_portfolio_risk + trade_risk;
        
        if let Err(violation) = self.limits.validate_portfolio_risk(potential_portfolio_risk) {
            violations.push(violation);
        }
        
        // 4. Check consecutive losses
        if let Err(violation) = self.limits.validate_consecutive_losses(self.consecutive_losses) {
            violations.push(violation);
        }
        
        // 5. Check open positions limit
        if self.open_positions >= self.limits.max_open_positions {
            violations.push(ProtocolViolation::ExceedsMaxOpenPositions {
                current: self.open_positions,
                limit: self.limits.max_open_positions,
            });
        }
        
        // 6. Check reward/risk ratio if take profit is set
        if let Some(ratio) = proposal.risk_reward_ratio() {
            if let Err(violation) = self.limits.validate_reward_risk_ratio(ratio) {
                violations.push(violation);
            }
        }
        
        // 7. Check daily loss limit
        let potential_daily_loss = self.daily_loss + trade_risk * proposal.account_equity.value();
        let daily_loss_percentage = potential_daily_loss / proposal.account_equity.value();
        
        if daily_loss_percentage > self.limits.max_daily_loss {
            violations.push(ProtocolViolation::ExceedsMaxDailyLoss {
                current: daily_loss_percentage,
                limit: self.limits.max_daily_loss,
            });
        }
        
        if violations.is_empty() {
            info!("Trade proposal {} passed Testudo Protocol validation", proposal.id);
            Ok(())
        } else {
            warn!(
                "Trade proposal {} failed Testudo Protocol validation with {} violations",
                proposal.id,
                violations.len()
            );
            Err(violations)
        }
    }
    
    /// Record a successful trade execution
    pub fn record_trade_execution(&mut self, proposal: &TradeProposal) {
        let trade_risk = proposal.risk_percentage.value();
        
        // Add to portfolio exposure
        let current_exposure = self.portfolio_exposure
            .get(&proposal.symbol)
            .copied()
            .unwrap_or(Decimal::ZERO);
        
        self.portfolio_exposure.insert(
            proposal.symbol.clone(),
            current_exposure + trade_risk,
        );
        
        // Update total portfolio risk
        self.total_portfolio_risk += trade_risk;
        
        // Increment open positions
        self.open_positions += 1;
        
        info!(
            "Recorded trade execution for {}: risk={:.2}%, total_portfolio_risk={:.2}%, open_positions={}",
            proposal.symbol,
            trade_risk * Decimal::from(100),
            self.total_portfolio_risk * Decimal::from(100),
            self.open_positions
        );
    }
    
    /// Record a trade outcome (win or loss)
    pub fn record_trade_outcome(&mut self, symbol: &str, trade_risk: Decimal, was_loss: bool, loss_amount: Option<Decimal>) {
        // Remove from portfolio exposure
        if let Some(current_exposure) = self.portfolio_exposure.get_mut(symbol) {
            *current_exposure = (*current_exposure - trade_risk).max(Decimal::ZERO);
            if current_exposure.is_zero() {
                self.portfolio_exposure.remove(symbol);
            }
        }
        
        // Update total portfolio risk
        self.total_portfolio_risk = (self.total_portfolio_risk - trade_risk).max(Decimal::ZERO);
        
        // Decrement open positions
        self.open_positions = self.open_positions.saturating_sub(1);
        
        // Handle consecutive loss tracking
        if was_loss {
            self.consecutive_losses += 1;
            self.last_loss_time = Some(SystemTime::now());
            
            // Add to daily loss if amount is provided
            if let Some(loss) = loss_amount {
                self.daily_loss += loss;
            }
            
            // Activate circuit breaker if limit reached
            if self.consecutive_losses >= self.limits.max_consecutive_losses {
                self.activate_circuit_breaker();
            }
            
            warn!(
                "Recorded loss for {}: consecutive_losses={}, daily_loss=${:.2}",
                symbol, self.consecutive_losses, self.daily_loss
            );
        } else {
            // Reset consecutive losses on win
            self.consecutive_losses = 0;
            self.last_loss_time = None;
            
            info!(
                "Recorded win for {}: consecutive losses reset, daily_loss=${:.2}",
                symbol, self.daily_loss
            );
        }
        
        info!(
            "Trade outcome recorded for {}: total_portfolio_risk={:.2}%, open_positions={}",
            symbol,
            self.total_portfolio_risk * Decimal::from(100),
            self.open_positions
        );
    }
    
    /// Activate the circuit breaker
    fn activate_circuit_breaker(&mut self) {
        if !self.circuit_breaker_active {
            self.circuit_breaker_active = true;
            self.circuit_breaker_activated_at = Some(SystemTime::now());
            
            warn!(
                "🚨 CIRCUIT BREAKER ACTIVATED: {} consecutive losses detected. Trading halted for safety.",
                self.consecutive_losses
            );
        }
    }
    
    /// Check if circuit breaker should be reset (after timeout or manual intervention)
    fn check_circuit_breaker_reset(&mut self) {
        if self.circuit_breaker_active {
            if let Some(activated_at) = self.circuit_breaker_activated_at {
                let elapsed = SystemTime::now().duration_since(activated_at).unwrap_or_default();
                
                // Reset after 1 hour (configurable in real system)
                if elapsed > Duration::from_secs(3600) {
                    self.reset_circuit_breaker();
                }
            }
        }
    }
    
    /// Manually reset the circuit breaker (admin function)
    pub fn reset_circuit_breaker(&mut self) {
        if self.circuit_breaker_active {
            self.circuit_breaker_active = false;
            self.circuit_breaker_activated_at = None;
            self.consecutive_losses = 0;
            self.last_loss_time = None;
            
            info!("✅ Circuit breaker reset. Trading can resume.");
        }
    }
    
    /// Reset daily tracking if we've crossed into a new day
    fn reset_daily_tracking_if_needed(&mut self) {
        let now = SystemTime::now();
        let elapsed = now.duration_since(self.last_daily_reset).unwrap_or_default();
        
        // Reset after 24 hours (simplified - real system would use market hours)
        if elapsed > Duration::from_secs(24 * 3600) {
            self.daily_loss = Decimal::ZERO;
            self.last_daily_reset = now;
            info!("🌅 Daily risk tracking reset");
        }
    }
    
    /// Get current protocol status
    pub fn get_status(&self) -> ProtocolStatus {
        ProtocolStatus {
            total_portfolio_risk: self.total_portfolio_risk,
            consecutive_losses: self.consecutive_losses,
            daily_loss: self.daily_loss,
            open_positions: self.open_positions,
            circuit_breaker_active: self.circuit_breaker_active,
            risk_utilization: self.total_portfolio_risk / self.limits.max_total_portfolio_risk,
            days_since_last_reset: SystemTime::now()
                .duration_since(self.last_daily_reset)
                .unwrap_or_default()
                .as_secs() / 86400,
            portfolio_exposure: self.portfolio_exposure.clone(),
        }
    }
    
    /// Get protocol limits
    pub fn limits(&self) -> &ProtocolLimits {
        &self.limits
    }
    
    /// Check if trading is currently allowed
    pub fn is_trading_allowed(&mut self) -> bool {
        self.reset_daily_tracking_if_needed();
        self.check_circuit_breaker_reset();
        !self.circuit_breaker_active
    }
    
    /// Calculate remaining risk budget
    pub fn remaining_risk_budget(&self) -> Decimal {
        (self.limits.max_total_portfolio_risk - self.total_portfolio_risk).max(Decimal::ZERO)
    }
    
    /// Calculate remaining daily loss budget
    pub fn remaining_daily_budget(&self, account_equity: Decimal) -> Decimal {
        if account_equity.is_zero() {
            return Decimal::ZERO;
        }
        
        let daily_limit = account_equity * self.limits.max_daily_loss;
        (daily_limit - self.daily_loss).max(Decimal::ZERO)
    }
}

impl Default for TestudoProtocol {
    fn default() -> Self {
        Self::new()
    }
}

/// Current status of the Testudo Protocol
#[derive(Debug, Clone)]
pub struct ProtocolStatus {
    pub total_portfolio_risk: Decimal,
    pub consecutive_losses: u32,
    pub daily_loss: Decimal,
    pub open_positions: u32,
    pub circuit_breaker_active: bool,
    pub risk_utilization: Decimal, // Percentage of max risk used
    pub days_since_last_reset: u64,
    pub portfolio_exposure: HashMap<String, Decimal>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TradeSide;
    use disciplina::{AccountEquity, RiskPercentage, PricePoint};
    use rust_decimal_macros::dec;
    
    fn create_test_proposal(risk_pct: Decimal) -> TradeProposal {
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
    fn test_protocol_creation() {
        let protocol = TestudoProtocol::new();
        let status = protocol.get_status();
        
        assert_eq!(status.total_portfolio_risk, Decimal::ZERO);
        assert_eq!(status.consecutive_losses, 0);
        assert_eq!(status.open_positions, 0);
        assert!(!status.circuit_breaker_active);
    }
    
    #[test]
    fn test_valid_trade_validation() {
        let mut protocol = TestudoProtocol::new();
        let proposal = create_test_proposal(dec!(0.02)); // 2% risk
        
        let result = protocol.validate_trade(&proposal);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_excessive_individual_risk_rejection() {
        let mut protocol = TestudoProtocol::new();
        let proposal = create_test_proposal(dec!(0.08)); // 8% risk (exceeds 6% limit)
        
        let result = protocol.validate_trade(&proposal);
        assert!(result.is_err());
        
        let violations = result.unwrap_err();
        assert!(violations.iter().any(|v| matches!(v, ProtocolViolation::ExceedsMaxIndividualRisk { .. })));
    }
    
    #[test]
    fn test_portfolio_risk_accumulation() {
        let mut protocol = TestudoProtocol::new();
        
        // Add several trades to approach portfolio limit
        let proposal1 = create_test_proposal(dec!(0.04)); // 4% risk
        let proposal2 = create_test_proposal(dec!(0.04)); // 4% risk
        let proposal3 = create_test_proposal(dec!(0.04)); // 4% risk (would exceed 10% total)
        
        // First two trades should pass
        assert!(protocol.validate_trade(&proposal1).is_ok());
        protocol.record_trade_execution(&proposal1);
        
        assert!(protocol.validate_trade(&proposal2).is_ok());
        protocol.record_trade_execution(&proposal2);
        
        // Third trade should fail due to portfolio limit
        let result = protocol.validate_trade(&proposal3);
        assert!(result.is_err());
        
        let violations = result.unwrap_err();
        assert!(violations.iter().any(|v| matches!(v, ProtocolViolation::ExceedsMaxPortfolioRisk { .. })));
    }
    
    #[test]
    fn test_consecutive_loss_circuit_breaker() {
        let mut protocol = TestudoProtocol::new();
        let proposal = create_test_proposal(dec!(0.02));
        
        // Record consecutive losses
        protocol.record_trade_outcome("BTCUSDT", dec!(0.02), true, Some(dec!(200))); // Loss 1
        assert!(protocol.is_trading_allowed());
        
        protocol.record_trade_outcome("ETHUSDT", dec!(0.02), true, Some(dec!(200))); // Loss 2
        assert!(protocol.is_trading_allowed());
        
        protocol.record_trade_outcome("ADAUSDT", dec!(0.02), true, Some(dec!(200))); // Loss 3
        
        // Circuit breaker should now be active
        assert!(!protocol.is_trading_allowed());
        
        let result = protocol.validate_trade(&proposal);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_consecutive_loss_reset_on_win() {
        let mut protocol = TestudoProtocol::new();
        
        // Record two losses
        protocol.record_trade_outcome("BTCUSDT", dec!(0.02), true, Some(dec!(200)));
        protocol.record_trade_outcome("ETHUSDT", dec!(0.02), true, Some(dec!(200)));
        
        let status = protocol.get_status();
        assert_eq!(status.consecutive_losses, 2);
        
        // Record a win - should reset consecutive losses
        protocol.record_trade_outcome("ADAUSDT", dec!(0.02), false, None);
        
        let status = protocol.get_status();
        assert_eq!(status.consecutive_losses, 0);
        assert!(protocol.is_trading_allowed());
    }
    
    #[test]
    fn test_risk_budget_calculations() {
        let mut protocol = TestudoProtocol::new();
        let proposal = create_test_proposal(dec!(0.04)); // 4% risk
        
        // Initial budget should be 10% (full limit)
        assert_eq!(protocol.remaining_risk_budget(), dec!(0.10));
        
        // Execute trade
        protocol.record_trade_execution(&proposal);
        
        // Budget should now be 6% (10% - 4%)
        assert_eq!(protocol.remaining_risk_budget(), dec!(0.06));
        
        // Close the trade
        protocol.record_trade_outcome("BTCUSDT", dec!(0.04), false, None);
        
        // Budget should return to 10%
        assert_eq!(protocol.remaining_risk_budget(), dec!(0.10));
    }
    
    #[test]
    fn test_daily_loss_tracking() {
        let mut protocol = TestudoProtocol::new();
        let account_equity = dec!(10000);
        
        // Initial daily budget should be 5% of account (500)
        assert_eq!(protocol.remaining_daily_budget(account_equity), dec!(500));
        
        // Record some losses
        protocol.record_trade_outcome("BTCUSDT", dec!(0.02), true, Some(dec!(200)));
        assert_eq!(protocol.remaining_daily_budget(account_equity), dec!(300));
        
        protocol.record_trade_outcome("ETHUSDT", dec!(0.02), true, Some(dec!(150)));
        assert_eq!(protocol.remaining_daily_budget(account_equity), dec!(150));
    }
    
    #[test]
    fn test_portfolio_exposure_tracking() {
        let mut protocol = TestudoProtocol::new();
        
        let btc_proposal = create_test_proposal(dec!(0.03));
        let eth_proposal = TradeProposal::new(
            "ETHUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(3000)).unwrap(),
            PricePoint::new(dec!(2800)).unwrap(),
            None,
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(),
        ).unwrap();
        
        // Execute trades
        protocol.record_trade_execution(&btc_proposal);
        protocol.record_trade_execution(&eth_proposal);
        
        let status = protocol.get_status();
        assert_eq!(status.portfolio_exposure.get("BTCUSDT"), Some(&dec!(0.03)));
        assert_eq!(status.portfolio_exposure.get("ETHUSDT"), Some(&dec!(0.02)));
        assert_eq!(status.total_portfolio_risk, dec!(0.05));
        
        // Close BTC trade
        protocol.record_trade_outcome("BTCUSDT", dec!(0.03), false, None);
        
        let status = protocol.get_status();
        assert_eq!(status.portfolio_exposure.get("BTCUSDT"), None);
        assert_eq!(status.portfolio_exposure.get("ETHUSDT"), Some(&dec!(0.02)));
        assert_eq!(status.total_portfolio_risk, dec!(0.02));
    }
}