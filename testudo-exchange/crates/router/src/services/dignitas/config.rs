//! Load tunable Dignitas formula weights from the `dignitas_config` table (ENG-01a, T5).
//!
//! Weights are read fresh on each daily snapshot run so that changes to the
//! `dignitas_config` table take effect without a redeploy (FR-6).

// @anchor exchange:router:config
// @tags api

use std::collections::HashMap;

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use sqlx::PgPool;

use super::types::DignitasWeights;

/// Defaults matching the migration seed (weights sum to 1.0).
fn default_weight(key: &str) -> Decimal {
    match key {
        "weight_drawdown_adherence" => dec!(0.25),
        "weight_risk_per_trade_consistency" => dec!(0.20),
        "weight_setup_adherence" => dec!(0.20),
        "weight_coach_severity_penalty" => dec!(0.20),
        "weight_journal_consistency" => dec!(0.15),
        "cold_start_min_trades" => dec!(10),
        _ => Decimal::ZERO,
    }
}

/// Load `DignitasWeights` from the `dignitas_config` table.
///
/// Falls back to migration-seed defaults for any missing row so that a partial
/// config (e.g. mid-migration state) is still valid.
pub async fn load_weights(pool: &PgPool) -> Result<DignitasWeights, sqlx::Error> {
    let rows: Vec<(String, Decimal)> =
        sqlx::query_as("SELECT key, value FROM dignitas_config")
            .fetch_all(pool)
            .await?;

    let map: HashMap<String, Decimal> = rows.into_iter().collect();

    let get = |key: &str| -> Decimal {
        map.get(key).copied().unwrap_or_else(|| default_weight(key))
    };

    let cold_start_min_trades = get("cold_start_min_trades")
        .to_i64()
        .unwrap_or(10)
        .max(1);

    Ok(DignitasWeights {
        drawdown_adherence: get("weight_drawdown_adherence"),
        risk_per_trade_consistency: get("weight_risk_per_trade_consistency"),
        setup_adherence: get("weight_setup_adherence"),
        coach_severity_penalty: get("weight_coach_severity_penalty"),
        journal_consistency: get("weight_journal_consistency"),
        cold_start_min_trades,
    })
}
