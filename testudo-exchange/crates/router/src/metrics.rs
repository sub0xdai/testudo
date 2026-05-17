//! AUD-05 FR-4/FR-5: Prometheus metrics for the Testudo router.

use lazy_static::lazy_static;
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, Opts, Registry, TextEncoder,
};

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    /// FR-5: Total orders placed (labels: side, status)
    pub static ref ORDERS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("testudo_orders_total", "Total orders placed"),
        &["side", "status"]
    )
    .unwrap();

    /// FR-5: Order placement latency in seconds (labels: exchange)
    pub static ref ORDER_LATENCY: HistogramVec = HistogramVec::new(
        HistogramOpts::new("testudo_order_latency_seconds", "Order placement latency")
            .buckets(vec![0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
        &["exchange"]
    )
    .unwrap();

    /// FR-5: Current active positions
    pub static ref ACTIVE_POSITIONS: IntGauge =
        IntGauge::new("testudo_active_positions", "Current active positions").unwrap();

    /// FR-5: Current WebSocket connections
    pub static ref WS_CONNECTIONS: IntGauge =
        IntGauge::new("testudo_ws_connections", "Current WebSocket connections").unwrap();

    /// FR-5: Total errors (labels: endpoint, status_code)
    pub static ref ERRORS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("testudo_errors_total", "Total errors"),
        &["endpoint", "status_code"]
    )
    .unwrap();
}

/// Register all metrics with the custom registry.
pub fn register_metrics() {
    REGISTRY
        .register(Box::new(ORDERS_TOTAL.clone()))
        .expect("failed to register testudo_orders_total");
    REGISTRY
        .register(Box::new(ORDER_LATENCY.clone()))
        .expect("failed to register testudo_order_latency_seconds");
    REGISTRY
        .register(Box::new(ACTIVE_POSITIONS.clone()))
        .expect("failed to register testudo_active_positions");
    REGISTRY
        .register(Box::new(WS_CONNECTIONS.clone()))
        .expect("failed to register testudo_ws_connections");
    REGISTRY
        .register(Box::new(ERRORS_TOTAL.clone()))
        .expect("failed to register testudo_errors_total");
}

/// Encode all registered metrics as Prometheus text format.
pub fn encode_metrics() -> Vec<u8> {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    buffer
}
