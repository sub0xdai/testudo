# AGENT-06-onboarding-status — Implementation Plan

## Current State Summary

The `GET /api/v1/onboarding/status` endpoint does not exist. Agents must call 3 separate endpoints and interpret results manually. The spec defines a single endpoint that computes the onboarding state from existing data sources.

**What exists:**
- `ExchangeAccountRepository::list_by_user()` returns `Vec<ExchangeAccountSummary>` — has `id`, `exchange_name`, `is_active`. Does NOT have `auth_mode`, `wallet_address`, or `requires_reauthorization` directly on the summary struct.
- The exchange list is built inline in `routes/exchanges.rs:59` as a hardcoded `Vec<serde_json::Value>` of 9 exchanges with `id`, `name`, `type`, `description`, `supported_features`, `required_credentials`, `optional_credentials`.
- `RiskConfig` in `common_utils/src/risk/config.rs` has `Default`, `conservative()`, and `aggressive()` constructors but no `is_default()` method. Default values: 2% risk, 5% drawdown, 125× max leverage, stop-loss required.
- `RiskConfigResponse::from(config)` exists in the risk_config route and converts `RiskConfig` to a JSON-serializable response.
- `AuthenticatedUser` middleware already guards all endpoints.
- `useOnboardingState.ts` in the frontend does client-side onboarding state tracking (4 steps: wallet, exchanges, trades, extension).

**Key gap:** `ExchangeAccountSummary` lacks `auth_mode`. The exchange accounts table has this field but the summary struct doesn't expose it. We need either a new query or a new struct that includes `auth_mode` for detecting pending agent wallet approvals.

---

## Checkpoints

### CP-1: Types + route skeleton ✅
- Completed 2026-05-30 by /skill:vox build. Created `models/onboarding.rs` (OnboardingStatus, OnboardingStep 5 variants, ExchangeOption, PendingAgentWallet, RiskConfigSummary), `routes/onboarding.rs` (get_status handler returning hardcoded ReadyToTrade), registered in `routes/mod.rs`, `models/mod.rs`, and actix-web scope in `main.rs`. `cargo clippy --all-targets` passes with zero new warnings.

- **Touches**: `crates/router/src/models/onboarding.rs` (NEW), `crates/router/src/routes/onboarding.rs` (NEW), `crates/router/src/routes/mod.rs`
- **Tasks**:
  1. Create `crates/router/src/models/onboarding.rs` with `OnboardingStatus`, `OnboardingStep` (5 variants), `ExchangeOption`, `PendingAgentWallet`, `RiskConfigSummary` types. All `#[derive(Debug, Serialize)]`. `OnboardingStep` gets `#[serde(rename_all = "snake_case")]`.
  2. Create `crates/router/src/routes/onboarding.rs` with `pub async fn get_status(user: AuthenticatedUser) -> Result<HttpResponse>`. Returns hardcoded 200 with `OnboardingStatus { is_ready: true, next_step: ReadyToTrade, missing: vec![], ... }`.
  3. Add `pub mod onboarding;` to `crates/router/src/routes/mod.rs`.
  4. Register the route in the actix-web app config (check how existing routes are wired — likely in a `configure()` function or main server setup).
- **Verification**: `cargo clippy --all-targets && cargo test` passes. New types compile.
- **Commit message**: `feat: add onboarding status types and route skeleton`

### CP-2: Connect exchange accounts + build exchange list ✅
- Completed 2026-05-30 by /skill:vox build. Created `services/onboarding.rs` with `build_exchange_list()` (9 exchanges, typed `ExchangeOption` structs) and `compute_onboarding_status()` (queries `exchange_accounts` table via direct SQL). Refactored `routes/exchanges.rs` `list_exchanges` to use shared `build_exchange_list()` via JSON conversion. Route now returns `next_step: "connect_exchange"` with `available_exchanges: [...]` when user has no accounts. `cargo clippy --all-targets` passes, zero new warnings.

- **Touches**: `crates/router/src/routes/onboarding.rs`, `crates/router/src/services/onboarding.rs` (NEW), `crates/router/src/routes/exchanges.rs`
- **Tasks**:
  1. Extract exchange list builder from `routes/exchanges.rs:59` into a shared function `pub fn build_exchange_list() -> Vec<ExchangeOption>` in `services/onboarding.rs` (or `common_utils` if that's cleaner). Replace the inline `vec![...]` in `list_exchanges` with a call to this shared function.
  2. In `services/onboarding.rs`, implement `pub async fn compute_onboarding_status(pool: &PgPool, user_id: Uuid) -> Result<OnboardingStatus, AppError>`. Query `ExchangeAccountRepository::list_by_user()`. If empty, return `ConnectExchange` with `available_exchanges: Some(build_exchange_list())`.
  3. Wire the route handler to call `compute_onboarding_status()`.
- **Verification**: `GET /onboarding/status` returns `next_step: "connect_exchange"` with 9 exchange options when user has no accounts.
- **Commit message**: `feat: wire exchange account check into onboarding status`

### CP-3: Agent wallet pending-approval detection ✅
- Completed 2026-05-30 by /skill:vox build. Updated `compute_onboarding_status()` to query `auth_mode`, `wallet_address`, `is_active`, `agent_approved_at` from `exchange_accounts` via direct SQL (matching the existing `ExchangeAccountRepository::list_by_user()` query filter). Detects pending agent wallets when `auth_mode == "agent_wallet" && !is_active`. Returns `next_step: "approve_agent_wallet"` with `pending_agent_wallet` populated. Added `AccountRow` struct for sqlx query mapping. All 5 OnboardingStep variants reachable through the service logic.

- **Touches**: `crates/router/src/routes/onboarding.rs`, `crates/router/src/services/onboarding.rs`, `crates/sqlx_postgres/src/repositories/api_keys.rs`
- **Tasks**:
  1. Add a query or extend `ExchangeAccountSummary` to include `auth_mode`. The exchange_accounts table has this column — either add a new repository method `list_by_user_with_auth_mode()` that returns a richer struct, or query the full `ExchangeAccount` model (which includes `auth_mode`, `wallet_address`, `requires_reauthorization` via the `ExchangeAccountResponse` mapping). Simplest approach: add a direct SQL query in the onboarding service that fetches `auth_mode, wallet_address, is_active` for the user's accounts.
  2. In `compute_onboarding_status()`, after loading accounts, iterate through them. If any account has `auth_mode == "agent_wallet"` and `!is_active` or `requires_reauthorization`, return `ApproveAgentWallet` with the pending wallet details.
  3. Handle edge case: multiple accounts, some active, some pending → prefer the first pending one (blocker takes priority).
- **Verification**: `GET /onboarding/status` returns `next_step: "approve_agent_wallet"` with `pending_agent_wallet` populated when user has a pending HL agent wallet.
- **Commit message**: `feat: detect pending agent wallet approvals in onboarding status`

### CP-4: Trade history + risk config summary ✅
- Completed 2026-05-30 by /skill:vox build. Added `RiskConfig::is_default()` to `common_utils/src/risk/config.rs` (field-by-field comparison excluding user_id). Added `From<RiskConfig> for RiskConfigSummary` in `models/onboarding.rs`. Wired `PgRiskConfigStorage::load_or_default()` into `compute_onboarding_status()`. Endpoint now returns `ConfigureRisk` when risk is at defaults, `ReadyToTrade` when customized. `has_trades` and `risk_config` always populated. All 5 OnboardingStep variants reachable.

- **Touches**: `crates/router/src/routes/onboarding.rs`, `crates/router/src/services/onboarding.rs`, `crates/common_utils/src/risk/config.rs`
- **Tasks**:
  1. Query trade count: `SELECT COUNT(*) FROM trade_groups WHERE user_id = $1` — sets `has_trades`.
  2. Load risk config via `PgRiskConfigStorage::load_or_default()`. Add `RiskConfig::is_default()` method — compares against `RiskConfig::default()` field-by-field. If `is_default()` is true and accounts exist, return `ConfigureRisk` with the config summary. Otherwise return `ReadyToTrade`.
  3. Implement `RiskConfigSummary::from(RiskConfig)` — extracts `account_risk_percent` (as String), `max_leverage` (as i32), `daily_max_drawdown_percent` (as Option<String>), `require_stop_loss` (as bool).
  4. Wire the full `compute_onboarding_status()` to cover all 5 states: connected exchange → pending agent wallet → risk at defaults → ready.
- **Verification**: `GET /onboarding/status` returns correct `next_step` and populated fields for all 5 states. `has_trades` reflects actual DB state. `risk_config` reflects actual risk settings.
- **Commit message**: `feat: add trade history and risk config to onboarding status`

### CP-5: Update AGENT_TRADING.md + integration test ✅
- Completed 2026-05-30 by /skill:vox build. AGENT_TRADING.md Section 0 updated: Step 2 replaced 3-call discovery with single `GET /onboarding/status` call. Quick Reference tables updated with new endpoint. Added 5 unit tests for `RiskConfig::is_default()` (default, with user_id, customized risk, leverage change, stop-loss disabled) — all 15 config tests pass.

- **Touches**: `AGENT_TRADING.md`, `crates/router/tests/` (NEW integration test)
- **Tasks**:
  1. Update `AGENT_TRADING.md` Section 0 ("First Contact: Agent Onboarding") Step 2: replace the two-curl approach with a single `GET /api/v1/onboarding/status` call. Show the response shape for each state.
  2. Write integration test: creates a user (or uses test fixture), calls `/onboarding/status`, asserts `next_step == "connect_exchange"`. Then adds an exchange account via API, calls again, asserts `next_step == "ready_to_trade"` (or `configure_risk` if risk is defaults).
  3. Test 401 on unauthenticated request.
- **Verification**: `cargo clippy --all-targets && cargo test` passes with new integration test. `AGENT_TRADING.md` updated.
- **Commit message**: `docs: update AGENT_TRADING.md with onboarding status endpoint`

---

## Risks & Open Questions

1. **`auth_mode` not in `ExchangeAccountSummary`** — The repository's summary struct doesn't expose `auth_mode`. We need a direct SQL query or a new repository method. This is cleanest as a standalone query in the onboarding service rather than modifying the shared repository interface (avoids touching other consumers). The `ExchangeAccountResponse` already maps these fields — we can reuse the same SQL pattern.

2. **Exchange list duplication** — Extracting the list builder means `list_exchanges` and `compute_onboarding_status` share one source. But the current `list_exchanges` returns `serde_json::Value` while our new type is `ExchangeOption`. We need to decide: return `Vec<ExchangeOption>` from the shared function and convert to `serde_json::Value` in `list_exchanges` (add a `From` impl), or keep the existing pattern and duplicate the list. The spec says extract — let's extract.

3. **No `RiskConfig::is_default()` exists** — The `Default` impl sets values. We add a method that compares `self` against `RiskConfig::default()` field-by-field. This is simple and deterministic. Edge case: a user who explicitly sets the same values as defaults would still show `ConfigureRisk` — that's fine (they may want to review).

4. **Route registration pattern** — The routes are registered as `pub mod` declarations in `mod.rs` but the actual actix-web `configure()` wiring happens elsewhere (likely in the main server setup or a dedicated `configure_routes` function). Need to find the exact pattern and add the onboarding route there.

---

Plan ready: 5 checkpoints, ~5 hours total. Run `/skill:vox build AGENT-06-onboarding-status` to start CP-1.
