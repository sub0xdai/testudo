//! Execution Service
//!
//! Provides mode-based order routing to the appropriate exchange adapter.
//! Currently supports shadow (paper) trading only. Live trading is handled
//! via the CEX sidecar service through `CexExchangeApi` and `TradeManagerService`.

use common_utils::adapters::execution_types::ExecutionMode;
use common_utils::StandardOrder;
use std::sync::Arc;

use crate::adapters::ShadowEngineAdapter;
use crate::exchange::{ExchangeAdapter, OrderResponse, RoutingError};

/// Service for routing orders to the appropriate exchange based on execution mode.
///
/// The legacy `/order` route uses this service with shadow-only mode.
/// Live trading uses the `CexExchangeApi` via `/trades` routes instead.
pub struct ExecutionService {
    shadow_adapter: Arc<ShadowEngineAdapter>,
}

impl ExecutionService {
    /// Create a new ExecutionService with both adapters.
    /// The binance_adapter parameter is kept for API compatibility but ignored.
    pub fn new(shadow_adapter: Arc<ShadowEngineAdapter>, _binance_adapter: Option<()>) -> Self {
        Self { shadow_adapter }
    }

    /// Create an ExecutionService with only the shadow adapter.
    pub fn shadow_only(shadow_adapter: Arc<ShadowEngineAdapter>) -> Self {
        Self { shadow_adapter }
    }

    /// Execute an order. Always routes to shadow adapter.
    pub async fn execute_order(
        &self,
        order: &StandardOrder,
        mode: ExecutionMode,
    ) -> Result<OrderResponse, RoutingError> {
        tracing::info!("Routing order {} to {:?}", order.id, mode);
        self.shadow_adapter.place_order(order).await
    }

    /// Cancel an order via shadow adapter.
    pub async fn cancel_order(
        &self,
        order_id: &str,
        _mode: ExecutionMode,
    ) -> Result<(), RoutingError> {
        tracing::info!("Cancelling order {}", order_id);
        self.shadow_adapter.cancel_order(order_id).await
    }

    /// Get order status via shadow adapter.
    pub async fn get_order_status(
        &self,
        order_id: &str,
        _mode: ExecutionMode,
    ) -> Result<OrderResponse, RoutingError> {
        self.shadow_adapter.get_order_status(order_id).await
    }

    /// Health check the shadow adapter.
    pub async fn health_check(&self) -> Result<HealthStatus, RoutingError> {
        let shadow_healthy = self.shadow_adapter.health_check().await.is_ok();

        Ok(HealthStatus {
            shadow: shadow_healthy,
        })
    }

    /// Get the shadow adapter.
    pub fn shadow(&self) -> &Arc<ShadowEngineAdapter> {
        &self.shadow_adapter
    }
}

/// Health status of the execution service.
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub shadow: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use common_utils::{OrderSide, OrderType, StandardOrderBuilder};
    use engine::{EngineActor, ShadowEngine};
    use rust_decimal_macros::dec;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use uuid::Uuid;

    /// Dummy handle for sync tests (no Tokio runtime needed).
    fn dummy_handle() -> engine::EngineHandle {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        engine::EngineHandle::new(tx)
    }

    fn create_test_service() -> ExecutionService {
        let shadow_adapter = Arc::new(ShadowEngineAdapter::new(dummy_handle()));
        ExecutionService::shadow_only(shadow_adapter)
    }

    fn create_live_test_service() -> ExecutionService {
        let engine = ShadowEngine::new();
        let (handle, _fill_rx, _trade_event_rx) = EngineActor::spawn(engine);
        let shadow_adapter = Arc::new(ShadowEngineAdapter::new(handle));
        ExecutionService::shadow_only(shadow_adapter)
    }

    fn create_test_order() -> StandardOrder {
        StandardOrderBuilder::new()
            .user_id(Uuid::new_v4())
            .symbol("BTC_USDT")
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(dec!(0.001))
            .price(dec!(50000))
            .build()
            .unwrap()
    }

    #[test]
    fn test_service_creation() {
        let _service = create_test_service();
    }

    #[tokio::test]
    async fn test_execute_order_shadow_mode() {
        let service = create_live_test_service();
        let order = create_test_order();

        let result = service.execute_order(&order, ExecutionMode::Shadow).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.exchange_order_id.starts_with("SHADOW-"));
    }

    #[tokio::test]
    async fn test_cancel_order_shadow_mode() {
        let service = create_live_test_service();
        let order = create_test_order();

        let placed = service
            .execute_order(&order, ExecutionMode::Shadow)
            .await
            .unwrap();

        let result = service
            .cancel_order(&placed.order_id, ExecutionMode::Shadow)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_order_status_shadow_mode() {
        let service = create_live_test_service();
        let order = create_test_order();

        let placed = service
            .execute_order(&order, ExecutionMode::Shadow)
            .await
            .unwrap();

        let result = service
            .get_order_status(&placed.order_id, ExecutionMode::Shadow)
            .await;
        assert!(result.is_ok());

        let status = result.unwrap();
        assert_eq!(status.order_id, placed.order_id);
    }

    #[tokio::test]
    async fn test_health_check() {
        let service = create_live_test_service();

        let result = service.health_check().await;
        assert!(result.is_ok());

        let status = result.unwrap();
        assert!(status.shadow);
    }
}
