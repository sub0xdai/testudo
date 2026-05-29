// @anchor exchange:router:raw_fills
// @tags api

use chrono::{DateTime, Utc};
use common_utils::journal::{FillSide, RawFill};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct RawFillRow {
    user_id: Uuid,
    exchange: String,
    exec_id: String,
    symbol: String,
    side: String,
    price: Decimal,
    qty: Decimal,
    fee: Decimal,
    fee_asset: String,
    exec_time: DateTime<Utc>,
    order_id: Option<String>,
    raw_json: serde_json::Value,
}

impl TryFrom<RawFillRow> for RawFill {
    type Error = String;

    fn try_from(row: RawFillRow) -> Result<Self, Self::Error> {
        let side = match row.side.as_str() {
            "Buy" => FillSide::Buy,
            "Sell" => FillSide::Sell,
            other => return Err(format!("unknown fill side: {other}")),
        };
        Ok(RawFill {
            user_id: row.user_id,
            exchange: row.exchange,
            exec_id: row.exec_id,
            symbol: row.symbol,
            side,
            price: row.price,
            qty: row.qty,
            fee: row.fee,
            fee_asset: row.fee_asset,
            exec_time: row.exec_time,
            order_id: row.order_id,
            raw_json: row.raw_json,
        })
    }
}

#[derive(Clone)]
pub struct RawFillRepository {
    pool: PgPool,
}

impl RawFillRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert fills, skipping any whose (user_id, exchange, exec_id) already exist.
    /// Returns the count of newly inserted rows.
    pub async fn upsert_many(&self, fills: &[RawFill]) -> Result<usize, sqlx::Error> {
        let mut inserted = 0usize;
        for fill in fills {
            let side_str = match fill.side {
                FillSide::Buy => "Buy",
                FillSide::Sell => "Sell",
            };
            let result = sqlx::query(
                "INSERT INTO raw_fills \
                 (user_id, exchange, exec_id, symbol, side, price, qty, fee, fee_asset, \
                  exec_time, order_id, raw_json) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
                 ON CONFLICT (user_id, exchange, exec_id) DO NOTHING",
            )
            .bind(fill.user_id)
            .bind(&fill.exchange)
            .bind(&fill.exec_id)
            .bind(&fill.symbol)
            .bind(side_str)
            .bind(fill.price)
            .bind(fill.qty)
            .bind(fill.fee)
            .bind(&fill.fee_asset)
            .bind(fill.exec_time)
            .bind(&fill.order_id)
            .bind(&fill.raw_json)
            .execute(&self.pool)
            .await?;

            inserted += result.rows_affected() as usize;
        }
        Ok(inserted)
    }

    /// Fetch all fills for a (user, exchange) pair, ordered chronologically.
    pub async fn fetch_for_account(
        &self,
        user_id: Uuid,
        exchange: &str,
    ) -> Result<Vec<RawFill>, sqlx::Error> {
        let rows = sqlx::query_as::<_, RawFillRow>(
            "SELECT user_id, exchange, exec_id, symbol, side, price, qty, fee, fee_asset, \
             exec_time, order_id, raw_json \
             FROM raw_fills \
             WHERE user_id = $1 AND exchange = $2 \
             ORDER BY exec_time ASC, exec_id ASC",
        )
        .bind(user_id)
        .bind(exchange)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|r| RawFill::try_from(r).map_err(sqlx::Error::Protocol))
            .collect()
    }

    /// Count fills for a (user, exchange) pair.
    pub async fn count_for_account(
        &self,
        user_id: Uuid,
        exchange: &str,
    ) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM raw_fills WHERE user_id = $1 AND exchange = $2",
        )
        .bind(user_id)
        .bind(exchange)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal_macros::dec;

    fn make_fill(user_id: Uuid, exec_id: &str, side: FillSide) -> RawFill {
        RawFill {
            user_id,
            exchange: "bybit".to_string(),
            exec_id: exec_id.to_string(),
            symbol: "BTC_USDT".to_string(),
            side,
            price: dec!(50000),
            qty: dec!(0.05),
            fee: dec!(1.25),
            fee_asset: "USDT".to_string(),
            exec_time: Utc.with_ymd_and_hms(2026, 5, 3, 10, 0, 0).unwrap(),
            order_id: Some("ord1".to_string()),
            raw_json: serde_json::json!({}),
        }
    }

    #[test]
    fn test_raw_fill_row_try_from_valid() {
        let row = RawFillRow {
            user_id: Uuid::new_v4(),
            exchange: "bybit".to_string(),
            exec_id: "e1".to_string(),
            symbol: "BTC_USDT".to_string(),
            side: "Buy".to_string(),
            price: dec!(50000),
            qty: dec!(0.1),
            fee: dec!(2.5),
            fee_asset: "USDT".to_string(),
            exec_time: Utc.with_ymd_and_hms(2026, 5, 3, 10, 0, 0).unwrap(),
            order_id: None,
            raw_json: serde_json::json!({}),
        };
        let fill = RawFill::try_from(row).unwrap();
        assert_eq!(fill.side, FillSide::Buy);
        assert_eq!(fill.exec_id, "e1");
    }

    #[test]
    fn test_raw_fill_row_try_from_sell() {
        let row = RawFillRow {
            user_id: Uuid::new_v4(),
            exchange: "bybit".to_string(),
            exec_id: "e2".to_string(),
            symbol: "BTC_USDT".to_string(),
            side: "Sell".to_string(),
            price: dec!(51000),
            qty: dec!(0.1),
            fee: dec!(2.55),
            fee_asset: "USDT".to_string(),
            exec_time: Utc.with_ymd_and_hms(2026, 5, 3, 11, 0, 0).unwrap(),
            order_id: None,
            raw_json: serde_json::json!({}),
        };
        let fill = RawFill::try_from(row).unwrap();
        assert_eq!(fill.side, FillSide::Sell);
    }

    #[test]
    fn test_raw_fill_row_try_from_unknown_side() {
        let row = RawFillRow {
            user_id: Uuid::new_v4(),
            exchange: "bybit".to_string(),
            exec_id: "e3".to_string(),
            symbol: "BTC_USDT".to_string(),
            side: "LONG".to_string(),
            price: dec!(50000),
            qty: dec!(0.1),
            fee: dec!(0),
            fee_asset: "USDT".to_string(),
            exec_time: Utc.with_ymd_and_hms(2026, 5, 3, 10, 0, 0).unwrap(),
            order_id: None,
            raw_json: serde_json::json!({}),
        };
        assert!(RawFill::try_from(row).is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn test_upsert_and_fetch() {
        let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for integration tests");
        let pool = sqlx::PgPool::connect(&db_url).await.unwrap();
        let repo = RawFillRepository::new(pool.clone());

        // Create a test user via raw SQL (no user repo available here)
        let user_id = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, email, password_hash) VALUES ($1, $2, $3)")
            .bind(user_id)
            .bind(format!("test_rf_{}@test.com", user_id))
            .bind("hash")
            .execute(&pool)
            .await
            .unwrap();

        // Build 5 fills
        let fills: Vec<RawFill> = (1..=5)
            .map(|i| {
                let side = if i % 2 == 1 { FillSide::Buy } else { FillSide::Sell };
                make_fill(user_id, &format!("exec_{i}"), side)
            })
            .collect();

        // First upsert — all 5 should be new
        let new_count = repo.upsert_many(&fills).await.unwrap();
        assert_eq!(new_count, 5);

        // Second upsert with 3 extra fills (total 8, 5 already exist, 3 new)
        let extra_fills: Vec<RawFill> = [
            // re-insert same 5
            fills.clone(),
            // 3 new
            vec![
                make_fill(user_id, "exec_6", FillSide::Buy),
                make_fill(user_id, "exec_7", FillSide::Sell),
                make_fill(user_id, "exec_8", FillSide::Buy),
            ],
        ]
        .concat();
        let new_count2 = repo.upsert_many(&extra_fills).await.unwrap();
        assert_eq!(new_count2, 3);

        // fetch_for_account should return all 8
        let fetched = repo.fetch_for_account(user_id, "bybit").await.unwrap();
        assert_eq!(fetched.len(), 8);

        // count_for_account
        let count = repo.count_for_account(user_id, "bybit").await.unwrap();
        assert_eq!(count, 8);

        // Cleanup — raw_fills cascade-deletes on user delete
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&pool)
            .await;
    }
}
