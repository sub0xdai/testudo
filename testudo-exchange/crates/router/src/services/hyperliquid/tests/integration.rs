//! Testnet integration tests for Hyperliquid native SDK.
//!
//! These tests validate against the live Hyperliquid testnet API.
//! They are `#[ignore]` by default and only run when credentials are provided:
//!
//! ```bash
//! HL_TESTNET_KEY=<private_key_hex> cargo test hyperliquid -- --ignored
//! ```
//!
//! The `HL_TESTNET_KEY` environment variable must contain an Ethereum private key
//! (hex, with or without 0x prefix) that has testnet funds.

use alloy::signers::local::PrivateKeySigner;
use hyperliquid_sdk_rs::{
    types::ws::{Message, Subscription},
    ExchangeProvider, InfoProvider, Network, RawWsProvider,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use std::time::Duration;

use crate::services::hyperliquid::auth::HyperliquidAuth;
use crate::services::hyperliquid::universe::AssetUniverse;

/// Load testnet private key from environment. Panics with helpful message if absent.
fn testnet_key() -> String {
    std::env::var("HL_TESTNET_KEY").expect(
        "HL_TESTNET_KEY environment variable required. \
         Set it to a hex-encoded Ethereum private key with testnet funds.",
    )
}

/// Construct a signer from the testnet key.
fn testnet_auth() -> HyperliquidAuth {
    let key = testnet_key();
    HyperliquidAuth::from_credentials("", &key)
        .expect("Failed to construct HyperliquidAuth from HL_TESTNET_KEY")
}

// ─── FR-4: Auth validation ───────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn testnet_construct_signer_and_derive_address() {
    let key = testnet_key();
    let signer: PrivateKeySigner = key.parse().expect("Invalid private key");
    let address = signer.address();

    // Address should be a valid non-zero Ethereum address
    assert!(!address.is_zero(), "Derived address should not be zero");

    // HyperliquidAuth should construct successfully
    let auth = HyperliquidAuth::from_credentials("", &key).unwrap();
    assert_eq!(auth.address, address);

    tracing::info!("Testnet address: {}", address);
}

// ─── FR-5: Universe fetch & BTC resolution ───────────────────────────

#[tokio::test]
#[ignore]
async fn testnet_fetch_meta_and_resolve_btc() {
    let universe = AssetUniverse::fetch(Network::Testnet)
        .await
        .expect("Failed to fetch testnet asset universe");

    // Testnet should have assets
    assert!(!universe.is_empty(), "Universe should not be empty");
    assert!(universe.len() > 10, "Testnet should have >10 assets");

    // BTC should be resolvable (index 0 on both mainnet and testnet)
    let btc_index = universe.resolve("BTC").expect("BTC should be in testnet universe");
    assert_eq!(btc_index, 0, "BTC should have asset index 0");

    // ETH should also exist
    let eth_index = universe.resolve("ETH").expect("ETH should be in testnet universe");
    assert_eq!(eth_index, 1, "ETH should have asset index 1");

    // sz_decimals should be reasonable
    let btc_decimals = universe.sz_decimals("BTC").unwrap();
    assert!(btc_decimals <= 8, "BTC sz_decimals should be <= 8");

    tracing::info!(
        "Universe loaded: {} assets, BTC index={}, ETH index={}, BTC sz_decimals={}",
        universe.len(),
        btc_index,
        eth_index,
        btc_decimals
    );
}

// ─── FR-6: Balance query ─────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn testnet_query_balance() {
    let auth = testnet_auth();
    let info = InfoProvider::new(Network::Testnet);

    let state = info
        .user_state(auth.address)
        .await
        .expect("Failed to fetch user state from testnet");

    // Parse account value — should be a valid decimal (may be zero if unfunded)
    let account_value =
        Decimal::from_str(&state.margin_summary.account_value).expect("Invalid account_value");

    // Account value should be non-negative
    assert!(
        account_value >= Decimal::ZERO,
        "Account value should be >= 0, got {}",
        account_value
    );

    tracing::info!(
        "Testnet balance: account_value={}, margin_used={}",
        state.margin_summary.account_value,
        state.margin_summary.total_margin_used
    );
}

// ─── FR-7: Order lifecycle (place → verify → cancel → verify) ───────

#[tokio::test]
#[ignore]
async fn testnet_order_lifecycle() {
    let auth = testnet_auth();
    let universe = AssetUniverse::fetch(Network::Testnet)
        .await
        .expect("Failed to fetch universe");

    let exchange = ExchangeProvider::testnet(auth.signer.clone());
    let info = InfoProvider::new(Network::Testnet);

    // Check if account has funds
    let state = info.user_state(auth.address).await.expect("user_state failed");
    let balance = Decimal::from_str(&state.margin_summary.account_value).unwrap_or_default();
    if balance < Decimal::new(1, 0) {
        tracing::warn!(
            "Testnet account has insufficient balance ({}), skipping order lifecycle",
            balance
        );
        return;
    }

    let btc_index = universe.resolve("BTC").expect("BTC not found");
    let sz_decimals = universe.sz_decimals("BTC").unwrap();

    // Place a limit buy at a very low price (won't fill)
    let sz = format!("{:.prec$}", 0.001, prec = sz_decimals as usize);
    let order = hyperliquid_sdk_rs::types::OrderRequest::limit(
        btc_index,
        true,      // is_buy
        "1000.0",  // very low price — won't fill
        &sz,
        "Gtc",
    );

    let response = exchange
        .place_order(&order)
        .await
        .expect("Failed to place testnet order");

    tracing::info!("Place order response: {:?}", response);

    // Extract OID from response
    let exchange_response = response
        .into_result()
        .expect("Exchange returned error on place_order");

    let oid = exchange_response
        .data
        .as_ref()
        .and_then(|d| {
            d.statuses.iter().find_map(|s| match s {
                hyperliquid_sdk_rs::types::ExchangeDataStatus::Resting(r) => Some(r.oid),
                _ => None,
            })
        });

    if let Some(oid) = oid {
        tracing::info!("Order placed with OID: {}", oid);

        // Verify order is open
        let open_orders = info
            .open_orders(auth.address)
            .await
            .expect("Failed to fetch open orders");

        let found = open_orders.iter().any(|o| o.oid == oid);
        assert!(
            found,
            "Placed order (OID={}) should appear in open orders",
            oid
        );

        // Cancel the order
        let cancel_response = exchange
            .cancel_order(btc_index, oid)
            .await
            .expect("Failed to cancel testnet order");

        cancel_response
            .into_result()
            .expect("Exchange returned error on cancel_order");

        tracing::info!("Order {} canceled", oid);

        // Brief delay for propagation
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Verify order is no longer open
        let open_after = info
            .open_orders(auth.address)
            .await
            .expect("Failed to fetch open orders after cancel");

        let still_open = open_after.iter().any(|o| o.oid == oid);
        assert!(
            !still_open,
            "Canceled order (OID={}) should not appear in open orders",
            oid
        );

        tracing::info!("Order lifecycle complete: place → verify → cancel → verify");
    } else {
        tracing::warn!(
            "Order did not return a Resting status — may have been rejected. Response: {:?}",
            exchange_response
        );
    }
}

// ─── FR-8: Position query ────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn testnet_fetch_positions() {
    let auth = testnet_auth();
    let info = InfoProvider::new(Network::Testnet);

    // Positions are part of user_state response
    let state = info
        .user_state(auth.address)
        .await
        .expect("Failed to fetch user state from testnet");

    let positions = &state.asset_positions;

    // Positions may be empty (no open positions) — that's fine.
    // We just verify the API call succeeds and returns a valid response.
    tracing::info!("Testnet positions: {} entries", positions.len());

    for pos in positions {
        tracing::info!(
            "  Position: coin={}, size={}, entry_px={}, unrealized_pnl={}",
            pos.position.coin,
            pos.position.szi,
            pos.position.entry_px.as_deref().unwrap_or("0"),
            pos.position.unrealized_pnl,
        );
    }
}

// ─── FR-9: WebSocket subscription ────────────────────────────────────

#[tokio::test]
#[ignore]
async fn testnet_websocket_order_updates() {
    let auth = testnet_auth();

    // Connect to testnet WebSocket
    let mut ws = RawWsProvider::connect(Network::Testnet)
        .await
        .expect("Failed to connect to testnet WebSocket");

    // Subscribe to order updates for our address
    let subscription = Subscription::OrderUpdates {
        user: auth.address,
    };

    let (_sub_id, mut receiver) = ws
        .subscribe(subscription)
        .await
        .expect("Failed to subscribe to order updates");

    tracing::info!("WebSocket subscribed");

    // Start reading messages (consumes the provider)
    let reader_handle = tokio::spawn(async move {
        let _ = ws.start_reading().await;
    });

    // Wait briefly for any messages (there may be none if no orders are active)
    let timeout_result = tokio::time::timeout(Duration::from_secs(3), receiver.recv()).await;

    match timeout_result {
        Ok(Some(msg)) => {
            // Verify message format
            match msg {
                Message::OrderUpdates(updates) => {
                    tracing::info!("Received {} order update(s)", updates.data.len());
                    for update in &updates.data {
                        // Verify expected fields exist
                        assert!(!update.order.coin.is_empty(), "coin should not be empty");
                        assert!(!update.order.side.is_empty(), "side should not be empty");
                        assert!(update.order.oid > 0, "oid should be positive");
                        tracing::info!(
                            "  Update: coin={}, side={}, status={}",
                            update.order.coin,
                            update.order.side,
                            update.status
                        );
                    }
                }
                other => {
                    tracing::info!("Received non-order message: {:?}", other);
                }
            }
        }
        Ok(None) => {
            tracing::info!("WebSocket channel closed (no active orders — expected)");
        }
        Err(_) => {
            tracing::info!(
                "No WebSocket messages within 3s timeout (no active orders — expected)"
            );
        }
    }

    // Clean up
    reader_handle.abort();
    tracing::info!("WebSocket subscription test complete");
}
