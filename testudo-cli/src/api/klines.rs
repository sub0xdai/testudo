// @anchor infra:cli:api:klines
// @tags api

//! GET /api/v1/klines — historical candlestick data.

use crate::api::client::ApiClient;
use crate::api::types::{ApiError, KlineData};

impl ApiClient {
    /// Get kline/candlestick data for a symbol.
    ///
    /// `interval`: "1min", "1h", "1d", "1w", "1m", "1y"
    /// `start_time`: optional ISO 8601 timestamp
    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<&str>,
    ) -> Result<Vec<KlineData>, ApiError> {
        let mut path = format!(
            "/api/v1/klines?symbol={}&interval={}",
            symbol, interval
        );
        if let Some(st) = start_time {
            path.push_str(&format!("&start_time={}", st));
        }
        self.get_json(&path).await
    }
}
