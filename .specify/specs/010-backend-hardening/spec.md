# Spec: 010-backend-hardening - Production Backend Hardening

> Priority: P1 | Depends on: 007, 008, EXT-09, EXT-10 | Status: Draft
> Date: 2026-02-12

---

## Overview

Replace all Phase 1 placeholder stubs across the router crate with real implementations backed by PostgreSQL, the shadow engine, and the Binance Futures API.

**Current:** ~25 TODO stubs return mock/hardcoded data across 5 subsystems: order routes return fake order lists, user repository returns `None`/passthrough, exchange account routes skip database persistence, account state adapter returns hardcoded `$10,000` balances, execution mode is hardcoded to `Shadow`, and market price for market orders is hardcoded to `dec!(50000)`.

**Target:** All routes operate against real PostgreSQL tables (users, exchange_accounts), shadow engine state, and Binance APIs. Execution mode derived from request context (JWT = Live, X-User-Id = Shadow). Market orders use real-time price from the shadow engine's last trade or Binance ticker.

---

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Credential encryption | AES-256-GCM via `aes-gcm` crate | Standard AEAD; key from env `CREDENTIAL_ENCRYPTION_KEY` |
| Market data source | Shadow engine last-trade price, Binance ticker fallback | Avoids new dependency; shadow engine already tracks prices |
| Execution mode detection | Header-based: JWT bearer = Live, X-User-Id = Shadow | Matches EXT-10 pattern already shipped in trade manager |
| User repo pool injection | Pass `PgPool` via `AppState` | Consistent with existing sqlx_postgres patterns |
| Exchange account repo | New `ExchangeAccountRepository` in `repositories/` | Mirrors user repository pattern |

---

## Functional Requirements

| ID | Requirement | Subsystem | Files | Status |
|----|-------------|-----------|-------|--------|
| FR-1 | `PostgresUserRepository` executes real SQLx queries for `find_by_email`, `create_user`, `update_user` against the `users` table | User Repository | `router/src/repositories/user.rs` | pending |
| FR-2 | User repository accepts `PgPool` via constructor; wire through `AppState` | User Repository | `router/src/repositories/user.rs`, `router/src/types/app.rs` | pending |
| FR-3 | `ExchangeAccountRepository` with CRUD operations against `exchange_accounts` table | Exchange Accounts | `router/src/repositories/exchange_account.rs` (new) | pending |
| FR-4 | `get_user_exchange_accounts` fetches real accounts from database (no mock data) | Exchange Accounts | `router/src/routes/exchanges.rs` | pending |
| FR-5 | `add_exchange_account` encrypts credentials with AES-256-GCM and persists to database | Exchange Accounts | `router/src/routes/exchanges.rs` | pending |
| FR-6 | `delete_exchange_account` performs real deletion with ownership verification | Exchange Accounts | `router/src/routes/exchanges.rs` | pending |
| FR-7 | `test_exchange_connection` loads credentials, decrypts, and pings Binance `/fapi/v1/ping` | Exchange Accounts | `router/src/routes/exchanges.rs`, `router/src/adapters/binance_adapter.rs` | pending |
| FR-8 | Execution mode derived from request: JWT bearer token = `Live`, X-User-Id header = `Shadow` | Order Routing | `router/src/routes/order.rs` | pending |
| FR-9 | Market order entry price fetched from shadow engine last-trade price for the symbol, with Binance `/fapi/v2/ticker/price` fallback | Order Routing | `router/src/routes/order.rs` | pending |
| FR-10 | `get_open_orders` fetches real orders from shadow engine (Shadow) or Binance (Live) | Order Routing | `router/src/routes/order.rs` | pending |
| FR-11 | `cancel_all_orders` performs real bulk cancellation against shadow engine or Binance | Order Routing | `router/src/routes/order.rs` | pending |
| FR-12 | `get_shadow_account_state` fetches real balance from shadow engine's `BalanceManager` | Account State | `common_utils/src/adapters/account_state.rs` | pending |
| FR-13 | `get_live_account_state` loads credentials, calls Binance `/fapi/v2/account`, returns real balance/positions/PnL | Account State | `common_utils/src/adapters/account_state.rs` | pending |
| FR-14 | `health_check` in Binance adapter pings `/fapi/v1/ping` and returns latency | Health Check | `router/src/adapters/binance_adapter.rs` | pending |
| FR-15 | User route integration tests with real test database | Testing | `router/src/routes/user.rs` | pending |

---

## Subsystem Breakdown

### 1. User Repository (FR-1, FR-2, FR-15)

Replace placeholder `PostgresUserRepository` with real SQLx queries.

**Existing schema** (`migrations/20250922164541_users.up.sql`):
```sql
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    email_verified BOOLEAN DEFAULT FALSE,
    is_active BOOLEAN DEFAULT TRUE
);
```

**Implementation:**
- Add `pool: PgPool` field to `PostgresUserRepository`
- `find_by_email`: `SELECT * FROM users WHERE email = $1`
- `create_user`: `INSERT INTO users (...) VALUES (...) RETURNING *`
- `update_user`: `UPDATE users SET ... WHERE id = $1`
- Map `sqlx::Error` to `AuthError` variants
- Replace placeholder tests with integration tests using `sqlx::test` macro

### 2. Exchange Account Repository (FR-3 through FR-7)

New repository + AES-256-GCM credential encryption.

**Existing schema** (`migrations/20250922173255_exchange_accounts.up.sql`):
```sql
CREATE TABLE exchange_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    exchange_name VARCHAR(50) NOT NULL,
    api_key_encrypted BYTEA NOT NULL,
    api_secret_encrypted BYTEA NOT NULL,
    permissions JSONB DEFAULT '{}',
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    UNIQUE(user_id, exchange_name)
);
```

**Implementation:**
- New `ExchangeAccountRepository` struct with `PgPool` + `AesGcmVault`
- `AesGcmVault` struct: encrypt/decrypt using `aes-gcm` crate with 256-bit key from env
- `list_by_user`: `SELECT ... FROM exchange_accounts WHERE user_id = $1`
- `insert`: Encrypt credentials, INSERT
- `delete`: `DELETE FROM exchange_accounts WHERE id = $1 AND user_id = $2`
- `find_by_id`: `SELECT ... WHERE id = $1 AND user_id = $2`
- Wire `ExchangeAccountRepository` into route handlers via `AppState`

### 3. Execution Mode Routing (FR-8, FR-9)

**Execution mode detection:**
```
Authorization: Bearer <JWT>  -> ExecutionMode::Live
X-User-Id: <uuid>            -> ExecutionMode::Shadow
```
Extract from request headers in `execute_order`, `get_open_order`, `cancel_order`.

**Market price resolution for market orders:**
1. Query shadow engine for last trade price on symbol
2. If no shadow price, call Binance `GET /fapi/v2/ticker/price?symbol=BTCUSDT`
3. If both fail, reject order with `"market_price_unavailable"` error

### 4. Order Query & Cancellation (FR-10, FR-11)

Replace mock JSON responses with real data:

- **Shadow mode:** Query shadow engine's order manager for user's open orders
- **Live mode:** Call `GET /fapi/v1/openOrders` via `ExchangeApi` trait (already implemented in EXT-10)
- **Bulk cancel (Shadow):** Iterate shadow engine orders and cancel each
- **Bulk cancel (Live):** Call `DELETE /fapi/v1/allOpenOrders` via Binance adapter

### 5. Account State Adapter (FR-12, FR-13)

- **Shadow:** Inject `Arc<ShadowEngine>` or expose balance query endpoint; read `BalanceManager` for user's available/locked USDT and open position count
- **Live:** Load user's exchange credentials from `ExchangeAccountRepository`, decrypt, call `GET /fapi/v2/account`, parse `totalWalletBalance`, `totalUnrealizedProfit`, count non-zero positions

### 6. Binance Health Check (FR-14)

Replace mock `Ok(())` with real `GET /fapi/v1/ping`:
- Measure round-trip latency
- Return `Err(RoutingError)` if ping fails or latency exceeds 5s

---

## Files to Modify

| File | Changes |
|------|---------|
| `crates/router/src/repositories/user.rs` | Real SQLx queries, PgPool injection |
| `crates/router/src/repositories/exchange_account.rs` | **NEW** - ExchangeAccountRepository + AesGcmVault |
| `crates/router/src/repositories/mod.rs` | Export new module |
| `crates/router/src/routes/order.rs` | Execution mode detection, market price, real order fetch/cancel |
| `crates/router/src/routes/exchanges.rs` | Wire to ExchangeAccountRepository, real CRUD |
| `crates/router/src/routes/user.rs` | Integration tests |
| `crates/router/src/adapters/binance_adapter.rs` | Real health_check ping |
| `crates/router/src/types/app.rs` | Add PgPool, ExchangeAccountRepository to AppState |
| `crates/common_utils/src/adapters/account_state.rs` | Real shadow + live balance fetching |
| `crates/router/Cargo.toml` | Add `aes-gcm` dependency |

---

## Dependencies (Crate Additions)

```toml
# router/Cargo.toml
aes-gcm = "0.10"       # AES-256-GCM credential encryption
```

No new migrations needed — `users` and `exchange_accounts` tables already exist.

---

## Acceptance Criteria

1. `cargo test` passes with no new failures
2. `cargo clippy` clean (no warnings)
3. All 25 `TODO` comments in scope are resolved
4. `PostgresUserRepository` queries execute against real PostgreSQL (verified via `sqlx::test`)
5. Exchange credentials are encrypted at rest (AES-256-GCM) — plaintext never stored
6. Execution mode correctly routes Shadow vs Live based on request headers
7. Market orders use real-time price, not `dec!(50000)`
8. `get_open_orders` returns real data from shadow engine or Binance
9. Binance health check measures actual `/fapi/v1/ping` latency

---

## Completion Signal

All Phase 1 TODO stubs replaced with production implementations. `grep -r "Phase 2 TODO\|TODO:.*Phase 2\|Phase 1.*mock\|Phase 1.*placeholder" crates/router/ crates/common_utils/` returns zero matches.

---

## Risks

| Risk | Mitigation |
|------|------------|
| Database tests require running PostgreSQL | Use `sqlx::test` with test database; CI already has Postgres |
| AES key management | Key from env var, not hardcoded; fail-fast if missing |
| Binance rate limits during tests | Mock Binance adapter in unit tests; integration tests use testnet |
| Shadow engine coupling | Access via `Arc<ShadowEngine>` in AppState, already available |
