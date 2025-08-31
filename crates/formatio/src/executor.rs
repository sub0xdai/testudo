//! Trade executor for OODA loop - Phase 4 (Act)

use crate::types::{ExecutionPlan, TradeSetup};
use chrono::Utc;
use testudo_types::{
    ExchangeAdapterTrait, OrderResult, OrderStatus, OrderType, TradeOrder,
};
use thiserror::Error;
use uuid::Uuid;

/// Errors that can occur during trade execution.
#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("Exchange reported an error: {0}")]
    ExchangeError(String),
    #[error("Order execution timed out after {0:?}")]
    Timeout(std::time::Duration),
    #[error("Pre-flight check failed: {0}")]
    PreFlightCheckFailed(String),
}

/// The result of a trade execution.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub order_id: String,
    pub status: OrderStatus,
    pub executed_at: chrono::DateTime<Utc>,
    pub execution_time_ms: u64,
}

/// The Executor component for the OODA loop's Act phase.
pub struct Executor {
    exchange: std::sync::Arc<dyn ExchangeAdapterTrait + Send + Sync>,
}

impl Executor {
    pub fn new(exchange: std::sync::Arc<dyn ExchangeAdapterTrait + Send + Sync>) -> Self {
        Self { exchange }
    }

    pub async fn execute_trade(
        &self,
        plan: ExecutionPlan,
    ) -> Result<ExecutionResult, ExecutorError> {
        let start_time = std::time::Instant::now();

        self.run_pre_flight_checks(&plan.setup).await?;

        let trade_order = self.create_trade_order(&plan.setup)?;

        let order_result = self
            .exchange
            .place_order(&trade_order)
            .await
            .map_err(|e| ExecutorError::ExchangeError(e.to_string()))?;

        Ok(ExecutionResult {
            order_id: order_result.order_id,
            status: order_result.status,
            executed_at: Utc::now(),
            execution_time_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    async fn run_pre_flight_checks(&self, setup: &TradeSetup) -> Result<(), ExecutorError> {
        if !self.exchange.health_check().await.unwrap_or(false) {
            return Err(ExecutorError::PreFlightCheckFailed(
                "Exchange is not healthy".to_string(),
            ));
        }
        if !self.exchange.is_symbol_supported(&setup.symbol).await.unwrap_or(false) {
            return Err(ExecutorError::PreFlightCheckFailed(format!(
                "Symbol {} is not supported by the exchange",
                setup.symbol
            )));
        }
        Ok(())
    }

    fn create_trade_order(&self, setup: &TradeSetup) -> Result<TradeOrder, ExecutorError> {
        Ok(TradeOrder {
            client_order_id: Uuid::new_v4().to_string(),
            symbol: setup.symbol.clone(),
            side: setup.side,
            order_type: OrderType::Market,
            quantity: setup.position_size,
            price: None, // Market order
            stop_price: Some(setup.stop_loss),
        })
    }
}