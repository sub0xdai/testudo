//! OODA Loop core implementation - The heart of Testudo's systematic trading

use crate::decider::{RiskDecision, RiskDecider};
use crate::executor::{ExecutionResult, Executor, ExecutorError};
use crate::orientator::{OrientationError, PositionOrientator};
use crate::types::{
    ExecutionPlan, LoopMetrics, MarketObservation, OodaPhase, TradeDirection, TradeIntent,
    TradeSetup,
};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use std::sync::Arc;
use testudo_types::ExchangeAdapterTrait;
use thiserror::Error;
use tokio::sync::RwLock;

/// Errors that can occur during OODA loop execution
#[derive(Debug, Error)]
pub enum OodaLoopError {
    #[error("OBSERVE phase failed: {message}")]
    ObserveFailed { message: String },
    #[error("ORIENT phase failed: {source}")]
    OrientFailed {
        #[from]
        source: OrientationError,
    },
    #[error("DECIDE phase failed: {message}")]
    DecideFailed { message: String },
    #[error("ACT phase failed: {source}")]
    ActFailed {
        #[from]
        source: ExecutorError,
    },
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition { from: OodaState, to: OodaState },
    #[error("OODA loop not configured with executor for Act phase")]
    NoExecutorConfigured,
    #[error("OODA loop not configured with orientator for Orient phase")]
    NoOrientatorConfigured,
    #[error("Execution plan not approved by risk management")]
    ExecutionNotApproved,
}

/// State machine representing the current phase of the OODA loop
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OodaState {
    Idle,
    Observing,
    Orienting,
    Deciding,
    Acting,
    Completed,
    Failed(String),
}

/// Core OODA Loop implementation following Roman military discipline
pub struct OodaLoop {
    state: Arc<RwLock<OodaState>>,
    metrics: Arc<RwLock<LoopMetrics>>,
    executor: Option<Arc<Executor>>,
    orientator: Option<Arc<PositionOrientator>>,
    decider: Option<Arc<RiskDecider>>,
    exchange: Option<Arc<dyn ExchangeAdapterTrait + Send + Sync>>,
}

impl OodaLoop {
    pub fn with_all_components(
        exchange: Arc<dyn ExchangeAdapterTrait + Send + Sync>,
        decider: Arc<RiskDecider>,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(OodaState::Idle)),
            metrics: Arc::new(RwLock::new(LoopMetrics::new())),
            executor: Some(Arc::new(Executor::new(exchange.clone()))),
            orientator: Some(Arc::new(PositionOrientator::new())),
            decider: Some(decider),
            exchange: Some(exchange),
        }
    }

    pub async fn get_state(&self) -> OodaState {
        self.state.read().await.clone()
    }

    pub async fn transition_to(&self, new_state: OodaState) -> Result<(), OodaLoopError> {
        let mut state = self.state.write().await;
        if !Self::is_valid_transition(&*state, &new_state) {
            return Err(OodaLoopError::InvalidStateTransition {
                from: state.clone(),
                to: new_state,
            });
        }
        *state = new_state;
        Ok(())
    }

    fn is_valid_transition(from: &OodaState, to: &OodaState) -> bool {
        use OodaState::*;
        match (from, to) {
            (Idle, Observing) => true,
            (Observing, Orienting) | (Observing, Failed(_)) => true,
            (Orienting, Deciding) | (Orienting, Failed(_)) => true,
            (Deciding, Acting) | (Deciding, Completed) | (Deciding, Failed(_)) => true,
            (Acting, Completed) | (Acting, Failed(_)) => true,
            (Completed, Idle) | (Failed(_), Idle) => true,
            _ => false,
        }
    }

    pub async fn execute_cycle(
        &self,
        intent: TradeIntent,
    ) -> Result<ExecutionPlan, OodaLoopError> {
        self.transition_to(OodaState::Observing).await?;
        let observation = self.observe_market_for_symbol(&intent.symbol).await?;

        self.transition_to(OodaState::Orienting).await?;
        let trade_setup = self.orient_situation(&observation, &intent).await?;

        self.transition_to(OodaState::Deciding).await?;
        let execution_plan = self.decide_action(trade_setup, &intent).await?;

        if execution_plan.approved {
            self.act(execution_plan.clone()).await?;
        } else {
            self.transition_to(OodaState::Completed).await?;
        }

        Ok(execution_plan)
    }

    async fn observe_market_for_symbol(
        &self,
        symbol: &str,
    ) -> Result<MarketObservation, OodaLoopError> {
        let exchange = self.exchange.as_ref().ok_or_else(|| {
            OodaLoopError::ObserveFailed { message: "Exchange adapter not configured".to_string() }
        })?;
        let market_data = exchange
            .get_market_data(symbol)
            .await
            .map_err(|e| OodaLoopError::ObserveFailed {
                message: format!("Failed to get market data: {:?}", e),
            })?;
        Ok(MarketObservation {
            symbol: market_data.symbol,
            price: market_data.last_price.to_f64().unwrap_or(0.0),
            volume: market_data.volume_24h.to_f64().unwrap_or(0.0),
            timestamp: std::time::Instant::now(),
        })
    }

    async fn orient_situation(
        &self,
        observation: &MarketObservation,
        intent: &TradeIntent,
    ) -> Result<TradeSetup, OodaLoopError> {
        let orientator = self
            .orientator
            .as_ref()
            .ok_or(OodaLoopError::NoOrientatorConfigured)?;
        let stop_loss_distance_percent = dec!(0.02);
        let orientation_result = orientator
            .orient(
                observation,
                self,
                intent.account_equity,
                intent.risk_percentage,
                stop_loss_distance_percent,
            )
            .await?;
        let proposal = orientation_result.proposal;
        Ok(TradeSetup {
            symbol: proposal.symbol,
            entry_price: proposal.entry_price,
            stop_loss: proposal.stop_loss,
            take_profit: proposal.take_profit,
            position_size: proposal.position_size,
            side: proposal.side,
        })
    }

    async fn decide_action(&self, setup: TradeSetup, intent: &TradeIntent) -> Result<ExecutionPlan, OodaLoopError> {
        let decider = self.decider.as_ref().ok_or_else(|| {
            OodaLoopError::DecideFailed { message: "Risk decider not configured".to_string() }
        })?;
        
        use disciplina::{AccountEquity, RiskPercentage, PricePoint};
        use prudentia::types::TradeSide;
        use testudo_types::OrderSide;
        use std::time::SystemTime;
        use uuid::Uuid;
        
        // Convert OrderSide to TradeSide
        let trade_side = match setup.side {
            OrderSide::Buy => TradeSide::Long,
            OrderSide::Sell => TradeSide::Short,
        };
        
        let trade_proposal = prudentia::types::TradeProposal {
            id: Uuid::new_v4(),
            symbol: setup.symbol.clone(),
            side: trade_side,
            entry_price: PricePoint::new(setup.entry_price).map_err(|e| 
                OodaLoopError::DecideFailed { message: format!("Invalid entry price: {:?}", e) })?,
            stop_loss: PricePoint::new(setup.stop_loss).map_err(|e| 
                OodaLoopError::DecideFailed { message: format!("Invalid stop loss: {:?}", e) })?,
            take_profit: match setup.take_profit {
                Some(tp) => Some(PricePoint::new(tp).map_err(|e| 
                    OodaLoopError::DecideFailed { message: format!("Invalid take profit: {:?}", e) })?),
                None => None,
            },
            account_equity: AccountEquity::new(intent.account_equity).map_err(|e| 
                OodaLoopError::DecideFailed { message: format!("Invalid account equity: {:?}", e) })?,
            risk_percentage: RiskPercentage::new(intent.risk_percentage).map_err(|e| 
                OodaLoopError::DecideFailed { message: format!("Invalid risk percentage: {:?}", e) })?,
            timestamp: SystemTime::now(),
            metadata: None,
        };
        let decision_result = decider
            .decide_trade(trade_proposal)
            .await
            .map_err(|e| OodaLoopError::DecideFailed {
                message: format!("Risk decision failed: {:?}", e),
            })?;
        match decision_result.decision {
            RiskDecision::Execute { approved_position_size, .. } => {
                let mut approved_setup = setup;
                approved_setup.position_size = approved_position_size;
                Ok(ExecutionPlan {
                    setup: approved_setup,
                    approved: true,
                    risk_assessment: "Trade approved by Testudo Protocol".to_string(),
                })
            }
            RiskDecision::Reject { rejection_reason, .. } => Ok(ExecutionPlan {
                setup,
                approved: false,
                risk_assessment: format!("Trade rejected: {}", rejection_reason),
            }),
            RiskDecision::AssessmentFailed { error_details } => {
                Err(OodaLoopError::DecideFailed {
                    message: format!("Risk assessment failed: {}", error_details),
                })
            }
        }
    }

    async fn act(&self, plan: ExecutionPlan) -> Result<ExecutionResult, OodaLoopError> {
        if !plan.approved {
            return Err(OodaLoopError::ExecutionNotApproved);
        }
        let executor = self.executor.as_ref().ok_or(OodaLoopError::NoExecutorConfigured)?;
        self.transition_to(OodaState::Acting).await?;
        let execution_result = executor.execute_trade(plan).await?;
        self.transition_to(OodaState::Completed).await?;
        Ok(execution_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TradeDirection;
    use prudentia::exchange::MockExchange;
    use prudentia::risk::{rules::MaxTradeRiskRule, PortfolioState, RiskManagementProtocol};
    use rust_decimal_macros::dec;
    use testudo_types::AccountBalance;

    #[tokio::test]
    async fn test_full_ooda_cycle_integration() {
        // 1. Setup
        let mut mock_exchange = MockExchange::new();
        mock_exchange.set_health(true);
        mock_exchange.set_balance(
            "USDT".to_string(),
            AccountBalance {
                asset: "USDT".to_string(),
                free: dec!(10000.0),
                locked: dec!(0.0),
                total: dec!(10000.0),
            },
        );
        mock_exchange.set_market_data("BTC/USDT", dec!(50000.0), dec!(100.0));
        let exchange = Arc::new(mock_exchange);

        let protocol = Arc::new(RiskManagementProtocol::new(
            vec![Box::new(MaxTradeRiskRule::new(dec!(0.06)))],
            Arc::new(RwLock::new(PortfolioState::new(dec!(10000.0)))),
        ));
        let decider = Arc::new(RiskDecider::new(protocol));

        let loop_instance = OodaLoop::with_all_components(exchange, decider);

        // 2. Define Intent
        let intent = TradeIntent {
            symbol: "BTC/USDT".to_string(),
            direction: TradeDirection::Long,
            account_equity: dec!(10000.0),
            risk_percentage: dec!(0.02),
        };

        // 3. Execute
        let result = loop_instance.execute_cycle(intent).await;

        // 4. Assert
        assert!(result.is_ok(), "Full OODA cycle failed: {:?}", result.err());
        let plan = result.unwrap();
        assert!(plan.approved);
        assert_eq!(loop_instance.get_state().await, OodaState::Completed);
        // Expected position size: (10000 * 0.02) / (50000 * 0.02) = 200 / 1000 = 0.2
        assert_eq!(plan.setup.position_size, dec!(0.2));
    }
}