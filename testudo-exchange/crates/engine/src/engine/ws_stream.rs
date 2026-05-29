// @anchor exchange:engine:ws_stream
// @tags domain

use super::engine::Engine;
use crate::types::{
    engine::{Fill, OrderSide},
    ws_stream::WsResponse,
};
use async_trait::async_trait;
use pg_queue::PgQueueManager;
use rust_decimal::Decimal;
use std::sync::Arc;

/// PostgreSQL-based WebSocket stream updates trait
#[async_trait]
pub trait WsStreamUpdatesPg {
    async fn publish_ws_trades_pg(
        &self,
        market: String,
        user_id: String,
        fills: &Vec<Fill>,
        timestamp: i64,
        pg_queue: &Arc<PgQueueManager>,
    );

    async fn publish_ws_depth_updates_pg(
        &mut self,
        market: String,
        price: Decimal,
        side: OrderSide,
        fills: &Vec<Fill>,
        pg_queue: &Arc<PgQueueManager>,
    );
}

#[async_trait]
impl WsStreamUpdatesPg for Engine {
    async fn publish_ws_trades_pg(
        &self,
        market: String,
        user_id: String,
        fills: &Vec<Fill>,
        timestamp: i64,
        pg_queue: &Arc<PgQueueManager>,
    ) {
        for fill in fills.iter() {
            let stream = format!("trade.{}", market);
            let data = serde_json::json!({
                "e": "trade",
                "t": fill.trade_id,
                "m": fill.other_user_id == user_id,
                "p": fill.price,
                "q": fill.quantity,
                "s": market,
                "T": timestamp,
            });

            let ws_response = WsResponse {
                stream: stream.clone(),
                data,
            };
            let ws_response_string = match serde_json::to_string(&ws_response) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!(error = %e, stream = %stream, "Failed to serialize WsResponse, skipping");
                    continue;
                }
            };

            let result = pg_queue.notify.notify(&stream, &ws_response_string).await;

            if let Err(e) = result {
                tracing::error!(error = %e, stream = %stream, "Error publishing via pg_notify");
            }
        }
    }

    async fn publish_ws_depth_updates_pg(
        &mut self,
        market: String,
        price: Decimal,
        side: OrderSide,
        fills: &Vec<Fill>,
        pg_queue: &Arc<PgQueueManager>,
    ) {
        let orderbook = match self
            .orderbooks
            .iter_mut()
            .find(|orderbook| orderbook.ticker() == market)
        {
            Some(ob) => ob,
            None => {
                tracing::warn!(market = %market, "No matching orderbook found");
                return;
            }
        };

        let depth = orderbook.get_depth();
        let depth_bids = depth.0;
        let depth_asks = depth.1;

        let (updated_bids, updated_asks) = match side {
            OrderSide::BUY => {
                let updated_asks = depth_asks
                    .into_iter()
                    .filter(|ask| fills.iter().any(|fill| fill.price == ask.0))
                    .collect::<Vec<(Decimal, Decimal)>>();
                let updated_bids = depth_bids
                    .into_iter()
                    .filter(|bid| bid.0 == price)
                    .collect::<Vec<(Decimal, Decimal)>>();
                (updated_bids, updated_asks)
            }
            OrderSide::SELL => {
                let updated_bids = depth_bids
                    .into_iter()
                    .filter(|bid| fills.iter().any(|fill| fill.price == bid.0))
                    .collect::<Vec<(Decimal, Decimal)>>();
                let updated_asks = depth_asks
                    .into_iter()
                    .filter(|ask| ask.0 == price)
                    .collect::<Vec<(Decimal, Decimal)>>();
                (updated_bids, updated_asks)
            }
        };

        let stream = format!("depth.{}", market);
        let data = serde_json::json!({
            "e": "depth",
            "s": market,
            "b": updated_bids,
            "a": updated_asks,
        });

        let ws_response = WsResponse {
            stream: stream.clone(),
            data,
        };

        let ws_response_string = match serde_json::to_string(&ws_response) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, stream = %stream, "Failed to serialize WsResponse, skipping");
                return;
            }
        };

        let result = pg_queue.notify.notify(&stream, &ws_response_string).await;

        if let Err(e) = result {
            tracing::error!(error = %e, stream = %stream, "Error publishing via pg_notify");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 017 FR-8: Verify that WsResponse serialization does not panic.
    /// The match-based error handling ensures graceful degradation instead
    /// of crashing the WebSocket publisher thread.
    #[test]
    fn test_ws_response_serialization_does_not_panic() {
        let response = WsResponse {
            stream: "trade.BTC_USDT".to_string(),
            data: serde_json::json!({
                "e": "trade",
                "p": "50000.00",
                "q": "0.1",
            }),
        };

        // Normal serialization succeeds
        let result = serde_json::to_string(&response);
        assert!(result.is_ok());

        // Verify the match pattern works — same pattern used in production code
        let _output = match serde_json::to_string(&response) {
            Ok(s) => s,
            Err(_e) => {
                // This path would be taken for non-serializable payloads.
                // serde_json::Value is always serializable, but the match
                // pattern protects against future type changes.
                "fallback".to_string()
            }
        };
    }

    /// 017 FR-8: Verify empty and complex payloads serialize safely.
    #[test]
    fn test_ws_response_edge_cases_do_not_panic() {
        // Empty data
        let empty = WsResponse {
            stream: "depth.ETH_USDT".to_string(),
            data: serde_json::json!(null),
        };
        assert!(serde_json::to_string(&empty).is_ok());

        // Deeply nested data
        let nested = WsResponse {
            stream: "order.user123".to_string(),
            data: serde_json::json!({
                "a": {"b": {"c": {"d": {"e": "deep"}}}}
            }),
        };
        assert!(serde_json::to_string(&nested).is_ok());

        // Large array data
        let large = WsResponse {
            stream: "depth.SOL_USDT".to_string(),
            data: serde_json::json!({
                "bids": (0..1000).map(|i| [i, i * 10]).collect::<Vec<_>>(),
                "asks": (0..1000).map(|i| [i + 1000, i * 10]).collect::<Vec<_>>(),
            }),
        };
        assert!(serde_json::to_string(&large).is_ok());
    }
}
