# Specification: Onboarding Status Endpoint — Single-Call Agent Readiness

**Spec ID:** AGENT-06-onboarding-status
**Date:** 2026-05-30
**Status:** Draft
**Class:** Feature / API
**Priority:** P1 — collapses 3-call agent discovery dance into 1 call; unlocks conversational onboarding
**Depends on:** AGENT-01-signal-endpoint
**Series:** AGENT-06 through AGENT-07 (Agent Onboarding UX)

---

## Problem Statement

When an AI agent (Hermes, OpenClaw, pi) connects to Testudo on behalf of a new user, it must discover the user's onboarding state through a multi-step script: call `GET /exchanges/accounts`, `GET /risk-config`, optionally `GET /journal/agent/summary`, interpret each empty/null response, and branch accordingly. This is documented in `AGENT_TRADING.md` Section 0 as a decision tree requiring 3+ round trips.

Industry research confirms the superior pattern: a single `GET /onboarding/status` endpoint that returns `{is_ready: false, blockers: [...]}` with prescriptive `next_step` guidance. Claude Code, Cursor, and Copilot all expose readiness endpoints that agents poll before starting work. Virtuals Protocol and ElizaOS return structured prerequisite arrays that the LLM agent converts to natural language in the chat UX.

The agent's first-contact experience should be:

```
Agent: GET /onboarding/status → { next_step: "connect_exchange", missing: [...], available_exchanges: [...] }
Agent (to user): "I need to connect to an exchange. Testudo supports Binance, Bybit, OKX, Hyperliquid, and 5 others. Which one?"
```

Not:

```
Agent: GET /exchanges/accounts → [] → GET /exchanges → [...] → interpret → ask user
```

The frontend already does this client-side (`useOnboardingState.ts` tracks Connect Wallet → Add Exchanges → Import History → Pair Extension as a 4-step wizard). The backend needs a server-side equivalent for headless agents.

---

## User Stories

- **As an AI agent**, I want a single endpoint that tells me exactly what's missing before I can trade, so that I can have a natural conversation with the user instead of running a multi-step discovery script.
- **As a user onboarding through an agent**, I want the agent to quickly assess my setup and guide me through only what's needed, so that I don't have to repeat information across multiple calls.
- **As a platform operator**, I want agent onboarding to be fast and conversational, so that users don't abandon setup mid-flow.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `GET /api/v1/onboarding/status` returns `is_ready`, `next_step`, `missing`, and context objects without additional queries | High | Router |
| FR-2 | `next_step` is a prescriptive enum: `"authenticate"`, `"connect_exchange"`, `"approve_agent_wallet"`, `"configure_risk"`, `"ready_to_trade"` | High | Router |
| FR-3 | `missing` array lists human-readable descriptions of each blocker (e.g., `"No exchange account connected"`) | High | Router |
| FR-4 | When `next_step` is `"connect_exchange"`, response includes `available_exchanges` with `id`, `name`, `type`, `required_credentials` — so the agent can present options without a second `GET /exchanges` call | High | Router |
| FR-5 | When user has a pending agent wallet awaiting EIP-712 signature, `next_step` is `"approve_agent_wallet"` with the `account_id` and `agent_address` inline | Medium | Router |
| FR-6 | Response includes `has_trades` (boolean) so agent knows whether to import history or display empty-state messaging | Low | Router |
| FR-7 | Endpoint requires authentication (SIWE bearer token) — same auth middleware as all other endpoints | High | Router |
| FR-8 | Response includes current `risk_config` summary (account_risk_percent, max_leverage, daily_drawdown_limit, stop_loss_required) so agent can surface settings to user | Medium | Router |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Route, types, and handler skeleton. Returns hardcoded `next_step: "ready_to_trade"` for authenticated user. | 200 response shape, auth guard works |
| CP-2 | Integrate `ExchangeAccountRepository::list_by_user()`. Compute `next_step` from accounts state. | Returns `"connect_exchange"` when no accounts, includes `available_exchanges` |
| CP-3 | Add agent wallet pending-approval detection. Read `auth_mode` and `requires_reauthorization` from exchange accounts. | Returns `"approve_agent_wallet"` with `account_id` + `agent_address` when agent wallet pending |
| CP-4 | Add `has_trades` check and `risk_config` summary. Wire up full state computation. | Complete response with all fields populated |

### Key Types

```rust
// crates/router/src/models/onboarding.rs — NEW

/// Response from GET /api/v1/onboarding/status.
#[derive(Debug, Serialize)]
pub struct OnboardingStatus {
    /// True when the user has everything needed to start trading.
    pub is_ready: bool,

    /// Prescriptive next action for the agent to guide the user through.
    pub next_step: OnboardingStep,

    /// Human-readable descriptions of what's missing.
    /// Empty when is_ready is true.
    pub missing: Vec<String>,

    // ── Context objects (present when relevant) ──

    /// Available exchanges with credential requirements.
    /// Present when next_step is "connect_exchange".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_exchanges: Option<Vec<ExchangeOption>>,

    /// Pending agent wallet that needs EIP-712 approval.
    /// Present when next_step is "approve_agent_wallet".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_agent_wallet: Option<PendingAgentWallet>,

    /// Whether the user has any trade history at all.
    pub has_trades: bool,

    /// Current risk configuration (so agent can surface settings).
    /// None if risk config not yet initialized (new user).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_config: Option<RiskConfigSummary>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingStep {
    /// User is not authenticated. Agent should guide through SIWE flow.
    Authenticate,
    /// No exchange account connected. Agent should present exchange options.
    ConnectExchange,
    /// Agent wallet initialized but not approved. Agent should guide through EIP-712 signing.
    ApproveAgentWallet,
    /// Risk config is at defaults. Agent should offer to customize.
    ConfigureRisk,
    /// Everything is ready. Agent can start trading.
    ReadyToTrade,
}

#[derive(Debug, Serialize)]
pub struct ExchangeOption {
    pub id: String,              // "binance", "bybit", "hyperliquid", etc.
    pub name: String,            // "Binance", "Bybit", "Hyperliquid"
    #[serde(rename = "type")]
    pub exchange_type: String,   // "cex" or "dex"
    pub required_credentials: Vec<String>,  // ["api_key", "secret"] or ["wallet"]
}

#[derive(Debug, Serialize)]
pub struct PendingAgentWallet {
    pub account_id: Uuid,
    pub agent_address: String,
    pub wallet_address: String,
    /// True if an existing agent wallet needs re-authorization.
    pub requires_reauthorization: bool,
}

#[derive(Debug, Serialize)]
pub struct RiskConfigSummary {
    pub account_risk_percent: String,  // e.g. "2.0"
    pub max_leverage: i32,
    pub daily_drawdown_limit: Option<String>,
    pub stop_loss_required: bool,
}
```

### State Computation Logic

```rust
// crates/router/src/services/onboarding.rs — NEW

pub async fn compute_onboarding_status(
    db: &PgPool,
    user_id: Uuid,
) -> Result<OnboardingStatus, AppError> {
    let repo = ExchangeAccountRepository::new(db.clone());

    // 1. Check exchange accounts
    let accounts = repo.list_by_user(user_id).await?;

    if accounts.is_empty() {
        return Ok(OnboardingStatus {
            is_ready: false,
            next_step: OnboardingStep::ConnectExchange,
            missing: vec!["No exchange account connected. You need to add an exchange before trading.".into()],
            available_exchanges: Some(build_exchange_list()),
            pending_agent_wallet: None,
            has_trades: false,
            risk_config: None,
        });
    }

    // 2. Check for pending agent wallet approvals
    for acct in &accounts {
        if acct.auth_mode == "agent_wallet" {
            if acct.requires_reauthorization.unwrap_or(false) || !acct.is_active {
                let wallet = load_agent_wallet_detail(db, acct.id).await?;
                return Ok(OnboardingStatus {
                    is_ready: false,
                    next_step: OnboardingStep::ApproveAgentWallet,
                    missing: vec![format!(
                        "Agent wallet {} needs EIP-712 approval to trade on Hyperliquid.",
                        wallet.agent_address
                    )],
                    available_exchanges: None,
                    pending_agent_wallet: Some(wallet),
                    has_trades: false, // checked below
                    risk_config: None, // checked below
                });
            }
        }
    }

    // 3. Check trades (for import-history display logic)
    let trade_count = count_trades_for_user(db, user_id).await?;
    let has_trades = trade_count > 0;

    // 4. Check risk config
    let risk = load_risk_config(db, user_id).await?;
    let is_default_risk = risk.is_default();

    if is_default_risk {
        return Ok(OnboardingStatus {
            is_ready: true, // can trade — defaults are conservative
            next_step: OnboardingStep::ConfigureRisk,
            missing: vec!["Risk config is at conservative defaults. Consider customizing.".into()],
            available_exchanges: None,
            pending_agent_wallet: None,
            has_trades,
            risk_config: Some(RiskConfigSummary::from(risk)),
        });
    }

    // 5. All clear
    Ok(OnboardingStatus {
        is_ready: true,
        next_step: OnboardingStep::ReadyToTrade,
        missing: vec![],
        available_exchanges: None,
        pending_agent_wallet: None,
        has_trades,
        risk_config: Some(RiskConfigSummary::from(risk)),
    })
}
```

### Route Wiring

```rust
// crates/router/src/routes/mod.rs — add:
cfg.route("/api/v1/onboarding/status", web::get().to(onboarding::get_status));
```

### Paved Roads

- `ExchangeAccountRepository::list_by_user()` in `sqlx_postgres/src/repositories/api_keys.rs` — already returns `auth_mode`, `is_active`, `requires_reauthorization`, `wallet_address`. No schema changes needed.
- `GET /api/v1/exchanges` route in `routes/exchanges.rs` — already returns the full exchange list with `type`, `required_credentials`. Extract the list builder into a shared helper.
- `RiskConfig` model and `load_risk_config()` — exists in `risk/service.rs` and `risk/config.rs`. Add `is_default()` method.
- `AuthenticatedUser` extractor in `middleware/auth.rs` — already guards all routes. Reuse verbatim.
- `useOnboardingState.ts` in `testudo-journal/src/components/onboarding/` — existing client-side equivalent. This spec implements the server-side version for headless agents.

### Files

- `crates/router/src/routes/onboarding.rs` — **NEW** — GET handler, calls `compute_onboarding_status()`
- `crates/router/src/models/onboarding.rs` — **NEW** — `OnboardingStatus`, `OnboardingStep`, `ExchangeOption`, `PendingAgentWallet`, `RiskConfigSummary` types
- `crates/router/src/services/onboarding.rs` — **NEW** — `compute_onboarding_status()` orchestrator
- `crates/router/src/routes/mod.rs` — add route registration
- `crates/router/src/routes/exchanges.rs` — extract `build_exchange_list()` into shared helper (or call existing handler logic)
- `crates/common_utils/src/risk/config.rs` — add `RiskConfig::is_default()` method
- `AGENT_TRADING.md` — update Section 0 to reference `GET /onboarding/status` as the preferred first call

### Dependencies Added

None. Reuses existing crates exclusively.

---

## Acceptance Criteria

- [ ] `GET /api/v1/onboarding/status` returns 200 with `{is_ready: false, next_step: "connect_exchange", available_exchanges: [...]}` for a new user with no accounts
- [ ] Returns `{is_ready: false, next_step: "approve_agent_wallet", pending_agent_wallet: {...}}` when agent wallet is pending EIP-712 approval
- [ ] Returns `{is_ready: true, next_step: "ready_to_trade"}` for a fully-configured user with active exchange accounts
- [ ] Returns `{is_ready: true, next_step: "configure_risk"}` when risk is at defaults but everything else ready
- [ ] `has_trades` is `true` when user has trade history, `false` otherwise
- [ ] `risk_config` includes `account_risk_percent`, `max_leverage`, `daily_drawdown_limit`, `stop_loss_required`
- [ ] Unauthenticated requests return 401
- [ ] `cargo clippy --all-targets && cargo test` passes in testudo-exchange
- [ ] Unit tests cover all 5 `OnboardingStep` variants
- [ ] `AGENT_TRADING.md` Section 0 updated to reference the new endpoint

---

## Risks

1. **Stale agent wallet state** — The `requires_reauthorization` flag might be out of sync with on-chain state if an agent was revoked externally on Hyperliquid. Mitigation: the `verify_registration()` function in `agent_approval.rs` already does on-chain verification. Call it before returning `"approve_agent_wallet"` to confirm the agent truly needs re-approval.
2. **Exchange list duplication** — Building the exchange list in two places (this endpoint + `GET /exchanges`) risks drift. Mitigation: extract the exchange list builder into a shared function in `common_utils` or a `services/exchanges.rs` helper.
3. **Overfetching for simple state** — The endpoint queries accounts + trades + risk config in one call, which could be slow. Mitigation: all three are indexed queries on indexed columns (user_id). Should be sub-10ms in production. If performance becomes an issue, add a materialized `onboarding_state` table updated on account/risk changes.

---

## Completion Signal

This spec is complete when:
1. `GET /api/v1/onboarding/status` returns the full structured response for all 5 states
2. All 10 acceptance criteria met
3. `cargo clippy --all-targets && cargo test` passes
4. `AGENT_TRADING.md` Section 0 updated
5. Code committed to master
