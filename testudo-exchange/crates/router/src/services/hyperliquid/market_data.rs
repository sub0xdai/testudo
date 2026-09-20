//! HL-12 CP-2: the router-owned boundary for Hyperliquid market data and fills.
//!
//! Before this module, the read paths named `InfoProvider` and `UserFillByTime`
//! directly, which spread two Hyperliquid SDK types across five modules and made
//! the CP-3 dependency swap touch all of them. Conversion now happens here and
//! nowhere else.
//!
//! `HlFill` mirrors the wire shape field for field, including the four fields the
//! router never reads (`crossed`, `hash`, `start_position`, `cloid`). It has to:
//! `journal_syncer::hyperliquid::convert_hl_fill` persists the whole fill as
//! `raw_json`, so a narrower type would silently change that column.
//!
//! The implementation is still the vendored `hyperliquid-sdk-rs`. HL-12 CP-3
//! replaces it with `hypersdk` behind this same surface.

// @anchor exchange:router:market_data
// @tags api

use alloy::primitives::Address;
use hyperliquid_sdk_rs::{types::info_types::UserFillByTime, InfoProvider};
use serde::{Deserialize, Serialize};

use crate::types::hl_network::HlNetwork;

/// A Hyperliquid fill, as the router consumes it.
///
/// The wire shape is the point: `raw_json` in `raw_fills` is this struct
/// serialized back out, so field names and presence must match the exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlFill {
    pub closed_pnl: String,
    pub coin: String,
    pub crossed: bool,
    pub dir: String,
    pub hash: String,
    pub oid: u64,
    pub px: String,
    pub side: String,
    pub start_position: String,
    pub sz: String,
    pub time: u64,
    pub fee: String,
    pub fee_token: String,
    pub tid: u64,
    pub cloid: Option<String>,
}

impl From<&UserFillByTime> for HlFill {
    // @waiver TB-01: pure field map with no branches, so the only assertion
    // available would restate an assignment. The invariant that actually matters
    // - that this mirrors the exchange wire shape - is enforced by
    // `hl_fill_matches_the_sdk_wire_shape`, which compares the serialized forms.
    fn from(fill: &UserFillByTime) -> Self {
        Self {
            closed_pnl: fill.closed_pnl.clone(),
            coin: fill.coin.clone(),
            crossed: fill.crossed,
            dir: fill.dir.clone(),
            hash: fill.hash.clone(),
            oid: fill.oid,
            px: fill.px.clone(),
            side: fill.side.clone(),
            start_position: fill.start_position.clone(),
            sz: fill.sz.clone(),
            time: fill.time,
            fee: fill.fee.clone(),
            fee_token: fill.fee_token.clone(),
            tid: fill.tid,
            cloid: fill.cloid.clone(),
        }
    }
}

/// A Hyperliquid perpetual market.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HlPerp {
    pub name: String,
    /// Positional index in the `meta` universe. Every order and cancel needs it.
    pub index: u32,
    pub sz_decimals: u32,
    pub max_leverage: u32,
}

impl HlPerp {
    /// Build one entry from the `meta` universe. The index is the position.
    pub(crate) fn from_universe_entry(
        index: usize,
        name: String,
        sz_decimals: u32,
        max_leverage: u32,
    ) -> Self {
        debug_assert!(index < u32::MAX as usize, "universe index must fit u32");
        debug_assert!(!name.is_empty(), "universe entry must carry a coin name");
        Self {
            name,
            index: index as u32,
            sz_decimals,
            max_leverage,
        }
    }
}

/// Errors from Hyperliquid market-data reads.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HlMarketDataError {
    /// The exchange rate limited the request. Callers retry with backoff.
    #[error("Hyperliquid rate limited the request: {0}")]
    RateLimited(String),
    /// Any other transport or deserialization failure.
    #[error("Hyperliquid market data request failed: {0}")]
    Fetch(String),
}

impl HlMarketDataError {
    /// Classify a transport error once, at the boundary.
    ///
    /// The rate-limit signal is a substring match because the vendored SDK
    /// exposes no typed status. It reproduces the predicate the journal syncer
    /// used to apply inline, so classification moves without changing behaviour.
    ///
    // @waiver TB-01: a two-branch classifier with one expression per branch. The
    // only assertion available would restate the match. Behaviour is pinned by
    // `rate_limit_text_is_classified_as_rate_limited`.
    fn classify(err: impl std::fmt::Display) -> Self {
        let msg = err.to_string();
        if msg.contains("429") || msg.to_lowercase().contains("rate") {
            Self::RateLimited(msg)
        } else {
            Self::Fetch(msg)
        }
    }
}

/// Reads Hyperliquid market metadata and fills.
///
/// Cheap to construct and stateless beyond its client, so callers may build one
/// per request. It is deliberately not a trait: there is one venue and one
/// implementation, and an interface with a single implementor is noise.
pub struct HlMarketData {
    info: InfoProvider,
}

impl HlMarketData {
    pub fn new(network: HlNetwork) -> Self {
        Self {
            info: InfoProvider::new(network.to_sdk()),
        }
    }

    /// Fetch the perpetual universe, indexed by position.
    pub async fn perps(&self) -> Result<Vec<HlPerp>, HlMarketDataError> {
        let meta = self
            .info
            .meta()
            .await
            .map_err(HlMarketDataError::classify)?;

        let perps: Vec<HlPerp> = meta
            .universe
            .into_iter()
            .enumerate()
            .map(|(index, asset)| {
                HlPerp::from_universe_entry(
                    index,
                    asset.name,
                    asset.sz_decimals,
                    asset.max_leverage,
                )
            })
            .collect();

        debug_assert!(
            perps.iter().any(|perp| !perp.name.is_empty()),
            "a non-empty universe must yield at least one named market"
        );
        Ok(perps)
    }

    /// Fetch fills in `[start_ms, end_ms)`, or open-ended when `end_ms` is `None`.
    pub async fn fills_between(
        &self,
        user: Address,
        start_ms: u64,
        end_ms: Option<u64>,
    ) -> Result<Vec<HlFill>, HlMarketDataError> {
        debug_assert!(
            user != Address::ZERO,
            "querying fills for the zero address is always a caller bug"
        );

        let raw = self
            .info
            .user_fills_by_time(user, start_ms, end_ms, None)
            .await
            .map_err(HlMarketDataError::classify)?;

        let fills: Vec<HlFill> = raw.iter().map(HlFill::from).collect();

        debug_assert_eq!(
            fills.len(),
            raw.len(),
            "conversion must not drop or duplicate fills"
        );
        Ok(fills)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sdk_fill() -> UserFillByTime {
        UserFillByTime {
            closed_pnl: "-12.5".to_string(),
            coin: "BTC".to_string(),
            crossed: true,
            dir: "Close Long".to_string(),
            hash: "0x9f2a".to_string(),
            oid: 880_101,
            px: "64250.5".to_string(),
            side: "A".to_string(),
            start_position: "0.4".to_string(),
            sz: "0.4".to_string(),
            time: 1_770_000_000_123,
            fee: "1.285".to_string(),
            fee_token: "USDC".to_string(),
            tid: 5_512_004,
            cloid: Some("0x0102".to_string()),
        }
    }

    #[test]
    fn hl_fill_matches_the_sdk_wire_shape() {
        let sdk = sdk_fill();
        let ours = HlFill::from(&sdk);

        assert_eq!(
            serde_json::to_value(&ours).expect("HlFill serializes"),
            serde_json::to_value(&sdk).expect("SDK fill serializes"),
            "HlFill must serialize to the same wire shape as the SDK type, because \
             raw_fills.raw_json stores this struct verbatim"
        );
    }

    #[test]
    fn hl_fill_maps_every_field() {
        let sdk = sdk_fill();
        let ours = HlFill::from(&sdk);

        assert_eq!(ours.closed_pnl, sdk.closed_pnl);
        assert_eq!(ours.coin, sdk.coin);
        assert_eq!(ours.crossed, sdk.crossed);
        assert_eq!(ours.dir, sdk.dir);
        assert_eq!(ours.hash, sdk.hash);
        assert_eq!(ours.oid, sdk.oid);
        assert_eq!(ours.px, sdk.px);
        assert_eq!(ours.side, sdk.side);
        assert_eq!(ours.start_position, sdk.start_position);
        assert_eq!(ours.sz, sdk.sz);
        assert_eq!(ours.time, sdk.time);
        assert_eq!(ours.fee, sdk.fee);
        assert_eq!(ours.fee_token, sdk.fee_token);
        assert_eq!(ours.tid, sdk.tid);
        assert_eq!(ours.cloid, sdk.cloid);
    }

    #[test]
    fn perp_index_follows_universe_position() {
        let perps: Vec<HlPerp> = vec![("BTC", 5u32, 50u32), ("ETH", 4, 50), ("SOL", 2, 20)]
            .into_iter()
            .enumerate()
            .map(|(index, (name, sz_decimals, max_leverage))| {
                HlPerp::from_universe_entry(index, name.to_string(), sz_decimals, max_leverage)
            })
            .collect();

        assert_eq!(perps.len(), 3);
        assert_eq!(perps[0].name, "BTC");
        assert_eq!(perps[0].index, 0);
        assert_eq!(perps[1].index, 1);
        assert_eq!(perps[2].index, 2);
        assert_eq!(perps[2].sz_decimals, 2);
        assert_eq!(perps[2].max_leverage, 20);
    }

    #[test]
    fn rate_limit_text_is_classified_as_rate_limited() {
        assert!(matches!(
            HlMarketDataError::classify("HTTP 429 too many requests"),
            HlMarketDataError::RateLimited(_)
        ));
        assert!(matches!(
            HlMarketDataError::classify("request exceeds rate limit"),
            HlMarketDataError::RateLimited(_)
        ));
    }

    #[test]
    fn other_transport_text_is_a_fetch_error() {
        assert!(matches!(
            HlMarketDataError::classify("connection reset by peer"),
            HlMarketDataError::Fetch(_)
        ));
    }
}
