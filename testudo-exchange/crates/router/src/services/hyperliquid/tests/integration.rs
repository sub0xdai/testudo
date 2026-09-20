//! Testnet integration tests for the Hyperliquid native adapter.
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

use alloy::primitives::Address;
use alloy::signers::local::PrivateKeySigner;
use futures_util::StreamExt;
use hypersdk::hypercore::{
    self,
    types::{Incoming, OrderRequest, OrderResponseStatus, OrderTypePlacement, Subscription},
    ws::Event,
    ActionError, BatchCancel, BatchOrder, Cancel, Chain, Cloid, HttpClient, NonceHandler,
    OrderGrouping, TimeInForce,
};
use rust_decimal::Decimal;
use std::str::FromStr;
use std::time::Duration;

use crate::services::hyperliquid::auth::HyperliquidAuth;
use crate::services::hyperliquid::exchange_api::round_sz;
use crate::services::hyperliquid::universe::AssetUniverse;
use crate::types::hl_network::HlNetwork;

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

fn testnet_client() -> HttpClient {
    HttpClient::new(Chain::Testnet)
}

/// Place a single order through `hypersdk`, the way the router does.
async fn place(
    client: &HttpClient,
    auth: &HyperliquidAuth,
    nonces: &NonceHandler,
    order: OrderRequest,
) -> Result<Vec<OrderResponseStatus>, ActionError<Cloid>> {
    client
        .place(
            &auth.signer,
            BatchOrder {
                orders: vec![order],
                grouping: OrderGrouping::Na,
                builder: None,
            },
            nonces.next(),
            None,
            None,
        )
        .await
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
    let universe = AssetUniverse::fetch(HlNetwork::Testnet)
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
    let client = testnet_client();

    let state = client
        .clearinghouse_state(auth.address, None)
        .await
        .expect("Failed to fetch clearinghouse state from testnet");

    let account_value = state.margin_summary.account_value;

    // Account value should be non-negative
    assert!(
        account_value >= Decimal::ZERO,
        "Account value should be >= 0, got {}",
        account_value
    );

    tracing::info!(
        "Testnet balance: account_value={}, margin_used={}",
        account_value,
        state.margin_summary.total_margin_used
    );
}

// ─── FR-7: Order lifecycle (place → verify → cancel → verify) ───────

#[tokio::test]
#[ignore]
async fn testnet_order_lifecycle() {
    let auth = testnet_auth();
    let universe = AssetUniverse::fetch(HlNetwork::Testnet)
        .await
        .expect("Failed to fetch universe");

    let client = testnet_client();
    let nonces = NonceHandler::default();

    // Check if account has funds
    let state = client
        .clearinghouse_state(auth.address, None)
        .await
        .expect("clearinghouse_state failed");
    let balance = state.margin_summary.account_value;
    if balance < Decimal::ONE {
        tracing::warn!(
            "Testnet account has insufficient balance ({}), skipping order lifecycle",
            balance
        );
        return;
    }

    let btc_index = universe.resolve("BTC").expect("BTC not found");
    let sz_decimals = universe.sz_decimals("BTC").unwrap();

    // Place a limit buy at a very low price (won't fill)
    let order = OrderRequest {
        asset: btc_index as usize,
        is_buy: true,
        limit_px: Decimal::from_str("1000.0").expect("limit price"),
        sz: round_sz(Decimal::from_str("0.001").expect("size"), sz_decimals),
        reduce_only: false,
        order_type: OrderTypePlacement::Limit {
            tif: TimeInForce::Gtc,
        },
        cloid: Cloid::ZERO,
    };

    let statuses = place(&client, &auth, &nonces, order)
        .await
        .expect("Failed to place testnet order");

    tracing::info!("Place order response: {:?}", statuses);

    let oid = statuses.iter().find_map(OrderResponseStatus::oid);

    if let Some(oid) = oid {
        tracing::info!("Order placed with OID: {}", oid);

        // Verify order is open
        let open_orders = client
            .open_orders(auth.address, None)
            .await
            .expect("Failed to fetch open orders");

        assert!(
            open_orders.iter().any(|o| o.oid == oid),
            "Placed order (OID={}) should appear in open orders",
            oid
        );

        // Cancel the order
        client
            .cancel(
                &auth.signer,
                BatchCancel {
                    cancels: vec![Cancel {
                        asset: btc_index as usize,
                        oid,
                    }],
                    fast: false,
                },
                nonces.next(),
                None,
                None,
            )
            .await
            .expect("Failed to cancel testnet order");

        tracing::info!("Order {} canceled", oid);

        // Brief delay for propagation
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Verify order is no longer open
        let open_after = client
            .open_orders(auth.address, None)
            .await
            .expect("Failed to fetch open orders after cancel");

        assert!(
            !open_after.iter().any(|o| o.oid == oid),
            "Canceled order (OID={}) should not appear in open orders",
            oid
        );

        tracing::info!("Order lifecycle complete: place → verify → cancel → verify");
    } else {
        tracing::warn!(
            "Order did not return a Resting status — may have been rejected. Response: {:?}",
            statuses
        );
    }
}

// ─── FR-8: Position query ────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn testnet_fetch_positions() {
    let auth = testnet_auth();
    let client = testnet_client();

    let state = client
        .clearinghouse_state(auth.address, None)
        .await
        .expect("Failed to fetch clearinghouse state from testnet");

    let positions = &state.asset_positions;

    // Positions may be empty (no open positions) — that's fine.
    // We just verify the API call succeeds and returns a valid response.
    tracing::info!("Testnet positions: {} entries", positions.len());

    for pos in positions {
        tracing::info!(
            "  Position: coin={}, size={}, entry_px={:?}, unrealized_pnl={}",
            pos.position.coin,
            pos.position.szi,
            pos.position.entry_px,
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
    let mut ws = hypercore::testnet_ws();
    ws.subscribe(Subscription::OrderUpdates { user: auth.address });

    tracing::info!("WebSocket subscribed");

    // Wait briefly for any messages (there may be none if no orders are active).
    // The stream also carries connection state events, so a message is not guaranteed.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            tracing::info!("No WebSocket messages within 3s timeout (no active orders — expected)");
            break;
        }

        match tokio::time::timeout(remaining, ws.next()).await {
            Ok(Some(Event::Message(Incoming::OrderUpdates(updates)))) => {
                tracing::info!("Received {} order update(s)", updates.len());
                for update in &updates {
                    // Verify expected fields exist
                    assert!(!update.order.coin.is_empty(), "coin should not be empty");
                    assert!(update.order.oid > 0, "oid should be positive");
                    tracing::info!(
                        "  Update: coin={}, side={:?}, status={:?}",
                        update.order.coin,
                        update.order.side,
                        update.status
                    );
                }
                break;
            }
            Ok(Some(Event::Message(other))) => {
                tracing::info!("Received non-order message: {:?}", other);
            }
            Ok(Some(Event::Connected)) | Ok(Some(Event::Disconnected)) => {}
            Ok(None) => {
                tracing::info!("WebSocket channel closed (no active orders — expected)");
                break;
            }
            Err(_) => {
                tracing::info!(
                    "No WebSocket messages within 3s timeout (no active orders — expected)"
                );
                break;
            }
        }
    }

    tracing::info!("WebSocket subscription test complete");
}

// ─── FR-3 / FR-4: Spot to perp class transfer ────────────────────────

/// Spot USDC and perp USDC, as the exchange reports them right now.
///
/// Spot comes from `user_balances`, perp from `clearinghouse_state.withdrawable`
/// (available USDC, so an open position's unrealised PnL cannot move the number).
// @waiver TB-01: a test-only read of two balances. It has nothing to assert; the
// assertions live in the test that calls it.
async fn usdc_split(client: &HttpClient, user: Address) -> (Decimal, Decimal) {
    let spot = client
        .user_balances(user)
        .await
        .expect("Failed to fetch spot balances")
        .into_iter()
        .find(|b| b.coin == "USDC")
        .map(|b| b.total)
        .unwrap_or(Decimal::ZERO);

    let perp = client
        .clearinghouse_state(user, None)
        .await
        .expect("Failed to fetch clearinghouse state")
        .withdrawable;

    (spot, perp)
}

/// Whether two USDC amounts agree to the precision the exchange reports.
///
/// A class transfer is free and instant, so the delta should be exact. The
/// tolerance bounds the comparison against wire precision, not against fees.
fn usdc_close(left: Decimal, right: Decimal) -> bool {
    (left - right).abs() <= Decimal::new(1, 6)
}

/// FR-3 and FR-4: both transfer directions move funds between spot and perp.
///
/// One round trip covers both criteria. It starts from whichever class actually
/// holds the funds, so it works on an account funded in either one, and the
/// second leg then exercises the opposite direction. The assertion is the
/// balances, not the call returning `Ok`: a transfer that silently moves nothing
/// returns `Ok` too, which is the defect this spec exists to fix.
#[tokio::test]
#[ignore]
async fn testnet_class_transfer_round_trip() {
    let auth = testnet_auth();
    let client = testnet_client();
    let user = auth.address;
    let nonces = NonceHandler::default();

    // Small enough to work on a lightly funded testnet account. Override with
    // HL_TESTNET_TRANSFER_USDC if the exchange rejects this size.
    let amount = match std::env::var("HL_TESTNET_TRANSFER_USDC") {
        Ok(raw) => Decimal::from_str(&raw).expect("HL_TESTNET_TRANSFER_USDC must parse"),
        Err(_) => Decimal::from_str("0.2").expect("parse amount"),
    };
    assert!(
        amount > Decimal::ZERO,
        "HL_TESTNET_TRANSFER_USDC must be positive, got {amount}"
    );

    let (spot_start, perp_start) = usdc_split(&client, user).await;
    tracing::info!(%spot_start, %perp_start, "class transfer: starting split");

    let to_perp_first = spot_start >= amount;
    assert!(
        to_perp_first || perp_start >= amount,
        "need at least {amount} USDC in one class to transfer: spot={spot_start}, perp={perp_start}"
    );

    client
        .usd_class_transfer(&auth.signer, amount, to_perp_first, nonces.next(), None, None)
        .await
        .expect("first class transfer was rejected");

    let (spot_mid, perp_mid) = usdc_split(&client, user).await;
    let (want_spot, want_perp) = if to_perp_first {
        (spot_start - amount, perp_start + amount)
    } else {
        (spot_start + amount, perp_start - amount)
    };
    tracing::info!(%spot_mid, %perp_mid, to_perp = to_perp_first, "class transfer: first leg");
    assert!(
        usdc_close(spot_mid, want_spot) && usdc_close(perp_mid, want_perp),
        "first leg moved the wrong amounts: wanted spot={want_spot} perp={want_perp}, \
         got spot={spot_mid} perp={perp_mid}"
    );

    client
        .usd_class_transfer(&auth.signer, amount, !to_perp_first, nonces.next(), None, None)
        .await
        .expect("return class transfer was rejected");

    let (spot_end, perp_end) = usdc_split(&client, user).await;
    assert!(
        usdc_close(spot_end, spot_start) && usdc_close(perp_end, perp_start),
        "return leg did not restore the split: started spot={spot_start} perp={perp_start}, \
         ended spot={spot_end} perp={perp_end}"
    );
    tracing::info!("class transfer round trip OK in both directions");
}

// ─── FR-5: agent wallet approval ──────────────────────────────

/// FR-5: the account has an approved agent wallet, read from `extraAgents`.
///
/// `extraAgents` is indexed by the master account, so membership can only be
/// asserted when the master address is known. Set `HL_TESTNET_MASTER_ADDRESS`
/// to get the strong check; without it the test records what the account
/// reports and says so, rather than passing on an unverified claim.
#[tokio::test]
#[ignore]
async fn testnet_agent_wallet_is_approved() {
    let auth = testnet_auth();
    let client = testnet_client();

    let own = client
        .api_agents(auth.address)
        .await
        .expect("Failed to query approved agents for the signing key's own address");
    tracing::info!(
        address = %auth.address,
        count = own.len(),
        names = ?own.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
        "agents approved on the signing key's own address"
    );

    let master = match std::env::var("HL_TESTNET_MASTER_ADDRESS") {
        Ok(raw) => raw.parse::<Address>().expect("HL_TESTNET_MASTER_ADDRESS must parse"),
        Err(_) => {
            tracing::warn!(
                "HL_TESTNET_MASTER_ADDRESS unset, so approval is reported but not asserted"
            );
            return;
        }
    };

    let approved = client
        .api_agents(master)
        .await
        .expect("Failed to query approved agents for the master account");
    tracing::info!(%master, count = approved.len(), "agents approved on the master account");
    assert!(
        approved.iter().any(|a| a.address == auth.address),
        "signing key {} is not among the master account's approved agents: {:?}",
        auth.address,
        approved.iter().map(|a| a.address).collect::<Vec<_>>()
    );
}
