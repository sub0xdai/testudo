//! OODA Loop core implementation - The heart of Testudo's systematic trading
//!
//! Implements the Observe-Orient-Decide-Act loop for disciplined crypto trading.
//! Each phase executes with sub-200ms latency for rapid market response.

use crate::types::{OodaPhase, TradeIntent, MarketObservation, TradeSetup, ExecutionPlan, LoopMetrics};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// State machine representing the current phase of the OODA loop
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OodaState {
    /// Initial state, ready to begin observation
    Idle,
    /// Actively observing market conditions
    Observing,
    /// Orienting to assess situation and calculate positions
    Orienting,
    /// Deciding whether to execute based on risk protocols
    Deciding,
    /// Acting by executing the trade on exchange
    Acting,
    /// Successfully completed the OODA cycle
    Completed,
    /// Failed during execution, contains error context
    Failed(String),
}

/// Core OODA Loop implementation following Roman military discipline
pub struct OodaLoop {
    /// Current state of the OODA loop
    state: Arc<RwLock<OodaState>>,
    /// Current phase for metrics tracking
    current_phase: Arc<RwLock<OodaPhase>>,
    /// Performance metrics for monitoring latency
    metrics: Arc<RwLock<LoopMetrics>>,
}

impl OodaLoop {
    /// Create a new OODA loop instance in idle state
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(OodaState::Idle)),
            current_phase: Arc::new(RwLock::new(OodaPhase::Observe)),
            metrics: Arc::new(RwLock::new(LoopMetrics::new())),
        }
    }

    /// Get the current state of the OODA loop
    pub async fn get_state(&self) -> OodaState {
        self.state.read().await.clone()
    }

    /// Transition to a new state with validation
    pub async fn transition_to(&self, new_state: OodaState) -> Result<(), String> {
        let mut state = self.state.write().await;
        
        // Validate state transition is allowed
        if !Self::is_valid_transition(&*state, &new_state) {
            return Err(format!(
                "Invalid state transition from {:?} to {:?}",
                *state, new_state
            ));
        }
        
        *state = new_state;
        Ok(())
    }

    /// Validate if a state transition is allowed
    fn is_valid_transition(from: &OodaState, to: &OodaState) -> bool {
        use OodaState::*;
        
        match (from, to) {
            // From Idle, can only start Observing
            (Idle, Observing) => true,
            // From Observing, can Orient or Fail
            (Observing, Orienting) | (Observing, Failed(_)) => true,
            // From Orienting, can Decide or Fail
            (Orienting, Deciding) | (Orienting, Failed(_)) => true,
            // From Deciding, can Act, Complete (if no action needed), or Fail
            (Deciding, Acting) | (Deciding, Completed) | (Deciding, Failed(_)) => true,
            // From Acting, can Complete or Fail
            (Acting, Completed) | (Acting, Failed(_)) => true,
            // Can reset from Completed or Failed back to Idle
            (Completed, Idle) | (Failed(_), Idle) => true,
            // All other transitions are invalid
            _ => false,
        }
    }

    /// Execute the complete OODA cycle
    pub async fn execute_cycle(
        &self,
        _intent: TradeIntent,
    ) -> Result<ExecutionPlan, String> {
        // Transition to Observing
        self.transition_to(OodaState::Observing).await?;
        
        // TODO: Implement actual observation logic
        let observation = self.observe_market().await?;
        
        // Transition to Orienting
        self.transition_to(OodaState::Orienting).await?;
        let trade_setup = self.orient_situation(observation).await?;
        
        // Transition to Deciding
        self.transition_to(OodaState::Deciding).await?;
        let execution_plan = self.decide_action(trade_setup).await?;
        
        // Check if action is needed
        if self.should_execute(&execution_plan).await {
            // Transition to Acting
            self.transition_to(OodaState::Acting).await?;
            self.act_on_decision(execution_plan.clone()).await?;
        }
        
        // Transition to Completed
        self.transition_to(OodaState::Completed).await?;
        
        Ok(execution_plan)
    }

    /// Observe phase - gather market data
    async fn observe_market(&self) -> Result<MarketObservation, String> {
        // TODO: Implement actual market observation
        Ok(MarketObservation {
            symbol: "BTC/USDT".to_string(),
            price: 50000.0,
            volume: 100.0,
            timestamp: std::time::Instant::now(),
        })
    }

    /// Orient phase - assess situation and calculate position
    async fn orient_situation(
        &self,
        _observation: MarketObservation,
    ) -> Result<TradeSetup, String> {
        // TODO: Implement situation assessment using Van Tharp calculations
        Ok(TradeSetup {
            symbol: "BTC/USDT".to_string(),
            entry_price: 50000.0,
            stop_loss: 49000.0,
            take_profit: Some(52000.0),
            position_size: 0.1,
        })
    }

    /// Decide phase - validate against Testudo Protocol
    async fn decide_action(&self, setup: TradeSetup) -> Result<ExecutionPlan, String> {
        // TODO: Implement protocol validation and risk checks
        Ok(ExecutionPlan {
            setup,
            approved: true,
            risk_assessment: "Trade approved by Testudo Protocol".to_string(),
        })
    }

    /// Act phase - execute the trade on exchange
    async fn act_on_decision(&self, _plan: ExecutionPlan) -> Result<(), String> {
        // TODO: Implement exchange execution
        Ok(())
    }

    /// Check if execution should proceed
    async fn should_execute(&self, _plan: &ExecutionPlan) -> bool {
        // TODO: Implement final safety checks
        true
    }
}

impl Default for OodaLoop {
    fn default() -> Self {
        Self::new()
    }
}

/// Controller for managing multiple OODA loops
pub struct OodaController {
    /// Active OODA loops for different trading pairs
    loops: Arc<RwLock<Vec<Arc<OodaLoop>>>>,
}

impl OodaController {
    /// Create a new OODA controller
    pub fn new() -> Self {
        Self {
            loops: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a new OODA loop
    pub async fn register_loop(&self, loop_instance: Arc<OodaLoop>) {
        let mut loops = self.loops.write().await;
        loops.push(loop_instance);
    }

    /// Get the count of active loops
    pub async fn active_loop_count(&self) -> usize {
        self.loops.read().await.len()
    }
}

impl Default for OodaController {
    fn default() -> Self {
        Self::new()
    }
}