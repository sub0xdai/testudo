use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FillSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeSide {
    Long,
    Short,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFill {
    pub user_id: Uuid,
    pub exchange: String,
    pub exec_id: String,
    pub symbol: String,
    pub side: FillSide,
    pub price: Decimal,
    pub qty: Decimal,
    pub fee: Decimal,
    pub fee_asset: String,
    pub exec_time: DateTime<Utc>,
    pub order_id: Option<String>,
    pub raw_json: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconstructedTrade {
    pub user_id: Uuid,
    pub exchange: String,
    pub symbol: String,
    pub side: TradeSide,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub quantity: Decimal,
    /// Sum of fees in their native fee asset (normalization deferred to JNL-SYNC-02).
    pub fees: Decimal,
    pub realized_pnl: Decimal,
    pub opened_at: DateTime<Utc>,
    pub closed_at: DateTime<Utc>,
    pub source_fills: Vec<String>,
    pub source_fills_hash: String,
}

/// Groups fills by symbol, sorts chronologically, and emits one `ReconstructedTrade`
/// per round trip (net position crosses zero). Pure — no I/O, no clock.
///
/// Open positions (trailing non-zero net) are NOT emitted (FR-12).
/// Fee-asset normalization is deferred to JNL-SYNC-02; fees are summed as-is.
pub fn reconstruct_trades(fills: &[RawFill]) -> Vec<ReconstructedTrade> {
    let mut by_symbol: HashMap<&str, Vec<&RawFill>> = HashMap::new();
    for fill in fills {
        by_symbol.entry(fill.symbol.as_str()).or_default().push(fill);
    }

    let mut out = Vec::new();
    // Sort by symbol key for deterministic ordering across calls.
    let mut keys: Vec<&str> = by_symbol.keys().copied().collect();
    keys.sort_unstable();

    for key in keys {
        let symbol_fills = by_symbol.get_mut(key).unwrap();
        symbol_fills.sort_by(|a, b| {
            a.exec_time
                .cmp(&b.exec_time)
                .then_with(|| a.exec_id.cmp(&b.exec_id))
        });
        out.extend(process_symbol(symbol_fills));
    }

    out
}

/// SHA-256 of sorted, deduplicated, colon-joined exec IDs.
/// Deterministic and order-independent.
pub fn hash_source_fills(exec_ids: &[String]) -> String {
    let mut sorted = exec_ids.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let key = sorted.join(":");
    let digest = Sha256::digest(key.as_bytes());
    hex::encode(digest)
}

// ─── private ─────────────────────────────────────────────────────────────────

/// Lightweight accumulator entry. Allows overriding qty/fee/exec_id for
/// split fills produced by a side-flip without cloning the raw_json blob.
struct Slot<'a> {
    fill: &'a RawFill,
    qty: Decimal,
    fee: Decimal,
    exec_id: String,
}

fn process_symbol(fills: &[&RawFill]) -> Vec<ReconstructedTrade> {
    let mut out = Vec::new();
    let mut net_qty = Decimal::ZERO;
    let mut opening_side: Option<FillSide> = None;
    let mut accum: Vec<Slot<'_>> = Vec::new();

    for &fill in fills {
        let signed = signed_qty(fill);
        let prev_net = net_qty;
        net_qty += signed;

        if opening_side.is_none() && !net_qty.is_zero() {
            opening_side = Some(fill.side);
        }

        let is_flip = !prev_net.is_zero()
            && !net_qty.is_zero()
            && (prev_net > Decimal::ZERO) != (net_qty > Decimal::ZERO);

        if is_flip {
            tracing::warn!(
                exec_id = %fill.exec_id,
                symbol = %fill.symbol,
                prev_net = %prev_net,
                new_net = %net_qty,
                "side flip in single fill — splitting into close and open portions"
            );

            let close_qty = prev_net.abs();
            let open_qty = fill.qty - close_qty;
            let (close_fee, open_fee) = split_fee(fill.fee, fill.qty, close_qty);

            accum.push(Slot {
                fill,
                qty: close_qty,
                fee: close_fee,
                exec_id: format!("{}_close", fill.exec_id),
            });

            if let Some(side) = opening_side {
                if let Some(trade) = build_trade(&accum, side) {
                    out.push(trade);
                }
            }
            accum.clear();

            accum.push(Slot {
                fill,
                qty: open_qty,
                fee: open_fee,
                exec_id: format!("{}_open", fill.exec_id),
            });
            opening_side = Some(fill.side);
        } else {
            accum.push(Slot {
                fill,
                qty: fill.qty,
                fee: fill.fee,
                exec_id: fill.exec_id.clone(),
            });

            if !prev_net.is_zero() && net_qty.is_zero() {
                if let Some(side) = opening_side {
                    if let Some(trade) = build_trade(&accum, side) {
                        out.push(trade);
                    }
                }
                accum.clear();
                opening_side = None;
            }
        }
    }

    out
}

fn signed_qty(fill: &RawFill) -> Decimal {
    match fill.side {
        FillSide::Buy => fill.qty,
        FillSide::Sell => -fill.qty,
    }
}

/// Splits `total_fee` proportionally by `close_qty / total_qty`.
/// Returns `(close_fee, open_fee)`.
fn split_fee(total_fee: Decimal, total_qty: Decimal, close_qty: Decimal) -> (Decimal, Decimal) {
    if total_qty.is_zero() {
        return (Decimal::ZERO, Decimal::ZERO);
    }
    let close_fee = total_fee * (close_qty / total_qty);
    (close_fee, total_fee - close_fee)
}

fn build_trade(accum: &[Slot<'_>], opening_side: FillSide) -> Option<ReconstructedTrade> {
    if accum.is_empty() {
        return None;
    }

    let first = accum[0].fill;

    let opening: Vec<&Slot<'_>> = accum.iter().filter(|s| s.fill.side == opening_side).collect();
    let closing: Vec<&Slot<'_>> = accum.iter().filter(|s| s.fill.side != opening_side).collect();

    let total_open_qty: Decimal = opening.iter().map(|s| s.qty).sum();
    let total_close_qty: Decimal = closing.iter().map(|s| s.qty).sum();

    if total_open_qty.is_zero() || total_close_qty.is_zero() {
        return None;
    }

    let entry_price =
        opening.iter().map(|s| s.fill.price * s.qty).sum::<Decimal>() / total_open_qty;
    let exit_price =
        closing.iter().map(|s| s.fill.price * s.qty).sum::<Decimal>() / total_close_qty;
    let quantity = total_open_qty;
    let fees: Decimal = accum.iter().map(|s| s.fee).sum();

    let realized_pnl = match opening_side {
        FillSide::Buy => (exit_price - entry_price) * quantity,
        FillSide::Sell => (entry_price - exit_price) * quantity,
    };

    let trade_side = match opening_side {
        FillSide::Buy => TradeSide::Long,
        FillSide::Sell => TradeSide::Short,
    };

    let opened_at = opening.iter().map(|s| s.fill.exec_time).min()?;
    let closed_at = closing.iter().map(|s| s.fill.exec_time).max()?;

    let mut exec_ids: Vec<String> = accum.iter().map(|s| s.exec_id.clone()).collect();
    exec_ids.sort_unstable();
    exec_ids.dedup();
    let source_fills_hash = hash_source_fills(&exec_ids);

    Some(ReconstructedTrade {
        user_id: first.user_id,
        exchange: first.exchange.clone(),
        symbol: first.symbol.clone(),
        side: trade_side,
        entry_price,
        exit_price,
        quantity,
        fees,
        realized_pnl,
        opened_at,
        closed_at,
        source_fills: exec_ids,
        source_fills_hash,
    })
}

#[cfg(test)]
mod tests;
