use crate::types::{DbTrade, KlineData, TickerData};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::{Pool, Postgres};

pub async fn insert_trade(pool: &Pool<Postgres>, trade: DbTrade) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO trades(
          trade_id, market, price, quantity, user_id, other_user_id, order_id, timestamp
      ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(trade.trade_id)
    .bind(trade.market)
    .bind(trade.price)
    .bind(trade.quantity)
    .bind(trade.user_id)
    .bind(trade.other_user_id)
    .bind(trade.order_id)
    .bind(trade.timestamp)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_trades_from_db(
    pool: &Pool<Postgres>,
    market: String,
) -> Result<Vec<DbTrade>, sqlx::Error> {
    let trades = sqlx::query!(
        "SELECT * FROM trades WHERE market = $1 ORDER BY timestamp desc LIMIT 100",
        market
    )
    .fetch_all(pool)
    .await?;

    let trades_vec: Vec<DbTrade> = trades
        .iter()
        .map(|trade| DbTrade {
            trade_id: trade.trade_id,
            market: trade.market.clone(),
            price: trade.price.to_string().parse::<Decimal>().unwrap(),
            quantity: trade.quantity.to_string().parse::<Decimal>().unwrap(),
            user_id: trade.user_id.clone(),
            other_user_id: trade.other_user_id.clone(),
            order_id: trade.order_id.clone(),
            timestamp: trade.timestamp,
        })
        .collect();

    Ok(trades_vec)
}

fn parse_custom_date(date_str: &str) -> String {
    // https://stackoverflow.com/questions/67774426/convert-postgres-timestamp-to-rust-chrono
    let simplified_date_str = date_str.replace("+00:00:00", "+00:00"); // as %:z expects +00:00, not +00:00:00

    let fixed_offset = DateTime::parse_from_str(&simplified_date_str, "%Y-%m-%d %H:%M:%S%.f %:z")
        .expect("Failed to parse date with timezone");

    fixed_offset.with_timezone(&Utc).to_string() // store as UTC, convert to relevant timezone on client side
}

pub async fn get_klines_timeseries_data(
    pool: &Pool<Postgres>,
    market: String,
    interval: String,
    start_time: String,
) -> Result<Vec<KlineData>, sqlx::Error> {
    let start_time_int = start_time.parse::<i64>().unwrap();

    let klines = sqlx::query!(
        // The SQL query for generating kline data
        "
        WITH timeseries_data AS (
            SELECT
                date_trunc($1, to_timestamp(timestamp / 1000)) AS bucket,
                price,
                quantity,
                trade_id,
                to_timestamp(timestamp / 1000) AS trade_time,
                ROW_NUMBER() OVER (PARTITION BY date_trunc($1, to_timestamp(timestamp / 1000)) ORDER BY timestamp ASC) AS row_num_asc,
                ROW_NUMBER() OVER (PARTITION BY date_trunc($1, to_timestamp(timestamp / 1000)) ORDER BY timestamp DESC) AS row_num_desc
            FROM trades
            WHERE timestamp >= $2
              AND market = $3
        )
        , aggregated_data AS (
            SELECT
                bucket,
                MAX(price) AS high,
                MIN(price) AS low,
                SUM(quantity) AS volume,
                COUNT(trade_id) AS trades,
                SUM(price * quantity) AS quote_volume,
                MIN(trade_time) AS start_time,
                MAX(trade_time) AS end_time,
                MAX(CASE WHEN row_num_asc = 1 THEN price END) AS open,
                MAX(CASE WHEN row_num_desc = 1 THEN price END) AS close
            FROM timeseries_data
            GROUP BY bucket
        )
        SELECT
            open,
            bucket AS end,
            high,
            low,
            close,
            quote_volume,
            start_time AS start,
            trades,
            volume
        FROM aggregated_data
        ORDER BY bucket ASC
        ",
        interval,      // $1: interval (e.g., 'day', 'week', 'month')
        start_time_int,    // $2: start time for filtering trades
        market         // $3: the market identifier
    )
    .fetch_all(pool)
    .await?;

    // Map the result set into a vector of KlineData structs
    let kline_data_vec: Vec<KlineData> = klines
        .iter()
        .map(|kline| {
            let start_time = parse_custom_date(kline.start.unwrap().to_string().as_str());
            let end_time = parse_custom_date(kline.end.unwrap().to_string().as_str());

            KlineData {
                open: kline.open.clone().unwrap().to_string(),
                high: kline.high.clone().unwrap().to_string(),
                low: kline.low.clone().unwrap().to_string(),
                close: kline.close.clone().unwrap().to_string(),
                quote_volume: kline.quote_volume.clone().unwrap().to_string(),
                start: start_time,
                end: end_time,
                trades: kline.trades.unwrap().to_string(),
                volume: kline.volume.clone().unwrap().to_string(),
            }
        })
        .collect();

    Ok(kline_data_vec)
}

pub async fn get_tickers_from_db(pool: &Pool<Postgres>) -> Result<Vec<TickerData>, sqlx::Error> {
    let now = Utc::now();
    let start_time_24h_ago = now - Duration::hours(24);
    let start_timestamp_24h_ago = start_time_24h_ago.timestamp_millis();

    let tickers = sqlx::query!(
        "
        WITH aggregated_trades AS (
            SELECT
                market AS symbol,
                MIN(price) FILTER (WHERE row_num_asc = 1) AS first_price,
                MAX(price) AS high,
                MIN(price) AS low,
                MIN(price) FILTER (WHERE row_num_desc = 1) AS last_price,
                MAX(price) FILTER (WHERE row_num_desc = 1) - MIN(price) FILTER (WHERE row_num_asc = 1) AS price_change,
                (MAX(price) FILTER (WHERE row_num_desc = 1) - MIN(price) FILTER (WHERE row_num_asc = 1)) / MIN(price) FILTER (WHERE row_num_asc = 1) AS price_change_percent,
                SUM(quantity) AS volume,
                SUM(price * quantity) AS quote_volume,
                COUNT(trade_id) AS trades
            FROM (
                SELECT
                    market,
                    price,
                    quantity,
                    trade_id,
                    ROW_NUMBER() OVER (PARTITION BY market ORDER BY timestamp ASC) AS row_num_asc,
                    ROW_NUMBER() OVER (PARTITION BY market ORDER BY timestamp DESC) AS row_num_desc
                FROM trades
                WHERE timestamp >= $1
            ) trade_data
            GROUP BY market
        )
        SELECT 
            symbol, 
            first_price, 
            high, 
            low, 
            last_price, 
            price_change, 
            price_change_percent, 
            quote_volume, 
            trades, 
            volume
        FROM aggregated_trades
        ORDER BY symbol ASC;
        ",
        start_timestamp_24h_ago
    )
    .fetch_all(pool)
    .await?;

    let ticker_data: Vec<TickerData> = tickers
        .iter()
        .map(|row| TickerData {
            symbol: row.symbol.clone().to_string(),
            first_price: row.first_price.clone().unwrap().to_string(),
            high: row.high.clone().unwrap().to_string(),
            low: row.low.clone().unwrap().to_string(),
            last_price: row.last_price.clone().unwrap().to_string(),
            price_change: row.price_change.clone().unwrap().to_string(),
            price_change_percent: row.price_change_percent.clone().unwrap().to_string(),
            quote_volume: row.quote_volume.clone().unwrap().to_string(),
            trades: row.trades.unwrap().to_string(),
            volume: row.volume.clone().unwrap().to_string(),
        })
        .collect();

    Ok(ticker_data)
}

pub async fn get_latest_trade_id_from_db(
    pool: &Pool<Postgres>,
    market: String,
) -> Result<i64, sqlx::Error> {
    let latest_trade = sqlx::query!(
        "SELECT trade_id FROM trades WHERE market = $1 ORDER BY trade_id desc LIMIT 1",
        market
    )
    .fetch_optional(pool)
    .await?;

    let trade_id = match latest_trade {
        Some(record) => record.trade_id,
        None => 0, // no trades found, so start from 0 (will increment to 1)
    };

    Ok(trade_id)
}
