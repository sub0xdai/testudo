use chrono::{Duration, Utc};
use rand::Rng;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use db_processor::{query::insert_trade, types::DbTrade};

pub async fn generate_random_trades(pool: &PgPool, num_trades: i32) -> Result<(), sqlx::Error> {
    let mut rng = rand::thread_rng();
    let mut trade_id = 50207;

    // Simulate distinct high/low prices by day
    for _ in 0..num_trades {
        trade_id += 1;

        // Introduce a daily trend that causes the price to fluctuate across days
        let trend = rng.gen_range(-1.0..1.0); // Trend for the day
        let days_back = rng.gen_range(0..30); // Random number of days back
        let seconds_back = rng.gen_range(0..86400); // Random number of seconds in a day

        // Base price for the trade, adjusted by the daily trend
        let base_price = rng.gen_range(102.0..115.0) + trend * (2 - days_back) as f64;

        // Introduce time-based fluctuation within the day
        let time_of_day_factor = (seconds_back as f64) / 86400.0; // A factor from 0 to 1 based on time of day
        let price_variation = rng.gen_range(-2.0..2.0); // Small random price variation

        // Final price is the base price plus time-based variation
        let price = Decimal::from_f64(base_price + time_of_day_factor * price_variation)
            .unwrap()
            .round_dp(2);

        // Ensure some quantity variation
        let quantity = Decimal::from_f64(rng.gen_range(0.01..0.2))
            .unwrap()
            .round_dp(2);

        // Generate other trade details
        let market = "SOL_USDC".to_string();
        let user_id = "test_user".to_string();
        let other_user_id = "test_user".to_string();
        let order_id = Uuid::new_v4().to_string();

        // Generate the timestamp based on days and seconds back
        let timestamp = Utc::now()
            .checked_sub_signed(Duration::days(days_back))
            .unwrap()
            .checked_sub_signed(Duration::seconds(seconds_back as i64))
            .unwrap()
            .timestamp_millis(); // UNIX timestamp in milliseconds

        let trade = DbTrade {
            trade_id,
            market,
            price,
            quantity,
            user_id,
            other_user_id,
            order_id,
            timestamp,
        };

        // Insert the trade into the database
        insert_trade(pool, trade).await?;
    }

    Ok(())
}
