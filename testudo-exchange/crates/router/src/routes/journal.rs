// @anchor exchange:router:journal
// @tags api

use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse, Result};
use chrono::{DateTime, NaiveDate, Utc};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    middleware::{content_negotiation::{wants_jsonld, wrap_jsonld, wrap_jsonld_collection, jsonld_response}, AuthenticatedUser},
    models::journal::{JournalEntry, JournalTag, JournalTrade},
    services::journal_stats::{StatsEngine, StatsFilter},
    services::journal_timeseries::TimeSeriesService,
    types::{app::AppState, auth::ErrorResponse},
};

// ---------------------------------------------------------------------------
// Query / request structs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListTradesQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub exchange: Option<String>,
    pub symbol: Option<String>,
    pub side: Option<String>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub tag: Option<String>,
    pub sort: Option<String>,
    pub order: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListEntriesQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub trade_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNotesRequest {
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateEntryRequest {
    pub trade_id: Option<Uuid>,
    pub entry_date: Option<NaiveDate>,
    pub title: String,
    pub body: String,
    pub entry_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEntryRequest {
    pub title: String,
    pub body: String,
    pub entry_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTagRequest {
    pub name: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddTradeTagsRequest {
    pub tag_ids: Vec<Uuid>,
}

// ---------------------------------------------------------------------------
// Response structs
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct TradeWithTags {
    #[serde(flatten)]
    pub trade: JournalTrade,
    pub tags: Vec<JournalTag>,
}

/// FR-8: Wire response for a single trade. Reconciling rows emit null for economic fields
/// so consumers cannot display phantom P&L values.
#[derive(Debug, Serialize)]
pub struct TradeApiResponse {
    id: Uuid,
    user_id: Uuid,
    exchange: String,
    symbol: String,
    side: String,
    entry_price: rust_decimal::Decimal,
    exit_price: rust_decimal::Decimal,
    quantity: rust_decimal::Decimal,
    leverage: i32,
    realized_pnl: rust_decimal::Decimal,
    realized_pnl_pct: rust_decimal::Decimal,
    fees: rust_decimal::Decimal,
    net_pnl: Option<rust_decimal::Decimal>,       // null when reconciling
    stop_price: Option<rust_decimal::Decimal>,
    target_price: Option<rust_decimal::Decimal>,
    risk_amount: Option<rust_decimal::Decimal>,
    r_multiple: Option<rust_decimal::Decimal>,    // null when reconciling
    opened_at: DateTime<Utc>,
    closed_at: DateTime<Utc>,
    duration_secs: i32,
    trade_group_id: Option<Uuid>,
    notes: Option<String>,
    source: String,
    exchange_fill_id: Option<i64>,
    setup_tag: Option<String>,
    kelly_inputs: Option<serde_json::Value>,
    needs_reconciliation: bool,
    close_reason: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    /// "final" for JNL-SYNC-01 pull-sync rows; "reconciling" for legacy import rows (backwards-compat).
    status: &'static str,
    tags: Vec<JournalTag>,
}

impl TradeApiResponse {
    fn from_trade_with_tags(twt: TradeWithTags) -> Self {
        let reconciling = twt.trade.needs_reconciliation;
        Self {
            id: twt.trade.id,
            user_id: twt.trade.user_id,
            exchange: twt.trade.exchange,
            symbol: twt.trade.symbol,
            side: twt.trade.side,
            entry_price: twt.trade.entry_price,
            exit_price: twt.trade.exit_price,
            quantity: twt.trade.quantity,
            leverage: twt.trade.leverage,
            realized_pnl: twt.trade.realized_pnl,
            realized_pnl_pct: twt.trade.realized_pnl_pct,
            fees: twt.trade.fees,
            net_pnl: if reconciling { None } else { Some(twt.trade.net_pnl) },
            stop_price: twt.trade.stop_price,
            target_price: twt.trade.target_price,
            risk_amount: twt.trade.risk_amount,
            r_multiple: if reconciling { None } else { twt.trade.r_multiple },
            opened_at: twt.trade.opened_at,
            closed_at: twt.trade.closed_at,
            duration_secs: twt.trade.duration_secs,
            trade_group_id: twt.trade.trade_group_id,
            notes: twt.trade.notes,
            source: twt.trade.source,
            exchange_fill_id: twt.trade.exchange_fill_id,
            setup_tag: twt.trade.setup_tag,
            kelly_inputs: twt.trade.kelly_inputs,
            needs_reconciliation: twt.trade.needs_reconciliation,
            close_reason: twt.trade.close_reason,
            created_at: twt.trade.created_at,
            updated_at: twt.trade.updated_at,
            status: if reconciling { "reconciling" } else { "final" },
            tags: twt.tags,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TradeDetailApiResponse {
    #[serde(flatten)]
    inner: TradeApiResponse,
    entries: Vec<JournalEntry>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedTrades {
    pub trades: Vec<TradeApiResponse>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize)]
pub struct TradeDetail {
    #[serde(flatten)]
    pub trade: JournalTrade,
    pub entries: Vec<JournalEntry>,
    pub tags: Vec<JournalTag>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedEntries {
    pub entries: Vec<JournalEntry>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}

// ---------------------------------------------------------------------------
// Filter options — data-driven dropdowns (UXP-09)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct FilterOptionsQuery {
    pub exchange: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FilterOptionsResponse {
    pub exchanges: Vec<String>,
    pub symbols: Vec<SymbolCount>,
}

#[derive(Debug, Serialize)]
pub struct SymbolCount {
    pub symbol: String,
    pub count: i64,
}

// ── JNL-SYNC-01 FR-7: Manual "Sync now" ──────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct ManualSyncBody {
    pub exchange_account_id: Option<uuid::Uuid>,
}

/// POST /api/v1/journal/sync — trigger an out-of-band sync for the caller's active account.
/// Server-side debounce: 5s minimum between triggers per account.
pub async fn trigger_manual_sync(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    body: web::Json<ManualSyncBody>,
) -> Result<HttpResponse> {
    // Resolve account: use body value or fall back to most-recently-used active account.
    let account_id = if let Some(id) = body.exchange_account_id {
        id
    } else {
        let row: Option<(uuid::Uuid,)> = sqlx::query_as(
            "SELECT id FROM exchange_accounts \
             WHERE user_id = $1 AND is_active = TRUE AND exchange_name != 'hyperliquid' \
             ORDER BY last_used_at DESC NULLS LAST LIMIT 1",
        )
        .bind(user.user_id)
        .fetch_optional(&app_state.pool)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

        match account_id_from_row(row) {
            Some(id) => id,
            None => {
                return Ok(HttpResponse::UnprocessableEntity()
                    .json(serde_json::json!({"error": "no active exchange account found"})));
            }
        }
    };

    // Verify account belongs to caller.
    let owned: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT id FROM exchange_accounts WHERE id = $1 AND user_id = $2",
    )
    .bind(account_id)
    .bind(user.user_id)
    .fetch_optional(&app_state.pool)
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;

    if owned.is_none() {
        return Ok(HttpResponse::NotFound()
            .json(serde_json::json!({"error": "exchange account not found"})));
    }

    // Server-side debounce: reject if last notify was <5s ago.
    let debounce = std::time::Duration::from_secs(5);
    if let Some(last) = app_state.journal_syncer_last_notified.get(&account_id) {
        if last.elapsed() < debounce {
            return Ok(HttpResponse::Conflict()
                .json(serde_json::json!({"error": "sync already running, retry in 5s"})));
        }
    }

    // Look up notifier.
    match app_state.journal_syncer_notifiers.get(&account_id) {
        Some(notify) => {
            app_state
                .journal_syncer_last_notified
                .insert(account_id, std::time::Instant::now());
            notify.notify_one();
            Ok(HttpResponse::Accepted().finish())
        }
        None => Ok(HttpResponse::ServiceUnavailable()
            .json(serde_json::json!({"error": "journal syncer not running for this account"}))),
    }
}

fn account_id_from_row(row: Option<(uuid::Uuid,)>) -> Option<uuid::Uuid> {
    row.map(|(id,)| id)
}

pub async fn filter_options(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    query: web::Query<FilterOptionsQuery>,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;

    let exchanges = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT exchange FROM journal_trades WHERE user_id = $1 ORDER BY exchange",
    )
    .bind(user.user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch filter exchanges: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    let symbols = if let Some(ref exchange) = query.exchange {
        sqlx::query_as::<_, (String, i64)>(
            r#"SELECT symbol, COUNT(*) as "count" FROM journal_trades
               WHERE user_id = $1 AND exchange = $2
               GROUP BY symbol ORDER BY count DESC"#,
        )
        .bind(user.user_id)
        .bind(exchange)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, (String, i64)>(
            r#"SELECT symbol, COUNT(*) as "count" FROM journal_trades
               WHERE user_id = $1
               GROUP BY symbol ORDER BY count DESC"#,
        )
        .bind(user.user_id)
        .fetch_all(pool)
        .await
    }
    .map_err(|e| {
        tracing::error!("Failed to fetch filter symbols: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    let symbols = symbols
        .into_iter()
        .map(|(symbol, count)| SymbolCount { symbol, count })
        .collect();

    Ok(HttpResponse::Ok().json(FilterOptionsResponse { exchanges, symbols }))
}

// ---------------------------------------------------------------------------
// Trades — read + annotate
// ---------------------------------------------------------------------------

const VALID_SORT_FIELDS: &[&str] = &["closed_at", "net_pnl", "r_multiple", "duration_secs"];

pub async fn list_trades(
    req: HttpRequest,
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    query: web::Query<ListTradesQuery>,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) * limit;

    let sort_field = query
        .sort
        .as_deref()
        .filter(|s| VALID_SORT_FIELDS.contains(s))
        .unwrap_or("closed_at");
    let sort_order = match query.order.as_deref() {
        Some("asc") => "ASC",
        _ => "DESC",
    };

    // Build dynamic WHERE clause
    let mut conditions = vec!["jt.user_id = $1".to_string()];
    let mut bind_idx = 2u32;

    // We collect string bindings in order
    let mut str_binds: Vec<String> = Vec::new();

    if let Some(ref exchange) = query.exchange {
        conditions.push(format!("jt.exchange = ${bind_idx}"));
        str_binds.push(exchange.clone());
        bind_idx += 1;
    }
    if let Some(ref symbol) = query.symbol {
        conditions.push(format!("jt.symbol = ${bind_idx}"));
        str_binds.push(symbol.clone());
        bind_idx += 1;
    }
    if let Some(ref side) = query.side {
        conditions.push(format!("jt.side = ${bind_idx}"));
        str_binds.push(side.clone());
        bind_idx += 1;
    }
    if let Some(ref date_from) = query.date_from {
        conditions.push(format!(
            "jt.closed_at >= ${bind_idx}::date",
        ));
        str_binds.push(date_from.to_string());
        bind_idx += 1;
    }
    if let Some(ref date_to) = query.date_to {
        conditions.push(format!(
            "jt.closed_at < (${bind_idx}::date + interval '1 day')",
        ));
        str_binds.push(date_to.to_string());
        bind_idx += 1;
    }
    if let Some(ref tag) = query.tag {
        conditions.push(format!(
            "EXISTS (SELECT 1 FROM journal_trade_tags jtt JOIN journal_tags jtag ON jtag.id = jtt.tag_id WHERE jtt.trade_id = jt.id AND jtag.name = ${bind_idx})"
        ));
        str_binds.push(tag.clone());
        bind_idx += 1;
    }

    let where_clause = conditions.join(" AND ");

    // Count query
    let count_sql = format!("SELECT COUNT(*) as count FROM journal_trades jt WHERE {where_clause}");
    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql).bind(user.user_id);
    for s in &str_binds {
        count_query = count_query.bind(s.as_str());
    }

    let total = count_query.fetch_one(pool).await.map_err(|e| {
        tracing::error!("Failed to count journal trades: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    // Data query — sort field is validated against allowlist, safe for interpolation
    let data_sql = format!(
        "SELECT jt.* FROM journal_trades jt WHERE {where_clause} ORDER BY jt.{sort_field} {sort_order} LIMIT ${bind_idx} OFFSET ${}",
        bind_idx + 1
    );
    let mut data_query = sqlx::query_as::<_, JournalTrade>(&data_sql).bind(user.user_id);
    for s in &str_binds {
        data_query = data_query.bind(s.as_str());
    }
    data_query = data_query.bind(limit).bind(offset);

    let trades = data_query.fetch_all(pool).await.map_err(|e| {
        tracing::error!("Failed to list journal trades: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    // Batch-load tags for all trades in the result set
    let trade_ids: Vec<Uuid> = trades.iter().map(|t| t.id).collect();
    let trades_with_tags = if trade_ids.is_empty() {
        vec![]
    } else {
        #[derive(sqlx::FromRow)]
        struct TagRow {
            trade_id: Uuid,
            id: Uuid,
            user_id: Uuid,
            name: String,
            color: Option<String>,
        }

        let tag_rows = sqlx::query_as::<_, TagRow>(
            "SELECT jtt.trade_id, t.id, t.user_id, t.name, t.color \
             FROM journal_trade_tags jtt \
             JOIN journal_tags t ON t.id = jtt.tag_id \
             WHERE jtt.trade_id = ANY($1) AND t.user_id = $2",
        )
        .bind(&trade_ids)
        .bind(user.user_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        // Group tags by trade_id
        let mut tags_by_trade: std::collections::HashMap<Uuid, Vec<JournalTag>> =
            std::collections::HashMap::new();
        for row in tag_rows {
            tags_by_trade
                .entry(row.trade_id)
                .or_default()
                .push(JournalTag {
                    id: row.id,
                    user_id: row.user_id,
                    name: row.name,
                    color: row.color,
                });
        }

        trades
            .into_iter()
            .map(|trade| {
                let tags = tags_by_trade.remove(&trade.id).unwrap_or_default();
                TradeWithTags { trade, tags }
            })
            .collect()
    };

    let response_trades: Vec<TradeApiResponse> = trades_with_tags
        .into_iter()
        .map(TradeApiResponse::from_trade_with_tags)
        .collect();

    if wants_jsonld(&req) {
        return Ok(jsonld_response(wrap_jsonld_collection(
            &response_trades,
            "Trade",
            "urn:testudo:trades",
        )));
    }

    Ok(HttpResponse::Ok().json(PaginatedTrades {
        trades: response_trades,
        total,
        page,
        limit,
    }))
}

pub async fn get_trade(
    req: HttpRequest,
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;
    let trade_id = path.into_inner();

    let trade = sqlx::query_as::<_, JournalTrade>(
        "SELECT * FROM journal_trades WHERE id = $1",
    )
    .bind(trade_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get journal trade: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    let Some(trade) = trade else {
        return Ok(
            HttpResponse::NotFound().json(ErrorResponse::new("not_found", "Trade not found"))
        );
    };

    if trade.user_id != user.user_id {
        return Ok(HttpResponse::Forbidden().json(ErrorResponse::forbidden()));
    }

    let entries = sqlx::query_as::<_, JournalEntry>(
        "SELECT * FROM journal_entries WHERE trade_id = $1 AND user_id = $2 ORDER BY created_at DESC",
    )
    .bind(trade_id)
    .bind(user.user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get trade entries: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    let tags = sqlx::query_as::<_, JournalTag>(
        "SELECT t.* FROM journal_tags t JOIN journal_trade_tags jtt ON t.id = jtt.tag_id WHERE jtt.trade_id = $1 AND t.user_id = $2",
    )
    .bind(trade_id)
    .bind(user.user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get trade tags: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    let trade_with_tags = TradeWithTags { trade, tags };
    let detail = TradeDetailApiResponse {
        inner: TradeApiResponse::from_trade_with_tags(trade_with_tags),
        entries,
    };

    if wants_jsonld(&req) {
        return Ok(jsonld_response(wrap_jsonld(
            &detail,
            "Trade",
            Some(format!("urn:testudo:trade:{trade_id}")),
        )));
    }

    Ok(HttpResponse::Ok().json(detail))
}

pub async fn update_trade_notes(
    req: HttpRequest,
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
    body: web::Json<UpdateNotesRequest>,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;
    let trade_id = path.into_inner();

    let result = sqlx::query_as::<_, JournalTrade>(
        "UPDATE journal_trades SET notes = $1, updated_at = NOW() WHERE id = $2 AND user_id = $3 RETURNING *",
    )
    .bind(&body.notes)
    .bind(trade_id)
    .bind(user.user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update trade notes: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    match result {
        Some(trade) => {
            if wants_jsonld(&req) {
                return Ok(jsonld_response(wrap_jsonld(
                    &trade,
                    "Trade",
                    Some(format!("urn:testudo:trade:{}", trade_id)),
                )));
            }
            Ok(HttpResponse::Ok().json(trade))
        }
        None => Ok(
            HttpResponse::NotFound().json(ErrorResponse::new("not_found", "Trade not found"))
        ),
    }
}

// ---------------------------------------------------------------------------
// Draft notes for active trades (JNL-20)
// ---------------------------------------------------------------------------

pub async fn get_draft_notes(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let group_id = path.into_inner();
    let pool = &app_state.pool;

    let draft: Option<(Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT notes, updated_at FROM journal_trade_drafts WHERE trade_group_id = $1 AND user_id = $2"
    )
    .bind(group_id)
    .bind(user.user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch draft notes: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    match draft {
        Some((notes, updated_at)) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "trade_group_id": group_id,
            "notes": notes,
            "updated_at": updated_at,
        }))),
        None => Ok(HttpResponse::Ok().json(serde_json::json!({
            "trade_group_id": group_id,
            "notes": null,
        }))),
    }
}

pub async fn save_draft_notes(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
    body: web::Json<UpdateNotesRequest>,
) -> Result<HttpResponse> {
    let group_id = path.into_inner();
    let pool = &app_state.pool;

    sqlx::query(
        "INSERT INTO journal_trade_drafts (trade_group_id, user_id, notes) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (trade_group_id) DO UPDATE SET notes = $3, updated_at = NOW()"
    )
    .bind(group_id)
    .bind(user.user_id)
    .bind(&body.notes)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to save draft notes: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
}

// ---------------------------------------------------------------------------
// Journal Entries — full CRUD
// ---------------------------------------------------------------------------

const VALID_ENTRY_TYPES: &[&str] = &["note", "pre-trade", "post-trade", "daily-review", "weekly-review"];

pub async fn list_entries(
    req: HttpRequest,
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    query: web::Query<ListEntriesQuery>,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) * limit;

    let (count_sql, data_sql) = if query.trade_id.is_some() {
        (
            "SELECT COUNT(*) FROM journal_entries WHERE user_id = $1 AND trade_id = $2",
            "SELECT * FROM journal_entries WHERE user_id = $1 AND trade_id = $2 ORDER BY created_at DESC LIMIT $3 OFFSET $4",
        )
    } else {
        (
            "SELECT COUNT(*) FROM journal_entries WHERE user_id = $1 AND ($2::uuid IS NULL OR trade_id = $2)",
            "SELECT * FROM journal_entries WHERE user_id = $1 AND ($2::uuid IS NULL OR trade_id = $2) ORDER BY created_at DESC LIMIT $3 OFFSET $4",
        )
    };

    let total = sqlx::query_scalar::<_, i64>(count_sql)
        .bind(user.user_id)
        .bind(query.trade_id)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to count journal entries: {e}");
            actix_web::error::ErrorInternalServerError("Database error")
        })?;

    let entries = sqlx::query_as::<_, JournalEntry>(data_sql)
        .bind(user.user_id)
        .bind(query.trade_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list journal entries: {e}");
            actix_web::error::ErrorInternalServerError("Database error")
        })?;

    if wants_jsonld(&req) {
        return Ok(jsonld_response(wrap_jsonld_collection(
            &entries,
            "JournalEntry",
            "urn:testudo:entries",
        )));
    }

    Ok(HttpResponse::Ok().json(PaginatedEntries {
        entries,
        total,
        page,
        limit,
    }))
}

pub async fn create_entry(
    req: HttpRequest,
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    body: web::Json<CreateEntryRequest>,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;

    if body.title.trim().is_empty() {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "validation_error",
            "Title is required",
        )));
    }
    if body.body.trim().is_empty() {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "validation_error",
            "Body is required",
        )));
    }

    let entry_type = body.entry_type.as_deref().unwrap_or("note");
    if !VALID_ENTRY_TYPES.contains(&entry_type) {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "validation_error",
            format!("Invalid entry_type. Must be one of: {}", VALID_ENTRY_TYPES.join(", ")),
        )));
    }

    // If trade_id is provided, verify ownership
    if let Some(trade_id) = body.trade_id {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM journal_trades WHERE id = $1 AND user_id = $2)",
        )
        .bind(trade_id)
        .bind(user.user_id)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to verify trade ownership: {e}");
            actix_web::error::ErrorInternalServerError("Database error")
        })?;

        if !exists {
            return Ok(HttpResponse::NotFound().json(ErrorResponse::new(
                "not_found",
                "Trade not found",
            )));
        }
    }

    let entry = sqlx::query_as::<_, JournalEntry>(
        "INSERT INTO journal_entries (user_id, trade_id, entry_date, title, body, entry_type) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
    )
    .bind(user.user_id)
    .bind(body.trade_id)
    .bind(body.entry_date)
    .bind(body.title.trim())
    .bind(&body.body)
    .bind(entry_type)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create journal entry: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    if wants_jsonld(&req) {
        return Ok(jsonld_response(wrap_jsonld(
            &entry,
            "JournalEntry",
            Some(format!("urn:testudo:entry:{}", entry.id)),
        )));
    }

    Ok(HttpResponse::Created().json(entry))
}

pub async fn get_entry(
    req: HttpRequest,
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;
    let entry_id = path.into_inner();

    let entry = sqlx::query_as::<_, JournalEntry>(
        "SELECT * FROM journal_entries WHERE id = $1",
    )
    .bind(entry_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get journal entry: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    match entry {
        Some(e) if e.user_id == user.user_id => {
            if wants_jsonld(&req) {
                return Ok(jsonld_response(wrap_jsonld(
                    &e,
                    "JournalEntry",
                    Some(format!("urn:testudo:entry:{}", e.id)),
                )));
            }
            Ok(HttpResponse::Ok().json(e))
        }
        Some(_) => Ok(HttpResponse::Forbidden().json(ErrorResponse::forbidden())),
        None => Ok(
            HttpResponse::NotFound().json(ErrorResponse::new("not_found", "Entry not found"))
        ),
    }
}

pub async fn update_entry(
    req: HttpRequest,
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
    body: web::Json<UpdateEntryRequest>,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;
    let entry_id = path.into_inner();

    if body.title.trim().is_empty() {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "validation_error",
            "Title is required",
        )));
    }
    if body.body.trim().is_empty() {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "validation_error",
            "Body is required",
        )));
    }

    let entry_type = body.entry_type.as_deref().unwrap_or("note");
    if !VALID_ENTRY_TYPES.contains(&entry_type) {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "validation_error",
            format!("Invalid entry_type. Must be one of: {}", VALID_ENTRY_TYPES.join(", ")),
        )));
    }

    // First check existence and ownership
    let existing = sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM journal_entries WHERE id = $1",
    )
    .bind(entry_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to check entry ownership: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    match existing {
        None => {
            return Ok(
                HttpResponse::NotFound().json(ErrorResponse::new("not_found", "Entry not found"))
            );
        }
        Some(owner_id) if owner_id != user.user_id => {
            return Ok(HttpResponse::Forbidden().json(ErrorResponse::forbidden()));
        }
        _ => {}
    }

    let entry = sqlx::query_as::<_, JournalEntry>(
        "UPDATE journal_entries SET title = $1, body = $2, entry_type = $3, updated_at = NOW() WHERE id = $4 AND user_id = $5 RETURNING *",
    )
    .bind(body.title.trim())
    .bind(&body.body)
    .bind(entry_type)
    .bind(entry_id)
    .bind(user.user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update journal entry: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    if wants_jsonld(&req) {
        return Ok(jsonld_response(wrap_jsonld(
            &entry,
            "JournalEntry",
            Some(format!("urn:testudo:entry:{}", entry.id)),
        )));
    }

    Ok(HttpResponse::Ok().json(entry))
}

pub async fn delete_entry(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;
    let entry_id = path.into_inner();

    let result = sqlx::query(
        "DELETE FROM journal_entries WHERE id = $1 AND user_id = $2",
    )
    .bind(entry_id)
    .bind(user.user_id)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to delete journal entry: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    if result.rows_affected() == 0 {
        return Ok(
            HttpResponse::NotFound().json(ErrorResponse::new("not_found", "Entry not found"))
        );
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({"deleted": true})))
}

// ---------------------------------------------------------------------------
// Tags — full CRUD
// ---------------------------------------------------------------------------

pub async fn list_tags(
    req: HttpRequest,
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;

    let tags = sqlx::query_as::<_, JournalTag>(
        "SELECT * FROM journal_tags WHERE user_id = $1 ORDER BY name",
    )
    .bind(user.user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list tags: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    if wants_jsonld(&req) {
        return Ok(jsonld_response(wrap_jsonld_collection(
            &tags,
            "Tag",
            "urn:testudo:tags",
        )));
    }

    Ok(HttpResponse::Ok().json(tags))
}

// ---------------------------------------------------------------------------
// RSK-02: setup-tag autocomplete
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListSetupTagsQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SetupTagEntry {
    pub name: String,
    pub last_used: DateTime<Utc>,
    pub uses: i64,
}

pub async fn list_setup_tags(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    query: web::Query<ListSetupTagsQuery>,
) -> Result<HttpResponse> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);

    let tags = sqlx::query_as::<_, SetupTagEntry>(
        "SELECT setup_tag AS name, MAX(closed_at) AS last_used, COUNT(*) AS uses \
         FROM journal_trades \
         WHERE user_id = $1 AND setup_tag IS NOT NULL AND setup_tag <> '' \
         GROUP BY setup_tag \
         ORDER BY last_used DESC, uses DESC \
         LIMIT $2",
    )
    .bind(user.user_id)
    .bind(limit)
    .fetch_all(&app_state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list setup tags: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    Ok(HttpResponse::Ok().json(tags))
}

pub async fn create_tag(
    req: HttpRequest,
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    body: web::Json<CreateTagRequest>,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;

    if body.name.trim().is_empty() {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "validation_error",
            "Tag name is required",
        )));
    }

    let tag = sqlx::query_as::<_, JournalTag>(
        "INSERT INTO journal_tags (user_id, name, color) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(user.user_id)
    .bind(body.name.trim())
    .bind(&body.color)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.constraint() == Some("journal_tags_user_id_name_key") {
                return actix_web::error::ErrorConflict("Tag already exists");
            }
        }
        tracing::error!("Failed to create tag: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    if wants_jsonld(&req) {
        return Ok(jsonld_response(wrap_jsonld(
            &tag,
            "Tag",
            Some(format!("urn:testudo:tag:{}", tag.id)),
        )));
    }

    Ok(HttpResponse::Created().json(tag))
}

pub async fn update_tag(
    req: HttpRequest,
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
    body: web::Json<UpdateTagRequest>,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;
    let tag_id = path.into_inner();

    if let Some(ref name) = body.name {
        if name.trim().is_empty() {
            return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
                "validation_error",
                "Tag name cannot be empty",
            )));
        }
    }

    // Build dynamic SET clause
    let mut sets = vec![];
    let mut bind_idx = 3u32; // $1 = tag_id, $2 = user_id

    if body.name.is_some() {
        sets.push(format!("name = ${bind_idx}"));
        bind_idx += 1;
    }
    if body.color.is_some() {
        sets.push(format!("color = ${bind_idx}"));
    }

    if sets.is_empty() {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "validation_error",
            "No fields to update",
        )));
    }

    let sql = format!(
        "UPDATE journal_tags SET {} WHERE id = $1 AND user_id = $2 RETURNING *",
        sets.join(", ")
    );
    let mut query = sqlx::query_as::<_, JournalTag>(&sql)
        .bind(tag_id)
        .bind(user.user_id);

    if let Some(ref name) = body.name {
        query = query.bind(name.trim());
    }
    if let Some(ref color) = body.color {
        query = query.bind(color.as_str());
    }

    let tag = query.fetch_optional(pool).await.map_err(|e| {
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.constraint() == Some("journal_tags_user_id_name_key") {
                return actix_web::error::ErrorConflict("Tag name already exists");
            }
        }
        tracing::error!("Failed to update tag: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    match tag {
        Some(t) => {
            if wants_jsonld(&req) {
                return Ok(jsonld_response(wrap_jsonld(
                    &t,
                    "Tag",
                    Some(format!("urn:testudo:tag:{}", t.id)),
                )));
            }
            Ok(HttpResponse::Ok().json(t))
        }
        None => Ok(
            HttpResponse::NotFound().json(ErrorResponse::new("not_found", "Tag not found"))
        ),
    }
}

pub async fn delete_tag(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;
    let tag_id = path.into_inner();

    // CASCADE on journal_trade_tags handles link cleanup
    let result = sqlx::query(
        "DELETE FROM journal_tags WHERE id = $1 AND user_id = $2",
    )
    .bind(tag_id)
    .bind(user.user_id)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to delete tag: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    if result.rows_affected() == 0 {
        return Ok(
            HttpResponse::NotFound().json(ErrorResponse::new("not_found", "Tag not found"))
        );
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({"deleted": true})))
}

// ---------------------------------------------------------------------------
// Trade-Tag linking
// ---------------------------------------------------------------------------

pub async fn add_trade_tags(
    req: HttpRequest,
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
    body: web::Json<AddTradeTagsRequest>,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;
    let trade_id = path.into_inner();

    // Verify trade ownership
    let trade_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM journal_trades WHERE id = $1 AND user_id = $2)",
    )
    .bind(trade_id)
    .bind(user.user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to verify trade: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    if !trade_exists {
        return Ok(
            HttpResponse::NotFound().json(ErrorResponse::new("not_found", "Trade not found"))
        );
    }

    // Insert tags (ON CONFLICT DO NOTHING for idempotency)
    for tag_id in &body.tag_ids {
        // Verify tag belongs to user
        let tag_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM journal_tags WHERE id = $1 AND user_id = $2)",
        )
        .bind(tag_id)
        .bind(user.user_id)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to verify tag: {e}");
            actix_web::error::ErrorInternalServerError("Database error")
        })?;

        if !tag_exists {
            return Ok(HttpResponse::NotFound().json(ErrorResponse::new(
                "not_found",
                format!("Tag {tag_id} not found"),
            )));
        }

        sqlx::query(
            "INSERT INTO journal_trade_tags (trade_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(trade_id)
        .bind(tag_id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to link tag: {e}");
            actix_web::error::ErrorInternalServerError("Database error")
        })?;
    }

    // Return updated tag list for the trade
    let tags = sqlx::query_as::<_, JournalTag>(
        "SELECT t.* FROM journal_tags t JOIN journal_trade_tags jtt ON t.id = jtt.tag_id WHERE jtt.trade_id = $1",
    )
    .bind(trade_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list trade tags: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    if wants_jsonld(&req) {
        return Ok(jsonld_response(wrap_jsonld_collection(
            &tags,
            "Tag",
            &format!("urn:testudo:trade:{trade_id}:tags"),
        )));
    }

    Ok(HttpResponse::Ok().json(tags))
}

pub async fn remove_trade_tag(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    path: web::Path<(Uuid, Uuid)>,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;
    let (trade_id, tag_id) = path.into_inner();

    // Verify trade ownership
    let trade_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM journal_trades WHERE id = $1 AND user_id = $2)",
    )
    .bind(trade_id)
    .bind(user.user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to verify trade: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    if !trade_exists {
        return Ok(
            HttpResponse::NotFound().json(ErrorResponse::new("not_found", "Trade not found"))
        );
    }

    let result = sqlx::query(
        "DELETE FROM journal_trade_tags WHERE trade_id = $1 AND tag_id = $2",
    )
    .bind(trade_id)
    .bind(tag_id)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to remove trade tag: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    if result.rows_affected() == 0 {
        return Ok(HttpResponse::NotFound().json(ErrorResponse::new(
            "not_found",
            "Tag not linked to this trade",
        )));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({"deleted": true})))
}

// ---------------------------------------------------------------------------
// Analytics — wires StatsEngine + TimeSeriesService to HTTP
// ---------------------------------------------------------------------------
/// Response adapter structs that match the frontend's expected field names.
/// Backend service types use different names (net_pnl, duration_minutes, etc.)
/// so we adapt at the boundary rather than changing internal types.

#[derive(Debug, Serialize)]
struct OverviewResponse {
    account: crate::services::journal_stats::AccountOverview,
    performance: crate::services::journal_stats::PerformanceStats,
    risk: crate::services::journal_stats::RiskStats,
}

#[derive(Debug, Serialize)]
struct DataWrapper<T: Serialize> {
    data: T,
}

#[derive(Debug, Serialize)]
struct DailyPnlResponse {
    date: NaiveDate,
    pnl: rust_decimal::Decimal,
    trade_count: i32,
}

#[derive(Debug, Serialize)]
struct SymbolBreakdownResponse {
    symbol: String,
    trade_count: i64,
    total_pnl: rust_decimal::Decimal,
    win_rate: rust_decimal::Decimal,
}

#[derive(Debug, Serialize)]
struct SetupBreakdownResponse {
    setup_tag: String,
    trade_count: i64,
    total_pnl: rust_decimal::Decimal,
    win_rate: rust_decimal::Decimal,
    avg_r_multiple: Option<rust_decimal::Decimal>,
    expectancy: rust_decimal::Decimal,
}

#[derive(Debug, Serialize)]
struct DurationProfitResponse {
    duration_secs: f64,
    pnl: rust_decimal::Decimal,
    symbol: String,
}

#[derive(Debug, Serialize)]
struct ReturnBucketResponse {
    bucket: String,
    count: i64,
}

#[derive(Debug, Serialize)]
struct TimeSlotResponse {
    day_of_week: i32,
    hour: i32,
    trade_count: i64,
    avg_pnl: rust_decimal::Decimal,
}

// ── Pure response-conversion helpers (T1) ──────────────────────────────────
// Both per-section handlers and the batch handler emit identical wire shapes.
// Centralizing the field-name remapping here makes drift hard to introduce.

fn to_overview_response(
    account: crate::services::journal_stats::AccountOverview,
    performance: crate::services::journal_stats::PerformanceStats,
    risk: crate::services::journal_stats::RiskStats,
) -> OverviewResponse {
    OverviewResponse { account, performance, risk }
}

fn to_daily_pnl_response(
    raw: Vec<crate::services::journal_timeseries::DailyPnlPoint>,
) -> Vec<DailyPnlResponse> {
    raw.into_iter()
        .map(|p| DailyPnlResponse {
            date: p.date,
            pnl: p.net_pnl,
            trade_count: p.trade_count,
        })
        .collect()
}

fn to_symbol_breakdown_response(
    raw: Vec<crate::services::journal_timeseries::SymbolBreakdown>,
) -> Vec<SymbolBreakdownResponse> {
    raw.into_iter()
        .map(|s| SymbolBreakdownResponse {
            symbol: s.symbol,
            trade_count: s.trade_count,
            total_pnl: s.net_pnl,
            win_rate: s.win_rate,
        })
        .collect()
}

fn to_setup_breakdown_response(
    raw: Vec<crate::services::journal_timeseries::SetupBreakdown>,
) -> Vec<SetupBreakdownResponse> {
    raw.into_iter()
        .map(|s| SetupBreakdownResponse {
            setup_tag: s.setup_tag,
            trade_count: s.trade_count,
            total_pnl: s.net_pnl,
            win_rate: s.win_rate,
            avg_r_multiple: s.avg_r,
            expectancy: s.expectancy,
        })
        .collect()
}

fn to_duration_profit_response(
    raw: Vec<crate::services::journal_timeseries::DurationProfitPoint>,
) -> Vec<DurationProfitResponse> {
    raw.into_iter()
        .map(|d| DurationProfitResponse {
            duration_secs: d.duration_minutes * 60.0,
            pnl: d.net_pnl,
            symbol: d.symbol,
        })
        .collect()
}

fn to_return_distribution_response(
    raw: Vec<crate::services::journal_timeseries::ReturnBucket>,
) -> Vec<ReturnBucketResponse> {
    raw.into_iter()
        .map(|r| ReturnBucketResponse {
            bucket: r.bucket_label,
            count: r.day_count,
        })
        .collect()
}

fn to_time_distribution_response(
    raw: Vec<crate::services::journal_timeseries::TimeDistribution>,
) -> Vec<TimeSlotResponse> {
    raw.into_iter()
        .map(|t| TimeSlotResponse {
            day_of_week: t.day_of_week,
            hour: t.hour,
            trade_count: t.trade_count,
            avg_pnl: t.net_pnl,
        })
        .collect()
}

// ── Section computation helpers ────────────────────────────────────────────
// Used by both per-section handlers and the batch handler so the wire shape
// is identical by construction (parity guarantee, FR-15).

async fn compute_overview(
    engine: &StatsEngine,
    user_id: Uuid,
    filter: &StatsFilter,
) -> std::result::Result<OverviewResponse, sqlx::Error> {
    let account = engine.account_overview(user_id, filter).await?;
    let performance = engine.performance_stats(user_id, filter).await?;
    let risk = engine.risk_stats(user_id, filter).await?;
    Ok(to_overview_response(account, performance, risk))
}

async fn compute_equity_curve(
    ts: &TimeSeriesService,
    user_id: Uuid,
    filter: &StatsFilter,
) -> std::result::Result<
    DataWrapper<Vec<crate::services::journal_timeseries::EquityCurvePoint>>,
    sqlx::Error,
> {
    let data = ts.equity_curve(user_id, filter).await?;
    Ok(DataWrapper { data })
}

async fn compute_daily_pnl(
    ts: &TimeSeriesService,
    user_id: Uuid,
    filter: &StatsFilter,
) -> std::result::Result<DataWrapper<Vec<DailyPnlResponse>>, sqlx::Error> {
    let raw = ts.daily_pnl(user_id, filter).await?;
    Ok(DataWrapper { data: to_daily_pnl_response(raw) })
}

async fn compute_symbol_breakdown(
    ts: &TimeSeriesService,
    user_id: Uuid,
    filter: &StatsFilter,
) -> std::result::Result<DataWrapper<Vec<SymbolBreakdownResponse>>, sqlx::Error> {
    let raw = ts.symbol_breakdown(user_id, filter).await?;
    Ok(DataWrapper { data: to_symbol_breakdown_response(raw) })
}

async fn compute_setup_breakdown(
    ts: &TimeSeriesService,
    user_id: Uuid,
    filter: &StatsFilter,
) -> std::result::Result<DataWrapper<Vec<SetupBreakdownResponse>>, sqlx::Error> {
    let raw = ts.setup_breakdown(user_id, filter).await?;
    Ok(DataWrapper { data: to_setup_breakdown_response(raw) })
}

async fn compute_duration_profit(
    ts: &TimeSeriesService,
    user_id: Uuid,
    filter: &StatsFilter,
) -> std::result::Result<DataWrapper<Vec<DurationProfitResponse>>, sqlx::Error> {
    let raw = ts.duration_profit(user_id, filter).await?;
    Ok(DataWrapper { data: to_duration_profit_response(raw) })
}

async fn compute_return_distribution(
    ts: &TimeSeriesService,
    user_id: Uuid,
    filter: &StatsFilter,
) -> std::result::Result<DataWrapper<Vec<ReturnBucketResponse>>, sqlx::Error> {
    let raw = ts.return_distribution(user_id, filter).await?;
    Ok(DataWrapper { data: to_return_distribution_response(raw) })
}

async fn compute_time_distribution(
    ts: &TimeSeriesService,
    user_id: Uuid,
    filter: &StatsFilter,
) -> std::result::Result<DataWrapper<Vec<TimeSlotResponse>>, sqlx::Error> {
    let raw = ts.time_distribution(user_id, filter).await?;
    Ok(DataWrapper { data: to_time_distribution_response(raw) })
}

// ── Per-section HTTP handlers (kept for backward compat / debugging) ───────

pub async fn overview(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    query: web::Query<StatsFilter>,
) -> Result<HttpResponse> {
    let engine = StatsEngine::new(app_state.analytics_pool.clone());
    let filter = query.into_inner();

    let resp = compute_overview(&engine, user.user_id, &filter).await.map_err(|e| {
        tracing::error!("Failed to compute overview: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    Ok(HttpResponse::Ok().json(resp))
}

pub async fn equity_curve(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    query: web::Query<StatsFilter>,
) -> Result<HttpResponse> {
    let ts = TimeSeriesService::new(app_state.analytics_pool.clone());
    let resp = compute_equity_curve(&ts, user.user_id, &query).await.map_err(|e| {
        tracing::error!("Failed to compute equity curve: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    Ok(HttpResponse::Ok().json(resp))
}

pub async fn daily_pnl(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    query: web::Query<StatsFilter>,
) -> Result<HttpResponse> {
    let ts = TimeSeriesService::new(app_state.analytics_pool.clone());
    let resp = compute_daily_pnl(&ts, user.user_id, &query).await.map_err(|e| {
        tracing::error!("Failed to compute daily P&L: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    Ok(HttpResponse::Ok().json(resp))
}

pub async fn symbol_breakdown(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    query: web::Query<StatsFilter>,
) -> Result<HttpResponse> {
    let ts = TimeSeriesService::new(app_state.analytics_pool.clone());
    let resp = compute_symbol_breakdown(&ts, user.user_id, &query).await.map_err(|e| {
        tracing::error!("Failed to compute symbol breakdown: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    Ok(HttpResponse::Ok().json(resp))
}

pub async fn setup_breakdown(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    query: web::Query<StatsFilter>,
) -> Result<HttpResponse> {
    let ts = TimeSeriesService::new(app_state.analytics_pool.clone());
    let resp = compute_setup_breakdown(&ts, user.user_id, &query).await.map_err(|e| {
        tracing::error!("Failed to compute setup breakdown: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    Ok(HttpResponse::Ok().json(resp))
}

pub async fn duration_profit(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    query: web::Query<StatsFilter>,
) -> Result<HttpResponse> {
    let ts = TimeSeriesService::new(app_state.analytics_pool.clone());
    let resp = compute_duration_profit(&ts, user.user_id, &query).await.map_err(|e| {
        tracing::error!("Failed to compute duration/profit: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    Ok(HttpResponse::Ok().json(resp))
}

pub async fn return_distribution(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    query: web::Query<StatsFilter>,
) -> Result<HttpResponse> {
    let ts = TimeSeriesService::new(app_state.analytics_pool.clone());
    let resp = compute_return_distribution(&ts, user.user_id, &query).await.map_err(|e| {
        tracing::error!("Failed to compute return distribution: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    Ok(HttpResponse::Ok().json(resp))
}

pub async fn time_distribution(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    query: web::Query<StatsFilter>,
) -> Result<HttpResponse> {
    let ts = TimeSeriesService::new(app_state.analytics_pool.clone());
    let resp = compute_time_distribution(&ts, user.user_id, &query).await.map_err(|e| {
        tracing::error!("Failed to compute time distribution: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    Ok(HttpResponse::Ok().json(resp))
}

// ── Batch endpoint (PERF-02 CP-1) ──────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SectionKey {
    Overview,
    EquityCurve,
    DailyPnl,
    SymbolBreakdown,
    SetupBreakdown,
    DurationProfit,
    ReturnDistribution,
    TimeDistribution,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BatchRequest {
    filter: StatsFilter,
    /// `None` (or omitted) = compute all sections.
    sections: Option<Vec<SectionKey>>,
}

/// Per-section result envelope. Untagged so the success arm serializes as the
/// raw response body (preserving wire parity with per-section endpoints) and
/// the error arm serializes as `{ "error": "…" }`. None of the existing
/// response types contain a top-level `error` field, so the dispatch is
/// unambiguous (plan risk #12).
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum SectionResult<T: Serialize> {
    Ok(T),
    Err { error: String },
}

#[derive(Debug, Serialize, Default)]
struct BatchResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    overview: Option<SectionResult<OverviewResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    equity_curve: Option<
        SectionResult<DataWrapper<Vec<crate::services::journal_timeseries::EquityCurvePoint>>>,
    >,
    #[serde(skip_serializing_if = "Option::is_none")]
    daily_pnl: Option<SectionResult<DataWrapper<Vec<DailyPnlResponse>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol_breakdown: Option<SectionResult<DataWrapper<Vec<SymbolBreakdownResponse>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    setup_breakdown: Option<SectionResult<DataWrapper<Vec<SetupBreakdownResponse>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_profit: Option<SectionResult<DataWrapper<Vec<DurationProfitResponse>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    return_distribution: Option<SectionResult<DataWrapper<Vec<ReturnBucketResponse>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    time_distribution: Option<SectionResult<DataWrapper<Vec<TimeSlotResponse>>>>,
}

/// Wraps a per-section future, returning `None` when the caller did not
/// request the section. The future is constructed eagerly inside `tokio::join!`
/// but only polled when `wanted == true` — `if !wanted { return None }`
/// short-circuits before `.await`.
async fn run_section_when<T, Fut>(wanted: bool, fut: Fut) -> Option<SectionResult<T>>
where
    T: Serialize,
    Fut: std::future::Future<Output = std::result::Result<T, sqlx::Error>>,
{
    if !wanted {
        return None;
    }
    Some(match fut.await {
        Ok(v) => SectionResult::Ok(v),
        Err(e) => {
            tracing::error!("analytics_batch section error: {e}");
            SectionResult::Err { error: e.to_string() }
        }
    })
}

pub(crate) async fn analytics_batch(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    body: web::Json<BatchRequest>,
) -> Result<HttpResponse> {
    let engine = StatsEngine::new(app_state.analytics_pool.clone());
    let ts = TimeSeriesService::new(app_state.analytics_pool.clone());
    let filter = body.filter.clone();
    let user_id = user.user_id;
    let want = |k: SectionKey| body.sections.as_ref().is_none_or(|s| s.contains(&k));

    let (ov, eq, dp, sb, setb, durp, ret, td) = tokio::join!(
        run_section_when(want(SectionKey::Overview),           compute_overview(&engine, user_id, &filter)),
        run_section_when(want(SectionKey::EquityCurve),        compute_equity_curve(&ts, user_id, &filter)),
        run_section_when(want(SectionKey::DailyPnl),           compute_daily_pnl(&ts, user_id, &filter)),
        run_section_when(want(SectionKey::SymbolBreakdown),    compute_symbol_breakdown(&ts, user_id, &filter)),
        run_section_when(want(SectionKey::SetupBreakdown),     compute_setup_breakdown(&ts, user_id, &filter)),
        run_section_when(want(SectionKey::DurationProfit),     compute_duration_profit(&ts, user_id, &filter)),
        run_section_when(want(SectionKey::ReturnDistribution), compute_return_distribution(&ts, user_id, &filter)),
        run_section_when(want(SectionKey::TimeDistribution),   compute_time_distribution(&ts, user_id, &filter)),
    );

    Ok(HttpResponse::Ok().json(BatchResponse {
        overview: ov,
        equity_curve: eq,
        daily_pnl: dp,
        symbol_breakdown: sb,
        setup_breakdown: setb,
        duration_profit: durp,
        return_distribution: ret,
        time_distribution: td,
    }))
}


// ---------------------------------------------------------------------------
// Image upload + storage quotas (JNL-18)
// ---------------------------------------------------------------------------

const MAX_UPLOAD_SIZE: usize = 5 * 1024 * 1024; // 5 MB
const DEFAULT_QUOTA_BYTES: i64 = 100 * 1024 * 1024; // 100 MB
const ALLOWED_EXTENSIONS: &[(&str, &str)] = &[
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/webp", "webp"),
    ("image/gif", "gif"),
];

pub async fn upload_journal_image(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    mut payload: Multipart,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;
    let upload_dir = std::path::PathBuf::from("./uploads/journal");
    tokio::fs::create_dir_all(&upload_dir).await.map_err(|e| {
        tracing::error!("Failed to create upload dir: {e}");
        actix_web::error::ErrorInternalServerError("Upload directory error")
    })?;

    let Some(item) = payload.next().await else {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "no_file",
            "No file provided",
        )));
    };

    let mut field = item.map_err(|e| {
        tracing::error!("Multipart error: {e}");
        actix_web::error::ErrorBadRequest("Invalid multipart data")
    })?;

    let content_type = field
        .content_type()
        .map(|ct| ct.to_string())
        .unwrap_or_default();

    let ext = ALLOWED_EXTENSIONS
        .iter()
        .find(|(mime, _)| *mime == content_type)
        .map(|(_, ext)| *ext);

    let ext = match ext {
        Some(e) => e,
        None => {
            return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
                "invalid_type",
                "Only PNG, JPG, WEBP, and GIF images are accepted",
            )));
        }
    };

    let mut data = Vec::new();
    while let Some(chunk) = field.next().await {
        let chunk = chunk.map_err(|e| {
            tracing::error!("Chunk read error: {e}");
            actix_web::error::ErrorBadRequest("Failed to read upload data")
        })?;
        data.extend_from_slice(&chunk);
        if data.len() > MAX_UPLOAD_SIZE {
            return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
                "too_large",
                "File exceeds 5MB limit",
            )));
        }
    }

    // JNL-18: Quota check before writing file
    let file_size = data.len() as i64;
    let used_bytes: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(file_size), 0) FROM journal_images WHERE user_id = $1",
    )
    .bind(user.user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to query storage usage: {e}");
        actix_web::error::ErrorInternalServerError("Storage query error")
    })?;

    let quota_bytes = DEFAULT_QUOTA_BYTES;
    let remaining = quota_bytes - used_bytes;
    if file_size > remaining {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::with_details(
            "quota_exceeded",
            format!(
                "Storage limit reached ({} / {}). Export your entries to free up space.",
                format_bytes(used_bytes),
                format_bytes(quota_bytes),
            ),
            serde_json::json!({
                "used_bytes": used_bytes,
                "quota_bytes": quota_bytes,
                "remaining_bytes": remaining.max(0),
            }),
        )));
    }

    let file_id = Uuid::new_v4();
    let filename = format!("{file_id}.{ext}");
    let storage_path = format!("/uploads/journal/{filename}");

    // Insert DB row first (tracks quota), then write file. Rollback on write failure.
    let image_id: Uuid = sqlx::query_scalar(
        "INSERT INTO journal_images (user_id, file_name, file_size, mime_type, storage_path) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(user.user_id)
    .bind(&filename)
    .bind(file_size)
    .bind(&content_type)
    .bind(&storage_path)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to insert journal_images row: {e}");
        actix_web::error::ErrorInternalServerError("Failed to record upload")
    })?;

    let file_path = upload_dir.join(&filename);
    if let Err(e) = tokio::fs::write(&file_path, &data).await {
        tracing::error!("Failed to write upload, rolling back DB row: {e}");
        // Rollback: delete the DB row we just inserted
        let _ = sqlx::query("DELETE FROM journal_images WHERE id = $1")
            .bind(image_id)
            .execute(pool)
            .await;
        return Err(actix_web::error::ErrorInternalServerError(
            "Failed to save file",
        ));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({ "url": storage_path })))
}

// ---------------------------------------------------------------------------
// Storage usage (JNL-18)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct StorageUsageResponse {
    used_bytes: i64,
    quota_bytes: i64,
    image_count: i64,
}

pub async fn storage_usage(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;

    let row: (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(file_size), 0), COUNT(*) FROM journal_images WHERE user_id = $1",
    )
    .bind(user.user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to query storage usage: {e}");
        actix_web::error::ErrorInternalServerError("Storage query error")
    })?;

    Ok(HttpResponse::Ok().json(StorageUsageResponse {
        used_bytes: row.0,
        quota_bytes: DEFAULT_QUOTA_BYTES,
        image_count: row.1,
    }))
}

// ---------------------------------------------------------------------------
// Image deletion (JNL-18)
// ---------------------------------------------------------------------------

pub async fn delete_image(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let pool = &app_state.pool;
    let image_id = path.into_inner();

    // Fetch the image row (scoped to user)
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT storage_path FROM journal_images WHERE id = $1 AND user_id = $2",
    )
    .bind(image_id)
    .bind(user.user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to query journal_images: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    let Some((storage_path,)) = row else {
        return Ok(HttpResponse::NotFound().json(ErrorResponse::new(
            "not_found",
            "Image not found",
        )));
    };

    // Delete the DB row
    sqlx::query("DELETE FROM journal_images WHERE id = $1")
        .bind(image_id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete journal_images row: {e}");
            actix_web::error::ErrorInternalServerError("Database error")
        })?;

    // Delete the file (best-effort — DB row is the source of truth for quota)
    let file_path = format!(".{storage_path}"); // storage_path is "/uploads/journal/..."
    if let Err(e) = tokio::fs::remove_file(&file_path).await {
        tracing::warn!("Failed to delete image file {file_path}: {e}");
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({ "deleted": true })))
}

fn format_bytes(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = 1024 * 1024;
    if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes}B")
    }
}

// ── Inline parity test module (T5) ─────────────────────────────────────────
//
// `#[ignore]`-gated; requires a live `DATABASE_URL`. `cargo test` skips this;
// run via `cargo test -- --ignored` against a developer-local Postgres.

#[cfg(test)]
mod batch_tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    /// Small helper: connect to the database pointed at by `DATABASE_URL`.
    /// Returns `None` when the env var is unset, so the test silently bails
    /// even if `--ignored` is passed without a live database.
    async fn connect_pool() -> Option<sqlx::PgPool> {
        let url = std::env::var("DATABASE_URL").ok()?;
        PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .ok()
    }

    /// Seed 3+ closed trades for a synthetic user covering ≥ 2 distinct
    /// symbols, ≥ 1 setup tag, ≥ 1 winning + 1 losing trade so every
    /// section returns data.
    async fn seed_fixture(pool: &sqlx::PgPool, user_id: Uuid) -> sqlx::Result<()> {
        // Idempotent cleanup before seeding.
        let _ = sqlx::query("DELETE FROM journal_trades WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM journal_daily_stats WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await;

        // Three trades: BTC win, ETH loss, BTC win (different setup).
        // JNL-DUR-01: duration_secs is generated by the DB from (closed_at - opened_at).
        // Each row encodes the desired duration as the offset between the two timestamps,
        // expressed as seconds in the `dur_secs` column of the tuple.
        let rows: &[(&str, &str, &str, f64, i32, &str)] = &[
            ("BTC_USDT", "long",  "breakout",  150.0,  3600, "2026-04-01"),
            ("ETH_USDT", "short", "fade",      -50.0,  1800, "2026-04-02"),
            ("BTC_USDT", "long",  "breakout",   75.0,  2400, "2026-04-03"),
        ];

        for (symbol, side, setup, pnl, dur_secs, day) in rows {
            sqlx::query(
                "INSERT INTO journal_trades \
                 (id, user_id, exchange, symbol, side, opened_at, closed_at, \
                  net_pnl, setup_tag, needs_reconciliation) \
                 VALUES ($1, $2, 'WOO', $3, $4, ($5::date)::timestamptz, \
                         (($5::date)::timestamptz + ($6::int * interval '1 second')), \
                         $7::numeric, $8, FALSE)",
            )
            .bind(Uuid::new_v4())
            .bind(user_id)
            .bind(*symbol)
            .bind(*side)
            .bind(*day)
            .bind(*dur_secs)
            .bind(pnl.to_string())
            .bind(*setup)
            .execute(pool)
            .await?;
        }
        Ok(())
    }

    async fn cleanup(pool: &sqlx::PgPool, user_id: Uuid) {
        let _ = sqlx::query("DELETE FROM journal_trades WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM journal_daily_stats WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
    }

    /// Parity test: per-section helpers and the batch fan-out must produce
    /// byte-identical JSON for the same fixture.
    #[tokio::test]
    #[ignore]
    async fn batch_parity_with_per_section() {
        let Some(pool) = connect_pool().await else { return };
        let user_id = Uuid::new_v4();
        seed_fixture(&pool, user_id).await.expect("seed");

        let engine = StatsEngine::new(pool.clone());
        let ts = TimeSeriesService::new(pool.clone());
        let filter = StatsFilter::default();

        // Per-section path.
        let per_overview = compute_overview(&engine, user_id, &filter).await.unwrap();
        let per_equity = compute_equity_curve(&ts, user_id, &filter).await.unwrap();
        let per_daily = compute_daily_pnl(&ts, user_id, &filter).await.unwrap();
        let per_symbol = compute_symbol_breakdown(&ts, user_id, &filter).await.unwrap();
        let per_setup = compute_setup_breakdown(&ts, user_id, &filter).await.unwrap();
        let per_duration = compute_duration_profit(&ts, user_id, &filter).await.unwrap();
        let per_return = compute_return_distribution(&ts, user_id, &filter).await.unwrap();
        let per_time = compute_time_distribution(&ts, user_id, &filter).await.unwrap();

        // Batch path (sections: None = all).
        let (ov, eq, dp, sb, setb, durp, ret, td) = tokio::join!(
            run_section_when(true, compute_overview(&engine, user_id, &filter)),
            run_section_when(true, compute_equity_curve(&ts, user_id, &filter)),
            run_section_when(true, compute_daily_pnl(&ts, user_id, &filter)),
            run_section_when(true, compute_symbol_breakdown(&ts, user_id, &filter)),
            run_section_when(true, compute_setup_breakdown(&ts, user_id, &filter)),
            run_section_when(true, compute_duration_profit(&ts, user_id, &filter)),
            run_section_when(true, compute_return_distribution(&ts, user_id, &filter)),
            run_section_when(true, compute_time_distribution(&ts, user_id, &filter)),
        );
        let envelope = BatchResponse {
            overview: ov,
            equity_curve: eq,
            daily_pnl: dp,
            symbol_breakdown: sb,
            setup_breakdown: setb,
            duration_profit: durp,
            return_distribution: ret,
            time_distribution: td,
        };

        assert_eq!(
            serde_json::to_value(&per_overview).unwrap(),
            serde_json::to_value(&envelope.overview).unwrap(),
            "overview parity",
        );
        assert_eq!(
            serde_json::to_value(&per_equity).unwrap(),
            serde_json::to_value(&envelope.equity_curve).unwrap(),
            "equity_curve parity",
        );
        assert_eq!(
            serde_json::to_value(&per_daily).unwrap(),
            serde_json::to_value(&envelope.daily_pnl).unwrap(),
            "daily_pnl parity",
        );
        assert_eq!(
            serde_json::to_value(&per_symbol).unwrap(),
            serde_json::to_value(&envelope.symbol_breakdown).unwrap(),
            "symbol_breakdown parity",
        );
        assert_eq!(
            serde_json::to_value(&per_setup).unwrap(),
            serde_json::to_value(&envelope.setup_breakdown).unwrap(),
            "setup_breakdown parity",
        );
        assert_eq!(
            serde_json::to_value(&per_duration).unwrap(),
            serde_json::to_value(&envelope.duration_profit).unwrap(),
            "duration_profit parity",
        );
        assert_eq!(
            serde_json::to_value(&per_return).unwrap(),
            serde_json::to_value(&envelope.return_distribution).unwrap(),
            "return_distribution parity",
        );
        assert_eq!(
            serde_json::to_value(&per_time).unwrap(),
            serde_json::to_value(&envelope.time_distribution).unwrap(),
            "time_distribution parity",
        );

        cleanup(&pool, user_id).await;
    }

    /// Partial-failure: one section returns Err, the envelope must still be
    /// 200 with `SectionResult::Err` populated and other sections Ok.
    #[tokio::test]
    #[ignore]
    async fn batch_partial_failure_envelopes_error() {
        let Some(pool) = connect_pool().await else { return };
        let user_id = Uuid::new_v4();
        seed_fixture(&pool, user_id).await.expect("seed");

        let engine = StatsEngine::new(pool.clone());
        let ts = TimeSeriesService::new(pool.clone());
        let filter = StatsFilter::default();

        // Inject an explicit error future for the equity_curve slot — exact
        // type signature must match what `compute_equity_curve` returns so
        // `run_section_when` can wrap it identically.
        let equity_err = async {
            Err::<DataWrapper<Vec<crate::services::journal_timeseries::EquityCurvePoint>>, sqlx::Error>(
                sqlx::Error::RowNotFound,
            )
        };

        let (ov, eq, dp, sb, setb, durp, ret, td) = tokio::join!(
            run_section_when(true, compute_overview(&engine, user_id, &filter)),
            run_section_when(true, equity_err),
            run_section_when(true, compute_daily_pnl(&ts, user_id, &filter)),
            run_section_when(true, compute_symbol_breakdown(&ts, user_id, &filter)),
            run_section_when(true, compute_setup_breakdown(&ts, user_id, &filter)),
            run_section_when(true, compute_duration_profit(&ts, user_id, &filter)),
            run_section_when(true, compute_return_distribution(&ts, user_id, &filter)),
            run_section_when(true, compute_time_distribution(&ts, user_id, &filter)),
        );
        let envelope = BatchResponse {
            overview: ov,
            equity_curve: eq,
            daily_pnl: dp,
            symbol_breakdown: sb,
            setup_breakdown: setb,
            duration_profit: durp,
            return_distribution: ret,
            time_distribution: td,
        };

        match envelope.equity_curve.as_ref().expect("section present") {
            SectionResult::Err { error } => assert!(!error.is_empty()),
            SectionResult::Ok(_) => panic!("expected equity_curve to be Err"),
        }
        match envelope.overview.as_ref().expect("section present") {
            SectionResult::Ok(_) => {}
            SectionResult::Err { error } => panic!("overview should be Ok, got Err: {error}"),
        }

        cleanup(&pool, user_id).await;
    }
}
