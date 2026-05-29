//! RSK-01 — Unified Risk Snapshot
//!
//! Aggregates balance + position state across every exchange a user has connected
//! into a single payload the frontend consumes for the Live Risk Strip, PulseStrip,
//! Positions-by-Venue, Margin-by-Venue, and Correlation widgets.
//!
//! T2 implements the real per-account fan-out, asset-family bucketing, and a 5s
//! server-side cache. Per-account fetches mirror the logic in `routes/exchanges.rs`
//! (HL native info API for Hyperliquid; CEX sidecar for everything else).

// @anchor exchange:router:risk_snapshot
// @tags api

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::repositories::exchange_account::{DecryptedCredentials, ExchangeAccountRow};
use crate::types::app::AppState;
use crate::types::exchange_names::exchanges;

/// Full risk snapshot returned by GET /api/v1/risk/snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskSnapshot {
    pub net_exposure_usd: Decimal,
    pub aggregate_leverage: Decimal,
    pub free_margin_usd: Decimal,
    pub long_pct: Decimal,
    pub short_pct: Decimal,
    pub net_delta_usd: Decimal,
    pub positions_by_venue: Vec<VenuePositions>,
    pub margin_by_venue: Vec<VenueMargin>,
    pub correlation_stack: Vec<CorrelationBucket>,
    pub as_of: DateTime<Utc>,
}

/// Exchange-side positions grouped per venue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenuePositions {
    pub exchange_id: Uuid,
    pub exchange_name: String,
    pub positions: Vec<PositionEntry>,
}

/// One live position row. Exchange-side — reflects the venue's view, not engine-managed OrderGroups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionEntry {
    pub symbol: String,
    /// "long" | "short"
    pub side: String,
    pub entry_price: Decimal,
    pub mark_price: Decimal,
    pub quantity: Decimal,
    pub notional_usd: Decimal,
    pub unrealized_pnl_usd: Decimal,
    /// Position-configured leverage multiplier (e.g. 8 for 8x). `None` when the
    /// exchange adapter did not report it — aggregation falls back to the
    /// gross notional/total-margin ratio in that case.
    pub leverage: Option<Decimal>,
}

/// Per-venue free / used / total USD-equivalent margin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenueMargin {
    pub exchange_id: Uuid,
    pub exchange_name: String,
    pub free_usd: Decimal,
    pub used_usd: Decimal,
    pub total_usd: Decimal,
}

/// Correlation bucket surfaces directional stacking across asset families.
/// MVP — hard-coded family map (see [`BUCKET_MAP`]); no statistical correlation yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationBucket {
    pub bucket: String,
    /// "long" | "short" | "mixed"
    pub direction: String,
    pub effective_notional_usd: Decimal,
    pub contributing_symbols: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum RiskError {
    #[error("internal error: {0}")]
    Internal(String),
}

// ── Asset-family bucketing (single source of truth) ──

/// Hard-coded asset family map. Unknown coins fall into the "other" bucket.
/// Lives backend-side so the snapshot wire payload carries pre-bucketed strings
/// — frontend only renders.
const BUCKET_MAP: &[(&str, &[&str])] = &[
    ("BTC-beta", &["BTC", "WBTC", "TBTC"]),
    ("ETH-beta", &["ETH", "WETH", "STETH"]),
    (
        "alt-L1",
        &[
            "SOL", "AVAX", "NEAR", "DOT", "ADA", "ATOM", "APT", "SUI", "TON", "ICP",
        ],
    ),
    ("L2", &["ARB", "OP", "MATIC", "BASE", "STRK"]),
    ("stables", &["USDT", "USDC", "DAI", "BUSD", "TUSD", "FDUSD"]),
];

/// Map a coin (e.g. "BTC", "ETH", "PEPE") to its asset-family bucket.
/// Unknown coins → "other".
pub(crate) fn bucket_for(coin: &str) -> &'static str {
    let upper = coin.to_uppercase();
    for (bucket, members) in BUCKET_MAP {
        if members.iter().any(|m| *m == upper) {
            return bucket;
        }
    }
    "other"
}

/// Extract the base asset from a symbol in any of: `BTC`, `BTC/USDT`, `BTC/USDT:USDT`,
/// `BTC_USDT`, `BTC-USDT`, `BTCUSDT`. Returns the base in uppercase.
pub(crate) fn extract_base_asset(symbol: &str) -> String {
    let upper = symbol.to_uppercase();
    for sep in ['/', ':', '_', '-'] {
        if let Some(idx) = upper.find(sep) {
            return upper[..idx].to_string();
        }
    }
    for quote in ["USDT", "USDC", "USD", "PERP"] {
        if let Some(stripped) = upper.strip_suffix(quote) {
            if !stripped.is_empty() {
                return stripped.to_string();
            }
        }
    }
    upper
}

// ── 5s server-side cache (amortizes WS-debounced refetches) ──

const CACHE_TTL: Duration = Duration::from_secs(5);

static SNAPSHOT_CACHE: OnceLock<DashMap<Uuid, (RiskSnapshot, Instant)>> = OnceLock::new();

fn cache() -> &'static DashMap<Uuid, (RiskSnapshot, Instant)> {
    SNAPSHOT_CACHE.get_or_init(DashMap::new)
}

fn cache_get(user_id: Uuid) -> Option<RiskSnapshot> {
    cache().get(&user_id).and_then(|entry| {
        if entry.1.elapsed() < CACHE_TTL {
            Some(entry.0.clone())
        } else {
            None
        }
    })
}

fn cache_put(user_id: Uuid, snapshot: RiskSnapshot) {
    cache().insert(user_id, (snapshot, Instant::now()));
}

/// Drop a single user's cached snapshot. Useful in tests and after admin actions.
pub fn invalidate_cache(user_id: Uuid) {
    cache().remove(&user_id);
}

// ── Per-account fetch helpers ──

/// Returns (free, used, total) USD-equivalent margin for the account.
async fn fetch_account_margin(
    exchange_name: &str,
    creds: &DecryptedCredentials,
    app_state: &AppState,
) -> (Decimal, Decimal, Decimal) {
    if exchange_name.eq_ignore_ascii_case(exchanges::HYPERLIQUID) {
        fetch_hl_margin(creds, app_state).await
    } else {
        fetch_cex_margin(exchange_name, creds, app_state).await
    }
}

/// Hyperliquid margin via native info API (mirrors `get_hyperliquid_balance` in
/// `routes/exchanges.rs`).
async fn fetch_hl_margin(
    creds: &DecryptedCredentials,
    app_state: &AppState,
) -> (Decimal, Decimal, Decimal) {
    let query_address = creds.wallet_address.as_deref().unwrap_or(&creds.api_key);
    let info_url = match app_state.hl_network {
        hyperliquid_sdk_rs::Network::Mainnet => "https://api.hyperliquid.xyz/info",
        hyperliquid_sdk_rs::Network::Testnet => "https://api.hyperliquid-testnet.xyz/info",
    };
    let payload = serde_json::json!({ "type": "clearinghouseState", "user": query_address });

    let body: serde_json::Value = match app_state
        .hl_http_client
        .post(info_url)
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => match resp.json().await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("risk_snapshot: HL margin parse failed: {e}");
                return (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
            }
        },
        Err(e) => {
            tracing::warn!("risk_snapshot: HL margin fetch failed: {e}");
            return (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
        }
    };

    let total = body
        .get("marginSummary")
        .and_then(|m| m.get("accountValue"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Decimal>().ok())
        .unwrap_or_default();
    let used = body
        .get("marginSummary")
        .and_then(|m| m.get("totalMarginUsed"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Decimal>().ok())
        .unwrap_or_default();
    let free = total - used;
    (free, used, total)
}

/// CEX margin via sidecar `/balance`. Sums USD-pegged stablecoin entries
/// (USDT/USDC/USD/BUSD/DAI) — a USD-equivalent approximation that's good enough
/// for "free margin per venue".
async fn fetch_cex_margin(
    exchange_name: &str,
    creds: &DecryptedCredentials,
    app_state: &AppState,
) -> (Decimal, Decimal, Decimal) {
    let sidecar_creds = crate::services::SidecarCredentials {
        api_key: creds.api_key.clone(),
        secret: creds.api_secret.clone(),
        password: creds.passphrase.clone(),
    };

    let entries = match app_state
        .cex_client
        .fetch_balance(exchange_name, &sidecar_creds, false, "future")
        .await
    {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("risk_snapshot: CEX balance fetch failed for {exchange_name}: {e}");
            return (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
        }
    };

    const STABLE_ASSETS: &[&str] = &["USDT", "USDC", "USD", "BUSD", "DAI", "TUSD", "FDUSD"];
    let mut free = Decimal::ZERO;
    let mut used = Decimal::ZERO;
    let mut total = Decimal::ZERO;
    for entry in entries {
        if !STABLE_ASSETS.contains(&entry.asset.to_uppercase().as_str()) {
            continue;
        }
        free += entry.free.parse::<Decimal>().unwrap_or_default();
        used += entry.used.parse::<Decimal>().unwrap_or_default();
        total += entry.total.parse::<Decimal>().unwrap_or_default();
    }
    (free, used, total)
}

/// Returns the live position list for one account.
async fn fetch_account_positions(
    exchange_name: &str,
    creds: &DecryptedCredentials,
    app_state: &AppState,
) -> Vec<PositionEntry> {
    if exchange_name.eq_ignore_ascii_case(exchanges::HYPERLIQUID) {
        fetch_hl_positions(creds, app_state).await
    } else {
        fetch_cex_positions(exchange_name, creds, app_state).await
    }
}

/// Hyperliquid positions via clearinghouseState (mirrors `get_hyperliquid_positions`).
async fn fetch_hl_positions(
    creds: &DecryptedCredentials,
    app_state: &AppState,
) -> Vec<PositionEntry> {
    let query_address = creds.wallet_address.as_deref().unwrap_or(&creds.api_key);
    let info_url = match app_state.hl_network {
        hyperliquid_sdk_rs::Network::Mainnet => "https://api.hyperliquid.xyz/info",
        hyperliquid_sdk_rs::Network::Testnet => "https://api.hyperliquid-testnet.xyz/info",
    };
    let payload = serde_json::json!({ "type": "clearinghouseState", "user": query_address });

    let body: serde_json::Value = match app_state
        .hl_http_client
        .post(info_url)
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => resp.json().await.unwrap_or_default(),
        Err(e) => {
            tracing::warn!("risk_snapshot: HL positions fetch failed: {e}");
            return Vec::new();
        }
    };

    body.get("assetPositions")
        .and_then(|v| v.as_array())
        .map(|positions| {
            positions
                .iter()
                .filter_map(|ap| {
                    let pos = ap.get("position")?;
                    let szi: Decimal = pos.get("szi")?.as_str()?.parse().ok()?;
                    if szi.is_zero() {
                        return None;
                    }
                    let entry_price: Decimal = pos
                        .get("entryPx")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok())
                        .unwrap_or_default();
                    let mark_price: Decimal = pos
                        .get("markPx")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(entry_price);
                    let unrealized_pnl: Decimal = pos
                        .get("unrealizedPnl")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok())
                        .unwrap_or_default();
                    let position_value: Decimal = pos
                        .get("positionValue")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok())
                        .unwrap_or_else(|| mark_price * szi.abs());
                    let symbol = pos.get("coin")?.as_str()?.to_string();
                    let side = if szi > Decimal::ZERO { "long" } else { "short" };
                    // HL's position.leverage is `{ type: "cross"|"isolated", value: N }`.
                    let leverage: Option<Decimal> = pos
                        .get("leverage")
                        .and_then(|l| l.get("value"))
                        .and_then(|v| v.as_u64())
                        .map(Decimal::from);
                    Some(PositionEntry {
                        symbol,
                        side: side.to_string(),
                        entry_price,
                        mark_price,
                        quantity: szi.abs(),
                        notional_usd: position_value,
                        unrealized_pnl_usd: unrealized_pnl,
                        leverage,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// CEX positions via sidecar `/position`. Mark price is approximated from
/// (entry_price ± unrealized_pnl/contracts) since the sidecar response doesn't
/// carry it explicitly.
async fn fetch_cex_positions(
    exchange_name: &str,
    creds: &DecryptedCredentials,
    app_state: &AppState,
) -> Vec<PositionEntry> {
    let sidecar_creds = crate::services::SidecarCredentials {
        api_key: creds.api_key.clone(),
        secret: creds.api_secret.clone(),
        password: creds.passphrase.clone(),
    };

    let positions = match app_state
        .cex_client
        .fetch_positions(exchange_name, &sidecar_creds, false, None)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("risk_snapshot: CEX positions fetch failed for {exchange_name}: {e}");
            return Vec::new();
        }
    };

    positions
        .into_iter()
        .filter_map(|p| {
            let contracts: Decimal = p.contracts.parse().ok()?;
            if contracts.is_zero() {
                return None;
            }
            let entry_price: Decimal = p.entry_price.parse().unwrap_or_default();
            let unrealized_pnl: Decimal = p.unrealized_pnl.parse().unwrap_or_default();
            let side_lower = p.side.to_lowercase();
            let mark_price = if contracts > Decimal::ZERO {
                let pnl_per_contract = unrealized_pnl / contracts;
                if side_lower == "long" {
                    entry_price + pnl_per_contract
                } else {
                    entry_price - pnl_per_contract
                }
            } else {
                entry_price
            };
            let notional = mark_price * contracts;
            let leverage: Option<Decimal> = p
                .leverage
                .as_deref()
                .and_then(|s| s.parse().ok())
                .filter(|lev: &Decimal| *lev > Decimal::ZERO);
            Some(PositionEntry {
                symbol: p.symbol,
                side: side_lower,
                entry_price,
                mark_price,
                quantity: contracts,
                notional_usd: notional,
                unrealized_pnl_usd: unrealized_pnl,
                leverage,
            })
        })
        .collect()
}

// ── Aggregation ──

/// Group positions by asset family and surface directional stacking.
/// Direction: "long" if every contributing position is long, "short" if every
/// position is short, otherwise "mixed". Effective notional = absolute value of
/// signed-sum within bucket.
fn build_correlation_stack(venues: &[VenuePositions]) -> Vec<CorrelationBucket> {
    use std::collections::BTreeMap;

    struct Agg {
        signed_notional: Decimal,
        long_count: u32,
        short_count: u32,
        symbols: Vec<String>,
    }

    let mut buckets: BTreeMap<&'static str, Agg> = BTreeMap::new();
    for venue in venues {
        for pos in &venue.positions {
            let base = extract_base_asset(&pos.symbol);
            let bucket = bucket_for(&base);
            let signed = if pos.side == "long" {
                pos.notional_usd.abs()
            } else {
                -pos.notional_usd.abs()
            };
            let agg = buckets.entry(bucket).or_insert(Agg {
                signed_notional: Decimal::ZERO,
                long_count: 0,
                short_count: 0,
                symbols: Vec::new(),
            });
            agg.signed_notional += signed;
            if pos.side == "long" {
                agg.long_count += 1;
            } else {
                agg.short_count += 1;
            }
            if !agg.symbols.contains(&pos.symbol) {
                agg.symbols.push(pos.symbol.clone());
            }
        }
    }

    buckets
        .into_iter()
        .map(|(bucket, agg)| {
            let direction = match (agg.long_count, agg.short_count) {
                (_, 0) => "long",
                (0, _) => "short",
                _ => "mixed",
            };
            CorrelationBucket {
                bucket: bucket.to_string(),
                direction: direction.to_string(),
                effective_notional_usd: agg.signed_notional.abs(),
                contributing_symbols: agg.symbols,
            }
        })
        .collect()
}

/// Pure aggregation: given per-venue margin + positions, compute all scalar
/// metrics and derived views (correlation stack, sorted margins). Extracted
/// from [`build_snapshot`] so aggregation math is testable without mocking
/// HTTP clients or databases.
pub fn aggregate_snapshot(
    positions_by_venue: Vec<VenuePositions>,
    margin_by_venue: Vec<VenueMargin>,
    as_of: DateTime<Utc>,
) -> RiskSnapshot {
    // Sort margin descending by free_usd (FR-3).
    let mut margin_by_venue = margin_by_venue;
    margin_by_venue.sort_by(|a, b| b.free_usd.cmp(&a.free_usd));

    let net_exposure_usd: Decimal = positions_by_venue
        .iter()
        .flat_map(|v| v.positions.iter())
        .map(|p| p.notional_usd.abs())
        .sum();

    let free_margin_usd: Decimal = margin_by_venue.iter().map(|m| m.free_usd).sum();
    let total_margin_usd: Decimal = margin_by_venue.iter().map(|m| m.total_usd).sum();

    // Aggregate leverage = notional-weighted average of per-position configured
    // leverage when the exchange reports it. Matches trader mental model
    // ("Bybit slider is on 8x, show 8x"), not gross account exposure ratio.
    // Fallback to net_exposure / total_margin only when no position carries a
    // leverage value (e.g. all positions on adapters that omit the field).
    let (lev_numer, lev_denom) = positions_by_venue
        .iter()
        .flat_map(|v| v.positions.iter())
        .filter_map(|p| p.leverage.map(|lev| (lev * p.notional_usd.abs(), p.notional_usd.abs())))
        .fold((Decimal::ZERO, Decimal::ZERO), |(n, d), (ni, di)| (n + ni, d + di));

    let aggregate_leverage = if lev_denom > Decimal::ZERO {
        lev_numer / lev_denom
    } else if total_margin_usd > Decimal::ZERO {
        net_exposure_usd / total_margin_usd
    } else {
        Decimal::ZERO
    };

    let long_notional: Decimal = positions_by_venue
        .iter()
        .flat_map(|v| v.positions.iter())
        .filter(|p| p.side == "long")
        .map(|p| p.notional_usd.abs())
        .sum();
    let short_notional: Decimal = positions_by_venue
        .iter()
        .flat_map(|v| v.positions.iter())
        .filter(|p| p.side == "short")
        .map(|p| p.notional_usd.abs())
        .sum();
    let total_directional = long_notional + short_notional;
    let (long_pct, short_pct) = if total_directional > Decimal::ZERO {
        (
            long_notional / total_directional,
            short_notional / total_directional,
        )
    } else {
        (Decimal::ZERO, Decimal::ZERO)
    };
    let net_delta_usd = long_notional - short_notional;

    let correlation_stack = build_correlation_stack(&positions_by_venue);

    RiskSnapshot {
        net_exposure_usd,
        aggregate_leverage,
        free_margin_usd,
        long_pct,
        short_pct,
        net_delta_usd,
        positions_by_venue,
        margin_by_venue,
        correlation_stack,
        as_of,
    }
}

/// Build a risk snapshot for the given user.
///
/// Real per-account fan-out + 5s cache. Inactive agent wallets (returned by
/// `list_by_user` but unable to fetch via `load_credentials`) are skipped.
pub async fn build_snapshot(
    user_id: Uuid,
    app_state: &AppState,
) -> Result<RiskSnapshot, RiskError> {
    if let Some(cached) = cache_get(user_id) {
        tracing::debug!("risk_snapshot: cache hit for user {user_id}");
        return Ok(cached);
    }

    let accounts = app_state
        .exchange_account_repo
        .list_by_user(user_id)
        .await
        .map_err(|e| RiskError::Internal(format!("list_by_user: {e}")))?;

    // Skip inactive accounts (load_credentials filters to is_active anyway,
    // and inactive agent wallets need re-auth before they can be fetched).
    let active: Vec<ExchangeAccountRow> = accounts
        .into_iter()
        .filter(|a| a.is_active.unwrap_or(false))
        .collect();

    // Per-account fan-out: load creds, then concurrently fetch margin + positions.
    let fetches = active.iter().map(|acc| {
        let acc_id = acc.id;
        let exchange_name = acc.exchange_name.clone();
        async move {
            let creds = match app_state
                .exchange_account_repo
                .load_credentials(acc_id, user_id)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        "risk_snapshot: load_credentials failed for {acc_id}: {e}"
                    );
                    return None;
                }
            };

            let (margin, positions) = tokio::join!(
                fetch_account_margin(&exchange_name, &creds, app_state),
                fetch_account_positions(&exchange_name, &creds, app_state),
            );

            Some((
                VenueMargin {
                    exchange_id: acc_id,
                    exchange_name: exchange_name.clone(),
                    free_usd: margin.0,
                    used_usd: margin.1,
                    total_usd: margin.2,
                },
                VenuePositions {
                    exchange_id: acc_id,
                    exchange_name,
                    positions,
                },
            ))
        }
    });

    let results: Vec<(VenueMargin, VenuePositions)> = futures_util::future::join_all(fetches)
        .await
        .into_iter()
        .flatten()
        .collect();

    let (margin_by_venue, positions_by_venue): (Vec<_>, Vec<_>) =
        results.into_iter().unzip();

    let snapshot = aggregate_snapshot(positions_by_venue, margin_by_venue, Utc::now());

    cache_put(user_id, snapshot.clone());
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn unknown_coin_falls_into_other_bucket() {
        // Spec risk #5 mitigation: ensure new memes / unknown L1s don't crash
        // the aggregator — they bucket into "other".
        assert_eq!(bucket_for("PEPE"), "other");
        assert_eq!(bucket_for("WIF"), "other");
        assert_eq!(bucket_for("RANDOMTOKEN"), "other");
    }

    #[test]
    fn known_coins_bucket_correctly() {
        assert_eq!(bucket_for("BTC"), "BTC-beta");
        assert_eq!(bucket_for("btc"), "BTC-beta"); // case-insensitive
        assert_eq!(bucket_for("WBTC"), "BTC-beta");
        assert_eq!(bucket_for("ETH"), "ETH-beta");
        assert_eq!(bucket_for("SOL"), "alt-L1");
        assert_eq!(bucket_for("ARB"), "L2");
        assert_eq!(bucket_for("USDC"), "stables");
    }

    #[test]
    fn extract_base_handles_common_symbol_formats() {
        assert_eq!(extract_base_asset("BTC"), "BTC");
        assert_eq!(extract_base_asset("BTC/USDT"), "BTC");
        assert_eq!(extract_base_asset("BTC/USDT:USDT"), "BTC");
        assert_eq!(extract_base_asset("BTC_USDT"), "BTC");
        assert_eq!(extract_base_asset("BTC-USDT"), "BTC");
        assert_eq!(extract_base_asset("BTCUSDT"), "BTC");
        assert_eq!(extract_base_asset("ETHUSDC"), "ETH");
    }

    fn pos(symbol: &str, side: &str, notional: Decimal) -> PositionEntry {
        PositionEntry {
            symbol: symbol.to_string(),
            side: side.to_string(),
            entry_price: Decimal::ZERO,
            mark_price: Decimal::ZERO,
            quantity: Decimal::ZERO,
            notional_usd: notional,
            unrealized_pnl_usd: Decimal::ZERO,
            leverage: None,
        }
    }

    #[test]
    fn correlation_stack_long_only() {
        let venues = vec![VenuePositions {
            exchange_id: Uuid::new_v4(),
            exchange_name: "bybit".to_string(),
            positions: vec![pos("BTC/USDT", "long", dec!(1000))],
        }];
        let stack = build_correlation_stack(&venues);
        assert_eq!(stack.len(), 1);
        assert_eq!(stack[0].bucket, "BTC-beta");
        assert_eq!(stack[0].direction, "long");
        assert_eq!(stack[0].effective_notional_usd, dec!(1000));
    }

    #[test]
    fn correlation_stack_mixed_direction() {
        let venues = vec![VenuePositions {
            exchange_id: Uuid::new_v4(),
            exchange_name: "bybit".to_string(),
            positions: vec![
                pos("ETH/USDT", "long", dec!(2000)),
                pos("ETH/USDT", "short", dec!(500)),
            ],
        }];
        let stack = build_correlation_stack(&venues);
        let eth = stack.iter().find(|b| b.bucket == "ETH-beta").unwrap();
        assert_eq!(eth.direction, "mixed");
        // Signed sum: +2000 - 500 = +1500 → abs = 1500
        assert_eq!(eth.effective_notional_usd, dec!(1500));
    }

    #[test]
    fn correlation_stack_groups_alt_l1() {
        // Multiple alts → single alt-L1 bucket, all directions tracked.
        let venues = vec![VenuePositions {
            exchange_id: Uuid::new_v4(),
            exchange_name: "hyperliquid".to_string(),
            positions: vec![
                pos("SOL", "long", dec!(800)),
                pos("AVAX", "long", dec!(200)),
            ],
        }];
        let stack = build_correlation_stack(&venues);
        let alt = stack.iter().find(|b| b.bucket == "alt-L1").unwrap();
        assert_eq!(alt.direction, "long");
        assert_eq!(alt.effective_notional_usd, dec!(1000));
        assert_eq!(alt.contributing_symbols.len(), 2);
    }

    #[test]
    fn correlation_stack_unknown_coin_in_other() {
        let venues = vec![VenuePositions {
            exchange_id: Uuid::new_v4(),
            exchange_name: "bybit".to_string(),
            positions: vec![pos("PEPE/USDT", "long", dec!(50))],
        }];
        let stack = build_correlation_stack(&venues);
        let other = stack.iter().find(|b| b.bucket == "other").unwrap();
        assert_eq!(other.direction, "long");
        assert_eq!(other.effective_notional_usd, dec!(50));
    }

    #[test]
    fn cache_invalidation_drops_entry() {
        let user_id = Uuid::new_v4();
        let snap = RiskSnapshot {
            net_exposure_usd: dec!(123),
            aggregate_leverage: Decimal::ZERO,
            free_margin_usd: Decimal::ZERO,
            long_pct: Decimal::ZERO,
            short_pct: Decimal::ZERO,
            net_delta_usd: Decimal::ZERO,
            positions_by_venue: Vec::new(),
            margin_by_venue: Vec::new(),
            correlation_stack: Vec::new(),
            as_of: Utc::now(),
        };
        cache_put(user_id, snap);
        assert!(cache_get(user_id).is_some());
        invalidate_cache(user_id);
        assert!(cache_get(user_id).is_none());
    }

    // ── Fixture-driven aggregation tests (T3) ──

    fn venue_id() -> Uuid {
        Uuid::new_v4()
    }

    fn full_pos(
        symbol: &str,
        side: &str,
        entry: Decimal,
        mark: Decimal,
        qty: Decimal,
        pnl: Decimal,
    ) -> PositionEntry {
        PositionEntry {
            symbol: symbol.to_string(),
            side: side.to_string(),
            entry_price: entry,
            mark_price: mark,
            quantity: qty,
            notional_usd: mark * qty,
            unrealized_pnl_usd: pnl,
            leverage: None,
        }
    }

    #[test]
    fn aggregate_empty_returns_all_zeros() {
        // Case 1: no accounts, no positions → all zeros, empty arrays, as_of set.
        let as_of = Utc::now();
        let snap = aggregate_snapshot(Vec::new(), Vec::new(), as_of);

        assert_eq!(snap.net_exposure_usd, Decimal::ZERO);
        assert_eq!(snap.aggregate_leverage, Decimal::ZERO);
        assert_eq!(snap.free_margin_usd, Decimal::ZERO);
        assert_eq!(snap.long_pct, Decimal::ZERO);
        assert_eq!(snap.short_pct, Decimal::ZERO);
        assert_eq!(snap.net_delta_usd, Decimal::ZERO);
        assert!(snap.positions_by_venue.is_empty());
        assert!(snap.margin_by_venue.is_empty());
        assert!(snap.correlation_stack.is_empty());
        assert_eq!(snap.as_of, as_of);
    }

    #[test]
    fn aggregate_single_long_single_venue() {
        // Case 2: one long BTC position on one venue with 10k total margin.
        // Expected: exposure = notional, long_pct = 1.0, short_pct = 0, net_delta = +notional,
        // single "BTC-beta" bucket, leverage = notional / total_margin.
        let venue = venue_id();
        // 0.5 BTC @ 60000 mark → notional 30000.
        let positions = vec![VenuePositions {
            exchange_id: venue,
            exchange_name: "bybit".to_string(),
            positions: vec![full_pos(
                "BTC/USDT",
                "long",
                dec!(59_000),
                dec!(60_000),
                dec!(0.5),
                dec!(500),
            )],
        }];
        let margins = vec![VenueMargin {
            exchange_id: venue,
            exchange_name: "bybit".to_string(),
            free_usd: dec!(7_000),
            used_usd: dec!(3_000),
            total_usd: dec!(10_000),
        }];

        let snap = aggregate_snapshot(positions, margins, Utc::now());

        assert_eq!(snap.net_exposure_usd, dec!(30_000));
        assert_eq!(snap.free_margin_usd, dec!(7_000));
        assert_eq!(snap.aggregate_leverage, dec!(3)); // 30k / 10k
        assert_eq!(snap.long_pct, dec!(1));
        assert_eq!(snap.short_pct, dec!(0));
        assert_eq!(snap.net_delta_usd, dec!(30_000));

        assert_eq!(snap.correlation_stack.len(), 1);
        assert_eq!(snap.correlation_stack[0].bucket, "BTC-beta");
        assert_eq!(snap.correlation_stack[0].direction, "long");
        assert_eq!(
            snap.correlation_stack[0].effective_notional_usd,
            dec!(30_000)
        );
    }

    #[test]
    fn aggregate_multi_venue_mixed_direction_two_families() {
        // Case 3: two venues, mixed directions across BTC + ETH families.
        //   bybit: +2 ETH @ 3000 = +6000 notional long
        //   hyperliquid: -0.1 BTC @ 60000 = 6000 notional short
        // Expected:
        //   exposure = 12000
        //   long_pct = short_pct = 0.5, net_delta = 0
        //   two buckets (ETH-beta long 6000, BTC-beta short 6000)
        //   leverage = 12000 / (10k + 10k) = 0.6
        let bybit = venue_id();
        let hl = venue_id();

        let positions = vec![
            VenuePositions {
                exchange_id: bybit,
                exchange_name: "bybit".to_string(),
                positions: vec![full_pos(
                    "ETH/USDT",
                    "long",
                    dec!(2_900),
                    dec!(3_000),
                    dec!(2),
                    dec!(200),
                )],
            },
            VenuePositions {
                exchange_id: hl,
                exchange_name: "hyperliquid".to_string(),
                positions: vec![full_pos(
                    "BTC",
                    "short",
                    dec!(61_000),
                    dec!(60_000),
                    dec!(0.1),
                    dec!(100),
                )],
            },
        ];
        let margins = vec![
            VenueMargin {
                exchange_id: bybit,
                exchange_name: "bybit".to_string(),
                free_usd: dec!(4_000),
                used_usd: dec!(6_000),
                total_usd: dec!(10_000),
            },
            VenueMargin {
                exchange_id: hl,
                exchange_name: "hyperliquid".to_string(),
                free_usd: dec!(8_000),
                used_usd: dec!(2_000),
                total_usd: dec!(10_000),
            },
        ];

        let snap = aggregate_snapshot(positions, margins, Utc::now());

        assert_eq!(snap.net_exposure_usd, dec!(12_000));
        assert_eq!(snap.free_margin_usd, dec!(12_000));
        assert_eq!(snap.aggregate_leverage, dec!(0.6)); // 12k / 20k
        assert_eq!(snap.long_pct, dec!(0.5));
        assert_eq!(snap.short_pct, dec!(0.5));
        assert_eq!(snap.net_delta_usd, dec!(0));

        // Margins sorted descending by free_usd → hyperliquid (8k) first.
        assert_eq!(snap.margin_by_venue[0].exchange_name, "hyperliquid");
        assert_eq!(snap.margin_by_venue[1].exchange_name, "bybit");

        // Two distinct buckets: BTC-beta (short) + ETH-beta (long).
        assert_eq!(snap.correlation_stack.len(), 2);
        let btc = snap
            .correlation_stack
            .iter()
            .find(|b| b.bucket == "BTC-beta")
            .expect("BTC-beta bucket present");
        assert_eq!(btc.direction, "short");
        assert_eq!(btc.effective_notional_usd, dec!(6_000));

        let eth = snap
            .correlation_stack
            .iter()
            .find(|b| b.bucket == "ETH-beta")
            .expect("ETH-beta bucket present");
        assert_eq!(eth.direction, "long");
        assert_eq!(eth.effective_notional_usd, dec!(6_000));
    }

    #[test]
    fn aggregate_leverage_uses_position_weighted_avg_when_available() {
        // Two positions with configured leverage 8x and 5x, notional 100 and 200.
        // Weighted avg = (8 * 100 + 5 * 200) / (100 + 200) = 1800 / 300 = 6.
        // Gross ratio (300 / 1000 = 0.3x) must NOT be used when leverage data exists.
        let venue_id = Uuid::new_v4();
        let mut p1 = pos("BTC_USDT", "long", dec!(100));
        p1.leverage = Some(dec!(8));
        let mut p2 = pos("ETH_USDT", "long", dec!(200));
        p2.leverage = Some(dec!(5));

        let positions = vec![VenuePositions {
            exchange_id: venue_id,
            exchange_name: "bybit".to_string(),
            positions: vec![p1, p2],
        }];
        let margins = vec![VenueMargin {
            exchange_id: venue_id,
            exchange_name: "bybit".to_string(),
            free_usd: dec!(700),
            used_usd: dec!(300),
            total_usd: dec!(1_000),
        }];

        let snap = aggregate_snapshot(positions, margins, Utc::now());
        assert_eq!(snap.aggregate_leverage, dec!(6));
    }

    #[test]
    fn aggregate_leverage_falls_back_to_gross_when_no_position_leverage() {
        // Single 5000-notional position with NO leverage reported by adapter.
        // Expected: gross formula (5000 / 2500 = 2x), not zero.
        let venue_id = Uuid::new_v4();
        let positions = vec![VenuePositions {
            exchange_id: venue_id,
            exchange_name: "bybit".to_string(),
            positions: vec![pos("BTC_USDT", "long", dec!(5_000))],
        }];
        let margins = vec![VenueMargin {
            exchange_id: venue_id,
            exchange_name: "bybit".to_string(),
            free_usd: dec!(500),
            used_usd: dec!(2_000),
            total_usd: dec!(2_500),
        }];

        let snap = aggregate_snapshot(positions, margins, Utc::now());
        assert_eq!(snap.aggregate_leverage, dec!(2));
    }

    #[test]
    fn empty_snapshot_serializes_with_expected_shape() {
        let snap = RiskSnapshot {
            net_exposure_usd: Decimal::ZERO,
            aggregate_leverage: Decimal::ZERO,
            free_margin_usd: Decimal::ZERO,
            long_pct: Decimal::ZERO,
            short_pct: Decimal::ZERO,
            net_delta_usd: Decimal::ZERO,
            positions_by_venue: Vec::new(),
            margin_by_venue: Vec::new(),
            correlation_stack: Vec::new(),
            as_of: Utc::now(),
        };
        let json = serde_json::to_string(&snap).expect("serializes");
        assert!(json.contains("\"net_exposure_usd\""));
        assert!(json.contains("\"positions_by_venue\":[]"));
        assert!(json.contains("\"margin_by_venue\":[]"));
        assert!(json.contains("\"correlation_stack\":[]"));
        assert!(json.contains("\"as_of\""));
    }
}
