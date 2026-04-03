# Specification: Surface Inactive Agent Wallets and Structured Error Codes

**Spec ID:** UXA-01-agent-wallet-visibility
**Date:** 2026-04-01
**Status:** Draft
**Class:** Core / Error Handling
**Priority:** P0 — Agent wallet failures are invisible to users; caused 1h+ debugging session on production
**Depends on:** None (first in series)
**Series:** UXA-01 through UXA-03 (Agent Wallet Resilience)

---

## Problem Statement

When a Hyperliquid agent wallet becomes inactive (`is_active = false`), the system hides it entirely. The `list_by_user()` query in `exchange_account.rs:145-156` filters `WHERE is_active = true`, making inactive agent wallets disappear from the API response, the Account page, and all credential lookups.

This creates a cascading invisibility problem. The `GET /exchanges/accounts` endpoint returns an empty list for the affected exchange. The frontend shows no card, no error, no hint. When the user tries to trade, `load_credentials()` throws `NotFound` which surfaces as `"Internal error: Exchange account {uuid} not found"` — a raw UUID dumped to the user. The WebSocket subscription silently terminates (`stream_terminated ... account_not_found_or_deactivated`). The reconciliation service force-cancels all pending trades without notification.

The root cause is that `ExchangeApiError` has no variant for "agent wallet needs re-authorization" — it collapses into the generic `Internal` or `Exchange` variants. And `format_exchange_error()` in `trade_management.rs:137-152` only maps 2 patterns (`insufficient` and `Authentication`), passing everything else through raw. HL-specific errors like `"User or API Wallet 0x... does not exist"` reach the user with hex addresses intact.

---

## User Stories

- **As a trader**, I want to see my Hyperliquid exchange account even when the agent wallet needs re-authorization, so that I know the problem exists and can fix it.
- **As a trader**, I want trade placement errors to tell me what went wrong and what to do next, so that I don't need SSH access to debug my own setup.
- **As the system operator**, I want structured error codes in API responses, so that frontend clients can map errors to actionable UX without parsing raw strings.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `list_by_user()` returns active accounts AND inactive agent wallets (`WHERE is_active = true OR (auth_mode = 'agent_wallet' AND is_active = false)`) | High | Repository |
| FR-2 | `ExchangeAccountResponse` includes `requires_reauthorization: bool` field (true when `auth_mode = 'agent_wallet' AND is_active = false`) | High | API |
| FR-3 | `ExchangeApiError` gains `AgentWalletInactive { account_id: Uuid }` variant | High | Exchange API |
| FR-4 | `load_auth()` in `exchange_api.rs` returns `AgentWalletInactive` instead of generic `Internal("...not found")` when agent wallet has `is_active = false` | High | Exchange API |
| FR-5 | `format_exchange_error()` maps `AgentWalletInactive` to `"Agent wallet needs re-authorization. Open Account settings to fix."` | High | Trade Routes |
| FR-6 | `format_exchange_error()` maps `"does not exist"` HL errors to `"Agent wallet expired — re-authorize in Account settings."` | High | Trade Routes |
| FR-7 | `format_exchange_error()` maps `"rate limit"` HL errors to `"Exchange is busy — wait a moment and retry."` | Medium | Trade Routes |
| FR-8 | API error responses include `error_code` string field alongside `error` message (e.g., `"agent_wallet_inactive"`, `"exchange_error"`, `"insufficient_margin"`) | High | API |
| FR-9 | `is_definitive_rejection()` returns `true` for `AgentWalletInactive` | High | Trade Routes |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | FR-1 + FR-2: Repository query + API response field | Inactive agent wallets appear in account list |
| CP-2 | FR-3 + FR-4 + FR-5 + FR-9: Error variant + load_auth + format | Trade errors say "re-authorize" not raw UUID |
| CP-3 | FR-6 + FR-7 + FR-8: HL error mapping + structured error codes | All error paths return actionable messages with codes |

### Error Variant

```rust
// exchange_api.rs
#[derive(Debug, Error)]
pub enum ExchangeApiError {
    #[error("Order not found: {0}")]
    OrderNotFound(String),
    #[error("Insufficient balance: need {required}, have {available}")]
    InsufficientBalance {
        required: Decimal,
        available: Decimal,
    },
    #[error("Exchange error: {0}")]
    Exchange(String),
    #[error("Internal error: {0}")]
    Internal(String),
    // NEW
    #[error("Agent wallet inactive: account {account_id} needs re-authorization")]
    AgentWalletInactive { account_id: Uuid },
}
```

### Error Code Mapping

| ExchangeApiError Variant | `error_code` | `error` (user message) |
|---|---|---|
| `AgentWalletInactive` | `"agent_wallet_inactive"` | "Agent wallet needs re-authorization. Open Account settings to fix." |
| `InsufficientBalance` | `"insufficient_margin"` | "Insufficient margin — reduce position size or increase leverage" |
| `Exchange("...does not exist...")` | `"agent_wallet_expired"` | "Agent wallet expired — re-authorize in Account settings." |
| `Exchange("...Authentication...")` | `"auth_failed"` | "Exchange authentication failed — check your API keys" |
| `Exchange("...rate limit...")` | `"rate_limited"` | "Exchange is busy — wait a moment and retry." |
| `Exchange(other)` | `"exchange_error"` | "Exchange error: {msg}" |
| `Internal(msg)` | `"internal_error"` | "Internal error: {msg}" |
| `OrderNotFound(id)` | `"order_not_found"` | "Order not found: {id}" |

### API Response Shape

```rust
// Extend ApiResponse to include error_code
#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub error_code: Option<String>,  // NEW
    pub warnings: Option<Vec<String>>,
}
```

### Repository Query Change

```rust
// exchange_account.rs — replace list_by_user
pub async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<ExchangeAccountRow>, RepoError> {
    sqlx::query_as::<_, ExchangeAccountRow>(
        "SELECT id, user_id, exchange_name, permissions, is_active, created_at, last_used_at, \
         auth_mode, wallet_address \
         FROM exchange_accounts WHERE user_id = $1 \
         AND (is_active = true OR (auth_mode = 'agent_wallet' AND is_active = false)) \
         ORDER BY is_active DESC, created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&self.pool)
    .await
    .map_err(|e| RepoError::Database(e.to_string()))
}
```

### Paved Roads

- `format_exchange_error()` in `trade_management.rs:137-152` — existing pattern-match location, extend with new arms.
- `ExchangeAccountResponse` in `routes/exchanges.rs:145-166` — existing response struct, add field.
- `ApiResponse` in `routes/` — existing generic response wrapper, add `error_code`.

### Files

- `crates/router/src/services/exchange_api.rs` — add `AgentWalletInactive` variant to `ExchangeApiError`
- `crates/router/src/repositories/exchange_account.rs` — modify `list_by_user()` query
- `crates/router/src/routes/exchanges.rs` — add `requires_reauthorization` to `ExchangeAccountResponse`
- `crates/router/src/routes/trade_management.rs` — expand `format_exchange_error()`, add `error_code_for()` helper
- `crates/router/src/services/hyperliquid/exchange_api.rs` — `load_auth()` returns `AgentWalletInactive` for inactive accounts

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `GET /exchanges/accounts` returns inactive agent wallet accounts with `requires_reauthorization: true`
- [ ] Active accounts have `requires_reauthorization: false`
- [ ] Non-agent-wallet inactive accounts are still excluded from the list
- [ ] Trade placement against inactive agent wallet returns `error_code: "agent_wallet_inactive"` with human-readable message
- [ ] HL "does not exist" errors return `error_code: "agent_wallet_expired"` with actionable message
- [ ] HL "rate limit" errors return `error_code: "rate_limited"` with retry guidance
- [ ] `is_definitive_rejection()` returns `true` for `AgentWalletInactive`
- [ ] All existing tests pass — no regressions in active account flows
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Query performance** — Adding `OR` clause to `list_by_user()` could affect query plan. Mitigation: The `exchange_accounts` table is small (single-digit rows per user). No index changes needed.
2. **API contract change** — Adding `requires_reauthorization` and `error_code` fields to responses could break frontend parsers that use strict validation. Mitigation: Both are optional fields (`Option<bool>`, `Option<String>`) — existing clients ignore unknown fields.

---

## Completion Signal

This spec is complete when:
1. Inactive agent wallets appear in account listings with `requires_reauthorization: true`
2. `AgentWalletInactive` error variant exists and propagates through trade routes
3. All HL error patterns have human-readable mappings with structured error codes
4. All acceptance criteria met
5. `cargo clippy --all-targets && cargo test` passes
6. Code committed to master
