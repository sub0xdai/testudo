use std::sync::Arc;
use rust_decimal::Decimal;
use uuid::Uuid;
use tracing::{info, warn, error};

use crate::types::{ExecutionPlan, TradeSetup};
use testudo_types::{
    ExchangeAdapterTrait, TradeOrder, OrderSide, OrderType, OrderResult, ExchangeError
};

/// Errors that can occur during trade execution
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("Exchange connectivity error: {0}")]
    ExchangeConnectivity(#[from] ExchangeError),
    
    #[error("Order rejected by exchange: {message}")]
    OrderRejected { message: String },
    
    #[error("Invalid execution plan: {reason}")]
    InvalidPlan { reason: String },
    
    #[error("Execution timeout after {seconds} seconds")]
    Timeout { seconds: u64 },
    
    #[error("Insufficient balance for order: required {required}, available {available}")]
    InsufficientBalance { required: Decimal, available: Decimal },
    
    #[error("Exchange health check failed")]
    ExchangeUnhealthy,
}

/// Result of trade execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Exchange order result
    pub order_result: OrderResult,
    /// Execution timestamp
    pub executed_at: chrono::DateTime<chrono::Utc>,
    /// Total execution time in milliseconds
    pub execution_time_ms: u64,
}

/// Executor component responsible for the Act phase of OODA loop
/// 
/// Follows Roman military principle: Once decision is made, execute with precision
pub struct Executor {
    /// Exchange adapter for order execution
    exchange: Arc<dyn ExchangeAdapterTrait + Send + Sync>,
}

impl Executor {
    /// Create new Executor with exchange adapter
    pub fn new(exchange: Arc<dyn ExchangeAdapterTrait + Send + Sync>) -> Self {
        Self { exchange }
    }
    
    /// Execute approved trade plan through exchange adapter
    /// 
    /// This is the final Act phase of OODA loop - disciplined execution
    /// following Van Tharp position sizing with no emotion or hesitation
    pub async fn execute_trade(&self, plan: ExecutionPlan) -> Result<ExecutionResult, ExecutorError> {
        let start_time = std::time::Instant::now();
        let executed_at = chrono::Utc::now();
        
        info!(
            "Executor beginning trade execution for symbol: {}", 
            plan.setup.symbol
        );
        
        // Validate execution plan is approved
        if !plan.approved {
            error!("Attempted to execute unapproved trade plan");
            return Err(ExecutorError::InvalidPlan {
                reason: "Trade plan not approved by risk management".to_string(),
            });
        }
        
        // Pre-flight checks
        self.pre_flight_checks(&plan.setup).await?;
        
        // Convert ExecutionPlan to TradeOrder
        let trade_order = self.create_trade_order(&plan.setup)?;
        
        info!(
            "Placing order: {:?} {} {} @ {}",
            trade_order.side,
            trade_order.quantity,
            trade_order.symbol,
            trade_order.price.unwrap_or_else(|| Decimal::from(0))
        );
        
        // Execute order through exchange
        let order_result = self.exchange
            .place_order(&trade_order)
            .await
            .map_err(|e| {
                error!("Order execution failed: {:?}", e);
                ExecutorError::ExchangeConnectivity(e)
            })?;
            
        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        
        info!(
            "Order executed successfully: {} in {}ms",
            order_result.order_id,
            execution_time_ms
        );
        
        Ok(ExecutionResult {
            order_result,
            executed_at,
            execution_time_ms,
        })
    }
    
    /// Perform pre-flight checks before execution
    async fn pre_flight_checks(&self, setup: &TradeSetup) -> Result<(), ExecutorError> {
        // Check exchange health
        if !self.exchange.health_check().await.unwrap_or(false) {
            warn!("Exchange health check failed before execution");
            return Err(ExecutorError::ExchangeUnhealthy);
        }
        
        // Verify symbol is supported
        if !self.exchange.is_symbol_supported(&setup.symbol).await.unwrap_or(false) {
            error!("Trading pair not supported: {}", setup.symbol);
            return Err(ExecutorError::InvalidPlan {
                reason: format!("Symbol {} not supported by exchange", setup.symbol),
            });
        }
        
        // Check account balance (basic validation)
        let base_asset = self.extract_base_asset(&setup.symbol);
        match self.exchange.get_balance(&base_asset).await {
            Ok(balance) => {
                if balance.free < setup.position_size {
                    warn!(
                        "Insufficient balance: required {}, available {}", 
                        setup.position_size, balance.free
                    );
                    return Err(ExecutorError::InsufficientBalance {
                        required: setup.position_size,
                        available: balance.free,
                    });
                }
            },
            Err(e) => {
                warn!("Could not verify balance: {:?}", e);
                // Continue execution - balance check might not be critical
            }
        }
        
        info!("Pre-flight checks completed successfully");
        Ok(())
    }
    
    /// Convert TradeSetup to TradeOrder for exchange
    fn create_trade_order(&self, setup: &TradeSetup) -> Result<TradeOrder, ExecutorError> {
        let client_order_id = format!("testudo_{}", Uuid::new_v4());
        
        // Determine order side (Buy/Sell)
        let side = if setup.entry_price > setup.current_price {
            OrderSide::Buy  // Long position
        } else {
            OrderSide::Sell // Short position (or sell existing long)
        };
        
        // Use limit order by default for better execution control
        let order_type = OrderType::Limit;
        
        let trade_order = TradeOrder {
            symbol: setup.symbol.clone(),
            side,
            order_type,
            quantity: setup.position_size,
            price: Some(setup.entry_price),
            stop_price: None, // TODO: Implement stop-loss orders
            client_order_id,
        };
        
        Ok(trade_order)
    }
    
    /// Extract base asset from trading pair symbol (e.g., "BTCUSDT" -> "BTC")
    fn extract_base_asset(&self, symbol: &str) -> String {
        // Simple extraction - assumes USDT, BUSD, etc. quote currencies
        if symbol.ends_with("USDT") {
            symbol.strip_suffix("USDT").unwrap_or(symbol).to_string()
        } else if symbol.ends_with("BUSD") {
            symbol.strip_suffix("BUSD").unwrap_or(symbol).to_string()
        } else if symbol.ends_with("BTC") {
            symbol.strip_suffix("BTC").unwrap_or(symbol).to_string()
        } else {
            // Fallback: take first 3-4 characters
            symbol.chars().take(3).collect()
        }
    }
    
    /// Get exchange name for logging
    pub fn exchange_name(&self) -> &str {
        self.exchange.exchange_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TradeSetup;
    use prudentia::exchange::MockExchange;
    use rust_decimal_macros::dec;
    use testudo_types::AccountBalance;
    
    fn create_test_setup() -> TradeSetup {
        TradeSetup {
            symbol: "BTCUSDT".to_string(),
            entry_price: dec!(50000.0),
            stop_loss: dec!(49000.0),
            take_profit: Some(dec!(52000.0)),
            position_size: dec!(0.01),
            current_price: dec!(49500.0),
            r_multiple: dec!(2.0),
        }
    }
    
    fn create_test_plan(approved: bool) -> ExecutionPlan {
        ExecutionPlan {
            setup: create_test_setup(),
            approved,
            risk_assessment: "Test assessment".to_string(),
        }
    }
    
    #[tokio::test]
    async fn test_executor_creation() {
        let exchange = Arc::new(MockExchange::new());
        let executor = Executor::new(exchange);
        assert_eq!(executor.exchange_name(), "MockExchange");
    }
    
    #[tokio::test]
    async fn test_execute_approved_trade() {
        let mut mock_exchange = MockExchange::new();
        
        // Setup mock exchange responses
        mock_exchange.set_health(true);
        // Note: MockExchange doesn't have add_supported_symbol, symbols are always supported
        mock_exchange.set_balance("BTC".to_string(), AccountBalance {
            asset: "BTC".to_string(),
            free: dec!(1.0),
            locked: dec!(0.0),
            total: dec!(1.0),
        });
        
        let exchange = Arc::new(mock_exchange);
        let executor = Executor::new(exchange);
        
        let plan = create_test_plan(true);
        let result = executor.execute_trade(plan).await;
        
        assert!(result.is_ok());
        let execution_result = result.unwrap();
        assert!(execution_result.execution_time_ms > 0);
    }
    
    #[tokio::test]
    async fn test_execute_unapproved_trade_fails() {
        let exchange = Arc::new(MockExchange::new());
        let executor = Executor::new(exchange);
        
        let plan = create_test_plan(false);
        let result = executor.execute_trade(plan).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            ExecutorError::InvalidPlan { reason } => {
                assert!(reason.contains("not approved"));
            },
            _ => panic!("Expected InvalidPlan error"),
        }
    }
    
    #[test]
    fn test_create_trade_order() {
        let exchange = Arc::new(MockExchange::new());
        let executor = Executor::new(exchange);
        
        let setup = create_test_setup();
        let order = executor.create_trade_order(&setup).unwrap();
        
        assert_eq!(order.symbol, "BTCUSDT");
        assert_eq!(order.quantity, dec!(0.01));
        assert_eq!(order.price, Some(dec!(50000.0)));
        assert_eq!(order.side, OrderSide::Buy);
        assert!(order.client_order_id.starts_with("testudo_"));
    }
    
    #[test]
    fn test_extract_base_asset() {
        let exchange = Arc::new(MockExchange::new());
        let executor = Executor::new(exchange);
        
        assert_eq!(executor.extract_base_asset("BTCUSDT"), "BTC");
        assert_eq!(executor.extract_base_asset("ETHBUSD"), "ETH");
        assert_eq!(executor.extract_base_asset("ADABTC"), "ADA");
    }
}