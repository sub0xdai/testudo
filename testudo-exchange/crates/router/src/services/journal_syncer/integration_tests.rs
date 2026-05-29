//! Integration tests for JournalSyncer — require DATABASE_URL env var.
//!
//! Run with:
//!   DATABASE_URL=postgres://... cargo test --ignored journal_syncer

// @anchor exchange:router:integration_tests
// @tags api

use async_trait::async_trait;
use chrono::{DateTime, Duration, TimeZone, Utc};
use common_utils::journal::{FillSide, RawFill};
use rust_decimal_macros::dec;
use std::sync::Arc;
use tokio::sync::{mpsc, Notify};
use uuid::Uuid;

use crate::repositories::exchange_account::{AesGcmVault, ExchangeAccountRepository};
use crate::repositories::raw_fills::RawFillRepository;
use crate::services::journal_service::JournalService;
use super::syncer::JournalSyncerBuilder;
use super::{FillSource, SyncError};

struct MockFillSource {
    fills: Vec<RawFill>,
    label: String,
}

#[async_trait]
impl FillSource for MockFillSource {
    async fn fetch_since(
        &self,
        _user_id: Uuid,
        _account_id: Uuid,
        _since: DateTime<Utc>,
    ) -> Result<Vec<RawFill>, SyncError> {
        Ok(self.fills.clone())
    }
    fn exchange_label(&self) -> &str {
        &self.label
    }
}

async fn make_pool() -> sqlx::PgPool {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    sqlx::PgPool::connect(&url).await.unwrap()
}

const TEST_KEY_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn make_fill(user_id: Uuid, exec_id: &str, side: FillSide, t_offset_secs: i64) -> RawFill {
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
        exec_time: Utc.with_ymd_and_hms(2026, 5, 3, 10, 0, 0).unwrap()
            + Duration::seconds(t_offset_secs),
        order_id: Some(format!("ord_{exec_id}")),
        raw_json: serde_json::json!({}),
    }
}

#[tokio::test]
#[ignore]
async fn test_journal_syncer_tick_upserts_fills_and_reconstructs_trades() {
    let pool = make_pool().await;
    let user_id = Uuid::new_v4();
    let account_id = Uuid::new_v4();

    // Insert test user + exchange account (FK constraints).
    sqlx::query("INSERT INTO users (id, email, password_hash) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(format!("syncer_test_{}@test.com", user_id))
        .bind("hash")
        .execute(&pool)
        .await
        .unwrap();

    let vault = AesGcmVault::from_hex(TEST_KEY_HEX).unwrap();
    let enc = vault.encrypt(b"key").unwrap();
    sqlx::query(
        "INSERT INTO exchange_accounts \
         (id, user_id, exchange_name, api_key_encrypted, api_secret_encrypted, auth_mode) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(account_id)
    .bind(user_id)
    .bind("bybit")
    .bind(&enc)
    .bind(&enc)
    .bind("api_key")
    .execute(&pool)
    .await
    .unwrap();

    // 5 fills = 2 round trips (Buy→Sell, Buy→Sell) + 1 open Buy.
    let fills = vec![
        make_fill(user_id, "e1", FillSide::Buy, 0),
        make_fill(user_id, "e2", FillSide::Sell, 60),
        make_fill(user_id, "e3", FillSide::Buy, 120),
        make_fill(user_id, "e4", FillSide::Sell, 180),
        make_fill(user_id, "e5", FillSide::Buy, 240), // open — not emitted
    ];

    let source = Arc::new(MockFillSource {
        fills: fills.clone(),
        label: "bybit".to_string(),
    });

    let vault2 = AesGcmVault::from_hex(TEST_KEY_HEX).unwrap();
    let exchange_account_repo = ExchangeAccountRepository::new(pool.clone(), vault2);
    let (event_tx, _rx) = mpsc::channel(16);

    let syncer = JournalSyncerBuilder {
        user_id,
        account_id,
        exchange_label: "bybit".to_string(),
        interval_secs: 30,
        source: source.clone(),
        pool: pool.clone(),
        exchange_account_repo: exchange_account_repo.clone(),
        journal_service: Arc::new(JournalService::new(pool.clone())),
        notify: Arc::new(Notify::new()),
        event_tx: Some(event_tx),
        cex_client: None,
    }
    .build();

    // First tick: 5 fills inserted, 2 round trips projected.
    let new_count = syncer.tick().await.unwrap();
    assert_eq!(new_count, 5, "all 5 fills should be new");

    let raw_fill_repo = RawFillRepository::new(pool.clone());
    let all_fills = raw_fill_repo.fetch_for_account(user_id, "bybit").await.unwrap();
    assert_eq!(all_fills.len(), 5);

    let trades_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM journal_trades WHERE user_id = $1 AND source = 'pull_sync'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(trades_count, 2, "should produce exactly 2 round-trip trades");

    // Second tick: no new fills, watermark unchanged.
    let new_count2 = syncer.tick().await.unwrap();
    assert_eq!(new_count2, 0, "no new fills on second tick");

    let trades_count2: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM journal_trades WHERE user_id = $1 AND source = 'pull_sync'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(trades_count2, 2, "upsert is idempotent — still 2 trades");

    // Watermark should be set.
    let watermark = exchange_account_repo
        .get_last_synced_exec_time(account_id)
        .await
        .unwrap();
    assert!(watermark.is_some(), "watermark should be advanced after first tick");

    // Cleanup — cascade on user delete.
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await;
}

#[tokio::test]
#[ignore]
async fn test_journal_syncer_tick_hyperliquid_fills() {
    let pool = make_pool().await;
    let user_id = Uuid::new_v4();
    let account_id = Uuid::new_v4();

    sqlx::query("INSERT INTO users (id, email, password_hash) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(format!("hl_syncer_test_{}@test.com", user_id))
        .bind("hash")
        .execute(&pool)
        .await
        .unwrap();

    let vault = AesGcmVault::from_hex(TEST_KEY_HEX).unwrap();
    let enc = vault.encrypt(b"key").unwrap();
    sqlx::query(
        "INSERT INTO exchange_accounts \
         (id, user_id, exchange_name, api_key_encrypted, api_secret_encrypted, auth_mode) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(account_id)
    .bind(user_id)
    .bind("hyperliquid")
    .bind(&enc)
    .bind(&enc)
    .bind("agent_wallet")
    .execute(&pool)
    .await
    .unwrap();

    // HL-shaped fills: BTC_USDT long round trip + 1 open position.
    let base_time = Utc.with_ymd_and_hms(2026, 5, 3, 12, 0, 0).unwrap();
    let hl_fills = vec![
        RawFill {
            user_id,
            exchange: "hyperliquid".to_string(),
            exec_id: "hl_e1".to_string(),
            symbol: "BTC_USDT".to_string(),
            side: FillSide::Buy,
            price: dec!(60000),
            qty: dec!(0.1),
            fee: dec!(0.6),
            fee_asset: "USDC".to_string(),
            exec_time: base_time,
            order_id: Some("oid_1".to_string()),
            raw_json: serde_json::json!({}),
        },
        RawFill {
            user_id,
            exchange: "hyperliquid".to_string(),
            exec_id: "hl_e2".to_string(),
            symbol: "BTC_USDT".to_string(),
            side: FillSide::Sell,
            price: dec!(61000),
            qty: dec!(0.1),
            fee: dec!(0.61),
            fee_asset: "USDC".to_string(),
            exec_time: base_time + Duration::seconds(60),
            order_id: Some("oid_2".to_string()),
            raw_json: serde_json::json!({}),
        },
        RawFill {
            user_id,
            exchange: "hyperliquid".to_string(),
            exec_id: "hl_e3".to_string(),
            symbol: "ETH_USDT".to_string(),
            side: FillSide::Buy,
            price: dec!(3000),
            qty: dec!(1.0),
            fee: dec!(0.3),
            fee_asset: "USDC".to_string(),
            exec_time: base_time + Duration::seconds(120),
            order_id: Some("oid_3".to_string()),
            raw_json: serde_json::json!({}),
        },
    ];

    let source = Arc::new(MockFillSource {
        fills: hl_fills,
        label: "hyperliquid".to_string(),
    });

    let vault2 = AesGcmVault::from_hex(TEST_KEY_HEX).unwrap();
    let exchange_account_repo = ExchangeAccountRepository::new(pool.clone(), vault2);
    let (event_tx, _rx) = mpsc::channel(16);

    let syncer = JournalSyncerBuilder {
        user_id,
        account_id,
        exchange_label: "hyperliquid".to_string(),
        interval_secs: 30,
        source: source.clone(),
        pool: pool.clone(),
        exchange_account_repo: exchange_account_repo.clone(),
        journal_service: Arc::new(JournalService::new(pool.clone())),
        notify: Arc::new(Notify::new()),
        event_tx: Some(event_tx),
        cex_client: None,
    }
    .build();

    // First tick: 3 fills, 1 closed BTC round trip, 1 open ETH position (not emitted).
    let new_count = syncer.tick().await.unwrap();
    assert_eq!(new_count, 3, "all 3 HL fills should be new");

    let raw_fill_repo = RawFillRepository::new(pool.clone());
    let all_fills = raw_fill_repo
        .fetch_for_account(user_id, "hyperliquid")
        .await
        .unwrap();
    assert_eq!(all_fills.len(), 3);

    let trades_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM journal_trades WHERE user_id = $1 AND source = 'pull_sync'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(trades_count, 1, "only the closed BTC round trip should be projected");

    // Verify the projected trade's economics.
    let (entry_price, exit_price): (rust_decimal::Decimal, rust_decimal::Decimal) =
        sqlx::query_as(
            "SELECT entry_price, exit_price FROM journal_trades \
             WHERE user_id = $1 AND source = 'pull_sync'",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(entry_price, dec!(60000));
    assert_eq!(exit_price, dec!(61000));

    // Second tick: idempotent — no new fills, no new trades.
    let new_count2 = syncer.tick().await.unwrap();
    assert_eq!(new_count2, 0, "no new fills on second tick");

    let trades_count2: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM journal_trades WHERE user_id = $1 AND source = 'pull_sync'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(trades_count2, 1, "upsert is idempotent — still 1 trade");

    // Cleanup.
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await;
}
