//! Wire contracts and domain types for the Dignitas Score pipeline (ENG-01a).
//!
//! All numeric fields use `Decimal` — never `f64`.
//! Wire-format: Decimals serialize as JSON strings (rust_decimal default),
//! dates as `"YYYY-MM-DD"` strings (chrono + serde).

// @anchor exchange:router:types
// @tags api

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Domain types ────────────────────────────────────────────────────────────

/// The five behavioural discipline inputs that compose the Dignitas score.
///
/// Each value is in `[0.0, 1.0]`. Stored as flat columns in `dignitas_history`
/// and returned nested in API responses for clarity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputContributions {
    /// Fraction of days in trailing 30d where drawdown stayed within the
    /// configured daily-max-drawdown limit. `1.0` if limit never breached.
    pub drawdown_adherence: Decimal,
    /// `1 − mean(|actual_pct − configured_pct| / configured_pct)` over
    /// trailing 30d, per-trade deviation capped at `1.0` before averaging.
    pub risk_per_trade_consistency: Decimal,
    /// `count(trades with non-null setup_tag) / count(*)` over trailing 30d.
    pub setup_adherence: Decimal,
    /// Weighted coach-report severity rate over last 4 reports.
    /// `0.0` is best (no flags); higher is worse. Renormalized out of the
    /// composite when the user has no coach reports yet.
    pub coach_severity_penalty: Decimal,
    /// `count(trades with notes or linked journal entries) / count(closed trades)`
    /// over trailing 30d.
    pub journal_consistency: Decimal,
}

/// A single daily Dignitas snapshot — maps 1-to-1 with a `dignitas_history` row.
///
/// The flat field layout allows `sqlx::FromRow` derivation for DB reads;
/// call `.contributions()` when you need the nested `InputContributions`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DignitasSnapshot {
    pub user_id: Uuid,
    pub date: NaiveDate,
    /// Composite score in `[0.00, 100.00]`.
    pub score: Decimal,
    /// `true` while the user has fewer than `cold_start_min_trades` closed
    /// trades in the trailing 30d window. The composite is still calculated
    /// from available data but flagged as preliminary in the UI.
    pub cold_start: bool,
    /// Closed trades counted in the trailing 30d window for this snapshot.
    /// Drives the cold_start gate and powers the "based on N trades" UI copy.
    pub trade_count_30d: i64,
    // Flat contributions — mirror `dignitas_history` columns.
    pub drawdown_adherence: Decimal,
    pub risk_per_trade_consistency: Decimal,
    pub setup_adherence: Decimal,
    pub coach_severity_penalty: Decimal,
    pub journal_consistency: Decimal,
}

impl DignitasSnapshot {
    pub fn contributions(&self) -> InputContributions {
        InputContributions {
            drawdown_adherence: self.drawdown_adherence,
            risk_per_trade_consistency: self.risk_per_trade_consistency,
            setup_adherence: self.setup_adherence,
            coach_severity_penalty: self.coach_severity_penalty,
            journal_consistency: self.journal_consistency,
        }
    }
}

/// Tunable formula weights loaded from `dignitas_config` at snapshot time.
///
/// Defaults defined in the migration (sum = 1.0). Changing a row in
/// `dignitas_config` takes effect on the next daily run (FR-6); prior
/// snapshots retain the weights in effect when they were written.
#[derive(Debug, Clone)]
pub struct DignitasWeights {
    pub drawdown_adherence: Decimal,
    pub risk_per_trade_consistency: Decimal,
    pub setup_adherence: Decimal,
    pub coach_severity_penalty: Decimal,
    pub journal_consistency: Decimal,
    /// Minimum closed trades in trailing 30d before `cold_start` lifts
    /// (default 10). Replaces the prior day-count gate; trade volume is
    /// the actual driver of statistical signal strength on the inputs.
    pub cold_start_min_trades: i64,
}

impl DignitasWeights {
    /// Renormalize the four non-coach weights to sum to 1.0 when no coach
    /// data is available for the user. The coach weight is zeroed out.
    pub fn without_coach(&self) -> Self {
        let four_sum = self.drawdown_adherence
            + self.risk_per_trade_consistency
            + self.setup_adherence
            + self.journal_consistency;

        if four_sum.is_zero() {
            // Degenerate config — return equal weights.
            let quarter = Decimal::new(25, 2);
            return Self {
                drawdown_adherence: quarter,
                risk_per_trade_consistency: quarter,
                setup_adherence: quarter,
                coach_severity_penalty: Decimal::ZERO,
                journal_consistency: quarter,
                cold_start_min_trades: self.cold_start_min_trades,
            };
        }

        Self {
            drawdown_adherence: self.drawdown_adherence / four_sum,
            risk_per_trade_consistency: self.risk_per_trade_consistency / four_sum,
            setup_adherence: self.setup_adherence / four_sum,
            coach_severity_penalty: Decimal::ZERO,
            journal_consistency: self.journal_consistency / four_sum,
            cold_start_min_trades: self.cold_start_min_trades,
        }
    }
}

// ─── API response types ───────────────────────────────────────────────────────

/// Response for `GET /api/dignitas/me`.
#[derive(Debug, Serialize)]
pub struct DignitasCurrent {
    /// Current Dignitas score `[0.00, 100.00]`.
    pub score: Decimal,
    /// Signed delta vs the snapshot from 7 days ago. `None` until 7 snapshots
    /// exist — frontend renders `DIGNITAS 72.4 —` with an em-dash in that case.
    pub delta_7d: Option<Decimal>,
    /// `true` while the cold-start window is active. Score is calculated
    /// from available data but flagged as preliminary.
    pub cold_start: bool,
    /// Closed trades counted in the trailing 30d window for the latest
    /// snapshot. Drives the "PRELIMINARY — N of M trades" UI copy.
    pub trade_count_30d: i64,
    /// User preference: pill hidden from top nav.
    pub pill_hidden: bool,
    /// Per-axis breakdown — drives the Overview radar (D1 resolution).
    pub input_contributions: InputContributions,
    /// `false` when the user has no coach reports yet; radar dims the Coach
    /// Alignment axis when this is false.
    pub coach_data_available: bool,
}

/// Response for `GET /api/dignitas/history?days=N`.
#[derive(Debug, Serialize)]
pub struct DignitasHistoryResponse {
    pub snapshots: Vec<DignitasHistoryPoint>,
}

/// One point in the 90-day sparkline series.
#[derive(Debug, Serialize)]
pub struct DignitasHistoryPoint {
    /// ISO date string `"YYYY-MM-DD"`.
    #[serde(serialize_with = "serialize_naive_date")]
    pub date: NaiveDate,
    pub score: Decimal,
}

fn serialize_naive_date<S>(date: &NaiveDate, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    s.serialize_str(&date.format("%Y-%m-%d").to_string())
}

/// Body for `PATCH /api/dignitas/preferences`.
#[derive(Debug, Deserialize)]
pub struct UpdateDignitasPreferences {
    pub pill_hidden: Option<bool>,
}
