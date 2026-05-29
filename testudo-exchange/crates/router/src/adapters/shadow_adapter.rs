//! Shadow Engine Adapter
//!
//! Implements `ExchangeAdapter` trait for the Shadow Engine, enabling paper trading
//! through the unified exchange interface.
//!
//! # Type Conversions
//!
//! - `StandardOrder` → `ShadowOrder`
//! - `OrderSide::{Buy,Long}` → `ShadowOrderSide::Buy`
//! - `OrderSide::{Sell,Short}` → `ShadowOrderSide::Sell`
//! - `OrderType::Market` → `ShadowOrderType::Market`
//! - `OrderType::Limit` → `ShadowOrderType::Limit`
//! - `OrderType::StopLoss` → `ShadowOrderType::StopLoss`
//! - `OrderType::TakeProfit` → `ShadowOrderType::TakeProfit`
//!
//! # FR-3.1 (008-unified-exchange-adapter)
//!
//! - FR-3.1.1: Create `ShadowEngineAdapter` struct wrapping `Arc<ShadowEngine>`
//! - FR-3.1.2: Implement `ExchangeAdapter::place_order` converting `StandardOrder` → `ShadowOrder`
//! - FR-3.1.3: Implement `ExchangeAdapter::cancel_order` calling `engine.cancel_order()`
//! - FR-3.1.4: Implement `ExchangeAdapter::get_order_status` querying engine state
//! - FR-3.1.5: Implement `ExchangeAdapter::health_check` returning Ok (local engine)
//! - FR-3.1.6: Mark order as risk-validated before passing to ShadowEngine

// @anchor exchange:router:shadow_adapter
// @tags api

use async_trait::async_trait;
use common_utils::{OrderSide, OrderType, StandardOrder};
use engine::{EngineHandle, ShadowOrder, ShadowOrderSide, ShadowOrderStatus, ShadowOrderType};
use uuid::Uuid;

use crate::exchange::{ExchangeAdapter, OrderResponse, RoutingError};

/// Adapter wrapping ShadowEngine to implement ExchangeAdapter trait.
/// Migrated to EngineHandle (019b): all operations routed through the actor.
///
/// # Example
///
/// ```ignore
/// use router::adapters::ShadowEngineAdapter;
/// use engine::{EngineActor, ShadowEngine};
///
/// let engine = ShadowEngine::new();
/// let (handle, _, _) = EngineActor::spawn(engine);
/// let adapter = ShadowEngineAdapter::new(handle);
///
/// let order = StandardOrder::market_buy(user_id, "BTC_USDC", dec!(0.1)).unwrap();
/// let result = adapter.place_order(&order).await?;
/// ```
pub struct ShadowEngineAdapter {
    engine: EngineHandle,
}

impl ShadowEngineAdapter {
    /// Create a new ShadowEngineAdapter wrapping the given engine handle.
    pub fn new(engine: EngineHandle) -> Self {
        Self { engine }
    }

    /// Convert StandardOrder to ShadowOrder.
    ///
    /// # Type Conversion Reference (from PROMPT.md)
    ///
    /// - `OrderSide::Buy | OrderSide::Long` → `ShadowOrderSide::Buy`
    /// - `OrderSide::Sell | OrderSide::Short` → `ShadowOrderSide::Sell`
    /// - `OrderType::Market` → `ShadowOrderType::Market`
    /// - `OrderType::Limit` → `ShadowOrderType::Limit`
    /// - `OrderType::StopLoss` → `ShadowOrderType::StopLoss`
    /// - `OrderType::TakeProfit` → `ShadowOrderType::TakeProfit`
    fn convert_to_shadow(&self, order: &StandardOrder) -> ShadowOrder {
        let side = match order.side {
            OrderSide::Buy | OrderSide::Long => ShadowOrderSide::Buy,
            OrderSide::Sell | OrderSide::Short => ShadowOrderSide::Sell,
        };

        let order_type = match order.order_type {
            OrderType::Market => ShadowOrderType::Market,
            OrderType::Limit => ShadowOrderType::Limit,
            OrderType::StopLoss | OrderType::StopLossLimit => ShadowOrderType::StopLoss,
            OrderType::TakeProfit | OrderType::TakeProfitLimit => ShadowOrderType::TakeProfit,
        };

        ShadowOrder::new(
            order.user_id,
            order.symbol.clone(),
            side,
            order_type,
            order.quantity,
            order.price,
            order.stop_price,
            None, // parent_order_id
        )
    }

    /// Convert ShadowOrder result to OrderResponse.
    fn convert_response(&self, order: &ShadowOrder) -> OrderResponse {
        let status = match order.status {
            ShadowOrderStatus::Open => "NEW",
            ShadowOrderStatus::PartiallyFilled => "PARTIALLY_FILLED",
            ShadowOrderStatus::Filled => "FILLED",
            ShadowOrderStatus::Cancelled => "CANCELED",
            ShadowOrderStatus::Rejected => "REJECTED",
        };

        OrderResponse {
            order_id: order.id.to_string(),
            exchange_order_id: format!("SHADOW-{}", order.id),
            status: status.to_string(),
            filled_quantity: order.filled_quantity,
            remaining_quantity: order.quantity - order.filled_quantity,
            average_price: order.average_fill_price,
        }
    }
}

#[async_trait]
impl ExchangeAdapter for ShadowEngineAdapter {
    /// Place an order on the Shadow Engine.
    ///
    /// # FR-3.1.6
    ///
    /// Orders are marked as risk-validated before passing to ShadowEngine.
    /// This assumes the caller (ExecutionService/order routes) has already
    /// performed risk validation via the Decision Loop.
    async fn place_order(&self, order: &StandardOrder) -> Result<OrderResponse, RoutingError> {
        let mut shadow_order = self.convert_to_shadow(order);

        // FR-3.1.6: Mark order as risk-validated
        // The Decision Loop has already validated this order at the route level
        shadow_order.mark_risk_validated();

        // Ensure user exists in shadow engine
        if !self.engine.user_exists(order.user_id).await {
            self.engine
                .init_user(order.user_id)
                .await
                .map_err(|e| RoutingError::ExchangeError(e.to_string().into()))?;
            tracing::info!("Auto-initialized shadow account for user {}", order.user_id);
        }

        // Place the order
        match self.engine.place_order(order.user_id, shadow_order).await {
            Ok(placed_order) => Ok(self.convert_response(&placed_order)),
            Err(e) => {
                tracing::error!("ShadowEngine place_order failed: {}", e);
                Err(RoutingError::ExchangeError(e.to_string().into()))
            }
        }
    }

    /// Cancel an order on the Shadow Engine.
    async fn cancel_order(&self, order_id: &str) -> Result<(), RoutingError> {
        let order_uuid = Uuid::parse_str(order_id)
            .map_err(|e| RoutingError::ExchangeError(format!("Invalid order ID: {}", e).into()))?;

        // Find the order to get the user_id
        let order = self.engine.get_order(order_uuid).await.ok_or_else(|| {
            RoutingError::ExchangeNotFound(format!("Order {} not found", order_id))
        })?;
        let user_id = order.user_id;

        // Cancel the order
        match self.engine.cancel_order(user_id, order_uuid).await {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!("ShadowEngine cancel_order failed: {}", e);
                Err(RoutingError::ExchangeError(e.to_string().into()))
            }
        }
    }

    /// Get order status from the Shadow Engine.
    async fn get_order_status(&self, order_id: &str) -> Result<OrderResponse, RoutingError> {
        let order_uuid = Uuid::parse_str(order_id)
            .map_err(|e| RoutingError::ExchangeError(format!("Invalid order ID: {}", e).into()))?;

        let order = self.engine.get_order(order_uuid).await.ok_or_else(|| {
            RoutingError::ExchangeNotFound(format!("Order {} not found", order_id))
        })?;

        Ok(self.convert_response(&order))
    }

    /// Return the adapter name.
    fn get_name(&self) -> &str {
        "shadow"
    }

    /// Health check for Shadow Engine (always healthy - local engine).
    async fn health_check(&self) -> Result<(), RoutingError> {
        // Shadow engine is local, always healthy
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common_utils::{OrderSide, OrderType, StandardOrderBuilder};
    use engine::{EngineActor, ShadowEngine};
    use rust_decimal_macros::dec;

    /// Dummy handle for sync tests that only test type conversions (no actor needed).
    fn dummy_handle() -> EngineHandle {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        EngineHandle::new(tx)
    }

    /// Live handle with running actor for async tests.
    fn create_test_handle() -> EngineHandle {
        let engine = ShadowEngine::new();
        let (handle, _fill_rx, _trade_event_rx) = EngineActor::spawn(engine);
        handle
    }

    // ==================== FR-3.1.1: Struct Creation Tests ====================

    #[test]
    fn test_adapter_creation() {
        let adapter = ShadowEngineAdapter::new(dummy_handle());
        assert_eq!(adapter.get_name(), "shadow");
    }

    // ==================== Type Conversion Tests ====================

    #[test]
    fn test_convert_buy_side() {
        let adapter = ShadowEngineAdapter::new(dummy_handle());

        let user_id = Uuid::new_v4();
        let order = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC_USDT")
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(dec!(0.1))
            .build()
            .unwrap();

        let shadow = adapter.convert_to_shadow(&order);
        assert_eq!(shadow.side, ShadowOrderSide::Buy);
    }

    #[test]
    fn test_convert_long_to_buy() {
        let adapter = ShadowEngineAdapter::new(dummy_handle());

        let user_id = Uuid::new_v4();
        let order = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC_USDT")
            .side(OrderSide::Long)
            .order_type(OrderType::Market)
            .quantity(dec!(0.1))
            .build()
            .unwrap();

        let shadow = adapter.convert_to_shadow(&order);
        assert_eq!(shadow.side, ShadowOrderSide::Buy);
    }

    #[test]
    fn test_convert_sell_side() {
        let adapter = ShadowEngineAdapter::new(dummy_handle());

        let user_id = Uuid::new_v4();
        let order = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC_USDT")
            .side(OrderSide::Sell)
            .order_type(OrderType::Market)
            .quantity(dec!(0.1))
            .build()
            .unwrap();

        let shadow = adapter.convert_to_shadow(&order);
        assert_eq!(shadow.side, ShadowOrderSide::Sell);
    }

    #[test]
    fn test_convert_short_to_sell() {
        let adapter = ShadowEngineAdapter::new(dummy_handle());

        let user_id = Uuid::new_v4();
        let order = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC_USDT")
            .side(OrderSide::Short)
            .order_type(OrderType::Market)
            .quantity(dec!(0.1))
            .build()
            .unwrap();

        let shadow = adapter.convert_to_shadow(&order);
        assert_eq!(shadow.side, ShadowOrderSide::Sell);
    }

    #[test]
    fn test_convert_market_order_type() {
        let adapter = ShadowEngineAdapter::new(dummy_handle());

        let user_id = Uuid::new_v4();
        let order = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC_USDT")
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(dec!(0.1))
            .build()
            .unwrap();

        let shadow = adapter.convert_to_shadow(&order);
        assert_eq!(shadow.order_type, ShadowOrderType::Market);
    }

    #[test]
    fn test_convert_limit_order_type() {
        let adapter = ShadowEngineAdapter::new(dummy_handle());

        let user_id = Uuid::new_v4();
        let order = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC_USDT")
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(dec!(0.1))
            .price(dec!(50000))
            .build()
            .unwrap();

        let shadow = adapter.convert_to_shadow(&order);
        assert_eq!(shadow.order_type, ShadowOrderType::Limit);
        assert_eq!(shadow.price, Some(dec!(50000)));
    }

    #[test]
    fn test_convert_stop_loss_order_type() {
        let adapter = ShadowEngineAdapter::new(dummy_handle());

        let user_id = Uuid::new_v4();
        let order = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC_USDT")
            .side(OrderSide::Sell)
            .order_type(OrderType::StopLoss)
            .quantity(dec!(0.1))
            .stop_price(dec!(48000))
            .build()
            .unwrap();

        let shadow = adapter.convert_to_shadow(&order);
        assert_eq!(shadow.order_type, ShadowOrderType::StopLoss);
        assert_eq!(shadow.stop_price, Some(dec!(48000)));
    }

    #[test]
    fn test_convert_take_profit_order_type() {
        let adapter = ShadowEngineAdapter::new(dummy_handle());

        let user_id = Uuid::new_v4();
        let order = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC_USDT")
            .side(OrderSide::Sell)
            .order_type(OrderType::TakeProfit)
            .quantity(dec!(0.1))
            .stop_price(dec!(55000))
            .build()
            .unwrap();

        let shadow = adapter.convert_to_shadow(&order);
        assert_eq!(shadow.order_type, ShadowOrderType::TakeProfit);
        assert_eq!(shadow.stop_price, Some(dec!(55000)));
    }

    // ==================== FR-3.1.2: place_order Tests ====================

    #[tokio::test]
    async fn test_place_order_success() {
        let handle = create_test_handle();
        let adapter = ShadowEngineAdapter::new(handle);

        let user_id = Uuid::new_v4();
        let order = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC_USDT")
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(dec!(0.01))
            .price(dec!(50000))
            .build()
            .unwrap();

        let result = adapter.place_order(&order).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.status, "NEW");
        assert!(response.exchange_order_id.starts_with("SHADOW-"));
        assert_eq!(response.filled_quantity, dec!(0));
        assert_eq!(response.remaining_quantity, dec!(0.01));
    }

    #[tokio::test]
    async fn test_place_order_auto_initializes_user() {
        let engine = ShadowEngine::new();
        let (handle, _fill_rx, _trade_event_rx) = EngineActor::spawn(engine);
        let adapter = ShadowEngineAdapter::new(handle.clone());

        let user_id = Uuid::new_v4();

        // User doesn't exist yet
        assert!(!handle.user_exists(user_id).await);

        let order = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC_USDT")
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(dec!(0.01))
            .price(dec!(50000))
            .build()
            .unwrap();

        let result = adapter.place_order(&order).await;
        assert!(result.is_ok());

        // User should now exist
        assert!(handle.user_exists(user_id).await);
    }

    // ==================== FR-3.1.3: cancel_order Tests ====================

    #[tokio::test]
    async fn test_cancel_order_success() {
        let handle = create_test_handle();
        let adapter = ShadowEngineAdapter::new(handle);

        let user_id = Uuid::new_v4();
        let order = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC_USDT")
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(dec!(0.01))
            .price(dec!(50000))
            .build()
            .unwrap();

        // Place order first
        let placed = adapter.place_order(&order).await.unwrap();

        // Cancel it
        let cancel_result = adapter.cancel_order(&placed.order_id).await;
        assert!(cancel_result.is_ok());
    }

    #[tokio::test]
    async fn test_cancel_order_not_found() {
        let handle = create_test_handle();
        let adapter = ShadowEngineAdapter::new(handle);

        let fake_id = Uuid::new_v4().to_string();
        let result = adapter.cancel_order(&fake_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cancel_order_invalid_uuid() {
        let handle = create_test_handle();
        let adapter = ShadowEngineAdapter::new(handle);

        let result = adapter.cancel_order("not-a-uuid").await;
        assert!(result.is_err());
    }

    // ==================== FR-3.1.4: get_order_status Tests ====================

    #[tokio::test]
    async fn test_get_order_status_success() {
        let handle = create_test_handle();
        let adapter = ShadowEngineAdapter::new(handle);

        let user_id = Uuid::new_v4();
        let order = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC_USDT")
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(dec!(0.01))
            .price(dec!(50000))
            .build()
            .unwrap();

        // Place order
        let placed = adapter.place_order(&order).await.unwrap();

        // Get status
        let status = adapter.get_order_status(&placed.order_id).await;
        assert!(status.is_ok());

        let response = status.unwrap();
        assert_eq!(response.order_id, placed.order_id);
        assert_eq!(response.status, "NEW");
    }

    #[tokio::test]
    async fn test_get_order_status_not_found() {
        let handle = create_test_handle();
        let adapter = ShadowEngineAdapter::new(handle);

        let fake_id = Uuid::new_v4().to_string();
        let result = adapter.get_order_status(&fake_id).await;
        assert!(result.is_err());
    }

    // ==================== FR-3.1.5: health_check Tests ====================

    #[tokio::test]
    async fn test_health_check_always_ok() {
        let handle = create_test_handle();
        let adapter = ShadowEngineAdapter::new(handle);

        let result = adapter.health_check().await;
        assert!(result.is_ok());
    }

    // ==================== FR-3.1.6: Risk Validation Tests ====================

    #[tokio::test]
    async fn test_place_order_marks_risk_validated() {
        let handle = create_test_handle();
        let adapter = ShadowEngineAdapter::new(handle);

        let user_id = Uuid::new_v4();
        let order = StandardOrderBuilder::new()
            .user_id(user_id)
            .symbol("BTC_USDT")
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(dec!(0.01))
            .price(dec!(50000))
            .build()
            .unwrap();

        // Order should be placed successfully (adapter marks it as risk-validated)
        let result = adapter.place_order(&order).await;
        assert!(
            result.is_ok(),
            "Order should succeed because adapter marks it risk-validated"
        );
    }
}
