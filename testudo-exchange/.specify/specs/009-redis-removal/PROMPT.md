# Vox Build Prompt: Redis Removal

You are completing the Redis → PostgreSQL migration (009-redis-removal).

## Context

Read the PRD at `.specify/specs/009-redis-removal/PRD.md` for full requirements.

**Current State:**
- Redis crate deprecated but still instantiated at runtime
- 3 route handlers use `CacheService` (Redis) for risk config
- 5 files carry dead `use redis::RedisManager` imports
- `PgCacheService` and `PgRiskConfigStorage` already exist as drop-in replacements

**Target State:**
- Zero redis references in the workspace
- `crates/redis/` deleted
- `REDIS_URL` env var no longer required
- All 972+ tests still pass

## Your Task

Work through the phases in order. Each phase has a verification gate.

### Phase 1: Migrate Live Cache Usage (FR-1.1 through FR-1.5)

1. **order.rs** (`crates/router/src/routes/order.rs:175`)
   - Replace `CacheService::from_client(app_state.redis_connection.client.clone())` with `PgCacheService::new(app_state.pool.clone())`
   - Replace `RiskConfigStorage::new(cache)` with `PgRiskConfigStorage::new(cache)`
   - Update imports: `use common_utils::services::pg_cache::PgCacheService;` and `use common_utils::risk::pg_storage::PgRiskConfigStorage;`
   - Remove old `CacheService` and `RiskConfigStorage` imports

2. **risk_config.rs** (`crates/router/src/routes/risk_config.rs:82,108`)
   - Same pattern as order.rs, two call sites
   - Update imports

3. **app.rs** (`crates/router/src/types/app.rs:17`)
   - Remove `pub redis_connection: RedisManager` field
   - Remove `use redis::RedisManager;` import

4. **main.rs** (`crates/router/src/main.rs:381`)
   - Remove `redis_connection: RedisManager::new().await.unwrap()` from AppState construction
   - Remove `use redis::RedisManager;` import (line 69)

**Verify:** `cd testudo-exchange && cargo clippy --all-targets && cargo test`

### Phase 2: Delete Dead Code (FR-2.1 through FR-2.6)

1. **engine/src/user.rs** — Delete `handle_user()` function (lines 9-55) and its `use redis::RedisManager;` import. Keep `handle_user_pg()`.

2. **engine/src/order.rs** — Delete `handle_order()` function (lines 9-251) and its `use redis::RedisManager;` import. Keep `handle_order_pg()`.

3. **engine/src/engine/engine.rs** — Remove the deprecated `create_order` method that takes `RedisManager` param and the import.

4. **engine/src/engine/ws_stream.rs** — Remove `use redis::RedisManager;` and any methods that take `RedisManager` as parameter (publish_ws_trades, publish_ws_depth_updates). Keep methods that don't reference Redis.

5. **ws-stream/src/ws_manager.rs** — Delete the old `WsManager` struct if it only exists to wrap Redis. (Verify `PgWsManager` is the active implementation.)

6. **Cargo.toml cleanup** — Remove `redis = { path = "../redis" }` from:
   - `crates/engine/Cargo.toml`
   - `crates/router/Cargo.toml`
   - `crates/ws-stream/Cargo.toml`
   - `crates/db-processor/Cargo.toml`

**Verify:** `cd testudo-exchange && cargo clippy --all-targets && cargo test`

### Phase 3: Delete Redis Crate & Cleanup (FR-3.1 through FR-3.6)

1. Delete `crates/redis/` directory entirely.
2. Delete `crates/common_utils/src/services/cache.rs` (Redis CacheService).
3. Delete `crates/common_utils/src/risk/storage.rs` (Redis RiskConfigStorage).
4. Update `crates/common_utils/src/services/mod.rs` — remove `pub use cache::...` re-exports.
5. Update `crates/common_utils/src/lib.rs` — remove `CacheService` from re-exports.
6. Remove `redis` workspace dependency from root `Cargo.toml:21`.
7. Remove `fred` dependency if no longer referenced.
8. Fix stale comment: `crates/router/src/exchange/mod.rs:117` — change "placeholder for future Task 3.2" to "Exchange adapter trait for pluggable exchange backends".

**Verify:** `cd testudo-exchange && cargo clippy --all-targets && cargo test`

## Completion Protocol

After all phases pass verification:
1. Run full test suite: `cargo test 2>&1 | tail -5`
2. Confirm zero redis references: `rg -l 'redis' --type rust` should return nothing (except possibly test comments)
3. Confirm no `REDIS_URL` requirement: `grep -r 'REDIS_URL' crates/` should return nothing
