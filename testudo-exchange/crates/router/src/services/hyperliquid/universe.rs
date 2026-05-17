//! HL-01: Asset Universe Cache & Symbol Resolution
//!
//! Hyperliquid identifies assets by integer index, not strings.
//! This module caches the `coin_name → (asset_index, sz_decimals)` mapping
//! fetched from the Hyperliquid `meta` API at startup.

use hyperliquid_sdk_rs::{InfoProvider, Network};
use std::collections::HashMap;
use thiserror::Error;

/// Errors from asset universe operations.
#[derive(Debug, Error)]
pub enum UniverseError {
    #[error("Asset not found: {0}")]
    AssetNotFound(String),
    #[error("Failed to fetch meta: {0}")]
    FetchFailed(String),
}

/// Cached metadata for a single Hyperliquid perpetual asset.
#[derive(Debug, Clone)]
pub struct AssetMeta {
    /// Positional index in the Hyperliquid universe (e.g., 0 = BTC).
    pub index: u32,
    /// Number of decimal places for order sizes.
    pub sz_decimals: u32,
    /// Maximum allowed leverage.
    pub max_leverage: u32,
}

/// Cached mapping of coin names to their Hyperliquid metadata.
///
/// Built from the `meta` API response at startup. Keyed by uppercase
/// coin name (e.g., "BTC", "ETH"). The asset index is the position
/// in the `universe` array — this is what all order/cancel APIs require.
pub struct AssetUniverse {
    assets: HashMap<String, AssetMeta>,
}

impl AssetUniverse {
    /// Fetch the asset universe from Hyperliquid and build the cache.
    pub async fn fetch(network: Network) -> Result<Self, UniverseError> {
        let info = InfoProvider::new(network);
        let meta = info
            .meta()
            .await
            .map_err(|e| UniverseError::FetchFailed(e.to_string()))?;

        let mut assets = HashMap::new();
        for (index, asset) in meta.universe.into_iter().enumerate() {
            assets.insert(
                asset.name.to_uppercase(),
                AssetMeta {
                    index: index as u32,
                    sz_decimals: asset.sz_decimals,
                    max_leverage: asset.max_leverage,
                },
            );
        }

        tracing::info!(
            "AssetUniverse loaded: {} perpetual assets from {:?}",
            assets.len(),
            network
        );

        Ok(Self { assets })
    }

    /// Build from a pre-fetched list (for testing or offline use).
    pub fn from_entries(entries: Vec<(String, AssetMeta)>) -> Self {
        Self {
            assets: entries.into_iter().collect(),
        }
    }

    /// Resolve a coin name to its asset index.
    /// Accepts "BTC", "ETH", etc. (case-insensitive).
    pub fn resolve(&self, coin: &str) -> Result<u32, UniverseError> {
        self.assets
            .get(&coin.to_uppercase())
            .map(|m| m.index)
            .ok_or_else(|| UniverseError::AssetNotFound(coin.to_string()))
    }

    /// Get the `sz_decimals` for a coin (number of decimal places for sizes).
    pub fn sz_decimals(&self, coin: &str) -> Result<u32, UniverseError> {
        self.assets
            .get(&coin.to_uppercase())
            .map(|m| m.sz_decimals)
            .ok_or_else(|| UniverseError::AssetNotFound(coin.to_string()))
    }

    /// Get the full metadata for a coin.
    pub fn get(&self, coin: &str) -> Result<&AssetMeta, UniverseError> {
        self.assets
            .get(&coin.to_uppercase())
            .ok_or_else(|| UniverseError::AssetNotFound(coin.to_string()))
    }

    /// Convert internal symbol format `BTC_USDT` to Hyperliquid coin name `BTC`.
    /// Hyperliquid uses bare coin names for perpetuals — no pair suffix.
    pub fn to_hl_coin(internal_symbol: &str) -> &str {
        internal_symbol
            .split('_')
            .next()
            .unwrap_or(internal_symbol)
    }

    /// Convert Hyperliquid coin name `BTC` back to internal format `BTC_USDT`.
    pub fn from_hl_coin(coin: &str) -> String {
        format!("{}_USDT", coin)
    }

    /// Number of cached assets.
    pub fn len(&self) -> usize {
        self.assets.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_universe() -> AssetUniverse {
        AssetUniverse::from_entries(vec![
            (
                "BTC".to_string(),
                AssetMeta {
                    index: 0,
                    sz_decimals: 5,
                    max_leverage: 50,
                },
            ),
            (
                "ETH".to_string(),
                AssetMeta {
                    index: 1,
                    sz_decimals: 4,
                    max_leverage: 50,
                },
            ),
            (
                "SOL".to_string(),
                AssetMeta {
                    index: 5,
                    sz_decimals: 2,
                    max_leverage: 20,
                },
            ),
        ])
    }

    #[test]
    fn resolve_known_asset() {
        let universe = test_universe();
        assert_eq!(universe.resolve("BTC").unwrap(), 0);
        assert_eq!(universe.resolve("ETH").unwrap(), 1);
        assert_eq!(universe.resolve("SOL").unwrap(), 5);
    }

    #[test]
    fn resolve_case_insensitive() {
        let universe = test_universe();
        assert_eq!(universe.resolve("btc").unwrap(), 0);
        assert_eq!(universe.resolve("Eth").unwrap(), 1);
        assert_eq!(universe.resolve("sol").unwrap(), 5);
    }

    #[test]
    fn resolve_unknown_asset_returns_error() {
        let universe = test_universe();
        let err = universe.resolve("DOGE").unwrap_err();
        assert!(matches!(err, UniverseError::AssetNotFound(_)));
    }

    #[test]
    fn sz_decimals_returns_correct_values() {
        let universe = test_universe();
        assert_eq!(universe.sz_decimals("BTC").unwrap(), 5);
        assert_eq!(universe.sz_decimals("ETH").unwrap(), 4);
        assert_eq!(universe.sz_decimals("SOL").unwrap(), 2);
    }

    #[test]
    fn get_returns_full_metadata() {
        let universe = test_universe();
        let meta = universe.get("BTC").unwrap();
        assert_eq!(meta.index, 0);
        assert_eq!(meta.sz_decimals, 5);
        assert_eq!(meta.max_leverage, 50);
    }

    #[test]
    fn to_hl_coin_strips_usdt_suffix() {
        assert_eq!(AssetUniverse::to_hl_coin("BTC_USDT"), "BTC");
        assert_eq!(AssetUniverse::to_hl_coin("ETH_USDT"), "ETH");
        assert_eq!(AssetUniverse::to_hl_coin("SOL_USDT"), "SOL");
    }

    #[test]
    fn to_hl_coin_handles_bare_names() {
        assert_eq!(AssetUniverse::to_hl_coin("BTC"), "BTC");
    }

    #[test]
    fn from_hl_coin_appends_usdt() {
        assert_eq!(AssetUniverse::from_hl_coin("BTC"), "BTC_USDT");
        assert_eq!(AssetUniverse::from_hl_coin("ETH"), "ETH_USDT");
    }

    #[test]
    fn len_and_is_empty() {
        let universe = test_universe();
        assert_eq!(universe.len(), 3);
        assert!(!universe.is_empty());

        let empty = AssetUniverse::from_entries(vec![]);
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
    }
}
