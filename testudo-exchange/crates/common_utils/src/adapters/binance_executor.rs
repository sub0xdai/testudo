//! Binance Order Executor
//!
//! This module provides functionality to execute orders on Binance exchange.
//! It handles authentication, order submission, and error handling.

use super::ccxt_auth::CCXTAuthenticator;
use super::execution_types::{
    BinanceOrderResult, BinanceOrderStatus, ExecutionError, ExecutionOrderSide, ExecutionOrderType,
    ValidatedOrder,
};
use chrono::Utc;
use rust_decimal::Decimal;
#[cfg(feature = "real-api")]
use serde::Deserialize;
use std::time::Duration;

/// Binance API endpoints
pub const BINANCE_API_URL: &str = "https://api.binance.com";
pub const BINANCE_TESTNET_URL: &str = "https://testnet.binance.vision";

/// Binance order executor
#[derive(Debug)]
pub struct BinanceExecutor {
    /// HTTP client for API requests
    #[allow(dead_code)]
    client: reqwest::Client,
    /// Authenticator for signing requests
    auth: CCXTAuthenticator,
    /// Base URL (production or testnet)
    base_url: String,
    /// Whether to use testnet
    testnet: bool,
}

impl BinanceExecutor {
    /// Create a new Binance executor with API credentials
    ///
    /// Uses a 5-second timeout to prevent thread starvation during network congestion.
    /// See FR-2.1.2 in 006-performance-overhaul.
    pub fn new(api_key: String, api_secret: String) -> Result<Self, ExecutionError> {
        let auth = CCXTAuthenticator::binance(api_key, api_secret)
            .map_err(|_| ExecutionError::AuthenticationFailed)?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| ExecutionError::NetworkError(e.to_string()))?;

        Ok(Self {
            client,
            auth,
            base_url: BINANCE_API_URL.to_string(),
            testnet: false,
        })
    }

    /// Create executor for testnet
    pub fn testnet(api_key: String, api_secret: String) -> Result<Self, ExecutionError> {
        let mut executor = Self::new(api_key, api_secret)?;
        executor.base_url = BINANCE_TESTNET_URL.to_string();
        executor.testnet = true;
        Ok(executor)
    }

    /// Check if using testnet
    pub fn is_testnet(&self) -> bool {
        self.testnet
    }

    /// Execute a validated order on Binance
    ///
    /// # Arguments
    /// * `order` - A validated order ready for execution
    ///
    /// # Returns
    /// * `Ok(BinanceOrderResult)` - Order execution result from Binance
    /// * `Err(ExecutionError)` - Error during execution
    pub async fn execute(
        &self,
        order: &ValidatedOrder,
    ) -> Result<BinanceOrderResult, ExecutionError> {
        // Build order parameters as tuples
        let params = self.build_order_params_tuples(order);
        let params_refs: Vec<(&str, &str)> = params
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        // Sign the request
        let _signed_query = self
            .auth
            .sign_binance_request(&params_refs)
            .map_err(|_| ExecutionError::AuthenticationFailed)?;

        // Make the API call
        #[cfg(feature = "real-api")]
        {
            self.execute_real(order, &_signed_query).await
        }

        #[cfg(not(feature = "real-api"))]
        {
            self.execute_mock(order).await
        }
    }

    /// Get order status from Binance
    pub async fn get_order(
        &self,
        order_id: &str,
        symbol: &str,
    ) -> Result<BinanceOrderResult, ExecutionError> {
        let params = [
            ("symbol".to_string(), symbol.to_string()),
            ("orderId".to_string(), order_id.to_string()),
        ];
        let params_refs: Vec<(&str, &str)> = params
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        let _signed_query = self
            .auth
            .sign_binance_request(&params_refs)
            .map_err(|_| ExecutionError::AuthenticationFailed)?;

        #[cfg(feature = "real-api")]
        {
            self.get_order_real(&_signed_query).await
        }

        #[cfg(not(feature = "real-api"))]
        {
            self.get_order_mock(order_id, symbol).await
        }
    }

    /// Cancel an order on Binance
    pub async fn cancel(&self, order_id: &str, symbol: &str) -> Result<(), ExecutionError> {
        let params = [
            ("symbol".to_string(), symbol.to_string()),
            ("orderId".to_string(), order_id.to_string()),
        ];
        let params_refs: Vec<(&str, &str)> = params
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        let _signed_query = self
            .auth
            .sign_binance_request(&params_refs)
            .map_err(|_| ExecutionError::AuthenticationFailed)?;

        #[cfg(feature = "real-api")]
        {
            self.cancel_real(&_signed_query).await
        }

        #[cfg(not(feature = "real-api"))]
        {
            self.cancel_mock(order_id, symbol).await
        }
    }

    /// Build order parameters as Vec of tuples for signing
    fn build_order_params_tuples(&self, order: &ValidatedOrder) -> Vec<(String, String)> {
        let mut params = vec![
            ("symbol".to_string(), order.symbol.clone()),
            (
                "side".to_string(),
                match order.side {
                    ExecutionOrderSide::Buy => "BUY".to_string(),
                    ExecutionOrderSide::Sell => "SELL".to_string(),
                },
            ),
            (
                "type".to_string(),
                match order.order_type {
                    ExecutionOrderType::Market => "MARKET".to_string(),
                    ExecutionOrderType::Limit => "LIMIT".to_string(),
                },
            ),
            ("quantity".to_string(), order.quantity.to_string()),
        ];

        if let Some(price) = order.price {
            params.push(("price".to_string(), price.to_string()));
        }

        if order.order_type == ExecutionOrderType::Limit {
            params.push((
                "timeInForce".to_string(),
                format!("{:?}", order.time_in_force).to_uppercase(),
            ));
        }

        if let Some(ref client_id) = order.client_order_id {
            params.push(("newClientOrderId".to_string(), client_id.clone()));
        }

        params
    }

    // ==================== Mock implementations (for testing without real API) ====================

    #[cfg(not(feature = "real-api"))]
    async fn execute_mock(
        &self,
        order: &ValidatedOrder,
    ) -> Result<BinanceOrderResult, ExecutionError> {
        // Simulate successful order execution
        let order_id = format!("{}", Utc::now().timestamp_millis());
        let client_order_id = order
            .client_order_id
            .clone()
            .unwrap_or_else(|| format!("testudo_{}", &order_id));

        // For market orders, simulate immediate fill
        let (status, filled_qty, avg_price) = match order.order_type {
            ExecutionOrderType::Market => (
                BinanceOrderStatus::Filled,
                order.quantity,
                order.price.unwrap_or_else(|| Decimal::from(50000)), // Mock price
            ),
            ExecutionOrderType::Limit => (BinanceOrderStatus::New, Decimal::ZERO, Decimal::ZERO),
        };

        Ok(BinanceOrderResult {
            order_id,
            client_order_id,
            status,
            filled_qty,
            avg_price,
            timestamp: Utc::now().timestamp_millis(),
            symbol: order.symbol.clone(),
            side: order.side,
            original_qty: order.quantity,
        })
    }

    #[cfg(not(feature = "real-api"))]
    async fn get_order_mock(
        &self,
        order_id: &str,
        symbol: &str,
    ) -> Result<BinanceOrderResult, ExecutionError> {
        Ok(BinanceOrderResult {
            order_id: order_id.to_string(),
            client_order_id: format!("testudo_{}", order_id),
            status: BinanceOrderStatus::New,
            filled_qty: Decimal::ZERO,
            avg_price: Decimal::ZERO,
            timestamp: Utc::now().timestamp_millis(),
            symbol: symbol.to_string(),
            side: ExecutionOrderSide::Buy,
            original_qty: Decimal::from(1),
        })
    }

    #[cfg(not(feature = "real-api"))]
    async fn cancel_mock(&self, _order_id: &str, _symbol: &str) -> Result<(), ExecutionError> {
        Ok(())
    }

    // ==================== Real API implementations ====================

    #[cfg(feature = "real-api")]
    async fn execute_real(
        &self,
        order: &ValidatedOrder,
        params: &HashMap<String, String>,
    ) -> Result<BinanceOrderResult, ExecutionError> {
        let url = format!("{}/api/v3/order", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("X-MBX-APIKEY", self.auth.api_key())
            .form(params)
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        self.handle_response(response, order).await
    }

    #[cfg(feature = "real-api")]
    async fn get_order_real(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<BinanceOrderResult, ExecutionError> {
        let url = format!("{}/api/v3/order", self.base_url);
        let query_string: String = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let response = self
            .client
            .get(&format!("{}?{}", url, query_string))
            .header("X-MBX-APIKEY", self.auth.api_key())
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        self.handle_order_response(response).await
    }

    #[cfg(feature = "real-api")]
    async fn cancel_real(&self, params: &HashMap<String, String>) -> Result<(), ExecutionError> {
        let url = format!("{}/api/v3/order", self.base_url);

        let response = self
            .client
            .delete(&url)
            .header("X-MBX-APIKEY", self.auth.api_key())
            .form(params)
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let error = self.parse_error_response(response).await;
            Err(error)
        }
    }

    #[cfg(feature = "real-api")]
    fn map_reqwest_error(&self, error: reqwest::Error) -> ExecutionError {
        if error.is_timeout() {
            ExecutionError::Timeout
        } else if error.is_connect() {
            ExecutionError::ExchangeUnavailable
        } else {
            ExecutionError::NetworkError(error.to_string())
        }
    }

    #[cfg(feature = "real-api")]
    async fn handle_response(
        &self,
        response: reqwest::Response,
        order: &ValidatedOrder,
    ) -> Result<BinanceOrderResult, ExecutionError> {
        let status = response.status();

        if status.is_success() {
            let binance_response: BinanceApiOrderResponse = response
                .json()
                .await
                .map_err(|e| ExecutionError::NetworkError(e.to_string()))?;

            Ok(self.convert_response(binance_response, order))
        } else if status.as_u16() == 429 {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(1000);
            Err(ExecutionError::RateLimited {
                retry_after_ms: retry_after,
            })
        } else {
            self.parse_error_response(response).await
        }
    }

    #[cfg(feature = "real-api")]
    async fn handle_order_response(
        &self,
        response: reqwest::Response,
    ) -> Result<BinanceOrderResult, ExecutionError> {
        let status = response.status();

        if status.is_success() {
            let binance_response: BinanceApiOrderResponse = response
                .json()
                .await
                .map_err(|e| ExecutionError::NetworkError(e.to_string()))?;

            Ok(BinanceOrderResult {
                order_id: binance_response.order_id.to_string(),
                client_order_id: binance_response.client_order_id,
                status: Self::parse_status(&binance_response.status),
                filled_qty: binance_response.executed_qty,
                avg_price: binance_response.avg_price.unwrap_or(Decimal::ZERO),
                timestamp: binance_response.time,
                symbol: binance_response.symbol,
                side: if binance_response.side == "BUY" {
                    ExecutionOrderSide::Buy
                } else {
                    ExecutionOrderSide::Sell
                },
                original_qty: binance_response.orig_qty,
            })
        } else {
            Err(self.parse_error_response(response).await)
        }
    }

    #[cfg(feature = "real-api")]
    fn convert_response(
        &self,
        response: BinanceApiOrderResponse,
        order: &ValidatedOrder,
    ) -> BinanceOrderResult {
        BinanceOrderResult {
            order_id: response.order_id.to_string(),
            client_order_id: response.client_order_id,
            status: Self::parse_status(&response.status),
            filled_qty: response.executed_qty,
            avg_price: response.avg_price.unwrap_or(Decimal::ZERO),
            timestamp: response.time,
            symbol: order.symbol.clone(),
            side: order.side,
            original_qty: order.quantity,
        }
    }

    #[cfg(feature = "real-api")]
    fn parse_status(status: &str) -> BinanceOrderStatus {
        match status {
            "NEW" => BinanceOrderStatus::New,
            "PARTIALLY_FILLED" => BinanceOrderStatus::PartiallyFilled,
            "FILLED" => BinanceOrderStatus::Filled,
            "CANCELED" => BinanceOrderStatus::Canceled,
            "PENDING_CANCEL" => BinanceOrderStatus::PendingCancel,
            "REJECTED" => BinanceOrderStatus::Rejected,
            "EXPIRED" => BinanceOrderStatus::Expired,
            _ => BinanceOrderStatus::New,
        }
    }

    #[cfg(feature = "real-api")]
    async fn parse_error_response(&self, response: reqwest::Response) -> ExecutionError {
        let status = response.status();

        if let Ok(error_body) = response.json::<BinanceApiError>().await {
            match error_body.code {
                -2010 => ExecutionError::InsufficientBalance {
                    required: Decimal::ZERO,
                    available: Decimal::ZERO,
                },
                -1013 => ExecutionError::InvalidOrder(error_body.msg),
                -1121 => ExecutionError::InvalidSymbol(error_body.msg),
                -2015 => ExecutionError::AuthenticationFailed,
                _ => ExecutionError::OrderRejected {
                    code: error_body.code,
                    message: error_body.msg,
                },
            }
        } else {
            ExecutionError::NetworkError(format!("HTTP {}", status))
        }
    }
}

/// Binance API order response structure
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg(feature = "real-api")]
struct BinanceApiOrderResponse {
    order_id: i64,
    client_order_id: String,
    symbol: String,
    status: String,
    side: String,
    #[serde(rename = "type")]
    order_type: String,
    orig_qty: Decimal,
    executed_qty: Decimal,
    #[serde(default)]
    avg_price: Option<Decimal>,
    time: i64,
}

/// Binance API error response
#[derive(Debug, Deserialize)]
#[cfg(feature = "real-api")]
struct BinanceApiError {
    code: i32,
    msg: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // ==================== BinanceExecutor Creation Tests ====================

    #[test]
    fn test_executor_creation() {
        let executor =
            BinanceExecutor::new("test_api_key".to_string(), "test_api_secret".to_string());

        assert!(executor.is_ok());
        let executor = executor.unwrap();
        assert!(!executor.is_testnet());
        assert_eq!(executor.base_url, BINANCE_API_URL);
    }

    #[test]
    fn test_executor_testnet_creation() {
        let executor =
            BinanceExecutor::testnet("test_api_key".to_string(), "test_api_secret".to_string());

        assert!(executor.is_ok());
        let executor = executor.unwrap();
        assert!(executor.is_testnet());
        assert_eq!(executor.base_url, BINANCE_TESTNET_URL);
    }

    // ==================== Order Execution Tests ====================

    #[tokio::test]
    async fn test_execute_limit_order() {
        let executor =
            BinanceExecutor::new("test_api_key".to_string(), "test_api_secret".to_string())
                .unwrap();

        let order = ValidatedOrder::limit(
            "BTCUSDT".to_string(),
            ExecutionOrderSide::Buy,
            Decimal::from_str("0.001").unwrap(),
            Decimal::from_str("50000.00").unwrap(),
        );

        let result = executor.execute(&order).await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.symbol, "BTCUSDT");
        assert_eq!(result.side, ExecutionOrderSide::Buy);
        assert_eq!(result.original_qty, Decimal::from_str("0.001").unwrap());
        // Limit orders should be NEW, not filled immediately
        assert_eq!(result.status, BinanceOrderStatus::New);
    }

    #[tokio::test]
    async fn test_execute_market_order() {
        let executor =
            BinanceExecutor::new("test_api_key".to_string(), "test_api_secret".to_string())
                .unwrap();

        let order = ValidatedOrder::market(
            "BTCUSDT".to_string(),
            ExecutionOrderSide::Buy,
            Decimal::from_str("0.001").unwrap(),
        );

        let result = executor.execute(&order).await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.symbol, "BTCUSDT");
        assert_eq!(result.side, ExecutionOrderSide::Buy);
        // Market orders should fill immediately
        assert_eq!(result.status, BinanceOrderStatus::Filled);
        assert_eq!(result.filled_qty, result.original_qty);
    }

    #[tokio::test]
    async fn test_execute_market_sell() {
        let executor =
            BinanceExecutor::new("test_api_key".to_string(), "test_api_secret".to_string())
                .unwrap();

        let order = ValidatedOrder::market(
            "ETHUSDT".to_string(),
            ExecutionOrderSide::Sell,
            Decimal::from_str("1.5").unwrap(),
        );

        let result = executor.execute(&order).await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.symbol, "ETHUSDT");
        assert_eq!(result.side, ExecutionOrderSide::Sell);
        assert_eq!(result.status, BinanceOrderStatus::Filled);
    }

    #[tokio::test]
    async fn test_execute_with_client_order_id() {
        let executor =
            BinanceExecutor::new("test_api_key".to_string(), "test_api_secret".to_string())
                .unwrap();

        let order = ValidatedOrder::market(
            "BTCUSDT".to_string(),
            ExecutionOrderSide::Buy,
            Decimal::from_str("0.001").unwrap(),
        )
        .with_client_order_id("my-custom-id-123".to_string());

        let result = executor.execute(&order).await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(result.client_order_id.contains("my-custom-id-123"));
    }

    // ==================== Get Order Tests ====================

    #[tokio::test]
    async fn test_get_order() {
        let executor =
            BinanceExecutor::new("test_api_key".to_string(), "test_api_secret".to_string())
                .unwrap();

        let result = executor.get_order("12345", "BTCUSDT").await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.order_id, "12345");
        assert_eq!(result.symbol, "BTCUSDT");
    }

    // ==================== Cancel Order Tests ====================

    #[tokio::test]
    async fn test_cancel_order() {
        let executor =
            BinanceExecutor::new("test_api_key".to_string(), "test_api_secret".to_string())
                .unwrap();

        let result = executor.cancel("12345", "BTCUSDT").await;
        assert!(result.is_ok());
    }

    // ==================== Order Params Building Tests ====================

    fn find_param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn test_build_order_params_market() {
        let executor =
            BinanceExecutor::new("test_api_key".to_string(), "test_api_secret".to_string())
                .unwrap();

        let order = ValidatedOrder::market(
            "BTCUSDT".to_string(),
            ExecutionOrderSide::Buy,
            Decimal::from_str("0.001").unwrap(),
        );

        let params = executor.build_order_params_tuples(&order);

        assert_eq!(find_param(&params, "symbol"), Some("BTCUSDT"));
        assert_eq!(find_param(&params, "side"), Some("BUY"));
        assert_eq!(find_param(&params, "type"), Some("MARKET"));
        assert_eq!(find_param(&params, "quantity"), Some("0.001"));
        assert!(find_param(&params, "price").is_none());
    }

    #[test]
    fn test_build_order_params_limit() {
        let executor =
            BinanceExecutor::new("test_api_key".to_string(), "test_api_secret".to_string())
                .unwrap();

        let order = ValidatedOrder::limit(
            "BTCUSDT".to_string(),
            ExecutionOrderSide::Sell,
            Decimal::from_str("0.5").unwrap(),
            Decimal::from_str("60000.00").unwrap(),
        );

        let params = executor.build_order_params_tuples(&order);

        assert_eq!(find_param(&params, "symbol"), Some("BTCUSDT"));
        assert_eq!(find_param(&params, "side"), Some("SELL"));
        assert_eq!(find_param(&params, "type"), Some("LIMIT"));
        assert_eq!(find_param(&params, "quantity"), Some("0.5"));
        assert_eq!(find_param(&params, "price"), Some("60000.00"));
        assert!(find_param(&params, "timeInForce").is_some());
    }
}
