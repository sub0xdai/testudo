//! OODA Loop core implementation - The heart of Testudo's systematic trading
//!
//! Implements the Observe-Orient-Decide-Act loop for disciplined crypto trading.
//! Each phase executes with sub-200ms latency for rapid market response.

use crate::types::{OodaPhase, TradeIntent, MarketObservation, TradeSetup, ExecutionPlan, LoopMetrics};
use crate::executor::{Executor, ExecutorError, ExecutionResult};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use testudo_types::ExchangeAdapterTrait;
use thiserror::Error;
use rust_decimal_macros::dec;

/// Errors that can occur during OODA loop execution
#[derive(Debug, Error)]
pub enum OodaLoopError {
    #[error("OBSERVE phase failed: {message}")]
    ObserveFailed { message: String },
    
    #[error("ORIENT phase failed: {message}")]
    OrientFailed { message: String },
    
    #[error("DECIDE phase failed: {message}")]
    DecideFailed { message: String },
    
    #[error("ACT phase failed: {source}")]
    ActFailed { 
        #[from]
        source: ExecutorError 
    },
    
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition { from: OodaState, to: OodaState },
    
    #[error("OODA loop not configured with executor for Act phase")]
    NoExecutorConfigured,
    
    #[error("Execution plan not approved by risk management")]
    ExecutionNotApproved,
    
    #[error("Phase timeout: {phase:?} took {duration:?}, max allowed: {max_allowed:?}")]
    PhaseTimeout {
        phase: OodaPhase,
        duration: std::time::Duration,
        max_allowed: std::time::Duration,
    },
}

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
    /// Executor for Act phase implementation
    executor: Option<Arc<Executor>>,
}

impl OodaLoop {
    /// Create a new OODA loop instance in idle state
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(OodaState::Idle)),
            current_phase: Arc::new(RwLock::new(OodaPhase::Observe)),
            metrics: Arc::new(RwLock::new(LoopMetrics::new())),
            executor: None,
        }
    }
    
    /// Create a new OODA loop with executor for Act phase
    pub fn with_executor(exchange: Arc<dyn ExchangeAdapterTrait + Send + Sync>) -> Self {
        Self {
            state: Arc::new(RwLock::new(OodaState::Idle)),
            current_phase: Arc::new(RwLock::new(OodaPhase::Observe)),
            metrics: Arc::new(RwLock::new(LoopMetrics::new())),
            executor: Some(Arc::new(Executor::new(exchange))),
        }
    }

    /// Get the current state of the OODA loop
    pub async fn get_state(&self) -> OodaState {
        self.state.read().await.clone()
    }

    /// Transition to a new state with validation
    pub async fn transition_to(&self, new_state: OodaState) -> Result<(), OodaLoopError> {
        let mut state = self.state.write().await;
        
        // Validate state transition is allowed
        if !Self::is_valid_transition(&*state, &new_state) {
            return Err(OodaLoopError::InvalidStateTransition {
                from: state.clone(),
                to: new_state,
            });
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
    ) -> Result<ExecutionPlan, OodaLoopError> {
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
            // Execute through the new act() method
            self.act(execution_plan.clone()).await?;
        } else {
            // If no execution needed, transition to Completed
            self.transition_to(OodaState::Completed).await?;
        }
        
        Ok(execution_plan)
    }

    /// Observe phase - gather market data
    async fn observe_market(&self) -> Result<MarketObservation, OodaLoopError> {
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
    ) -> Result<TradeSetup, OodaLoopError> {
        // TODO: Implement situation assessment using Van Tharp calculations
        Ok(TradeSetup {
            symbol: "BTC/USDT".to_string(),
            entry_price: dec!(50000.0),
            stop_loss: dec!(49000.0),
            take_profit: Some(dec!(52000.0)),
            position_size: dec!(0.1),
            current_price: dec!(49500.0),
            r_multiple: dec!(2.0),
        })
    }

    /// Decide phase - validate against Testudo Protocol
    async fn decide_action(&self, setup: TradeSetup) -> Result<ExecutionPlan, OodaLoopError> {
        // TODO: Implement protocol validation and risk checks
        Ok(ExecutionPlan {
            setup,
            approved: true,
            risk_assessment: "Trade approved by Testudo Protocol".to_string(),
        })
    }

    /// Act phase - execute approved trade plan through exchange
    /// 
    /// This is the final phase of OODA loop - disciplined execution
    /// following Van Tharp position sizing with Roman military precision
    pub async fn act(&self, plan: ExecutionPlan) -> Result<ExecutionResult, OodaLoopError> {
        // Validate execution plan is approved
        if !plan.approved {
            return Err(OodaLoopError::ExecutionNotApproved);
        }
        
        // Start act phase metrics
        let act_start = Instant::now();
        
        // Update current phase
        {
            let mut current_phase = self.current_phase.write().await;
            *current_phase = OodaPhase::Act;
        }
        
        // Ensure we have an executor
        let executor = self.executor.as_ref()
            .ok_or(OodaLoopError::NoExecutorConfigured)?;
        
        // Transition to Acting state
        self.transition_to(OodaState::Acting).await?;
        
        // Execute the trade
        let execution_result = match executor.execute_trade(plan).await {
            Ok(result) => {
                // Update metrics on successful execution
                let mut metrics = self.metrics.write().await;
                metrics.act_duration = act_start.elapsed();
                metrics.last_execution_time = Some(result.executed_at);
                
                // Check for timeout
                let max_act_duration = std::time::Duration::from_millis(100);
                if act_start.elapsed() > max_act_duration {
                    tracing::warn!(
                        "Act phase took {}ms, exceeds target of {}ms", 
                        act_start.elapsed().as_millis(),
                        max_act_duration.as_millis()
                    );
                }
                
                result
            },
            Err(executor_error) => {
                // Transition to Failed state on error
                let error_msg = format!("Execution failed: {}", executor_error);
                self.transition_to(OodaState::Failed(error_msg)).await.ok(); // Don't fail on transition error
                return Err(OodaLoopError::ActFailed { source: executor_error });
            }
        };
        
        // Transition to Completed state on success
        self.transition_to(OodaState::Completed).await?;
        
        Ok(execution_result)
    }
    
    /// Act phase - execute the trade on exchange (deprecated, use act() instead)
    async fn act_on_decision(&self, plan: ExecutionPlan) -> Result<(), String> {
        self.act(plan).await.map(|_| ()).map_err(|e| e.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TradeSetup;
    use prudentia::exchange::MockExchange;
    use rust_decimal_macros::dec;
    use testudo_types::AccountBalance;
    
    fn create_test_execution_plan(approved: bool) -> ExecutionPlan {
        ExecutionPlan {
            setup: TradeSetup {
                symbol: "BTCUSDT".to_string(),
                entry_price: dec!(50000.0),
                stop_loss: dec!(49000.0),
                take_profit: Some(dec!(52000.0)),
                position_size: dec!(0.01),
                current_price: dec!(49500.0),
                r_multiple: dec!(2.0),
            },
            approved,
            risk_assessment: "Test assessment".to_string(),
        }
    }
    
    #[tokio::test]
    async fn test_ooda_loop_creation() {
        let loop_instance = OodaLoop::new();
        assert_eq!(loop_instance.get_state().await, OodaState::Idle);
    }
    
    #[tokio::test]
    async fn test_ooda_loop_with_executor() {
        let exchange = Arc::new(MockExchange::new());
        let loop_instance = OodaLoop::with_executor(exchange);
        assert_eq!(loop_instance.get_state().await, OodaState::Idle);
        assert!(loop_instance.executor.is_some());
    }
    
    #[tokio::test]
    async fn test_state_transitions() {
        let loop_instance = OodaLoop::new();
        
        // Valid transition: Idle -> Observing
        assert!(loop_instance.transition_to(OodaState::Observing).await.is_ok());
        assert_eq!(loop_instance.get_state().await, OodaState::Observing);
        
        // Valid transition: Observing -> Orienting
        assert!(loop_instance.transition_to(OodaState::Orienting).await.is_ok());
        assert_eq!(loop_instance.get_state().await, OodaState::Orienting);
        
        // Invalid transition: Orienting -> Observing
        let result = loop_instance.transition_to(OodaState::Observing).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            OodaLoopError::InvalidStateTransition { from, to } => {
                assert_eq!(from, OodaState::Orienting);
                assert_eq!(to, OodaState::Observing);
            },
            _ => panic!("Expected InvalidStateTransition error"),
        }
    }
    
    #[tokio::test]
    async fn test_act_without_executor() {
        let loop_instance = OodaLoop::new();
        let plan = create_test_execution_plan(true);
        
        let result = loop_instance.act(plan).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            OodaLoopError::NoExecutorConfigured => {},
            _ => panic!("Expected NoExecutorConfigured error"),
        }
    }
    
    #[tokio::test]
    async fn test_act_with_unapproved_plan() {
        let exchange = Arc::new(MockExchange::new());
        let loop_instance = OodaLoop::with_executor(exchange);
        let plan = create_test_execution_plan(false); // Not approved
        
        let result = loop_instance.act(plan).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            OodaLoopError::ExecutionNotApproved => {},
            _ => panic!("Expected ExecutionNotApproved error"),
        }
    }
    
    #[tokio::test]
    async fn test_act_successful_execution() {
        let mut mock_exchange = MockExchange::new();
        
        // Setup mock exchange for successful execution
        mock_exchange.set_health(true);
        // Note: MockExchange doesn't have add_supported_symbol, symbols are always supported
        mock_exchange.set_balance("BTC".to_string(), AccountBalance {
            asset: "BTC".to_string(),
            free: dec!(1.0),
            locked: dec!(0.0),
            total: dec!(1.0),
        });
        
        let exchange = Arc::new(mock_exchange);
        let loop_instance = OodaLoop::with_executor(exchange);
        let plan = create_test_execution_plan(true);
        
        // Must be in Deciding state before Acting
        loop_instance.transition_to(OodaState::Deciding).await.unwrap();
        
        let result = loop_instance.act(plan).await;
        assert!(result.is_ok());
        
        // Should be in Completed state after successful execution
        assert_eq!(loop_instance.get_state().await, OodaState::Completed);
        
        let execution_result = result.unwrap();
        assert!(execution_result.execution_time_ms > 0);
    }
    
    #[tokio::test]
    async fn test_end_to_end_ooda_loop() {
        let mut mock_exchange = MockExchange::new();
        
        // Setup mock exchange
        mock_exchange.set_health(true);
        // Note: MockExchange doesn't have add_supported_symbol, symbols are always supported
        mock_exchange.set_balance("BTC".to_string(), AccountBalance {
            asset: "BTC".to_string(),
            free: dec!(1.0),
            locked: dec!(0.0),
            total: dec!(1.0),
        });
        
        let exchange = Arc::new(mock_exchange);
        let loop_instance = OodaLoop::with_executor(exchange);
        
        // Start with Idle state
        assert_eq!(loop_instance.get_state().await, OodaState::Idle);
        
        // OBSERVE: Transition to observing
        loop_instance.transition_to(OodaState::Observing).await.unwrap();
        assert_eq!(loop_instance.get_state().await, OodaState::Observing);
        
        // ORIENT: Transition to orienting  
        loop_instance.transition_to(OodaState::Orienting).await.unwrap();
        assert_eq!(loop_instance.get_state().await, OodaState::Orienting);
        
        // DECIDE: Transition to deciding
        loop_instance.transition_to(OodaState::Deciding).await.unwrap();
        assert_eq!(loop_instance.get_state().await, OodaState::Deciding);
        
        // ACT: Execute approved plan
        let plan = create_test_execution_plan(true);
        let execution_result = loop_instance.act(plan).await.unwrap();
        
        // Should be in Completed state
        assert_eq!(loop_instance.get_state().await, OodaState::Completed);
        assert!(execution_result.execution_time_ms > 0);
        
        // Can reset back to Idle for next cycle
        loop_instance.transition_to(OodaState::Idle).await.unwrap();
        assert_eq!(loop_instance.get_state().await, OodaState::Idle);
    }
    
    #[tokio::test]
    async fn test_ooda_controller() {
        let controller = OodaController::new();
        assert_eq!(controller.active_loop_count().await, 0);
        
        let loop1 = Arc::new(OodaLoop::new());
        let loop2 = Arc::new(OodaLoop::new());
        
        controller.register_loop(loop1).await;
        assert_eq!(controller.active_loop_count().await, 1);
        
        controller.register_loop(loop2).await;
        assert_eq!(controller.active_loop_count().await, 2);
    }
    
    #[tokio::test]
    async fn test_error_recovery() {
        let exchange = Arc::new(MockExchange::new()); // Unhealthy exchange
        let loop_instance = OodaLoop::with_executor(exchange);
        let plan = create_test_execution_plan(true);
        
        // Set to deciding state
        loop_instance.transition_to(OodaState::Deciding).await.unwrap();
        
        // Execution should fail due to unhealthy exchange
        let result = loop_instance.act(plan).await;
        assert!(result.is_err());
        
        // Should be in Failed state
        let state = loop_instance.get_state().await;
        match state {
            OodaState::Failed(_) => {},
            _ => panic!("Expected Failed state, got {:?}", state),
        }
        
        // Can recover by transitioning back to Idle
        loop_instance.transition_to(OodaState::Idle).await.unwrap();
        assert_eq!(loop_instance.get_state().await, OodaState::Idle);
    }
}