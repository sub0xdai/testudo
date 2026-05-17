//! Binance Futures Data Service
//!
//! Fetches live perpetual futures market data from Binance Futures API.
//! No authentication required for public market data.
//!
//! # Endpoints Used
//! - GET /fapi/v1/ticker/24hr - 24hr ticker price change
//! - GET /fapi/v1/depth - Order book depth
//! - GET /fapi/v1/klines - Candlestick/Kline data
//! - GET /fapi/v1/exchangeInfo - Exchange trading rules and symbol info

use crate::adapters::ccxt_types::{CCXTError, CCXTOrderBook, CCXTTicker};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Binance Futures API base URL
const BINANCE_API_URL: &str = "https://fapi.binance.com";

/// Candle/Kline data from Binance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub timestamp: i64,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub quote_volume: Decimal,
}

/// Market info from Binance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub symbol: String,
    pub base_asset: String,
    pub quote_asset: String,
    pub status: String,
}

/// Binance Data Service for fetching live market data
#[derive(Debug, Clone)]
pub struct BinanceDataService {
    client: reqwest::Client,
    base_url: String,
}

impl Default for BinanceDataService {
    fn default() -> Self {
        Self::new()
    }
}

impl BinanceDataService {
    /// Create a new BinanceDataService with default configuration
    ///
    /// Uses a 2-second timeout to prevent thread starvation during network congestion.
    /// See FR-2.1.1 in 006-performance-overhaul.
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: BINANCE_API_URL.to_string(),
        }
    }

    /// Create with custom base URL (for testing or alternate endpoints)
    ///
    /// Uses a 2-second timeout to prevent thread starvation during network congestion.
    /// See FR-2.1.1 in 006-performance-overhaul.
    pub fn with_base_url(base_url: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: base_url.to_string(),
        }
    }

    /// Convert symbol to Binance Futures format (BTCUSDT)
    ///
    /// Handles multiple input formats:
    /// - Native: BTCUSDT -> BTCUSDT
    /// - Underscore: BTC_USDT -> BTCUSDT
    /// - Legacy USDC: BTC_USDC -> BTCUSDT (converted to USDT)
    pub fn to_binance_symbol(symbol: &str) -> String {
        symbol
            .replace("_", "")
            .replace("/", "")
            .replace("USDC", "USDT")
    }

    /// Convert Binance symbol format (BTCUSDT) to internal format (BTC_USDT)
    pub fn from_binance_symbol(symbol: &str) -> String {
        // Handle common quote assets
        for quote in ["USDT", "USDC", "BUSD", "USD", "BTC", "ETH"] {
            if let Some(base) = symbol.strip_suffix(quote) {
                return format!("{}_{}", base, quote);
            }
        }
        symbol.to_string()
    }

    /// Fetch 24hr ticker data for a symbol with real bid/ask from orderbook
    ///
    /// # Arguments
    /// * `symbol` - Trading pair in internal format (e.g., "BTC_USDC")
    pub async fn get_ticker(&self, symbol: &str) -> Result<CCXTTicker, CCXTError> {
        let binance_symbol = Self::to_binance_symbol(symbol);

        // Fetch 24hr ticker for price stats
        let ticker_url = format!(
            "{}/fapi/v1/ticker/24hr?symbol={}",
            self.base_url, binance_symbol
        );

        let ticker_response =
            self.client
                .get(&ticker_url)
                .send()
                .await
                .map_err(|e| CCXTError::NetworkError {
                    message: format!("Failed to fetch ticker: {}", e),
                })?;

        if !ticker_response.status().is_success() {
            let error_text = ticker_response.text().await.unwrap_or_default();
            return Err(CCXTError::ExchangeError {
                message: format!("Binance API error: {}", error_text),
            });
        }

        let ticker_data: BinanceTickerResponse =
            ticker_response
                .json()
                .await
                .map_err(|e| CCXTError::NetworkError {
                    message: format!("Failed to parse ticker response: {}", e),
                })?;

        // Fetch top of book for real bid/ask prices
        let depth_url = format!(
            "{}/fapi/v1/depth?symbol={}&limit=5",
            self.base_url, binance_symbol
        );

        let (bid, ask) = match self.client.get(&depth_url).send().await {
            Ok(depth_response) if depth_response.status().is_success() => {
                match depth_response.json::<BinanceDepthResponse>().await {
                    Ok(depth_data) => {
                        let best_bid = depth_data
                            .bids
                            .first()
                            .and_then(|b| b.first())
                            .and_then(|p| parse_decimal_opt(p));
                        let best_ask = depth_data
                            .asks
                            .first()
                            .and_then(|a| a.first())
                            .and_then(|p| parse_decimal_opt(p));
                        (best_bid, best_ask)
                    }
                    Err(_) => (None, None),
                }
            }
            _ => (None, None),
        };

        Ok(CCXTTicker {
            symbol: symbol.to_string(),
            bid, // Real best bid from orderbook
            ask, // Real best ask from orderbook
            last: parse_decimal_opt(&ticker_data.last_price),
            high: parse_decimal_opt(&ticker_data.high_price), // Actual 24h high
            low: parse_decimal_opt(&ticker_data.low_price),   // Actual 24h low
            base_volume: parse_decimal_opt(&ticker_data.volume),
            quote_volume: parse_decimal_opt(&ticker_data.quote_volume),
            percentage: parse_decimal_opt(&ticker_data.price_change_percent),
            timestamp: chrono::Utc::now().timestamp_millis(),
        })
    }

    /// Fetch order book depth for a symbol
    ///
    /// # Arguments
    /// * `symbol` - Trading pair in internal format (e.g., "BTC_USDC")
    /// * `limit` - Number of price levels (5, 10, 20, 50, 100, 500, 1000, 5000)
    pub async fn get_orderbook(
        &self,
        symbol: &str,
        limit: Option<i32>,
    ) -> Result<CCXTOrderBook, CCXTError> {
        let binance_symbol = Self::to_binance_symbol(symbol);
        let limit = limit.unwrap_or(20).min(1000);
        let url = format!(
            "{}/fapi/v1/depth?symbol={}&limit={}",
            self.base_url, binance_symbol, limit
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CCXTError::NetworkError {
                message: format!("Failed to fetch orderbook: {}", e),
            })?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(CCXTError::ExchangeError {
                message: format!("Binance API error: {}", error_text),
            });
        }

        let data: BinanceDepthResponse =
            response.json().await.map_err(|e| CCXTError::NetworkError {
                message: format!("Failed to parse orderbook response: {}", e),
            })?;

        let mut orderbook = CCXTOrderBook::new(symbol.to_string());
        orderbook.nonce = Some(data.last_update_id);

        // Parse bids (buy orders) - already sorted high to low by Binance
        for bid in data.bids {
            if bid.len() >= 2 {
                if let (Some(price), Some(qty)) =
                    (parse_decimal_opt(&bid[0]), parse_decimal_opt(&bid[1]))
                {
                    orderbook.add_bid(price, qty);
                }
            }
        }

        // Parse asks (sell orders) - already sorted low to high by Binance
        for ask in data.asks {
            if ask.len() >= 2 {
                if let (Some(price), Some(qty)) =
                    (parse_decimal_opt(&ask[0]), parse_decimal_opt(&ask[1]))
                {
                    orderbook.add_ask(price, qty);
                }
            }
        }

        Ok(orderbook)
    }

    /// Fetch candlestick/kline data for a symbol
    ///
    /// # Arguments
    /// * `symbol` - Trading pair in internal format (e.g., "BTC_USDC")
    /// * `interval` - Kline interval (1m, 5m, 15m, 1h, 4h, 1d, 1w, 1M)
    /// * `limit` - Number of candles to fetch (max 1000)
    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: Option<i32>,
    ) -> Result<Vec<Candle>, CCXTError> {
        let binance_symbol = Self::to_binance_symbol(symbol);
        let limit = limit.unwrap_or(100).min(1000);

        // Map internal interval format to Binance format
        let binance_interval = match interval {
            "1m" | "1min" => "1m",
            "5m" | "5min" => "5m",
            "15m" | "15min" => "15m",
            "1h" | "hour" | "60m" => "1h",
            "4h" | "4hour" => "4h",
            "1d" | "day" | "1D" => "1d",
            "1w" | "week" | "1W" => "1w",
            "1M" | "month" => "1M",
            other => other,
        };

        let url = format!(
            "{}/fapi/v1/klines?symbol={}&interval={}&limit={}",
            self.base_url, binance_symbol, binance_interval, limit
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CCXTError::NetworkError {
                message: format!("Failed to fetch klines: {}", e),
            })?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(CCXTError::ExchangeError {
                message: format!("Binance API error: {}", error_text),
            });
        }

        let data: Vec<Vec<serde_json::Value>> =
            response.json().await.map_err(|e| CCXTError::NetworkError {
                message: format!("Failed to parse klines response: {}", e),
            })?;

        let candles: Vec<Candle> = data
            .into_iter()
            .filter_map(|kline| {
                if kline.len() < 7 {
                    return None;
                }
                Some(Candle {
                    timestamp: kline[0].as_i64()?,
                    open: parse_value_to_decimal(&kline[1])?,
                    high: parse_value_to_decimal(&kline[2])?,
                    low: parse_value_to_decimal(&kline[3])?,
                    close: parse_value_to_decimal(&kline[4])?,
                    volume: parse_value_to_decimal(&kline[5])?,
                    quote_volume: parse_value_to_decimal(&kline[7])?,
                })
            })
            .collect();

        Ok(candles)
    }

    /// Fetch list of supported markets from Binance
    pub async fn get_markets(&self) -> Result<Vec<Market>, CCXTError> {
        let url = format!("{}/fapi/v1/exchangeInfo", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CCXTError::NetworkError {
                message: format!("Failed to fetch exchange info: {}", e),
            })?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(CCXTError::ExchangeError {
                message: format!("Binance API error: {}", error_text),
            });
        }

        let data: BinanceExchangeInfoResponse =
            response.json().await.map_err(|e| CCXTError::NetworkError {
                message: format!("Failed to parse exchange info: {}", e),
            })?;

        // Filter to only USDT perpetual futures that are actively trading
        let markets: Vec<Market> = data
            .symbols
            .into_iter()
            .filter(|s| {
                s.quote_asset == "USDT"
                    && s.status == "TRADING"
                    && s.contract_type.as_deref() == Some("PERPETUAL")
            })
            .map(|s| Market {
                symbol: s.symbol.clone(), // Use Binance symbol directly (e.g., BTCUSDT)
                base_asset: s.base_asset,
                quote_asset: s.quote_asset,
                status: s.status,
            })
            .collect();

        Ok(markets)
    }
}

// Binance API Response Types

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceTickerResponse {
    #[allow(dead_code)]
    symbol: String,
    #[allow(dead_code)]
    price_change: String,
    price_change_percent: String,
    #[allow(dead_code)]
    weighted_avg_price: String,
    last_price: String,
    #[allow(dead_code)]
    open_price: String,
    high_price: String,
    low_price: String,
    volume: String,
    quote_volume: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceDepthResponse {
    last_update_id: i64,
    bids: Vec<Vec<String>>,
    asks: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct BinanceExchangeInfoResponse {
    symbols: Vec<BinanceSymbolInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceSymbolInfo {
    symbol: String,
    status: String,
    base_asset: String,
    quote_asset: String,
    #[serde(default)]
    contract_type: Option<String>,
}

// Helper functions

fn parse_decimal_opt(s: &str) -> Option<Decimal> {
    Decimal::from_str(s).ok()
}

fn parse_value_to_decimal(v: &serde_json::Value) -> Option<Decimal> {
    match v {
        serde_json::Value::String(s) => Decimal::from_str(s).ok(),
        serde_json::Value::Number(n) => n.as_f64().and_then(|f| Decimal::try_from(f).ok()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_conversion() {
        assert_eq!(BinanceDataService::to_binance_symbol("BTC_USDC"), "BTCUSDT");
        assert_eq!(BinanceDataService::to_binance_symbol("ETH_USDC"), "ETHUSDT");
        assert_eq!(BinanceDataService::to_binance_symbol("SOL_USDC"), "SOLUSDT");
        assert_eq!(BinanceDataService::to_binance_symbol("BTC/USDC"), "BTCUSDT");
    }

    #[test]
    fn test_from_binance_symbol() {
        assert_eq!(
            BinanceDataService::from_binance_symbol("BTCUSDT"),
            "BTC_USDT"
        );
        assert_eq!(
            BinanceDataService::from_binance_symbol("ETHUSDT"),
            "ETH_USDT"
        );
        assert_eq!(BinanceDataService::from_binance_symbol("SOLBTC"), "SOL_BTC");
    }

    #[tokio::test]
    #[ignore] // Requires network access
    async fn test_get_ticker_live() {
        let service = BinanceDataService::new();
        let ticker = service.get_ticker("BTC_USDC").await;
        assert!(ticker.is_ok());
        let ticker = ticker.unwrap();
        assert!(ticker.last.is_some());
        assert!(ticker.bid.is_some());
        assert!(ticker.ask.is_some());
    }

    #[tokio::test]
    #[ignore] // Requires network access
    async fn test_get_orderbook_live() {
        let service = BinanceDataService::new();
        let orderbook = service.get_orderbook("BTC_USDC", Some(10)).await;
        assert!(orderbook.is_ok());
        let orderbook = orderbook.unwrap();
        assert!(!orderbook.bids.is_empty());
        assert!(!orderbook.asks.is_empty());
    }

    #[tokio::test]
    #[ignore] // Requires network access
    async fn test_get_klines_live() {
        let service = BinanceDataService::new();
        let klines = service.get_klines("BTC_USDC", "1h", Some(10)).await;
        assert!(klines.is_ok());
        let klines = klines.unwrap();
        assert!(!klines.is_empty());
    }

    #[tokio::test]
    #[ignore] // Requires network access
    async fn test_get_markets_live() {
        let service = BinanceDataService::new();
        let markets = service.get_markets().await;
        assert!(markets.is_ok());
        let markets = markets.unwrap();
        assert!(!markets.is_empty());
    }
}
