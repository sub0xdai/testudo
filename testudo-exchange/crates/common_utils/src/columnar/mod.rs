//! Columnar Data Structures for Wire-Efficient Serialization
//!
//! Implements Structure-of-Arrays (SoA) pattern for ~25% smaller JSON payloads.
//! See: https://en.wikipedia.org/wiki/AoS_and_SoA

// @anchor exchange:common_utils:mod
// @tags infra

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// =============================================================================
// Constants
// =============================================================================

/// Column schema for depth data (DRY: single source of truth)
const DEPTH_COLUMNS: &[&str] = &["price", "quantity"];

/// Default capacity for depth data vectors.
/// 64 chosen as typical orderbook displays 20-50 levels, so 64 avoids reallocation.
const DEFAULT_DEPTH_CAPACITY: usize = 64;

/// Helper to create depth column schema as owned Strings
fn depth_columns() -> Vec<String> {
    DEPTH_COLUMNS.iter().map(|s| (*s).to_string()).collect()
}

/// Columnar representation of order book depth data.
///
/// Instead of `[{price, quantity}, {price, quantity}, ...]` (AoS),
/// we use `{columns: [...], data: [[...], [...]]}` (SoA).
///
/// # Wire Format
/// ```json
/// {
///     "columns": ["price", "quantity"],
///     "data": [["50123.45", "1.234"], ["50122.00", "0.567"]],
///     "timestamp": 1705123456789
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthColumnStore {
    /// Column schema (always ["price", "quantity"] for depth)
    pub columns: Vec<String>,
    /// Dense row data - each inner vec is one price level
    pub data: Vec<Vec<String>>,
    /// Timestamp in milliseconds
    pub timestamp: i64,
    /// Optional sequence number for ordering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<i64>,
}

impl DepthColumnStore {
    /// Create new empty column store with depth schema
    pub fn new(timestamp: i64) -> Self {
        Self {
            columns: depth_columns(),
            data: Vec::with_capacity(DEFAULT_DEPTH_CAPACITY),
            timestamp,
            nonce: None,
        }
    }

    /// Convert from Decimal tuple slice (internal orderbook format)
    pub fn from_depth_tuples(depth: &[(Decimal, Decimal)], timestamp: i64) -> Self {
        let data = depth
            .iter()
            .map(|(price, qty)| vec![price.to_string(), qty.to_string()])
            .collect();

        Self {
            columns: depth_columns(),
            data,
            timestamp,
            nonce: None,
        }
    }

    /// Convert from Decimal array slice (CCXT orderbook format)
    pub fn from_decimal_arrays(levels: &[[Decimal; 2]], timestamp: i64) -> Self {
        let data = levels
            .iter()
            .map(|[price, qty]| vec![price.to_string(), qty.to_string()])
            .collect();

        Self {
            columns: depth_columns(),
            data,
            timestamp,
            nonce: None,
        }
    }

    /// Set nonce for sequence tracking
    pub fn with_nonce(mut self, nonce: i64) -> Self {
        self.nonce = Some(nonce);
        self
    }

    /// Get number of price levels
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Columnar order book with bids and asks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnarOrderBook {
    /// Trading symbol
    pub symbol: String,
    /// Bid levels in columnar format
    pub bids: DepthColumnStore,
    /// Ask levels in columnar format
    pub asks: DepthColumnStore,
    /// Sequence number for ordering (moved to top level to avoid duplication)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<i64>,
}

impl ColumnarOrderBook {
    /// Create from CCXT order book format
    pub fn from_ccxt(
        symbol: String,
        bids: &[[Decimal; 2]],
        asks: &[[Decimal; 2]],
        timestamp: i64,
        nonce: Option<i64>,
    ) -> Self {
        let bids_store = DepthColumnStore::from_decimal_arrays(bids, timestamp);
        let asks_store = DepthColumnStore::from_decimal_arrays(asks, timestamp);

        Self {
            symbol,
            bids: bids_store,
            asks: asks_store,
            nonce,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_depth_column_store_from_tuples() {
        let depth = vec![(dec!(100.50), dec!(1.5)), (dec!(100.25), dec!(2.0))];
        let store = DepthColumnStore::from_depth_tuples(&depth, 1234567890);

        assert_eq!(store.columns, vec!["price", "quantity"]);
        assert_eq!(store.data.len(), 2);
        assert_eq!(store.data[0], vec!["100.50", "1.5"]);
        assert_eq!(store.data[1], vec!["100.25", "2.0"]);
        assert_eq!(store.timestamp, 1234567890);
    }

    #[test]
    fn test_depth_column_store_from_arrays() {
        let levels = vec![[dec!(100.50), dec!(1.5)], [dec!(100.25), dec!(2.0)]];
        let store = DepthColumnStore::from_decimal_arrays(&levels, 1234567890);

        assert_eq!(store.len(), 2);
        assert_eq!(store.data[0], vec!["100.50", "1.5"]);
    }

    #[test]
    fn test_columnar_orderbook_serialization() {
        let bids = vec![[dec!(100.50), dec!(1.5)]];
        let asks = vec![[dec!(100.75), dec!(0.5)]];

        let book =
            ColumnarOrderBook::from_ccxt("BTCUSDT".to_string(), &bids, &asks, 1234567890, Some(42));

        let json = serde_json::to_string_pretty(&book).unwrap();
        assert!(json.contains("\"columns\""));
        assert!(json.contains("\"data\""));
        assert!(json.contains("\"100.50\""));
    }

    #[test]
    fn test_wire_size_comparison() {
        // Generate 60 levels of depth data
        let levels: Vec<[Decimal; 2]> = (0..60)
            .map(|i| {
                [
                    dec!(50000) + Decimal::from(i),
                    dec!(1) + Decimal::from(i) / dec!(10),
                ]
            })
            .collect();

        // Columnar format
        let columnar = DepthColumnStore::from_decimal_arrays(&levels, 1234567890);
        let columnar_json = serde_json::to_string(&columnar).unwrap();

        // Row format (simulating [{price, quantity}, ...])
        let row_data: Vec<serde_json::Value> = levels
            .iter()
            .map(|[p, q]| {
                serde_json::json!({
                    "price": p.to_string(),
                    "quantity": q.to_string()
                })
            })
            .collect();
        let row_json = serde_json::to_string(&row_data).unwrap();

        // Columnar should be smaller
        println!("Columnar: {} bytes", columnar_json.len());
        println!("Row: {} bytes", row_json.len());
        assert!(
            columnar_json.len() < row_json.len(),
            "Columnar ({}) should be smaller than row ({})",
            columnar_json.len(),
            row_json.len()
        );
    }
}
