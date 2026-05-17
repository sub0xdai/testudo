use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// JNL-18: Image storage tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct JournalImage {
    pub id: Uuid,
    pub user_id: Uuid,
    pub file_name: String,
    pub file_size: i64,
    pub mime_type: String,
    pub storage_path: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct JournalTrade {
    pub id: Uuid,
    pub user_id: Uuid,
    pub exchange: String,
    pub symbol: String,
    pub side: String,
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub quantity: Decimal,
    pub leverage: i32,
    pub realized_pnl: Decimal,
    pub realized_pnl_pct: Decimal,
    pub fees: Decimal,
    pub net_pnl: Decimal,
    pub stop_price: Option<Decimal>,
    pub target_price: Option<Decimal>,
    pub risk_amount: Option<Decimal>,
    pub r_multiple: Option<Decimal>,
    pub opened_at: DateTime<Utc>,
    pub closed_at: DateTime<Utc>,
    pub duration_secs: i32,
    pub trade_group_id: Option<Uuid>,
    pub notes: Option<String>,
    pub source: String,
    pub exchange_fill_id: Option<i64>,
    pub setup_tag: Option<String>,
    /// QNT-01a: calibration snapshot at entry, populated only for Dynamic Risk
    /// trades with a `setup_tag`. NULL for fixed-mode trades and untagged fallbacks.
    pub kelly_inputs: Option<serde_json::Value>,
    /// FIX-08: true when exit_price is a placeholder (0) pending REST reconciliation.
    #[serde(default)]
    pub needs_reconciliation: bool,
    /// FIX-09: 'sl' | 'tp' | 'manual' — close leg classification (legacy; unwritten by JNL-SYNC-01+).
    #[serde(default)]
    pub close_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// JNL-SYNC-01: SHA-256 of sorted exec_ids; NULL for live trades and imports.
    #[serde(default)]
    #[sqlx(default)]
    pub source_fills_hash: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: Uuid,
    pub user_id: Uuid,
    pub trade_id: Option<Uuid>,
    pub entry_date: Option<NaiveDate>,
    pub title: String,
    pub body: String,
    pub entry_type: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct JournalTag {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct JournalDailyStat {
    pub id: Uuid,
    pub user_id: Uuid,
    pub stat_date: NaiveDate,
    pub exchange: Option<String>,
    pub trade_count: i32,
    pub win_count: i32,
    pub loss_count: i32,
    pub gross_profit: Decimal,
    pub gross_loss: Decimal,
    pub net_pnl: Decimal,
    pub fees: Decimal,
    pub cumulative_pnl: Decimal,
    pub peak_cumulative_pnl: Decimal,
    pub drawdown: Decimal,
    pub drawdown_pct: Decimal,
}
