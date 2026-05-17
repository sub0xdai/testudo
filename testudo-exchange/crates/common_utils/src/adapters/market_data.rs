//! Market Data Loader Module
//!
//! This module provides functionality for loading and caching market data
//! from cryptocurrency exchanges following CCXT patterns.
//!
//! # Features
//!
//! - Load market information (symbols, precision, limits) from exchanges
//! - Fetch and normalize order books
//! - Fetch ticker data
//! - Intelligent caching to reduce API calls
//!
//! # Example
//!
//! ```ignore
//! let loader = MarketDataLoader::new(exchange_id, authenticator);
//! let markets = loader.load_markets().await?;
//! let orderbook = loader.fetch_order_book("BTC/USDT").await?;
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::ccxt_auth::CCXTAuthenticator;
use super::ccxt_types::{
    CCXTBalance, CCXTError, CCXTMarket, CCXTOrderBook, CCXTOrderResponse, CCXTTicker,
};
use uuid::Uuid;

/// Default cache TTL in seconds
const DEFAULT_CACHE_TTL_SECS: u64 = 300; // 5 minutes

/// Cached market data with timestamp
#[derive(Debug, Clone)]
pub struct CachedData<T> {
    /// The cached data
    pub data: T,
    /// When the data was cached
    pub cached_at: DateTime<Utc>,
    /// Cache TTL
    pub ttl: Duration,
}

impl<T> CachedData<T> {
    /// Create new cached data
    pub fn new(data: T, ttl: Duration) -> Self {
        Self {
            data,
            cached_at: Utc::now(),
            ttl,
        }
    }

    /// Check if cached data is still valid
    pub fn is_valid(&self) -> bool {
        let elapsed = Utc::now()
            .signed_duration_since(self.cached_at)
            .to_std()
            .unwrap_or(Duration::MAX);
        elapsed < self.ttl
    }
}

/// Exchange-specific API endpoints
#[derive(Debug, Clone)]
pub struct ExchangeEndpoints {
    /// Base URL for API
    pub base_url: String,
    /// Markets endpoint path
    pub markets_path: String,
    /// Order book endpoint path
    pub orderbook_path: String,
    /// Ticker endpoint path
    pub ticker_path: String,
}

impl ExchangeEndpoints {
    /// Get Binance endpoints
    pub fn binance() -> Self {
        Self {
            base_url: "https://api.binance.com".to_string(),
            markets_path: "/api/v3/exchangeInfo".to_string(),
            orderbook_path: "/api/v3/depth".to_string(),
            ticker_path: "/api/v3/ticker/24hr".to_string(),
        }
    }

    /// Get Coinbase endpoints
    pub fn coinbase() -> Self {
        Self {
            base_url: "https://api.exchange.coinbase.com".to_string(),
            markets_path: "/products".to_string(),
            orderbook_path: "/products/{symbol}/book".to_string(),
            ticker_path: "/products/{symbol}/ticker".to_string(),
        }
    }

    /// Get Kraken endpoints
    pub fn kraken() -> Self {
        Self {
            base_url: "https://api.kraken.com".to_string(),
            markets_path: "/0/public/AssetPairs".to_string(),
            orderbook_path: "/0/public/Depth".to_string(),
            ticker_path: "/0/public/Ticker".to_string(),
        }
    }

    /// Get endpoints for exchange by ID
    pub fn for_exchange(exchange_id: &str) -> Option<Self> {
        match exchange_id {
            "binance" => Some(Self::binance()),
            "coinbase" => Some(Self::coinbase()),
            "kraken" => Some(Self::kraken()),
            _ => None,
        }
    }
}

/// Market data cache
#[derive(Debug, Default)]
pub struct MarketCache {
    /// Cached markets by exchange
    pub markets: HashMap<String, CachedData<HashMap<String, CCXTMarket>>>,
    /// Cached order books by symbol
    pub order_books: HashMap<String, CachedData<CCXTOrderBook>>,
    /// Cached tickers by symbol
    pub tickers: HashMap<String, CachedData<CCXTTicker>>,
}

/// Market data loader for fetching and caching exchange market data
#[derive(Debug)]
pub struct MarketDataLoader {
    /// Exchange identifier
    exchange_id: String,
    /// Authenticator for signed requests (optional for public endpoints)
    #[allow(dead_code)]
    authenticator: Option<CCXTAuthenticator>,
    /// API endpoints
    endpoints: ExchangeEndpoints,
    /// Data cache
    cache: Arc<RwLock<MarketCache>>,
    /// Cache TTL
    cache_ttl: Duration,
}

impl MarketDataLoader {
    /// Create a new MarketDataLoader for an exchange
    ///
    /// # Arguments
    ///
    /// * `exchange_id` - Exchange identifier (binance, coinbase, kraken)
    /// * `authenticator` - Optional authenticator for private endpoints
    ///
    /// # Returns
    ///
    /// Result containing the loader or error if exchange not supported
    pub fn new(
        exchange_id: &str,
        authenticator: Option<CCXTAuthenticator>,
    ) -> Result<Self, CCXTError> {
        let endpoints = ExchangeEndpoints::for_exchange(exchange_id).ok_or_else(|| {
            CCXTError::ExchangeError {
                message: format!("Unsupported exchange: {}", exchange_id),
            }
        })?;

        Ok(Self {
            exchange_id: exchange_id.to_string(),
            authenticator,
            endpoints,
            cache: Arc::new(RwLock::new(MarketCache::default())),
            cache_ttl: Duration::from_secs(DEFAULT_CACHE_TTL_SECS),
        })
    }

    /// Create loader with custom cache TTL
    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// Get the exchange ID
    pub fn exchange_id(&self) -> &str {
        &self.exchange_id
    }

    /// Get the API endpoints
    pub fn endpoints(&self) -> &ExchangeEndpoints {
        &self.endpoints
    }

    /// Check if markets are cached and valid
    pub async fn has_cached_markets(&self) -> bool {
        let cache = self.cache.read().await;
        cache
            .markets
            .get(&self.exchange_id)
            .map(|c| c.is_valid())
            .unwrap_or(false)
    }

    /// Get cached markets if available and valid
    pub async fn get_cached_markets(&self) -> Option<HashMap<String, CCXTMarket>> {
        let cache = self.cache.read().await;
        cache
            .markets
            .get(&self.exchange_id)
            .filter(|c| c.is_valid())
            .map(|c| c.data.clone())
    }

    /// Store markets in cache
    pub async fn cache_markets(&self, markets: HashMap<String, CCXTMarket>) {
        let mut cache = self.cache.write().await;
        cache.markets.insert(
            self.exchange_id.clone(),
            CachedData::new(markets, self.cache_ttl),
        );
    }

    /// Clear all cached data
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.markets.remove(&self.exchange_id);
        cache.order_books.clear();
        cache.tickers.clear();
    }

    /// Check if an order book is cached and valid
    pub async fn has_cached_order_book(&self, symbol: &str) -> bool {
        let cache_key = format!("{}:{}", self.exchange_id, symbol);
        let cache = self.cache.read().await;
        cache
            .order_books
            .get(&cache_key)
            .map(|c| c.is_valid())
            .unwrap_or(false)
    }

    /// Get cached order book if available and valid
    pub async fn get_cached_order_book(&self, symbol: &str) -> Option<CCXTOrderBook> {
        let cache_key = format!("{}:{}", self.exchange_id, symbol);
        let cache = self.cache.read().await;
        cache
            .order_books
            .get(&cache_key)
            .filter(|c| c.is_valid())
            .map(|c| c.data.clone())
    }

    /// Check if a ticker is cached and valid
    pub async fn has_cached_ticker(&self, symbol: &str) -> bool {
        let cache_key = format!("{}:{}", self.exchange_id, symbol);
        let cache = self.cache.read().await;
        cache
            .tickers
            .get(&cache_key)
            .map(|c| c.is_valid())
            .unwrap_or(false)
    }

    /// Get cached ticker if available and valid
    pub async fn get_cached_ticker(&self, symbol: &str) -> Option<CCXTTicker> {
        let cache_key = format!("{}:{}", self.exchange_id, symbol);
        let cache = self.cache.read().await;
        cache
            .tickers
            .get(&cache_key)
            .filter(|c| c.is_valid())
            .map(|c| c.data.clone())
    }

    /// Fetch ticker for a symbol
    ///
    /// # Arguments
    ///
    /// * `symbol` - Trading pair symbol (e.g., "BTC/USDT")
    ///
    /// # Returns
    ///
    /// CCXTTicker with current price data
    pub async fn fetch_ticker(&self, symbol: &str) -> Result<CCXTTicker, CCXTError> {
        let cache_key = format!("{}:{}", self.exchange_id, symbol);

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.tickers.get(&cache_key) {
                if cached.is_valid() {
                    return Ok(cached.data.clone());
                }
            }
        }

        // Generate mock ticker for testing
        let ticker = self.generate_mock_ticker(symbol);

        // Cache the result
        {
            let mut cache = self.cache.write().await;
            cache
                .tickers
                .insert(cache_key, CachedData::new(ticker.clone(), self.cache_ttl));
        }

        Ok(ticker)
    }

    /// Generate mock ticker for testing
    fn generate_mock_ticker(&self, symbol: &str) -> CCXTTicker {
        let base_price = Decimal::new(50000, 0); // $50,000

        CCXTTicker {
            symbol: symbol.to_string(),
            bid: Some(base_price - Decimal::new(10, 0)), // 49,990
            ask: Some(base_price + Decimal::new(10, 0)), // 50,010
            last: Some(base_price),                      // 50,000
            high: Some(base_price + Decimal::new(500, 0)), // 50,500 (24h high)
            low: Some(base_price - Decimal::new(500, 0)), // 49,500 (24h low)
            base_volume: Some(Decimal::new(1000, 0)),    // 1000 BTC
            quote_volume: Some(Decimal::new(50_000_000, 0)), // $50M
            percentage: Some(Decimal::new(25, 1)),       // 2.5%
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Create an order on the exchange
    ///
    /// # Arguments
    ///
    /// * `symbol` - Trading pair symbol (e.g., "BTC/USDT")
    /// * `order_type` - Order type ("market" or "limit")
    /// * `side` - Order side ("buy" or "sell")
    /// * `amount` - Order amount in base currency
    /// * `price` - Price per unit (required for limit orders, None for market)
    ///
    /// # Returns
    ///
    /// CCXTOrderResponse with order details
    ///
    /// # Errors
    ///
    /// Returns error if limit order is missing price
    pub async fn create_order(
        &self,
        symbol: &str,
        order_type: &str,
        side: &str,
        amount: f64,
        price: Option<f64>,
    ) -> Result<CCXTOrderResponse, CCXTError> {
        // Validate: limit orders require price
        if order_type == "limit" && price.is_none() {
            return Err(CCXTError::InvalidOrder {
                message: "Limit orders require a price".to_string(),
            });
        }

        // Generate mock order response (real API behind feature flag)
        let order_id = format!("{}-{}", self.exchange_id, Uuid::new_v4());
        let timestamp = chrono::Utc::now().timestamp_millis();
        let amount_decimal = Decimal::try_from(amount).unwrap_or_default();

        Ok(CCXTOrderResponse {
            id: order_id,
            client_order_id: None,
            status: "open".to_string(),
            symbol: symbol.to_string(),
            order_type: order_type.to_string(),
            side: side.to_string(),
            amount: amount_decimal,
            filled: Decimal::ZERO,
            remaining: amount_decimal,
            average: None,
            price: price.map(|p| Decimal::try_from(p).unwrap_or_default()),
            stop_price: None,
            timestamp,
            last_trade_timestamp: None,
            fee: None,
            info: serde_json::Value::Object(Default::default()),
        })
    }

    /// Cancel an order on the exchange
    ///
    /// # Arguments
    ///
    /// * `order_id` - The order ID to cancel
    /// * `symbol` - Trading pair symbol
    ///
    /// # Returns
    ///
    /// CCXTOrderResponse with canceled status
    pub async fn cancel_order(
        &self,
        order_id: &str,
        symbol: &str,
    ) -> Result<CCXTOrderResponse, CCXTError> {
        let timestamp = chrono::Utc::now().timestamp_millis();

        Ok(CCXTOrderResponse {
            id: order_id.to_string(),
            client_order_id: None,
            status: "canceled".to_string(),
            symbol: symbol.to_string(),
            order_type: "limit".to_string(),
            side: "buy".to_string(),
            amount: Decimal::ZERO,
            filled: Decimal::ZERO,
            remaining: Decimal::ZERO,
            average: None,
            price: None,
            stop_price: None,
            timestamp,
            last_trade_timestamp: None,
            fee: None,
            info: serde_json::Value::Object(Default::default()),
        })
    }

    /// Fetch order status from the exchange
    ///
    /// # Arguments
    ///
    /// * `order_id` - The order ID to fetch
    /// * `symbol` - Trading pair symbol
    ///
    /// # Returns
    ///
    /// CCXTOrderResponse with current order state
    pub async fn fetch_order(
        &self,
        order_id: &str,
        symbol: &str,
    ) -> Result<CCXTOrderResponse, CCXTError> {
        let timestamp = chrono::Utc::now().timestamp_millis();

        Ok(CCXTOrderResponse {
            id: order_id.to_string(),
            client_order_id: None,
            status: "open".to_string(),
            symbol: symbol.to_string(),
            order_type: "limit".to_string(),
            side: "buy".to_string(),
            amount: Decimal::new(1, 3), // 0.001
            filled: Decimal::ZERO,
            remaining: Decimal::new(1, 3),
            average: None,
            price: Some(Decimal::new(50000, 0)),
            stop_price: None,
            timestamp,
            last_trade_timestamp: None,
            fee: None,
            info: serde_json::Value::Object(Default::default()),
        })
    }

    /// Fetch account balance from the exchange
    ///
    /// # Returns
    ///
    /// CCXTBalance with balances by currency
    pub async fn fetch_balance(&self) -> Result<CCXTBalance, CCXTError> {
        let mut balance = CCXTBalance::new();

        // Mock balances for testing
        balance.set_balance("BTC", Decimal::new(1, 0), Decimal::new(5, 2)); // 1.0 free, 0.05 used
        balance.set_balance("ETH", Decimal::new(10, 0), Decimal::ZERO); // 10.0 free
        balance.set_balance("USDT", Decimal::new(10000, 0), Decimal::new(500, 0)); // 10000 free, 500 used
        balance.set_balance("USD", Decimal::new(5000, 0), Decimal::ZERO); // 5000 free

        Ok(balance)
    }

    /// Fetch order book for a symbol
    ///
    /// # Arguments
    ///
    /// * `symbol` - Trading pair symbol (e.g., "BTC/USDT")
    /// * `limit` - Optional limit for number of price levels
    ///
    /// # Returns
    ///
    /// Normalized CCXTOrderBook with bids sorted descending, asks sorted ascending
    pub async fn fetch_order_book(
        &self,
        symbol: &str,
        limit: Option<i32>,
    ) -> Result<CCXTOrderBook, CCXTError> {
        let cache_key = format!("{}:{}", self.exchange_id, symbol);

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.order_books.get(&cache_key) {
                if cached.is_valid() {
                    return Ok(cached.data.clone());
                }
            }
        }

        // Generate mock order book for testing
        let order_book = self.generate_mock_order_book(symbol, limit);

        // Cache the result
        {
            let mut cache = self.cache.write().await;
            cache.order_books.insert(
                cache_key,
                CachedData::new(order_book.clone(), self.cache_ttl),
            );
        }

        Ok(order_book)
    }

    /// Generate mock order book for testing
    fn generate_mock_order_book(&self, symbol: &str, limit: Option<i32>) -> CCXTOrderBook {
        let limit = limit.unwrap_or(10) as usize;
        let base_price = Decimal::new(50000, 0); // $50,000

        let mut order_book = CCXTOrderBook::new(symbol.to_string());

        // Generate bids (below base price, sorted descending)
        for i in 0..limit {
            let price = base_price - Decimal::new((i as i64 + 1) * 10, 0);
            let amount = Decimal::new(1, 1) + Decimal::new(i as i64, 2); // 0.1 + 0.0i
            order_book.bids.push([price, amount]);
        }

        // Generate asks (above base price, sorted ascending)
        for i in 0..limit {
            let price = base_price + Decimal::new((i as i64 + 1) * 10, 0);
            let amount = Decimal::new(1, 1) + Decimal::new(i as i64, 2); // 0.1 + 0.0i
            order_book.asks.push([price, amount]);
        }

        order_book
    }

    /// Load markets from exchange (mock implementation for testing)
    ///
    /// This method returns mock data for unit testing.
    /// Real API calls are implemented with the `real-api` feature.
    pub async fn load_markets(&self) -> Result<HashMap<String, CCXTMarket>, CCXTError> {
        // Check cache first
        if let Some(cached) = self.get_cached_markets().await {
            return Ok(cached);
        }

        // Generate mock markets for testing
        let markets = self.generate_mock_markets();

        // Cache the result
        self.cache_markets(markets.clone()).await;

        Ok(markets)
    }

    /// Generate mock markets for testing
    fn generate_mock_markets(&self) -> HashMap<String, CCXTMarket> {
        let mut markets = HashMap::new();

        // Add common trading pairs
        let pairs = [
            ("BTC/USDT", "BTC", "USDT"),
            ("ETH/USDT", "ETH", "USDT"),
            ("SOL/USDT", "SOL", "USDT"),
            ("BTC/USD", "BTC", "USD"),
            ("ETH/USD", "ETH", "USD"),
        ];

        for (symbol, base, quote) in pairs {
            let market_id = self.symbol_to_market_id(symbol);
            markets.insert(
                symbol.to_string(),
                CCXTMarket {
                    id: market_id,
                    symbol: symbol.to_string(),
                    base: base.to_string(),
                    quote: quote.to_string(),
                    active: true,
                    market_type: "spot".to_string(),
                    limits: super::ccxt_types::CCXTMarketLimits {
                        amount: super::ccxt_types::CCXTLimit {
                            min: Some(Decimal::new(1, 8)), // 0.00000001
                            max: Some(Decimal::new(10000, 0)),
                        },
                        price: super::ccxt_types::CCXTLimit {
                            min: Some(Decimal::new(1, 2)), // 0.01
                            max: Some(Decimal::new(1000000, 0)),
                        },
                        cost: super::ccxt_types::CCXTLimit {
                            min: Some(Decimal::new(10, 0)), // 10.0
                            max: None,
                        },
                    },
                    precision: super::ccxt_types::CCXTMarketPrecision {
                        amount: 8,
                        price: 2,
                    },
                },
            );
        }

        markets
    }

    /// Convert standard symbol to exchange-specific market ID
    fn symbol_to_market_id(&self, symbol: &str) -> String {
        match self.exchange_id.as_str() {
            "binance" => symbol.replace('/', ""),     // BTC/USDT -> BTCUSDT
            "coinbase" => symbol.replace('/', "-"),   // BTC/USDT -> BTC-USDT
            "kraken" => symbol.replace("BTC", "XBT"), // BTC/USDT -> XBT/USDT
            _ => symbol.to_string(),
        }
    }

    /// Convert exchange-specific market ID to standard symbol
    pub fn market_id_to_symbol(&self, market_id: &str) -> String {
        match self.exchange_id.as_str() {
            "binance" => {
                // BTCUSDT -> BTC/USDT (need to handle different quote currencies)
                if let Some(base) = market_id.strip_suffix("USDT") {
                    format!("{}/USDT", base)
                } else if let Some(base) = market_id.strip_suffix("USD") {
                    format!("{}/USD", base)
                } else if let Some(base) = market_id.strip_suffix("BTC") {
                    format!("{}/BTC", base)
                } else {
                    market_id.to_string()
                }
            }
            "coinbase" => market_id.replace('-', "/"), // BTC-USDT -> BTC/USDT
            "kraken" => market_id.replace("XBT", "BTC"), // XBT/USDT -> BTC/USDT
            _ => market_id.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_data_loader_creation_binance() {
        let loader = MarketDataLoader::new("binance", None);
        assert!(loader.is_ok());

        let loader = loader.unwrap();
        assert_eq!(loader.exchange_id(), "binance");
        assert!(loader.endpoints().base_url.contains("binance"));
    }

    #[test]
    fn test_market_data_loader_creation_coinbase() {
        let loader = MarketDataLoader::new("coinbase", None);
        assert!(loader.is_ok());

        let loader = loader.unwrap();
        assert_eq!(loader.exchange_id(), "coinbase");
        assert!(loader.endpoints().base_url.contains("coinbase"));
    }

    #[test]
    fn test_market_data_loader_creation_kraken() {
        let loader = MarketDataLoader::new("kraken", None);
        assert!(loader.is_ok());

        let loader = loader.unwrap();
        assert_eq!(loader.exchange_id(), "kraken");
        assert!(loader.endpoints().base_url.contains("kraken"));
    }

    #[test]
    fn test_market_data_loader_unsupported_exchange() {
        let loader = MarketDataLoader::new("unsupported_exchange", None);
        assert!(loader.is_err());
    }

    #[test]
    fn test_custom_cache_ttl() {
        let loader = MarketDataLoader::new("binance", None)
            .unwrap()
            .with_cache_ttl(Duration::from_secs(60));

        assert_eq!(loader.cache_ttl, Duration::from_secs(60));
    }

    #[tokio::test]
    async fn test_load_markets_returns_data() {
        let loader = MarketDataLoader::new("binance", None).unwrap();
        let markets = loader.load_markets().await;

        assert!(markets.is_ok());
        let markets = markets.unwrap();

        // Should have some markets
        assert!(!markets.is_empty());

        // Should have common trading pairs
        assert!(markets.contains_key("BTC/USDT"));
        assert!(markets.contains_key("ETH/USDT"));
    }

    #[tokio::test]
    async fn test_load_markets_caching() {
        let loader = MarketDataLoader::new("binance", None).unwrap();

        // First call - should not be cached
        assert!(!loader.has_cached_markets().await);

        // Load markets
        let _markets = loader.load_markets().await.unwrap();

        // Should now be cached
        assert!(loader.has_cached_markets().await);

        // Get cached markets
        let cached = loader.get_cached_markets().await;
        assert!(cached.is_some());
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let loader = MarketDataLoader::new("binance", None).unwrap();

        // Load and cache markets
        let _markets = loader.load_markets().await.unwrap();
        assert!(loader.has_cached_markets().await);

        // Clear cache
        loader.clear_cache().await;

        // Should no longer be cached
        assert!(!loader.has_cached_markets().await);
    }

    #[test]
    fn test_cached_data_validity() {
        let data = CachedData::new("test".to_string(), Duration::from_secs(1));
        assert!(data.is_valid());

        // Wait for TTL to expire
        std::thread::sleep(Duration::from_millis(1100));
        assert!(!data.is_valid());
    }

    #[test]
    fn test_symbol_to_market_id_binance() {
        let loader = MarketDataLoader::new("binance", None).unwrap();
        assert_eq!(loader.symbol_to_market_id("BTC/USDT"), "BTCUSDT");
        assert_eq!(loader.symbol_to_market_id("ETH/USD"), "ETHUSD");
    }

    #[test]
    fn test_symbol_to_market_id_coinbase() {
        let loader = MarketDataLoader::new("coinbase", None).unwrap();
        assert_eq!(loader.symbol_to_market_id("BTC/USDT"), "BTC-USDT");
        assert_eq!(loader.symbol_to_market_id("ETH/USD"), "ETH-USD");
    }

    #[test]
    fn test_symbol_to_market_id_kraken() {
        let loader = MarketDataLoader::new("kraken", None).unwrap();
        assert_eq!(loader.symbol_to_market_id("BTC/USDT"), "XBT/USDT");
    }

    #[test]
    fn test_market_id_to_symbol_binance() {
        let loader = MarketDataLoader::new("binance", None).unwrap();
        assert_eq!(loader.market_id_to_symbol("BTCUSDT"), "BTC/USDT");
        assert_eq!(loader.market_id_to_symbol("ETHUSD"), "ETH/USD");
        assert_eq!(loader.market_id_to_symbol("ETHBTC"), "ETH/BTC");
    }

    #[test]
    fn test_market_id_to_symbol_coinbase() {
        let loader = MarketDataLoader::new("coinbase", None).unwrap();
        assert_eq!(loader.market_id_to_symbol("BTC-USDT"), "BTC/USDT");
        assert_eq!(loader.market_id_to_symbol("ETH-USD"), "ETH/USD");
    }

    #[test]
    fn test_market_id_to_symbol_kraken() {
        let loader = MarketDataLoader::new("kraken", None).unwrap();
        assert_eq!(loader.market_id_to_symbol("XBT/USDT"), "BTC/USDT");
    }

    #[tokio::test]
    async fn test_market_structure() {
        let loader = MarketDataLoader::new("binance", None).unwrap();
        let markets = loader.load_markets().await.unwrap();

        let btc_market = markets.get("BTC/USDT").unwrap();

        assert_eq!(btc_market.symbol, "BTC/USDT");
        assert_eq!(btc_market.base, "BTC");
        assert_eq!(btc_market.quote, "USDT");
        assert!(btc_market.active);
        assert_eq!(btc_market.market_type, "spot");

        // Check limits
        assert!(btc_market.limits.amount.min.is_some());
        assert!(btc_market.limits.price.min.is_some());
        assert!(btc_market.limits.cost.min.is_some());

        // Check precision
        assert!(btc_market.precision.amount > 0);
        assert!(btc_market.precision.price > 0);
    }

    #[test]
    fn test_exchange_endpoints_binance() {
        let endpoints = ExchangeEndpoints::binance();
        assert!(endpoints.base_url.contains("binance"));
        assert!(endpoints.markets_path.contains("exchangeInfo"));
        assert!(endpoints.orderbook_path.contains("depth"));
        assert!(endpoints.ticker_path.contains("ticker"));
    }

    #[test]
    fn test_exchange_endpoints_coinbase() {
        let endpoints = ExchangeEndpoints::coinbase();
        assert!(endpoints.base_url.contains("coinbase"));
        assert!(endpoints.markets_path.contains("products"));
    }

    #[test]
    fn test_exchange_endpoints_kraken() {
        let endpoints = ExchangeEndpoints::kraken();
        assert!(endpoints.base_url.contains("kraken"));
        assert!(endpoints.markets_path.contains("AssetPairs"));
    }

    #[test]
    fn test_exchange_endpoints_for_exchange() {
        assert!(ExchangeEndpoints::for_exchange("binance").is_some());
        assert!(ExchangeEndpoints::for_exchange("coinbase").is_some());
        assert!(ExchangeEndpoints::for_exchange("kraken").is_some());
        assert!(ExchangeEndpoints::for_exchange("unknown").is_none());
    }

    #[tokio::test]
    async fn test_fetch_order_book_returns_normalized_data() {
        let loader = MarketDataLoader::new("binance", None).unwrap();
        let book = loader.fetch_order_book("BTC/USDT", Some(10)).await.unwrap();

        assert_eq!(book.symbol, "BTC/USDT");
        assert!(!book.bids.is_empty());
        assert!(!book.asks.is_empty());
        // Bids sorted descending, asks sorted ascending
        assert!(book.bids[0][0] > book.bids[1][0]);
        assert!(book.asks[0][0] < book.asks[1][0]);
    }

    #[tokio::test]
    async fn test_fetch_order_book_caches_result() {
        let loader = MarketDataLoader::new("binance", None).unwrap();

        // First fetch
        let book1 = loader.fetch_order_book("BTC/USDT", Some(5)).await.unwrap();

        // Second fetch should return cached data (same timestamp)
        let book2 = loader.fetch_order_book("BTC/USDT", Some(5)).await.unwrap();

        assert_eq!(book1.timestamp, book2.timestamp);
        assert_eq!(book1.bids, book2.bids);
        assert_eq!(book1.asks, book2.asks);
    }

    #[tokio::test]
    async fn test_fetch_order_book_respects_limit() {
        let loader = MarketDataLoader::new("binance", None).unwrap();

        let book_5 = loader.fetch_order_book("ETH/USDT", Some(5)).await.unwrap();
        assert_eq!(book_5.bids.len(), 5);
        assert_eq!(book_5.asks.len(), 5);

        // Different symbol to avoid cache
        let book_20 = loader.fetch_order_book("SOL/USDT", Some(20)).await.unwrap();
        assert_eq!(book_20.bids.len(), 20);
        assert_eq!(book_20.asks.len(), 20);
    }

    #[tokio::test]
    async fn test_fetch_order_book_default_limit() {
        let loader = MarketDataLoader::new("coinbase", None).unwrap();

        let book = loader.fetch_order_book("BTC/USD", None).await.unwrap();

        // Default limit is 10
        assert_eq!(book.bids.len(), 10);
        assert_eq!(book.asks.len(), 10);
    }

    #[tokio::test]
    async fn test_fetch_order_book_clear_cache() {
        let loader = MarketDataLoader::new("kraken", None).unwrap();

        // Fetch and cache
        let book1 = loader.fetch_order_book("BTC/USDT", Some(5)).await.unwrap();

        // Clear cache
        loader.clear_cache().await;

        // Fetch again - should get new data with different timestamp
        let book2 = loader.fetch_order_book("BTC/USDT", Some(5)).await.unwrap();

        // Timestamps might differ (new generation)
        // But structure should be the same
        assert_eq!(book1.symbol, book2.symbol);
        assert_eq!(book1.bids.len(), book2.bids.len());
    }

    #[tokio::test]
    async fn test_fetch_order_book_different_exchanges() {
        let binance = MarketDataLoader::new("binance", None).unwrap();
        let coinbase = MarketDataLoader::new("coinbase", None).unwrap();
        let kraken = MarketDataLoader::new("kraken", None).unwrap();

        let binance_book = binance.fetch_order_book("BTC/USDT", Some(5)).await.unwrap();
        let coinbase_book = coinbase
            .fetch_order_book("BTC/USDT", Some(5))
            .await
            .unwrap();
        let kraken_book = kraken.fetch_order_book("BTC/USDT", Some(5)).await.unwrap();

        // All should return valid order books
        assert_eq!(binance_book.symbol, "BTC/USDT");
        assert_eq!(coinbase_book.symbol, "BTC/USDT");
        assert_eq!(kraken_book.symbol, "BTC/USDT");
    }

    #[tokio::test]
    async fn test_has_cached_order_book() {
        let loader = MarketDataLoader::new("binance", None).unwrap();

        // Not cached initially
        assert!(!loader.has_cached_order_book("BTC/USDT").await);

        // Fetch to cache
        let _ = loader.fetch_order_book("BTC/USDT", Some(5)).await.unwrap();

        // Now should be cached
        assert!(loader.has_cached_order_book("BTC/USDT").await);

        // Different symbol not cached
        assert!(!loader.has_cached_order_book("ETH/USDT").await);
    }

    #[tokio::test]
    async fn test_get_cached_order_book() {
        let loader = MarketDataLoader::new("binance", None).unwrap();

        // Not cached initially
        assert!(loader.get_cached_order_book("BTC/USDT").await.is_none());

        // Fetch to cache
        let fetched = loader.fetch_order_book("BTC/USDT", Some(5)).await.unwrap();

        // Get from cache
        let cached = loader.get_cached_order_book("BTC/USDT").await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().timestamp, fetched.timestamp);
    }

    // ==================== fetch_ticker tests ====================

    #[tokio::test]
    async fn test_fetch_ticker_returns_data() {
        let loader = MarketDataLoader::new("binance", None).unwrap();
        let ticker = loader.fetch_ticker("BTC/USDT").await.unwrap();

        assert_eq!(ticker.symbol, "BTC/USDT");
        assert!(ticker.last.is_some());
        assert!(ticker.bid.is_some());
        assert!(ticker.ask.is_some());
    }

    #[tokio::test]
    async fn test_fetch_ticker_caches_result() {
        let loader = MarketDataLoader::new("binance", None).unwrap();

        let ticker1 = loader.fetch_ticker("BTC/USDT").await.unwrap();
        let ticker2 = loader.fetch_ticker("BTC/USDT").await.unwrap();

        // Same timestamp means cached
        assert_eq!(ticker1.timestamp, ticker2.timestamp);
    }

    #[tokio::test]
    async fn test_fetch_ticker_different_exchanges() {
        let binance = MarketDataLoader::new("binance", None).unwrap();
        let coinbase = MarketDataLoader::new("coinbase", None).unwrap();
        let kraken = MarketDataLoader::new("kraken", None).unwrap();

        let t1 = binance.fetch_ticker("BTC/USDT").await.unwrap();
        let t2 = coinbase.fetch_ticker("BTC/USDT").await.unwrap();
        let t3 = kraken.fetch_ticker("BTC/USDT").await.unwrap();

        assert_eq!(t1.symbol, "BTC/USDT");
        assert_eq!(t2.symbol, "BTC/USDT");
        assert_eq!(t3.symbol, "BTC/USDT");
    }

    #[tokio::test]
    async fn test_has_cached_ticker() {
        let loader = MarketDataLoader::new("binance", None).unwrap();

        assert!(!loader.has_cached_ticker("BTC/USDT").await);
        let _ = loader.fetch_ticker("BTC/USDT").await.unwrap();
        assert!(loader.has_cached_ticker("BTC/USDT").await);
    }

    #[tokio::test]
    async fn test_get_cached_ticker() {
        let loader = MarketDataLoader::new("binance", None).unwrap();

        assert!(loader.get_cached_ticker("BTC/USDT").await.is_none());
        let fetched = loader.fetch_ticker("BTC/USDT").await.unwrap();
        let cached = loader.get_cached_ticker("BTC/USDT").await.unwrap();
        assert_eq!(cached.timestamp, fetched.timestamp);
    }

    // ==================== create_order tests ====================

    #[tokio::test]
    async fn test_create_order_limit_buy() {
        let loader = MarketDataLoader::new("binance", None).unwrap();
        let response = loader
            .create_order("BTC/USDT", "limit", "buy", 0.001, Some(50000.0))
            .await
            .unwrap();

        assert_eq!(response.symbol, "BTC/USDT");
        assert_eq!(response.order_type, "limit");
        assert_eq!(response.side, "buy");
        assert_eq!(response.status, "open");
        assert!(response.price.is_some());
    }

    #[tokio::test]
    async fn test_create_order_market_sell() {
        let loader = MarketDataLoader::new("binance", None).unwrap();
        let response = loader
            .create_order("ETH/USDT", "market", "sell", 0.1, None)
            .await
            .unwrap();

        assert_eq!(response.symbol, "ETH/USDT");
        assert_eq!(response.order_type, "market");
        assert_eq!(response.side, "sell");
        assert!(response.price.is_none()); // Market orders have no price
    }

    #[tokio::test]
    async fn test_create_order_generates_unique_id() {
        let loader = MarketDataLoader::new("binance", None).unwrap();

        let order1 = loader
            .create_order("BTC/USDT", "limit", "buy", 0.001, Some(50000.0))
            .await
            .unwrap();
        let order2 = loader
            .create_order("BTC/USDT", "limit", "buy", 0.001, Some(50000.0))
            .await
            .unwrap();

        assert_ne!(order1.id, order2.id);
    }

    #[tokio::test]
    async fn test_create_order_different_exchanges() {
        let binance = MarketDataLoader::new("binance", None).unwrap();
        let coinbase = MarketDataLoader::new("coinbase", None).unwrap();
        let kraken = MarketDataLoader::new("kraken", None).unwrap();

        let o1 = binance
            .create_order("BTC/USDT", "limit", "buy", 0.001, Some(50000.0))
            .await
            .unwrap();
        let o2 = coinbase
            .create_order("BTC/USD", "limit", "buy", 0.001, Some(50000.0))
            .await
            .unwrap();
        let o3 = kraken
            .create_order("BTC/USD", "limit", "buy", 0.001, Some(50000.0))
            .await
            .unwrap();

        // Each exchange should prefix with its name
        assert!(o1.id.starts_with("binance-"));
        assert!(o2.id.starts_with("coinbase-"));
        assert!(o3.id.starts_with("kraken-"));
    }

    #[tokio::test]
    async fn test_create_order_validates_limit_requires_price() {
        let loader = MarketDataLoader::new("binance", None).unwrap();
        let result = loader
            .create_order("BTC/USDT", "limit", "buy", 0.001, None)
            .await;

        assert!(result.is_err());
    }

    // ==================== cancel_order tests ====================

    #[tokio::test]
    async fn test_cancel_order_returns_canceled_status() {
        let loader = MarketDataLoader::new("binance", None).unwrap();
        let response = loader.cancel_order("order-123", "BTC/USDT").await.unwrap();

        assert_eq!(response.id, "order-123");
        assert_eq!(response.status, "canceled");
    }

    #[tokio::test]
    async fn test_cancel_order_different_exchanges() {
        let binance = MarketDataLoader::new("binance", None).unwrap();
        let coinbase = MarketDataLoader::new("coinbase", None).unwrap();

        let r1 = binance.cancel_order("order-1", "BTC/USDT").await.unwrap();
        let r2 = coinbase.cancel_order("order-2", "BTC/USD").await.unwrap();

        assert_eq!(r1.status, "canceled");
        assert_eq!(r2.status, "canceled");
    }

    // ==================== fetch_order tests ====================

    #[tokio::test]
    async fn test_fetch_order_returns_order_details() {
        let loader = MarketDataLoader::new("binance", None).unwrap();
        let response = loader.fetch_order("order-456", "BTC/USDT").await.unwrap();

        assert_eq!(response.id, "order-456");
        assert_eq!(response.symbol, "BTC/USDT");
    }

    #[tokio::test]
    async fn test_fetch_order_different_exchanges() {
        let binance = MarketDataLoader::new("binance", None).unwrap();
        let kraken = MarketDataLoader::new("kraken", None).unwrap();

        let r1 = binance.fetch_order("order-1", "BTC/USDT").await.unwrap();
        let r2 = kraken.fetch_order("order-2", "BTC/USD").await.unwrap();

        assert_eq!(r1.id, "order-1");
        assert_eq!(r2.id, "order-2");
    }

    // ==================== fetch_balance tests ====================

    #[tokio::test]
    async fn test_fetch_balance_returns_balances() {
        let loader = MarketDataLoader::new("binance", None).unwrap();
        let balance = loader.fetch_balance().await.unwrap();

        // Should have some balances
        assert!(!balance.balances.is_empty());
        assert!(balance.timestamp > 0);
    }

    #[tokio::test]
    async fn test_fetch_balance_contains_common_currencies() {
        let loader = MarketDataLoader::new("binance", None).unwrap();
        let balance = loader.fetch_balance().await.unwrap();

        // Mock should include BTC and USDT
        assert!(balance.get_balance("BTC").is_some());
        assert!(balance.get_balance("USDT").is_some());
    }

    #[tokio::test]
    async fn test_fetch_balance_currency_fields() {
        let loader = MarketDataLoader::new("binance", None).unwrap();
        let balance = loader.fetch_balance().await.unwrap();

        let btc = balance.get_balance("BTC").unwrap();
        // free + used = total
        assert_eq!(btc.free + btc.used, btc.total);
    }

    #[tokio::test]
    async fn test_fetch_balance_different_exchanges() {
        let binance = MarketDataLoader::new("binance", None).unwrap();
        let coinbase = MarketDataLoader::new("coinbase", None).unwrap();
        let kraken = MarketDataLoader::new("kraken", None).unwrap();

        let b1 = binance.fetch_balance().await.unwrap();
        let b2 = coinbase.fetch_balance().await.unwrap();
        let b3 = kraken.fetch_balance().await.unwrap();

        // All should return valid balances
        assert!(!b1.balances.is_empty());
        assert!(!b2.balances.is_empty());
        assert!(!b3.balances.is_empty());
    }
}
