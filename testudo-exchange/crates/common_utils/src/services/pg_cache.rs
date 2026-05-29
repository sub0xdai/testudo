//! PostgreSQL Cache Service
//!
//! Provides caching capabilities using PostgreSQL UNLOGGED table with
//! per-query TTL checking.

// @anchor exchange:common_utils:pg_cache
// @tags infra

use pg_queue::{CacheRepository, PgPool};
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;

/// Cache key prefixes for different data types
pub mod cache_keys {
    pub const TICKER: &str = "binance:ticker";
    pub const ORDERBOOK: &str = "binance:orderbook";
    pub const KLINES: &str = "binance:klines";
    pub const MARKETS: &str = "binance:markets";
}

/// Default TTL values for different cache types
pub mod cache_ttl {
    use std::time::Duration;

    pub const TICKER: Duration = Duration::from_secs(5);
    pub const ORDERBOOK: Duration = Duration::from_secs(1);
    pub const KLINES: Duration = Duration::from_secs(60);
    pub const MARKETS: Duration = Duration::from_secs(300); // 5 minutes
}

/// Cache error types
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Cache connection error: {0}")]
    ConnectionError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Cache miss for key: {0}")]
    CacheMiss(String),
}

/// Cache service using PostgreSQL for storing and retrieving data
#[derive(Clone)]
pub struct PgCacheService {
    cache: CacheRepository,
}

impl PgCacheService {
    /// Create a new PostgreSQL cache service from connection pool
    pub fn new(pool: PgPool) -> Self {
        Self {
            cache: CacheRepository::new(pool),
        }
    }

    /// Get a value from cache
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T, CacheError> {
        match self.cache.get::<T>(key).await {
            Ok(Some(value)) => Ok(value),
            Ok(None) => Err(CacheError::CacheMiss(key.to_string())),
            Err(e) => Err(CacheError::ConnectionError(e.to_string())),
        }
    }

    /// Set a value in cache with TTL
    pub async fn set<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl: Duration,
    ) -> Result<(), CacheError> {
        self.cache
            .set(key, value, ttl.as_secs())
            .await
            .map_err(|e| CacheError::ConnectionError(e.to_string()))
    }

    /// Get value from cache, or compute and cache it if missing
    pub async fn get_or_set<T, F, Fut>(
        &self,
        key: &str,
        ttl: Duration,
        fetch: F,
    ) -> Result<T, CacheError>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, CacheError>>,
    {
        // Try to get from cache first
        match self.get::<T>(key).await {
            Ok(value) => return Ok(value),
            Err(CacheError::CacheMiss(_)) => {
                // Cache miss, need to fetch
            }
            Err(e) => return Err(e),
        }

        // Fetch the value
        let value = fetch().await?;

        // Cache it
        self.set(key, &value, ttl).await?;

        Ok(value)
    }

    /// Delete a key from cache
    pub async fn delete(&self, key: &str) -> Result<(), CacheError> {
        self.cache
            .delete(key)
            .await
            .map_err(|e| CacheError::ConnectionError(e.to_string()))?;
        Ok(())
    }

    /// Clean up expired entries
    pub async fn cleanup_expired(&self) -> Result<u64, CacheError> {
        self.cache
            .cleanup_expired()
            .await
            .map_err(|e| CacheError::ConnectionError(e.to_string()))
    }

    /// Invalidate trade cache for a symbol
    pub async fn invalidate_trade_cache(&self, symbol: &str) -> Result<(), CacheError> {
        let key = format!("trades:{}:100", symbol);
        self.delete(&key).await
    }

    /// Build cache key for ticker data
    pub fn ticker_key(symbol: &str) -> String {
        format!("{}:{}", cache_keys::TICKER, symbol)
    }

    /// Build cache key for orderbook data
    pub fn orderbook_key(symbol: &str) -> String {
        format!("{}:{}", cache_keys::ORDERBOOK, symbol)
    }

    /// Build cache key for klines data
    pub fn klines_key(symbol: &str, interval: &str) -> String {
        format!("{}:{}:{}", cache_keys::KLINES, symbol, interval)
    }

    /// Build cache key for markets list
    pub fn markets_key() -> String {
        cache_keys::MARKETS.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_keys() {
        assert_eq!(
            PgCacheService::ticker_key("BTC_USDC"),
            "binance:ticker:BTC_USDC"
        );
        assert_eq!(
            PgCacheService::orderbook_key("ETH_USDC"),
            "binance:orderbook:ETH_USDC"
        );
        assert_eq!(
            PgCacheService::klines_key("SOL_USDC", "1h"),
            "binance:klines:SOL_USDC:1h"
        );
        assert_eq!(PgCacheService::markets_key(), "binance:markets");
    }

    #[test]
    fn test_trade_cache_key() {
        let symbol = "BTC_USDC";
        let key = format!("trades:{}:100", symbol);
        assert_eq!(key, "trades:BTC_USDC:100");
    }
}
