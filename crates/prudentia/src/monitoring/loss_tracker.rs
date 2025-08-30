//! Consecutive loss tracking and circuit breaker functionality

use rust_decimal::Decimal;
use std::time::SystemTime;

/// Circuit breaker state for tracking consecutive losses
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    /// Circuit breaker is closed, trading allowed
    Closed,
    /// Circuit breaker is open, trading halted
    Open { reason: String, opened_at: SystemTime },
    /// Circuit breaker is half-open, allowing limited testing
    HalfOpen,
}

/// Action to take based on circuit breaker state
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerAction {
    /// Continue normal trading
    Continue,
    /// Continue with extra caution (approaching limit)
    ContinueWithCaution,
    /// Halt all trading immediately
    HaltTrading,
}

/// Tracks consecutive losses and manages circuit breaker state
#[derive(Debug, Clone)]
pub struct ConsecutiveLossTracker {
    consecutive_losses: u32,
    max_consecutive_losses: u32,
    total_daily_loss: Decimal,
    last_loss_timestamp: Option<SystemTime>,
    circuit_breaker_state: CircuitBreakerState,
}

impl ConsecutiveLossTracker {
    /// Create new consecutive loss tracker
    pub fn new(max_consecutive_losses: u32) -> Self {
        Self {
            consecutive_losses: 0,
            max_consecutive_losses,
            total_daily_loss: Decimal::ZERO,
            last_loss_timestamp: None,
            circuit_breaker_state: CircuitBreakerState::Closed,
        }
    }
    
    /// Record a trade outcome (win or loss)
    pub fn record_trade_outcome(&mut self, was_loss: bool, loss_amount: Option<Decimal>) -> CircuitBreakerAction {
        if was_loss {
            self.consecutive_losses += 1;
            self.last_loss_timestamp = Some(SystemTime::now());
            
            if let Some(amount) = loss_amount {
                self.total_daily_loss += amount;
            }
            
            // Check if we need to trigger circuit breaker
            if self.consecutive_losses >= self.max_consecutive_losses {
                self.circuit_breaker_state = CircuitBreakerState::Open {
                    reason: format!("{} consecutive losses reached", self.consecutive_losses),
                    opened_at: SystemTime::now(),
                };
                CircuitBreakerAction::HaltTrading
            } else if self.consecutive_losses == self.max_consecutive_losses - 1 {
                // One away from circuit breaker
                CircuitBreakerAction::ContinueWithCaution
            } else {
                CircuitBreakerAction::Continue
            }
        } else {
            // Win - reset consecutive losses
            self.consecutive_losses = 0;
            self.last_loss_timestamp = None;
            CircuitBreakerAction::Continue
        }
    }
    
    /// Get current consecutive loss count
    pub fn consecutive_losses(&self) -> u32 {
        self.consecutive_losses
    }
    
    /// Get total daily loss amount
    pub fn daily_loss(&self) -> Decimal {
        self.total_daily_loss
    }
    
    /// Check if circuit breaker is active
    pub fn is_circuit_breaker_active(&self) -> bool {
        matches!(self.circuit_breaker_state, CircuitBreakerState::Open { .. })
    }
    
    /// Get circuit breaker state
    pub fn circuit_breaker_state(&self) -> &CircuitBreakerState {
        &self.circuit_breaker_state
    }
    
    /// Manually reset circuit breaker (admin function)
    pub fn reset_circuit_breaker(&mut self) {
        self.circuit_breaker_state = CircuitBreakerState::Closed;
        self.consecutive_losses = 0;
    }
    
    /// Reset daily counters (should be called at start of trading day)
    pub fn reset_daily_counters(&mut self) {
        self.total_daily_loss = Decimal::ZERO;
    }
}

impl Default for ConsecutiveLossTracker {
    fn default() -> Self {
        Self::new(3) // Default to 3 consecutive losses
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    
    #[test]
    fn test_consecutive_loss_tracking() {
        let mut tracker = ConsecutiveLossTracker::new(3);
        
        // First loss
        let action = tracker.record_trade_outcome(true, Some(dec!(100)));
        assert_eq!(action, CircuitBreakerAction::Continue);
        assert_eq!(tracker.consecutive_losses(), 1);
        
        // Second loss
        let action = tracker.record_trade_outcome(true, Some(dec!(150)));
        assert_eq!(action, CircuitBreakerAction::ContinueWithCaution);
        assert_eq!(tracker.consecutive_losses(), 2);
        
        // Third loss - should trigger circuit breaker
        let action = tracker.record_trade_outcome(true, Some(dec!(200)));
        assert_eq!(action, CircuitBreakerAction::HaltTrading);
        assert_eq!(tracker.consecutive_losses(), 3);
        assert!(tracker.is_circuit_breaker_active());
    }
    
    #[test]
    fn test_win_resets_consecutive_losses() {
        let mut tracker = ConsecutiveLossTracker::new(3);
        
        // Two losses
        tracker.record_trade_outcome(true, Some(dec!(100)));
        tracker.record_trade_outcome(true, Some(dec!(150)));
        assert_eq!(tracker.consecutive_losses(), 2);
        
        // Win resets counter
        let action = tracker.record_trade_outcome(false, None);
        assert_eq!(action, CircuitBreakerAction::Continue);
        assert_eq!(tracker.consecutive_losses(), 0);
    }
    
    #[test]
    fn test_daily_loss_tracking() {
        let mut tracker = ConsecutiveLossTracker::new(3);
        
        tracker.record_trade_outcome(true, Some(dec!(100)));
        tracker.record_trade_outcome(true, Some(dec!(200)));
        tracker.record_trade_outcome(false, None); // Win doesn't affect daily loss
        tracker.record_trade_outcome(true, Some(dec!(50)));
        
        assert_eq!(tracker.daily_loss(), dec!(350));
    }
    
    #[test]
    fn test_circuit_breaker_reset() {
        let mut tracker = ConsecutiveLossTracker::new(2);
        
        // Trigger circuit breaker
        tracker.record_trade_outcome(true, Some(dec!(100)));
        tracker.record_trade_outcome(true, Some(dec!(100)));
        assert!(tracker.is_circuit_breaker_active());
        
        // Reset circuit breaker
        tracker.reset_circuit_breaker();
        assert!(!tracker.is_circuit_breaker_active());
        assert_eq!(tracker.consecutive_losses(), 0);
    }
}