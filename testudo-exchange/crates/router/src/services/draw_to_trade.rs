//! Draw-to-Trade Service (006-execution-latency)
//!
//! Low-latency execution service for processing user drawing intents
//! into signed orders ready for CEX dispatch.
//!
//! # Performance Targets
//!
//! | Operation | Target |
//! |-----------|--------|
//! | Risk Calc & Sizing | < 1ms |
//! | Signing & Payload Gen | < 3ms |
//! | **Total Internal Pipeline** | **< 10ms** |
//!
//! # FR-3: Latency Guard
//!
//! The pipeline will reject/fail if internal processing exceeds 50ms.

use common_utils::adapters::execution_types::{
    CexGateway, DispatchResult, ExecutionError, LatencyExceededError, SignedOrder, TradeIntent,
};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

/// Maximum allowed internal latency in microseconds (FR-3)
const MAX_LATENCY_MICROS: u128 = 50_000; // 50ms

/// Draw-to-Trade execution service
///
/// Processes TradeIntent payloads from user chart drawings and produces
/// SignedOrders ready for CEX dispatch.
///
/// # Example
///
/// ```ignore
/// use router::services::DrawToTradeService;
/// use common_utils::adapters::execution_types::{MockCexGateway, TradeIntent};
/// use rust_decimal_macros::dec;
///
/// let service = DrawToTradeService::new(Arc::new(MockCexGateway::new()));
/// let intent = TradeIntent::new(
///     "BTC/USDT",
///     dec!(50000),
///     dec!(49000),
///     dec!(10000),
///     dec!(0.02),
/// );
///
/// let result = service.process_order(&intent).await?;
/// println!("Order size: {}", result.quantity);
/// ```
pub struct DrawToTradeService<G: CexGateway> {
    gateway: Arc<G>,
}

impl<G: CexGateway> DrawToTradeService<G> {
    /// Create a new DrawToTradeService with the given CEX gateway
    pub fn new(gateway: Arc<G>) -> Self {
        Self { gateway }
    }

    /// Process a trade intent into a signed order (FR-2)
    ///
    /// This is the main entry point for the Draw-to-Trade pipeline.
    ///
    /// # Performance
    ///
    /// - Risk calculation: inline, < 1ms
    /// - Order signing: inline, < 1ms
    /// - Gateway dispatch: depends on gateway (mock: ~100μs)
    ///
    /// # FR-3: Latency Guard
    ///
    /// If internal processing exceeds 50ms, returns `ExecutionError::Timeout`.
    pub async fn process_order(
        &self,
        intent: &TradeIntent,
    ) -> Result<ProcessedOrder, ProcessOrderError> {
        let start = Instant::now();

        // Step 1: Generate order ID (fast - just UUID generation)
        let order_id = Uuid::new_v4().to_string();

        // Step 2: Create signed order (includes FR-1 size calculation)
        // This is pure computation, should be < 1ms
        let signed_order = SignedOrder::from_intent(intent, order_id);

        // Check latency guard before dispatch
        let elapsed = start.elapsed().as_micros();
        if elapsed > MAX_LATENCY_MICROS {
            return Err(ProcessOrderError::LatencyExceeded(LatencyExceededError {
                elapsed_micros: elapsed,
                threshold_micros: MAX_LATENCY_MICROS,
            }));
        }

        // Step 3: Dispatch to gateway
        let dispatch_result = self.gateway.dispatch(&signed_order).await?;

        // Final latency check
        let total_elapsed = start.elapsed().as_micros();
        if total_elapsed > MAX_LATENCY_MICROS {
            return Err(ProcessOrderError::LatencyExceeded(LatencyExceededError {
                elapsed_micros: total_elapsed,
                threshold_micros: MAX_LATENCY_MICROS,
            }));
        }

        Ok(ProcessedOrder {
            signed_order,
            dispatch_result,
            processing_time_micros: total_elapsed,
        })
    }

    /// Get a reference to the gateway (for health checks etc.)
    pub fn gateway(&self) -> &G {
        &self.gateway
    }
}

/// Result of processing an order
#[derive(Debug, Clone)]
pub struct ProcessedOrder {
    /// The signed order that was created
    pub signed_order: SignedOrder,
    /// Result from the CEX gateway
    pub dispatch_result: DispatchResult,
    /// Total processing time in microseconds
    pub processing_time_micros: u128,
}

/// Errors that can occur during order processing
#[derive(Debug, thiserror::Error)]
pub enum ProcessOrderError {
    #[error("Execution error: {0}")]
    Execution(#[from] ExecutionError),

    #[error("{0}")]
    LatencyExceeded(#[from] LatencyExceededError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use common_utils::adapters::execution_types::{
        ExecutionOrderSide, MockCexGateway, TradeIntent,
    };
    use rust_decimal_macros::dec;

    fn create_test_intent() -> TradeIntent {
        TradeIntent::new(
            "BTC/USDT",
            dec!(50000), // entry
            dec!(49000), // stop (2% below)
            dec!(10000), // equity
            dec!(0.02),  // 2% risk
        )
    }

    // ==================== FR-1: Auto-Sizing Tests ====================

    #[test]
    fn test_trade_intent_size_calculation() {
        let intent = create_test_intent();

        // Size = (10000 * 0.02) / abs(50000 - 49000)
        // Size = 200 / 1000 = 0.2 BTC
        let size = intent.calculate_size();
        assert_eq!(size, dec!(0.2));
    }

    #[test]
    fn test_trade_intent_size_zero_stop_distance() {
        let intent = TradeIntent::new(
            "BTC/USDT",
            dec!(50000),
            dec!(50000), // Same as entry
            dec!(10000),
            dec!(0.02),
        );

        assert_eq!(intent.calculate_size(), dec!(0));
    }

    #[test]
    fn test_trade_intent_short_position() {
        // Short: entry below stop
        let intent = TradeIntent::new(
            "BTC/USDT",
            dec!(49000), // entry
            dec!(50000), // stop above
            dec!(10000),
            dec!(0.02),
        );

        assert_eq!(intent.calculate_size(), dec!(0.2));
        assert_eq!(intent.side(), ExecutionOrderSide::Sell);
    }

    #[test]
    fn test_trade_intent_long_position() {
        let intent = create_test_intent();
        assert_eq!(intent.side(), ExecutionOrderSide::Buy);
    }

    // ==================== FR-2: ExecutionService Tests ====================

    #[tokio::test]
    async fn test_process_order_returns_signed_order() {
        let gateway = Arc::new(MockCexGateway::new());
        let service = DrawToTradeService::new(gateway);
        let intent = create_test_intent();

        let result = service.process_order(&intent).await;
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert_eq!(processed.signed_order.symbol, "BTC/USDT");
        assert_eq!(processed.signed_order.quantity, dec!(0.2));
        assert_eq!(processed.signed_order.price, dec!(50000));
        assert_eq!(processed.signed_order.stop_loss, dec!(49000));
    }

    #[tokio::test]
    async fn test_process_order_dispatch_result() {
        let gateway = Arc::new(MockCexGateway::new());
        let service = DrawToTradeService::new(gateway);
        let intent = create_test_intent();

        let result = service.process_order(&intent).await.unwrap();

        assert!(result
            .dispatch_result
            .exchange_order_id
            .starts_with("MOCK-"));
    }

    // ==================== FR-3: Latency Guard Tests ====================

    #[tokio::test]
    async fn test_latency_guard_slow_gateway() {
        // Create a gateway with 60ms latency (over the 50ms limit)
        let gateway = Arc::new(MockCexGateway::with_latency(60_000));
        let service = DrawToTradeService::new(gateway);
        let intent = create_test_intent();

        let result = service.process_order(&intent).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            ProcessOrderError::LatencyExceeded(e) => {
                assert!(e.elapsed_micros > 50_000);
            }
            _ => panic!("Expected LatencyExceeded error"),
        }
    }

    #[tokio::test]
    async fn test_latency_guard_fast_gateway() {
        // Fast gateway should pass
        let gateway = Arc::new(MockCexGateway::with_latency(100)); // 100μs
        let service = DrawToTradeService::new(gateway);
        let intent = create_test_intent();

        let result = service.process_order(&intent).await;
        assert!(result.is_ok());
    }

    // ==================== FR-4: MockCexGateway Tests ====================

    #[tokio::test]
    async fn test_mock_gateway_dispatch() {
        let gateway = MockCexGateway::new();
        let intent = create_test_intent();
        let signed_order = SignedOrder::from_intent(&intent, "test-123".to_string());

        let result = gateway.dispatch(&signed_order).await;
        assert!(result.is_ok());

        let dispatch = result.unwrap();
        assert_eq!(dispatch.exchange_order_id, "MOCK-test-123");
    }

    #[test]
    fn test_mock_gateway_health_check() {
        let gateway = MockCexGateway::new();
        assert!(gateway.health_check());
    }

    // ==================== Performance Benchmark ====================

    #[tokio::test]
    async fn test_end_to_end_execution_latency() {
        // Setup
        let gateway = Arc::new(MockCexGateway::with_latency(0)); // Zero latency mock
        let service = DrawToTradeService::new(gateway);
        let intent = create_test_intent();

        // Warmup (run once to prime caches/instruction cache)
        let _ = service.process_order(&intent).await;

        // Benchmark loop (100 iterations)
        let mut total_duration: u128 = 0;
        let iterations: u128 = 100;

        for _ in 0..iterations {
            let start = Instant::now();

            // ACT: The critical path
            let result = service.process_order(&intent).await;

            let elapsed = start.elapsed().as_micros();
            total_duration += elapsed;

            assert!(result.is_ok());
        }

        // Assertions
        let avg_latency = total_duration / iterations;
        println!("Average Internal Latency: {} microseconds", avg_latency);

        // Fail if average processing > 10ms (10,000 micros)
        assert!(
            avg_latency < 10_000,
            "Latency too high! Average: {}μs",
            avg_latency
        );
    }

    #[tokio::test]
    async fn test_throughput_100_orders_per_second() {
        // Setup
        let gateway = Arc::new(MockCexGateway::with_latency(0));
        let service = DrawToTradeService::new(gateway);
        let intent = create_test_intent();

        // Measure time to process 100 orders
        let start = Instant::now();

        for _ in 0..100 {
            let result = service.process_order(&intent).await;
            assert!(result.is_ok());
        }

        let total_time = start.elapsed();
        let orders_per_second = 100.0 / total_time.as_secs_f64();

        println!(
            "Throughput: {:.2} orders/sec (total time: {:?})",
            orders_per_second, total_time
        );

        // Must handle at least 100 orders/sec
        assert!(
            orders_per_second >= 100.0,
            "Throughput too low: {:.2} orders/sec",
            orders_per_second
        );
    }
}
