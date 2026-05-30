//! Onboarding status computation — single-call agent readiness.
//!
//! Collapses multi-step discovery into one endpoint for AI agents.

// @anchor exchange:router:onboarding-service
// @tags api

use sqlx::PgPool;
use uuid::Uuid;

use common_utils::risk::PgRiskConfigStorage;
use common_utils::services::PgCacheService;

use crate::models::onboarding::{
    ExchangeOption, OnboardingStatus, OnboardingStep, PendingAgentWallet,
};

/// Row from exchange_accounts for onboarding status.
#[derive(Debug, sqlx::FromRow)]
struct AccountRow {
    #[allow(dead_code)]
    id: Uuid,
    auth_mode: String,
    wallet_address: Option<String>,
    is_active: bool,
    agent_approved_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Build the full exchange list as typed `ExchangeOption` structs.
/// Shared between GET /exchanges and GET /onboarding/status.
pub fn build_exchange_list() -> Vec<ExchangeOption> {
    vec![
        ExchangeOption {
            id: "binance".into(),
            name: "Binance".into(),
            exchange_type: "cex".into(),
            required_credentials: vec!["api_key".into(), "secret".into()],
        },
        ExchangeOption {
            id: "woo".into(),
            name: "WOO X".into(),
            exchange_type: "cex".into(),
            required_credentials: vec!["api_key".into(), "secret".into()],
        },
        ExchangeOption {
            id: "bybit".into(),
            name: "Bybit".into(),
            exchange_type: "cex".into(),
            required_credentials: vec!["api_key".into(), "secret".into()],
        },
        ExchangeOption {
            id: "okx".into(),
            name: "OKX".into(),
            exchange_type: "cex".into(),
            required_credentials: vec!["api_key".into(), "secret".into(), "passphrase".into()],
        },
        ExchangeOption {
            id: "bitget".into(),
            name: "Bitget".into(),
            exchange_type: "cex".into(),
            required_credentials: vec!["api_key".into(), "secret".into(), "passphrase".into()],
        },
        ExchangeOption {
            id: "gate".into(),
            name: "Gate.io".into(),
            exchange_type: "cex".into(),
            required_credentials: vec!["api_key".into(), "secret".into()],
        },
        ExchangeOption {
            id: "phemex".into(),
            name: "Phemex".into(),
            exchange_type: "cex".into(),
            required_credentials: vec!["api_key".into(), "secret".into()],
        },
        ExchangeOption {
            id: "blofin".into(),
            name: "BloFin".into(),
            exchange_type: "cex".into(),
            required_credentials: vec!["api_key".into(), "secret".into(), "passphrase".into()],
        },
        ExchangeOption {
            id: "hyperliquid".into(),
            name: "Hyperliquid".into(),
            exchange_type: "dex".into(),
            required_credentials: vec!["wallet".into()],
        },
    ]
}

/// Count trades for a user. Returns 0 on error (treats missing history as empty).
async fn count_trades_for_user(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: (Option<i64>,) = sqlx::query_as(
        "SELECT COUNT(*) FROM trade_groups WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0.unwrap_or(0))
}

/// Compute the user's onboarding status from live data.
///
/// Order of checks (short-circuit on first blocker):
/// 1. No exchange accounts → ConnectExchange with available_exchanges
/// 2. Pending agent wallet → ApproveAgentWallet
/// 3. Risk at defaults → ConfigureRisk (CP-4)
/// 4. All clear → ReadyToTrade
pub async fn compute_onboarding_status(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<OnboardingStatus, actix_web::Error> {
    // 1. Query exchange accounts with auth_mode
    let accounts: Vec<AccountRow> = sqlx::query_as(
        "SELECT id, auth_mode, wallet_address, is_active, agent_approved_at \
         FROM exchange_accounts WHERE user_id = $1 \
         AND (is_active = true OR (auth_mode = 'agent_wallet' AND is_active = false)) \
         ORDER BY is_active DESC, created_at DESC"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!(
            "Failed to query exchange accounts: {}", e
        ))
    })?;

    if accounts.is_empty() {
        return Ok(OnboardingStatus {
            is_ready: false,
            next_step: OnboardingStep::ConnectExchange,
            missing: vec![
                "No exchange account connected. You need to add an exchange before trading."
                    .into(),
            ],
            available_exchanges: Some(build_exchange_list()),
            pending_agent_wallet: None,
            has_trades: false,
            risk_config: None,
        });
    }

    // 2. Check for pending agent wallet approvals.
    //    An agent wallet is pending if:
    //    - auth_mode is 'agent_wallet'
    //    - is_active is false (needs re-approval)
    //    OR agent_approved_at is older than 30 days (expired)
    for acct in &accounts {
        if acct.auth_mode == "agent_wallet" && !acct.is_active {
            let wallet_addr = acct.wallet_address.clone().unwrap_or_default();
            // agent_address for HL agent wallets is stored in the api_key field
            // (see the exchange accounts handler). For onboarding status, we use
            // the wallet_address — the agent address is generated server-side.
            return Ok(OnboardingStatus {
                is_ready: false,
                next_step: OnboardingStep::ApproveAgentWallet,
                missing: vec![format!(
                    "Agent wallet for {} needs EIP-712 approval to trade on Hyperliquid.",
                    wallet_addr
                )],
                available_exchanges: None,
                pending_agent_wallet: Some(PendingAgentWallet {
                    account_id: acct.id,
                    agent_address: wallet_addr.clone(),
                    wallet_address: wallet_addr,
                    requires_reauthorization: true,
                }),
                has_trades: false,
                risk_config: None,
            });
        }
    }

    // 3. Check trades
    let trade_count = count_trades_for_user(pool, user_id).await.unwrap_or(0);
    let has_trades = trade_count > 0;

    // 4. Load risk config and check if it's at defaults.
    let cache = PgCacheService::new(pool.clone());
    let storage = PgRiskConfigStorage::new(cache);
    let risk_config = storage
        .load_or_default(user_id)
        .await
        .unwrap_or_default();

    let risk_summary = crate::models::onboarding::RiskConfigSummary::from(risk_config.clone());
    let is_default = risk_config.is_default();

    if is_default {
        return Ok(OnboardingStatus {
            is_ready: true, // can trade — defaults are conservative
            next_step: OnboardingStep::ConfigureRisk,
            missing: vec![
                "Risk config is at conservative defaults. Consider customizing.".into(),
            ],
            available_exchanges: None,
            pending_agent_wallet: None,
            has_trades,
            risk_config: Some(risk_summary),
        });
    }

    // 5. All clear — accounts exist, active, risk customized.
    Ok(OnboardingStatus {
        is_ready: true,
        next_step: OnboardingStep::ReadyToTrade,
        missing: vec![],
        available_exchanges: None,
        pending_agent_wallet: None,
        has_trades,
        risk_config: Some(risk_summary),
    })
}
