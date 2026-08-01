# Quality Checklist: 010-backend-hardening

> Spec ID: 010-backend-hardening
> Date: 2026-02-12

## Build & Lint
- [ ] `cargo build` succeeds with zero errors
- [ ] `cargo clippy` passes with zero warnings
- [ ] `cargo fmt --check` passes
- [ ] No new `unsafe` blocks introduced

## User Repository (FR-1, FR-2)
- [ ] `find_by_email` executes real `SELECT` query against `users` table
- [ ] `create_user` executes real `INSERT` with `RETURNING *`
- [ ] `update_user` executes real `UPDATE` with all mutable fields
- [ ] `PgPool` injected via constructor, wired through `AppState`
- [ ] `sqlx::Error` mapped to `AuthError` variants
- [ ] No `log::warn!("PLACEHOLDER")` messages remain
- [ ] Integration tests pass with `sqlx::test` macro

## Exchange Account Repository (FR-3 through FR-7)
- [ ] `ExchangeAccountRepository` created with CRUD operations
- [ ] `AesGcmVault` encrypts with AES-256-GCM, key from env
- [ ] Credentials never stored as plaintext (`.as_bytes().to_vec()` removed)
- [ ] `list_by_user` returns real accounts from database
- [ ] `insert` encrypts then persists
- [ ] `delete` verifies ownership (`user_id` in WHERE clause)
- [ ] `test_exchange_connection` decrypts and pings real Binance API
- [ ] Missing `CREDENTIAL_ENCRYPTION_KEY` env var causes fail-fast at startup

## Execution Mode Routing (FR-8, FR-9)
- [ ] JWT bearer token routes to `ExecutionMode::Live`
- [ ] X-User-Id header routes to `ExecutionMode::Shadow`
- [ ] `dec!(50000)` hardcoded market price removed
- [ ] Market price fetched from shadow engine last-trade or Binance ticker
- [ ] Market order rejected if no price source available

## Order Query & Cancellation (FR-10, FR-11)
- [ ] `get_open_orders` returns real shadow engine orders (Shadow mode)
- [ ] `cancel_all_orders` cancels real shadow engine orders (Shadow mode)
- [ ] No mock `serde_json::json!` order data remains in responses
- [ ] Live mode delegates to Binance adapter

## Account State (FR-12, FR-13)
- [ ] Shadow mode reads from `BalanceManager` (not hardcoded `dec!(10000)`)
- [ ] Live mode calls Binance `/fapi/v2/account`
- [ ] Open position count derived from real data

## Health Check (FR-14)
- [ ] Binance adapter `health_check` pings `/fapi/v1/ping`
- [ ] Latency measured and returned
- [ ] Timeout at 5 seconds

## Testing (FR-15)
- [ ] User repository has `sqlx::test` integration tests
- [ ] Exchange account repository has unit tests with mock pool
- [ ] Order route tests cover both Shadow and Live mode paths
- [ ] AesGcmVault encrypt/decrypt round-trip test

## No Regressions
- [ ] Existing 365 backend tests still pass
- [ ] Extension E2E tests still pass (paper mode unaffected)
- [ ] Shadow engine order execution unaffected
- [ ] Risk validation pipeline unaffected

## TODO Cleanup
- [ ] `grep -r "Phase 2 TODO\|TODO:.*Phase 2\|Phase 1.*mock\|Phase 1.*placeholder" crates/router/ crates/common_utils/` returns zero matches
