//! Formatio - OODA Loop Trading Operations

// 1. Module Declarations
pub mod decider;
pub mod executor;
pub mod ooda;
pub mod orientator;
pub mod types;

// 2. Public API Exports
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