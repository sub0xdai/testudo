//! HIST-02: Direct REST API trade history fetcher for CEX exchanges.
//!
//! Bypasses the CCXT/safe-cex sidecar — calls exchange REST APIs directly
//! with HMAC signing. Only used for read-only history import.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use sha2::{Digest, Sha256, Sha512};
use std::collections::BTreeMap;
use std::str::FromStr;

type HmacSha256 = Hmac<Sha256>;
type HmacSha512 = Hmac<Sha512>;

/// A normalized trade fill from any CEX.
#[derive(Debug, Clone)]
pub struct CexFill {
    pub id: String,
    pub symbol: String,
    pub side: String, // "buy" | "sell"
    pub price: Decimal,
    pub quantity: Decimal,
    pub fee: Decimal,
    /// Realized PnL from this fill (non-zero = closing fill).
    /// Used to bypass reconstruction and derive entry price directly.
    pub closed_pnl: Option<Decimal>,
    pub timestamp: i64, // unix ms
}

/// Errors from CEX history fetching.
#[derive(Debug, thiserror::Error)]
pub enum CexHistoryError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("API error: {0}")]
    Api(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Unsupported exchange: {0}")]
    Unsupported(String),
}

/// Fetch trade history fills from a CEX exchange.
pub async fn fetch_trade_history(
    client: &Client,
    exchange: &str,
    api_key: &str,
    api_secret: &str,
    passphrase: Option<&str>,
    start_time_ms: i64,
    end_time_ms: i64,
) -> Result<Vec<CexFill>, CexHistoryError> {
    match exchange {
        "woo" => fetch_woo_trades(client, api_key, api_secret, start_time_ms, end_time_ms).await,
        "binance" => {
            fetch_binance_trades(client, api_key, api_secret, start_time_ms, end_time_ms).await
        }
        "bybit" => {
            fetch_bybit_trades(client, api_key, api_secret, start_time_ms, end_time_ms).await
        }
        "okx" => {
            let pass = passphrase.ok_or_else(|| {
                CexHistoryError::Parse("OKX requires a passphrase".to_string())
            })?;
            fetch_okx_trades(client, api_key, api_secret, pass, start_time_ms, end_time_ms).await
        }
        "bitget" => {
            let pass = passphrase.ok_or_else(|| {
                CexHistoryError::Parse("Bitget requires a passphrase".to_string())
            })?;
            fetch_bitget_trades(client, api_key, api_secret, pass, start_time_ms, end_time_ms)
                .await
        }
        "gate" | "gateio" => {
            fetch_gate_trades(client, api_key, api_secret, start_time_ms, end_time_ms).await
        }
        "phemex" => {
            fetch_phemex_trades(client, api_key, api_secret, start_time_ms, end_time_ms).await
        }
        "blofin" => {
            let pass = passphrase.ok_or_else(|| {
                CexHistoryError::Parse("BloFin requires a passphrase".to_string())
            })?;
            fetch_blofin_trades(client, api_key, api_secret, pass, start_time_ms, end_time_ms)
                .await
        }
        _ => Err(CexHistoryError::Unsupported(exchange.to_string())),
    }
}

// ─── WOO ───

const WOO_BASE: &str = "https://api.woox.io";

#[derive(Deserialize)]
struct WooTradesResponse {
    success: bool,
    #[serde(default)]
    data: serde_json::Value,
    message: Option<String>,
}

#[derive(Deserialize)]
struct WooTrade {
    id: i64,
    symbol: String,
    side: String, // "BUY" | "SELL"
    executed_price: f64,
    executed_quantity: f64,
    fee: f64,
    executed_timestamp: String, // "1234567890.123"
}

async fn fetch_woo_trades(
    client: &Client,
    api_key: &str,
    api_secret: &str,
    start_time_ms: i64,
    end_time_ms: i64,
) -> Result<Vec<CexFill>, CexHistoryError> {
    let mut all_fills = Vec::new();
    let mut page = 1;

    loop {
        let mut params = BTreeMap::new();
        params.insert("start_t", start_time_ms.to_string());
        params.insert("end_t", end_time_ms.to_string());
        params.insert("size", "500".to_string());
        params.insert("page", page.to_string());

        let query_string = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        let timestamp = Utc::now().timestamp_millis();
        let sign_string = format!("{query_string}|{timestamp}");

        let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes())
            .map_err(|e| CexHistoryError::Parse(format!("HMAC key error: {e}")))?;
        mac.update(sign_string.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let url = format!("{WOO_BASE}/v1/client/hist_trades?{query_string}");

        let resp = client
            .get(&url)
            .header("x-api-key", api_key)
            .header("x-api-signature", &signature)
            .header("x-api-timestamp", timestamp.to_string())
            .send()
            .await
            .map_err(|e| CexHistoryError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(CexHistoryError::Api(format!("WOO HTTP {status}: {body}")));
        }

        let body_text = resp
            .text()
            .await
            .map_err(|e| CexHistoryError::Http(format!("WOO body read: {e}")))?;

        tracing::debug!(body = %body_text.chars().take(500).collect::<String>(), "WOO hist_trades response");

        let data: WooTradesResponse = serde_json::from_str(&body_text)
            .map_err(|e| CexHistoryError::Parse(format!("WOO JSON parse: {e}")))?;

        if !data.success {
            return Err(CexHistoryError::Api(
                data.message.unwrap_or_else(|| "Unknown WOO error".into()),
            ));
        }

        // WOO may return data.rows (object with rows array) or data as array
        let rows: Vec<WooTrade> = if let Some(rows_val) = data.data.get("rows") {
            serde_json::from_value(rows_val.clone()).unwrap_or_default()
        } else if data.data.is_array() {
            serde_json::from_value(data.data.clone()).unwrap_or_default()
        } else {
            tracing::warn!("WOO: unexpected data shape: {}", data.data);
            vec![]
        };
        let page_size = rows.len();

        for trade in rows {
            // Filter to perp trades only (PERP_ prefix)
            if !trade.symbol.starts_with("PERP_") {
                continue;
            }

            // Convert symbol: PERP_BTC_USDT → BTC_USDT
            let symbol = trade.symbol.strip_prefix("PERP_").unwrap_or(&trade.symbol);

            // Parse timestamp: "1234567890.123" → ms
            let ts_ms = trade
                .executed_timestamp
                .parse::<f64>()
                .map(|t| (t * 1000.0) as i64)
                .unwrap_or(0);

            all_fills.push(CexFill {
                id: trade.id.to_string(),
                symbol: symbol.to_string(),
                side: trade.side.to_lowercase(),
                price: Decimal::from_str(&trade.executed_price.to_string())
                    .unwrap_or(Decimal::ZERO),
                quantity: Decimal::from_str(&trade.executed_quantity.to_string())
                    .unwrap_or(Decimal::ZERO),
                fee: Decimal::from_str(&trade.fee.to_string()).unwrap_or(Decimal::ZERO),
                timestamp: ts_ms,
                closed_pnl: None,
            });
        }

        if page_size < 500 {
            break;
        }

        page += 1;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Ok(all_fills)
}

// ─── BINANCE FUTURES ───

const BINANCE_BASE: &str = "https://fapi.binance.com";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceTrade {
    id: i64,
    symbol: String,
    side: String, // "BUY" | "SELL"
    price: String,
    qty: String,
    commission: String,
    time: i64,
}

async fn fetch_binance_trades(
    client: &Client,
    api_key: &str,
    api_secret: &str,
    start_time_ms: i64,
    end_time_ms: i64,
) -> Result<Vec<CexFill>, CexHistoryError> {
    let mut all_fills = Vec::new();
    let mut from_id: Option<i64> = None;

    loop {
        let timestamp = Utc::now().timestamp_millis();
        let mut params = BTreeMap::new();
        params.insert("startTime", start_time_ms.to_string());
        params.insert("endTime", end_time_ms.to_string());
        params.insert("limit", "1000".to_string());
        params.insert("timestamp", timestamp.to_string());
        params.insert("recvWindow", "5000".to_string());
        if let Some(id) = from_id {
            params.insert("fromId", id.to_string());
        }

        let query_string = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        // Binance: signature = HMAC-SHA256(secret, queryString), appended as &signature=
        let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes())
            .map_err(|e| CexHistoryError::Parse(format!("HMAC key error: {e}")))?;
        mac.update(query_string.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let url = format!(
            "{BINANCE_BASE}/fapi/v1/userTrades?{query_string}&signature={signature}"
        );

        let resp = client
            .get(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await
            .map_err(|e| CexHistoryError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(CexHistoryError::Api(format!("Binance HTTP {status}: {body}")));
        }

        let trades: Vec<BinanceTrade> = resp
            .json()
            .await
            .map_err(|e| CexHistoryError::Parse(format!("Binance JSON parse: {e}")))?;

        let page_size = trades.len();

        for trade in &trades {
            // Convert symbol: BTCUSDT → BTC_USDT
            let symbol = normalize_binance_symbol(&trade.symbol);

            all_fills.push(CexFill {
                id: trade.id.to_string(),
                symbol,
                side: trade.side.to_lowercase(),
                price: Decimal::from_str(&trade.price).unwrap_or(Decimal::ZERO),
                quantity: Decimal::from_str(&trade.qty).unwrap_or(Decimal::ZERO),
                fee: Decimal::from_str(&trade.commission).unwrap_or(Decimal::ZERO),
                timestamp: trade.time,
                closed_pnl: None,
            });
        }

        if page_size < 1000 {
            break;
        }

        // Paginate by fromId (last trade id + 1)
        from_id = trades.last().map(|t| t.id + 1);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Ok(all_fills)
}

/// Convert Binance symbol format: BTCUSDT → BTC_USDT
fn normalize_binance_symbol(symbol: &str) -> String {
    // Most perp pairs end in USDT
    if let Some(base) = symbol.strip_suffix("USDT") {
        format!("{base}_USDT")
    } else if let Some(base) = symbol.strip_suffix("BUSD") {
        format!("{base}_BUSD")
    } else {
        symbol.to_string()
    }
}

// ─── BYBIT ───

const BYBIT_BASE: &str = "https://api.bybit.com";

#[derive(Deserialize)]
struct BybitResponse {
    #[serde(rename = "retCode")]
    ret_code: i32,
    #[serde(rename = "retMsg")]
    ret_msg: String,
    result: Option<BybitTradeResult>,
}

#[derive(Deserialize)]
struct BybitTradeResult {
    #[serde(default)]
    list: Vec<BybitTrade>,
    #[serde(rename = "nextPageCursor")]
    next_page_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BybitTrade {
    #[serde(rename = "execId")]
    exec_id: String,
    symbol: String,
    side: String, // "Buy" | "Sell"
    #[serde(rename = "execPrice")]
    exec_price: String,
    #[serde(rename = "execQty")]
    exec_qty: String,
    #[serde(rename = "execFee")]
    exec_fee: String,
    #[serde(rename = "execTime")]
    exec_time: String, // ms as string
    #[serde(rename = "closedPnl", default)]
    closed_pnl: String,
}

/// Bybit /v5/execution/list only allows 7-day windows.
/// Walk backwards in 7-day chunks from end_time to start_time.
const BYBIT_MAX_WINDOW_MS: i64 = 7 * 24 * 60 * 60 * 1000;

async fn fetch_bybit_trades(
    client: &Client,
    api_key: &str,
    api_secret: &str,
    start_time_ms: i64,
    end_time_ms: i64,
) -> Result<Vec<CexFill>, CexHistoryError> {
    let mut all_fills = Vec::new();
    let mut window_start = start_time_ms;

    while window_start < end_time_ms {
        let window_end = (window_start + BYBIT_MAX_WINDOW_MS).min(end_time_ms);

        tracing::info!(
            window_start,
            window_end,
            "Bybit: fetching 7-day window"
        );

        let mut fills =
            fetch_bybit_window(client, api_key, api_secret, window_start, window_end).await?;

        all_fills.append(&mut fills);
        window_start = window_end;

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Ok(all_fills)
}

async fn fetch_bybit_window(
    client: &Client,
    api_key: &str,
    api_secret: &str,
    start_time_ms: i64,
    end_time_ms: i64,
) -> Result<Vec<CexFill>, CexHistoryError> {
    let mut all_fills = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let timestamp = Utc::now().timestamp_millis();
        let recv_window = 5000;

        let mut params = BTreeMap::new();
        params.insert("category", "linear".to_string());
        params.insert("startTime", start_time_ms.to_string());
        params.insert("endTime", end_time_ms.to_string());
        params.insert("limit", "100".to_string());
        if let Some(ref c) = cursor {
            params.insert("cursor", c.clone());
        }

        let query_string = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        let sign_payload = format!("{timestamp}{api_key}{recv_window}{query_string}");
        let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes())
            .map_err(|e| CexHistoryError::Parse(format!("HMAC key error: {e}")))?;
        mac.update(sign_payload.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let url = format!("{BYBIT_BASE}/v5/execution/list?{query_string}");

        let resp = client
            .get(&url)
            .header("X-BAPI-API-KEY", api_key)
            .header("X-BAPI-SIGN", &signature)
            .header("X-BAPI-TIMESTAMP", timestamp.to_string())
            .header("X-BAPI-RECV-WINDOW", recv_window.to_string())
            .send()
            .await
            .map_err(|e| CexHistoryError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(CexHistoryError::Api(format!("Bybit HTTP {status}: {body}")));
        }

        let body_text = resp
            .text()
            .await
            .map_err(|e| CexHistoryError::Http(format!("Bybit body read: {e}")))?;

        let data: BybitResponse = serde_json::from_str(&body_text)
            .map_err(|e| CexHistoryError::Parse(format!("Bybit JSON parse: {e}")))?;

        if data.ret_code != 0 {
            return Err(CexHistoryError::Api(format!(
                "Bybit error {}: {}",
                data.ret_code, data.ret_msg
            )));
        }

        let result = data.result.unwrap_or(BybitTradeResult {
            list: vec![],
            next_page_cursor: None,
        });

        for trade in &result.list {
            let symbol = normalize_binance_symbol(&trade.symbol);
            let ts_ms = trade.exec_time.parse::<i64>().unwrap_or(0);
            let pnl = Decimal::from_str(&trade.closed_pnl).ok()
                .filter(|d| *d != Decimal::ZERO);

            all_fills.push(CexFill {
                id: trade.exec_id.clone(),
                symbol,
                side: trade.side.to_lowercase(),
                price: Decimal::from_str(&trade.exec_price).unwrap_or(Decimal::ZERO),
                quantity: Decimal::from_str(&trade.exec_qty).unwrap_or(Decimal::ZERO),
                fee: Decimal::from_str(&trade.exec_fee).unwrap_or(Decimal::ZERO).abs(),
                timestamp: ts_ms,
                closed_pnl: pnl,
            });
        }

        match result.next_page_cursor {
            Some(ref c) if !c.is_empty() => cursor = Some(c.clone()),
            _ => break,
        }

        if result.list.is_empty() {
            break;
        }

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Ok(all_fills)
}

// ─── OKX ───

const OKX_BASE: &str = "https://www.okx.com";

#[derive(Deserialize)]
struct OkxResponse {
    code: String,
    #[serde(default)]
    data: Vec<OkxTrade>,
    msg: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OkxTrade {
    trade_id: String,
    inst_id: String,
    side: String, // "buy" | "sell"
    fill_px: String,
    fill_sz: String,
    fee: String,
    ts: String,      // unix ms as string
    bill_id: String, // cursor for pagination
}

/// Normalize OKX symbol: `BTC-USDT-SWAP` → `BTC_USDT`
fn normalize_okx_symbol(inst_id: &str) -> String {
    let base = inst_id.strip_suffix("-SWAP").unwrap_or(inst_id);
    base.replace('-', "_")
}

async fn fetch_okx_trades(
    client: &Client,
    api_key: &str,
    api_secret: &str,
    passphrase: &str,
    start_time_ms: i64,
    end_time_ms: i64,
) -> Result<Vec<CexFill>, CexHistoryError> {
    let mut all_fills = Vec::new();
    let mut after_cursor: Option<String> = None;

    loop {
        let mut params = BTreeMap::new();
        params.insert("instType", "SWAP".to_string());
        params.insert("begin", start_time_ms.to_string());
        params.insert("end", end_time_ms.to_string());
        params.insert("limit", "100".to_string());
        if let Some(ref cursor) = after_cursor {
            params.insert("after", cursor.clone());
        }

        let query_string = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        let request_path = format!("/api/v5/trade/fills-history?{query_string}");

        // OKX timestamp: ISO 8601 UTC
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let prehash = format!("{timestamp}GET{request_path}");

        let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes())
            .map_err(|e| CexHistoryError::Parse(format!("HMAC key error: {e}")))?;
        mac.update(prehash.as_bytes());
        let signature = BASE64.encode(mac.finalize().into_bytes());

        let url = format!("{OKX_BASE}{request_path}");

        tracing::info!(
            cursor = ?after_cursor,
            fills_so_far = all_fills.len(),
            "OKX: fetching fills page"
        );

        let resp = client
            .get(&url)
            .header("OK-ACCESS-KEY", api_key)
            .header("OK-ACCESS-SIGN", &signature)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", passphrase)
            .send()
            .await
            .map_err(|e| CexHistoryError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(CexHistoryError::Api(format!("OKX HTTP {status}: {body}")));
        }

        let body_text = resp
            .text()
            .await
            .map_err(|e| CexHistoryError::Http(format!("OKX body read: {e}")))?;

        let data: OkxResponse = serde_json::from_str(&body_text)
            .map_err(|e| CexHistoryError::Parse(format!("OKX JSON parse: {e}")))?;

        if data.code != "0" {
            return Err(CexHistoryError::Api(format!(
                "OKX error {}: {}",
                data.code,
                data.msg.unwrap_or_default()
            )));
        }

        let page_size = data.data.len();

        // Capture last billId for cursor before consuming data
        let last_bill_id = data.data.last().map(|t| t.bill_id.clone());

        for trade in &data.data {
            let symbol = normalize_okx_symbol(&trade.inst_id);
            let ts_ms = trade.ts.parse::<i64>().unwrap_or(0);

            all_fills.push(CexFill {
                id: trade.trade_id.clone(),
                symbol,
                side: trade.side.to_lowercase(),
                price: Decimal::from_str(&trade.fill_px).unwrap_or(Decimal::ZERO),
                quantity: Decimal::from_str(&trade.fill_sz).unwrap_or(Decimal::ZERO),
                fee: Decimal::from_str(&trade.fee).unwrap_or(Decimal::ZERO).abs(),
                timestamp: ts_ms,
            closed_pnl: None,
            });
        }

        if page_size < 100 {
            break;
        }

        after_cursor = last_bill_id;
        if after_cursor.is_none() {
            break;
        }

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Ok(all_fills)
}

// ─── BITGET ───

const BITGET_BASE: &str = "https://api.bitget.com";

/// Bitget uses 7-day window chunks like Bybit.
const BITGET_MAX_WINDOW_MS: i64 = 7 * 24 * 60 * 60 * 1000;

#[derive(Deserialize)]
struct BitgetResponse {
    code: String,
    #[serde(default)]
    data: Option<BitgetTradeData>,
    msg: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BitgetTradeData {
    #[serde(default)]
    fill_list: Vec<BitgetTrade>,
    end_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BitgetTrade {
    trade_id: String,
    symbol: String,
    side: String, // "buy" | "sell"
    price: String,
    base_volume: String,
    #[serde(default)]
    fee_detail: Vec<BitgetFeeDetail>,
    #[serde(rename = "cTime")]
    c_time: String, // creation time ms as string
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BitgetFeeDetail {
    total_fee: String,
}

async fn fetch_bitget_trades(
    client: &Client,
    api_key: &str,
    api_secret: &str,
    passphrase: &str,
    start_time_ms: i64,
    end_time_ms: i64,
) -> Result<Vec<CexFill>, CexHistoryError> {
    let mut all_fills = Vec::new();
    let mut window_start = start_time_ms;

    while window_start < end_time_ms {
        let window_end = (window_start + BITGET_MAX_WINDOW_MS).min(end_time_ms);

        tracing::info!(
            window_start,
            window_end,
            "Bitget: fetching 7-day window"
        );

        let mut fills = fetch_bitget_window(
            client,
            api_key,
            api_secret,
            passphrase,
            window_start,
            window_end,
        )
        .await?;

        all_fills.append(&mut fills);
        window_start = window_end;

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Ok(all_fills)
}

async fn fetch_bitget_window(
    client: &Client,
    api_key: &str,
    api_secret: &str,
    passphrase: &str,
    start_time_ms: i64,
    end_time_ms: i64,
) -> Result<Vec<CexFill>, CexHistoryError> {
    let mut all_fills = Vec::new();
    let mut id_less_than: Option<String> = None;

    loop {
        let mut params = BTreeMap::new();
        params.insert("productType", "USDT-FUTURES".to_string());
        params.insert("startTime", start_time_ms.to_string());
        params.insert("endTime", end_time_ms.to_string());
        params.insert("limit", "100".to_string());
        if let Some(ref cursor) = id_less_than {
            params.insert("idLessThan", cursor.clone());
        }

        let query_string = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        let request_path = "/api/v2/mix/order/fill-history";

        // Bitget prehash: timestamp + "GET" + requestPath + "?" + queryString
        let timestamp = Utc::now().timestamp_millis().to_string();
        let prehash = format!("{timestamp}GET{request_path}?{query_string}");

        let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes())
            .map_err(|e| CexHistoryError::Parse(format!("HMAC key error: {e}")))?;
        mac.update(prehash.as_bytes());
        let signature = BASE64.encode(mac.finalize().into_bytes());

        let url = format!("{BITGET_BASE}{request_path}?{query_string}");

        tracing::info!(
            cursor = ?id_less_than,
            fills_so_far = all_fills.len(),
            "Bitget: fetching fills page"
        );

        let resp = client
            .get(&url)
            .header("ACCESS-KEY", api_key)
            .header("ACCESS-SIGN", &signature)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", passphrase)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| CexHistoryError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(CexHistoryError::Api(format!(
                "Bitget HTTP {status}: {body}"
            )));
        }

        let body_text = resp
            .text()
            .await
            .map_err(|e| CexHistoryError::Http(format!("Bitget body read: {e}")))?;

        let data: BitgetResponse = serde_json::from_str(&body_text)
            .map_err(|e| CexHistoryError::Parse(format!("Bitget JSON parse: {e}")))?;

        if data.code != "00000" {
            return Err(CexHistoryError::Api(format!(
                "Bitget error {}: {}",
                data.code,
                data.msg.unwrap_or_default()
            )));
        }

        let trade_data = data.data.unwrap_or(BitgetTradeData {
            fill_list: vec![],
            end_id: None,
        });

        let page_size = trade_data.fill_list.len();

        for trade in &trade_data.fill_list {
            let symbol = normalize_binance_symbol(&trade.symbol);
            let ts_ms = trade.c_time.parse::<i64>().unwrap_or(0);

            // Sum fees from feeDetail array
            let fee: Decimal = trade
                .fee_detail
                .iter()
                .map(|fd| Decimal::from_str(&fd.total_fee).unwrap_or(Decimal::ZERO))
                .sum();

            all_fills.push(CexFill {
                id: trade.trade_id.clone(),
                symbol,
                side: trade.side.to_lowercase(),
                price: Decimal::from_str(&trade.price).unwrap_or(Decimal::ZERO),
                quantity: Decimal::from_str(&trade.base_volume).unwrap_or(Decimal::ZERO),
                fee: fee.abs(),
                timestamp: ts_ms,
            closed_pnl: None,
            });
        }

        if page_size < 100 {
            break;
        }

        id_less_than = trade_data.end_id;
        if id_less_than.is_none() {
            break;
        }

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Ok(all_fills)
}

// ─── GATE.IO ───

const GATE_BASE: &str = "https://api.gateio.ws";

#[derive(Deserialize)]
struct GateTrade {
    id: i64,
    #[serde(default)]
    create_time_ms: Option<f64>,
    #[serde(default)]
    create_time: Option<f64>,
    contract: String,
    size: i64, // negative = short; absolute value = quantity
    price: String,
    fee: f64,
}

async fn fetch_gate_trades(
    client: &Client,
    api_key: &str,
    api_secret: &str,
    start_time_ms: i64,
    end_time_ms: i64,
) -> Result<Vec<CexFill>, CexHistoryError> {
    let mut all_fills = Vec::new();
    let mut offset: u32 = 0;
    let limit: u32 = 1000;

    loop {
        let mut params = BTreeMap::new();
        params.insert("limit", limit.to_string());
        params.insert("offset", offset.to_string());

        let query_string = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        let request_path = "/api/v4/futures/usdt/my_trades";

        // Gate.io auth: HMAC-SHA512
        // Prehash: "GET\n" + requestPath + "\n" + queryString + "\n" + hex(SHA512("")) + "\n" + timestamp
        let timestamp = Utc::now().timestamp().to_string();
        let empty_body_hash = hex::encode(Sha512::digest(b""));
        let prehash = format!("GET\n{request_path}\n{query_string}\n{empty_body_hash}\n{timestamp}");

        let mut mac = HmacSha512::new_from_slice(api_secret.as_bytes())
            .map_err(|e| CexHistoryError::Parse(format!("HMAC key error: {e}")))?;
        mac.update(prehash.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let url = format!("{GATE_BASE}{request_path}?{query_string}");

        tracing::info!(
            offset,
            fills_so_far = all_fills.len(),
            "Gate.io: fetching fills page"
        );

        let resp = client
            .get(&url)
            .header("KEY", api_key)
            .header("SIGN", &signature)
            .header("Timestamp", &timestamp)
            .send()
            .await
            .map_err(|e| CexHistoryError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(CexHistoryError::Api(format!(
                "Gate.io HTTP {status}: {body}"
            )));
        }

        let body_text = resp
            .text()
            .await
            .map_err(|e| CexHistoryError::Http(format!("Gate.io body read: {e}")))?;

        let trades: Vec<GateTrade> = serde_json::from_str(&body_text)
            .map_err(|e| CexHistoryError::Parse(format!("Gate.io JSON parse: {e}")))?;

        let page_size = trades.len();

        for trade in &trades {
            // Determine timestamp in ms
            let ts_ms = if let Some(ms) = trade.create_time_ms {
                ms as i64
            } else if let Some(s) = trade.create_time {
                (s * 1000.0) as i64
            } else {
                0
            };

            // Filter by time range
            if ts_ms < start_time_ms || ts_ms > end_time_ms {
                continue;
            }

            // contract field is already in BTC_USDT format
            let symbol = trade.contract.clone();

            // size: negative = short side, positive = long side
            let side = if trade.size >= 0 { "buy" } else { "sell" };
            let quantity =
                Decimal::from_str(&trade.size.unsigned_abs().to_string()).unwrap_or(Decimal::ZERO);

            let fee = Decimal::from_str(&trade.fee.to_string())
                .unwrap_or(Decimal::ZERO)
                .abs();

            all_fills.push(CexFill {
                id: trade.id.to_string(),
                symbol,
                side: side.to_string(),
                price: Decimal::from_str(&trade.price).unwrap_or(Decimal::ZERO),
                quantity,
                fee,
                timestamp: ts_ms,
            closed_pnl: None,
            });
        }

        if page_size < limit as usize {
            break;
        }

        offset += limit;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Ok(all_fills)
}

// ─── PHEMEX ───

const PHEMEX_BASE: &str = "https://api.phemex.com";

#[derive(Deserialize)]
struct PhemexResponse {
    code: i32,
    #[serde(default)]
    data: Option<PhemexTradeData>,
    msg: Option<String>,
}

#[derive(Deserialize)]
struct PhemexTradeData {
    #[serde(default)]
    rows: Vec<PhemexTrade>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhemexTrade {
    #[serde(rename = "execID")]
    exec_id: String,
    symbol: String,
    side: String, // "Buy" | "Sell"
    #[serde(rename = "execPriceRp")]
    exec_price_rp: String,
    #[serde(rename = "execQtyRq")]
    exec_qty_rq: String,
    #[serde(rename = "execFeeRv")]
    exec_fee_rv: String,
    #[serde(rename = "transactTimeNs")]
    transact_time_ns: i64, // nanoseconds
}

async fn fetch_phemex_trades(
    client: &Client,
    api_key: &str,
    api_secret: &str,
    start_time_ms: i64,
    end_time_ms: i64,
) -> Result<Vec<CexFill>, CexHistoryError> {
    let mut all_fills = Vec::new();
    let mut offset: u32 = 0;
    let limit: u32 = 200;

    loop {
        let request_path = "/exchange/order/v2/tradingList";

        let mut params = BTreeMap::new();
        params.insert("currency", "USDT".to_string());
        params.insert("offset", offset.to_string());
        params.insert("limit", limit.to_string());

        let query_string = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        // Phemex auth: expiry = now + 60s
        let expiry = (Utc::now().timestamp() + 60).to_string();

        // Prehash: requestPath + queryString + expiry + body (empty for GET)
        let prehash = format!("{request_path}{query_string}{expiry}");

        let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes())
            .map_err(|e| CexHistoryError::Parse(format!("HMAC key error: {e}")))?;
        mac.update(prehash.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let url = format!("{PHEMEX_BASE}{request_path}?{query_string}");

        tracing::info!(
            offset,
            fills_so_far = all_fills.len(),
            "Phemex: fetching fills page"
        );

        let resp = client
            .get(&url)
            .header("x-phemex-access-token", api_key)
            .header("x-phemex-request-signature", &signature)
            .header("x-phemex-request-expiry", &expiry)
            .send()
            .await
            .map_err(|e| CexHistoryError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(CexHistoryError::Api(format!(
                "Phemex HTTP {status}: {body}"
            )));
        }

        let body_text = resp
            .text()
            .await
            .map_err(|e| CexHistoryError::Http(format!("Phemex body read: {e}")))?;

        let data: PhemexResponse = serde_json::from_str(&body_text)
            .map_err(|e| CexHistoryError::Parse(format!("Phemex JSON parse: {e}")))?;

        if data.code != 0 {
            return Err(CexHistoryError::Api(format!(
                "Phemex error {}: {}",
                data.code,
                data.msg.unwrap_or_default()
            )));
        }

        let trade_data = data.data.unwrap_or(PhemexTradeData { rows: vec![] });
        let page_size = trade_data.rows.len();

        for trade in &trade_data.rows {
            // Convert nanosecond timestamp to milliseconds
            let ts_ms = trade.transact_time_ns / 1_000_000;

            // Filter by time range
            if ts_ms < start_time_ms || ts_ms > end_time_ms {
                continue;
            }

            let symbol = normalize_binance_symbol(&trade.symbol);

            all_fills.push(CexFill {
                id: trade.exec_id.clone(),
                symbol,
                side: trade.side.to_lowercase(),
                price: Decimal::from_str(&trade.exec_price_rp).unwrap_or(Decimal::ZERO),
                quantity: Decimal::from_str(&trade.exec_qty_rq).unwrap_or(Decimal::ZERO),
                fee: Decimal::from_str(&trade.exec_fee_rv)
                    .unwrap_or(Decimal::ZERO)
                    .abs(),
                timestamp: ts_ms,
            closed_pnl: None,
            });
        }

        if page_size < limit as usize {
            break;
        }

        offset += limit;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Ok(all_fills)
}

// ─── BLOFIN ───

const BLOFIN_BASE: &str = "https://openapi.blofin.com";

#[derive(Deserialize)]
struct BlofinResponse {
    code: String,
    #[serde(default)]
    data: Vec<BlofinTrade>,
    msg: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlofinTrade {
    bill_id: String,
    inst_id: String,
    side: String, // "buy" | "sell"
    fill_size: String,
    fill_price: String,
    commission: String,
    ts: String, // unix ms as string
}

/// Normalize BloFin symbol: `BTC-USDT` → `BTC_USDT`
fn normalize_blofin_symbol(inst_id: &str) -> String {
    inst_id.replace('-', "_")
}

async fn fetch_blofin_trades(
    client: &Client,
    api_key: &str,
    api_secret: &str,
    passphrase: &str,
    start_time_ms: i64,
    end_time_ms: i64,
) -> Result<Vec<CexFill>, CexHistoryError> {
    let mut all_fills = Vec::new();
    let mut after_cursor: Option<String> = None;

    loop {
        let mut params = BTreeMap::new();
        params.insert("begin", start_time_ms.to_string());
        params.insert("end", end_time_ms.to_string());
        params.insert("limit", "100".to_string());
        if let Some(ref cursor) = after_cursor {
            params.insert("after", cursor.clone());
        }

        let query_string = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        let request_path = format!("/api/v1/trade/fills-history?{query_string}");

        // BloFin auth: timestamp (ms), nonce (UUID)
        let timestamp = Utc::now().timestamp_millis().to_string();
        let nonce = uuid::Uuid::new_v4().to_string();

        // Prehash: requestPath + "GET" + timestamp + nonce + "" (empty body)
        let prehash = format!("{request_path}GET{timestamp}{nonce}");

        let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes())
            .map_err(|e| CexHistoryError::Parse(format!("HMAC key error: {e}")))?;
        mac.update(prehash.as_bytes());
        // BloFin: Base64(hex(HMAC-SHA256(prehash, secret)))
        let hex_sig = hex::encode(mac.finalize().into_bytes());
        let signature = BASE64.encode(hex_sig.as_bytes());

        let url = format!("{BLOFIN_BASE}{request_path}");

        tracing::info!(
            cursor = ?after_cursor,
            fills_so_far = all_fills.len(),
            "BloFin: fetching fills page"
        );

        let resp = client
            .get(&url)
            .header("ACCESS-KEY", api_key)
            .header("ACCESS-SIGN", &signature)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-NONCE", &nonce)
            .header("ACCESS-PASSPHRASE", passphrase)
            .send()
            .await
            .map_err(|e| CexHistoryError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(CexHistoryError::Api(format!(
                "BloFin HTTP {status}: {body}"
            )));
        }

        let body_text = resp
            .text()
            .await
            .map_err(|e| CexHistoryError::Http(format!("BloFin body read: {e}")))?;

        let data: BlofinResponse = serde_json::from_str(&body_text)
            .map_err(|e| CexHistoryError::Parse(format!("BloFin JSON parse: {e}")))?;

        if data.code != "0" {
            return Err(CexHistoryError::Api(format!(
                "BloFin error {}: {}",
                data.code,
                data.msg.unwrap_or_default()
            )));
        }

        let page_size = data.data.len();

        // Capture last billId for cursor before consuming
        let last_bill_id = data.data.last().map(|t| t.bill_id.clone());

        for trade in &data.data {
            let symbol = normalize_blofin_symbol(&trade.inst_id);
            let ts_ms = trade.ts.parse::<i64>().unwrap_or(0);

            all_fills.push(CexFill {
                id: trade.bill_id.clone(),
                symbol,
                side: trade.side.to_lowercase(),
                price: Decimal::from_str(&trade.fill_price).unwrap_or(Decimal::ZERO),
                quantity: Decimal::from_str(&trade.fill_size).unwrap_or(Decimal::ZERO),
                fee: Decimal::from_str(&trade.commission)
                    .unwrap_or(Decimal::ZERO)
                    .abs(),
                timestamp: ts_ms,
            closed_pnl: None,
            });
        }

        if page_size < 100 {
            break;
        }

        after_cursor = last_bill_id;
        if after_cursor.is_none() {
            break;
        }

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Ok(all_fills)
}
