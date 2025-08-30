//! Formatio - OODA Loop Trading Operations
//!
//! This crate implements the OODA Loop (Observe, Orient, Decide, Act) pattern for
//! systematic trade execution. Every trade follows the same disciplined process
//! maintaining formation integrity under all market conditions.
//!
//! ## OODA Loop Phases
//!
//! 1. **OBSERVE**: Market data ingestion and validation (<5 seconds freshness)
//! 2. **ORIENT**: Van Tharp position sizing and risk/reward analysis  
//! 3. **DECIDE**: Testudo Protocol validation and risk approval
//! 4. **ACT**: Exchange order execution with confirmation (<200ms target)
//!
//! ## Performance Requirements
//!
//! - Complete OODA cycle: <200ms from initiation to exchange confirmation
//! - Market data freshness: <5 seconds maximum age
//! - Decision validation: <50ms for risk calculations
//! - Order execution: <150ms average to exchange
//!
//! ## Roman Military Principle: Formatio
//!
//! Systematic formation and execution. Every trade follows the same disciplined
//! process, maintaining formation integrity under all market conditions.

pub mod ooda;
pub mod observer;
pub mod orientator;
pub mod decider;
pub mod executor;
pub mod types;
pub mod metrics;

pub use ooda::{OodaLoop, OodaController};
pub use observer::{MarketObserver, ObservationResult};
pub use orientator::{PositionOrientator, TradeOrientation};
pub use decider::{RiskDecider, DecisionMatrix};
pub use executor::{OrderExecutor, ExecutionResult};
pub use types::{
    TradeIntent, MarketObservation, TradeSetup, 
    ExecutionPlan, OodaPhase, LoopMetrics
};

use disciplina::PositionSizingError;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Formatio execution errors with phase context
#[derive(Debug, Error, Clone)]
pub enum FormatioError {
    #[error("OBSERVE phase failed: {reason}")]
    ObservationFailure { reason: String },
    
    #[error("ORIENT phase failed: {source}")]
    OrientationFailure { source: PositionSizingError },
    
    #[error("DECIDE phase blocked: {violation}")]
    DecisionBlocked { violation: String },
    
    #[error("ACT phase failed: {reason}")]
    ExecutionFailure { reason: String },
    
    #[error("OODA loop timeout: {phase:?} exceeded {max_duration:?}")]
    PhaseTimeout { phase: OodaPhase, max_duration: Duration },
    
    #[error("Market data stale: {age:?} exceeds maximum {max_age:?}")]
    StaleMarketData { age: Duration, max_age: Duration },
    
    #[error("Circuit breaker triggered: {reason}")]
    CircuitBreakerTriggered { reason: String },
}

/// Result type for all Formatio operations
pub type Result<T> = std::result::Result<T, FormatioError>;

/// Core trait for OODA loop execution
#[async_trait::async_trait]
pub trait OodaExecutor {
    /// Execute complete OODA loop for a trade intent
    async fn execute_ooda_loop(&self, intent: TradeIntent) -> Result<ExecutionResult>;
    
    /// Get current loop metrics and performance statistics
    async fn get_loop_metrics(&self) -> LoopMetrics;
    
    /// Check if circuit breakers are active
    async fn circuit_breaker_status(&self) -> bool;
}

/// Configuration for OODA loop timing and thresholds
#[derive(Debug, Clone)]
pub struct OodaConfig {
    /// Maximum time for complete OODA cycle
    pub max_loop_duration: Duration,
    
    /// Maximum age for market data before considered stale
    pub max_market_data_age: Duration,
    
    /// Maximum time for each phase
    pub max_observe_duration: Duration,
    pub max_orient_duration: Duration, 
    pub max_decide_duration: Duration,
    pub max_act_duration: Duration,
    
    /// Circuit breaker thresholds
    pub max_consecutive_failures: u32,
    pub circuit_breaker_timeout: Duration,
}

impl Default for OodaConfig {
    fn default() -> Self {
        Self {
            max_loop_duration: Duration::from_millis(200),
            max_market_data_age: Duration::from_secs(5),
            max_observe_duration: Duration::from_millis(20),
            max_orient_duration: Duration::from_millis(50),
            max_decide_duration: Duration::from_millis(30),
            max_act_duration: Duration::from_millis(100),
            max_consecutive_failures: 3,
            circuit_breaker_timeout: Duration::from_secs(5 * 60),
        }
    }
}

/// Timing information for OODA loop phases
#[derive(Debug, Clone)]
pub struct OodaTiming {
    pub start_time: Instant,
    pub observe_duration: Duration,
    pub orient_duration: Duration,
    pub decide_duration: Duration,
    pub act_duration: Duration,
    pub total_duration: Duration,
}

impl OodaTiming {
    /// Check if any phase exceeded its maximum allowed duration
    pub fn check_phase_timeouts(&self, config: &OodaConfig) -> Result<()> {
        if self.observe_duration > config.max_observe_duration {
            return Err(FormatioError::PhaseTimeout {
                phase: OodaPhase::Observe,
                max_duration: config.max_observe_duration,
            });
        }
        
        if self.orient_duration > config.max_orient_duration {
            return Err(FormatioError::PhaseTimeout {
                phase: OodaPhase::Orient,
                max_duration: config.max_orient_duration,
            });
        }
        
        if self.decide_duration > config.max_decide_duration {
            return Err(FormatioError::PhaseTimeout {
                phase: OodaPhase::Decide,
                max_duration: config.max_decide_duration,
            });
        }
        
        if self.act_duration > config.max_act_duration {
            return Err(FormatioError::PhaseTimeout {
                phase: OodaPhase::Act,
                max_duration: config.max_act_duration,
            });
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_ooda_timing_validation() {
        let config = OodaConfig::default();
        let timing = OodaTiming {
            start_time: Instant::now(),
            observe_duration: Duration::from_millis(10),
            orient_duration: Duration::from_millis(25),
            decide_duration: Duration::from_millis(15),
            act_duration: Duration::from_millis(80),
            total_duration: Duration::from_millis(130),
        };
        
        // Should pass with all phases under limits
        assert!(timing.check_phase_timeouts(&config).is_ok());
    }
    
    #[tokio::test]
    async fn test_ooda_timeout_detection() {
        let config = OodaConfig::default();
        let timing = OodaTiming {
            start_time: Instant::now(),
            observe_duration: Duration::from_millis(25), // Exceeds 20ms limit
            orient_duration: Duration::from_millis(25),
            decide_duration: Duration::from_millis(15),
            act_duration: Duration::from_millis(80),
            total_duration: Duration::from_millis(145),
        };
        
        // Should fail due to observe phase timeout
        let result = timing.check_phase_timeouts(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            FormatioError::PhaseTimeout { phase: OodaPhase::Observe, .. } => {},
            _ => panic!("Expected observe phase timeout error"),
        }
    }
}