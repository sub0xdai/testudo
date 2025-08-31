//! OODA types - Core types for the trading loop

use std::time::{Duration, Instant};

/// User's intent to trade
#[derive(Debug, Clone)]
pub struct TradeIntent {
    pub symbol: String,
    pub direction: TradeDirection,
}

/// Market observation data
#[derive(Debug, Clone)]
pub struct MarketObservation {
    pub symbol: String,
    pub price: f64,
    pub volume: f64,
    pub timestamp: Instant,
}

/// Validated trade setup with position sizing
#[derive(Debug, Clone)]
pub struct TradeSetup {
    pub symbol: String,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub take_profit: Option<f64>,
    pub position_size: f64,
}

/// Final execution plan after risk validation
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub setup: TradeSetup,
    pub approved: bool,
    pub risk_assessment: String,
}

/// Trade direction
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeDirection {
    Long,
    Short,
}

/// Current phase of the OODA loop
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OodaPhase {
    Observe,
    Orient,
    Decide,
    Act,
}

/// Performance metrics for the OODA loop
#[derive(Debug, Clone)]
pub struct LoopMetrics {
    pub observe_latency: Option<Duration>,
    pub orient_latency: Option<Duration>,
    pub decide_latency: Option<Duration>,
    pub act_latency: Option<Duration>,
    pub total_latency: Option<Duration>,
    pub last_updated: Instant,
}

impl LoopMetrics {
    /// Create new metrics instance
    pub fn new() -> Self {
        Self {
            observe_latency: None,
            orient_latency: None,
            decide_latency: None,
            act_latency: None,
            total_latency: None,
            last_updated: Instant::now(),
        }
    }

    /// Check if all latencies meet the target (<200ms total)
    pub fn meets_performance_targets(&self) -> bool {
        if let Some(total) = self.total_latency {
            total < Duration::from_millis(200)
        } else {
            false
        }
    }
}

impl Default for LoopMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Error types for the decision process in Phase 4 (Decide) of OODA loop
#[derive(Debug, Clone, thiserror::Error)]
pub enum DecisionError {
    /// Protocol assessment error
    #[error("Risk protocol error: {0}")]
    ProtocolError(String),
    
    /// Decision timeout exceeded
    #[error("Decision timeout: {0}")]
    AssessmentTimeout(String),
    
    /// Invalid trade proposal format
    #[error("Invalid proposal: {0}")]
    InvalidProposal(String),
    
    /// System error during decision process
    #[error("Decision system error: {0}")]
    SystemError(String),
    
    /// Type conversion error
    #[error("Type conversion error: {0}")]
    ConversionError(String),
}
impl From<disciplina::PositionSizingError> for DecisionError {
    fn from(err: disciplina::PositionSizingError) -> Self {
        DecisionError::ConversionError(err.to_string())
    }
}
