//! Formatio - OODA Loop Trading Operations

use thiserror::Error;
use std::sync::Arc;

// 1. Module Declarations
pub mod decider;
pub mod executor;
pub mod ooda;
pub mod orientator;
pub mod types;

// 2. Consolidated Error Type for Imperium Integration
/// Consolidated error type for all Formatio operations
#[derive(Debug, Error)]
pub enum FormatioError {
    #[error("OODA loop error: {source}")]
    OodaLoopError {
        #[from]
        source: OodaLoopError,
    },
    
    #[error("Execution error: {source}")]
    ExecutorError {
        #[from]
        source: ExecutorError,
    },
    
    #[error("Orientation error: {source}")]
    OrientationError {
        #[from]
        source: OrientationError,
    },
    
    #[error("Decision error: {source}")]
    DecisionError {
        #[from]
        source: DecisionError,
    },
    
    #[error("Observation failure: {reason}")]
    ObservationFailure { reason: String },
    
    #[error("Stale market data: symbol {symbol}, age {age_ms}ms exceeds limit")]
    StaleMarketData { symbol: String, age_ms: u64 },
    
    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },
}

// 3. Controller Type for Imperium Integration
/// Controller interface for OODA loop operations
/// Provides high-level control and coordination for trading operations
pub struct OodaController {
    ooda_loop: Arc<OodaLoop>,
}

impl OodaController {
    /// Create a new OODA controller with the given OODA loop
    pub fn new(ooda_loop: Arc<OodaLoop>) -> Self {
        Self { ooda_loop }
    }
    
    /// Get the current state of the OODA loop
    pub async fn current_state(&self) -> OodaState {
        self.ooda_loop.get_state().await
    }
    
    /// Execute a complete OODA cycle with the given trade intent
    pub async fn execute_cycle(&self, intent: TradeIntent) -> Result<ExecutionPlan, FormatioError> {
        self.ooda_loop.execute_cycle(intent).await
            .map_err(FormatioError::from)
    }
    
    /// Force transition to a specific state (for testing/recovery)
    pub async fn force_state_transition(&self, new_state: OodaState) -> Result<(), FormatioError> {
        self.ooda_loop.transition_to(new_state).await
            .map_err(FormatioError::from)
    }
}

// 4. Public API Exports
pub use decider::{DecisionResult, RiskDecision, RiskDecider};
pub use executor::{ExecutionResult, Executor, ExecutorError};
pub use ooda::{OodaLoop, OodaLoopError, OodaState};
pub use orientator::{OrientationError, PositionOrientator, TradeOrientation};
pub use types::{
    DecisionError,
    ExecutionPlan,
    LoopMetrics,
    MarketObservation,
    OodaPhase,
    TradeDirection,
    TradeIntent,
    TradeProposal,
    TradeSetup,
};