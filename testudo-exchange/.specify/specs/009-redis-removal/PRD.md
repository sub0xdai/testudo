# PRD: Complete Redis Removal

| Metadata | Details |
|----------|---------|
| **Project** | Testudo Exchange Core |
| **Component** | Infrastructure — Cache & Messaging |
| **Status** | Ready |
| **Priority** | **P1** |
| **Spec ID** | 009-redis-removal |
| **Owner** | Engineering Lead |
| **Depends on** | None (pg_queue already fully operational) |

## 1. Executive Summary

The `crates/redis/` crate was deprecated when `pg_queue` was introduced, but the migration was never finished. Redis is still instantiated at runtime and used for risk config caching in 3 route handlers. Meanwhile, 5 other files carry dead `use redis::RedisManager` imports from already-migrated code paths.

The PostgreSQL replacements (`PgCacheService`, `PgRiskConfigStorage`) already exist and are API-compatible. This spec completes the migration, removes the redis crate, and eliminates ~1,626 lines of dead code.

**Operational benefit:** Removes the Redis runtime dependency entirely — one fewer service to deploy, monitor, and secure.

---

## 2. Current State

### 2.1. Live Redis Usage (3 call sites)

All three construct `CacheService::from_client(app_state.redis_connection.client.clone())` to build a `RiskConfigStorage`:

| File | Line | Usage |
|------|------|-------|
| `crates/router/src/routes/order.rs` | 175 | Risk config load for Decision Loop |
| `crates/router/src/routes/risk_config.rs` | 82 | `GET /api/v1/risk-config` |
| `crates/router/src/routes/risk_config.rs` | 108 | `PUT /api/v1/risk-config` |

**AppState fields:**
- `app.rs:17` — `pub redis_connection: RedisManager`
- `main.rs:381` — `RedisManager::new().await.unwrap()`

### 2.2. Dead Redis Imports (5 files, never called at runtime)

| File | Lines | Why Dead |
|------|-------|----------|
| `crates/engine/src/user.rs` | 9-55 | `handle_user()` replaced by `handle_user_pg()` |
| `crates/engine/src/order.rs` | 9-251 | `handle_order()` replaced by `handle_order_pg()` |
| `crates/engine/src/engine/engine.rs` | 103-110 | `#[deprecated]` create_order, rejects all calls |
| `crates/engine/src/engine/ws_stream.rs` | 8+ | Publish methods never invoked anywhere |
| `crates/ws-stream/src/ws_manager.rs` | 2+ | Old `WsManager` replaced by `PgWsManager` |

### 2.3. Drop-In Replacements Already Exist

| Redis Version | PostgreSQL Replacement | Location |
|---------------|----------------------|----------|
| `CacheService` | `PgCacheService` | `common_utils/src/services/pg_cache.rs` |
| `RiskConfigStorage` | `PgRiskConfigStorage` | `common_utils/src/risk/pg_storage.rs` |
| `RedisManager` (pub/sub) | `PgQueueManager` (LISTEN/NOTIFY) | Already in `AppState.pg_queue` |

---

## 3. Functional Requirements

### Phase 1: Migrate Live Cache Usage (3 call sites)

| ID | Requirement | File | Priority |
|----|-------------|------|----------|
| FR-1.1 | Replace `CacheService` with `PgCacheService` in order route Decision Loop | `routes/order.rs:175` | High |
| FR-1.2 | Replace `CacheService` with `PgCacheService` in GET risk-config | `routes/risk_config.rs:82` | High |
| FR-1.3 | Replace `CacheService` with `PgCacheService` in PUT risk-config | `routes/risk_config.rs:108` | High |
| FR-1.4 | Remove `redis_connection` field from `AppState` | `types/app.rs:17` | High |
| FR-1.5 | Remove `RedisManager::new()` construction from `main.rs` | `main.rs:381` | High |

**Migration pattern (all 3 sites are identical):**

Before:
```rust
use common_utils::services::CacheService;
use common_utils::risk::storage::RiskConfigStorage;

let cache = CacheService::from_client(app_state.redis_connection.client.clone());
let storage = RiskConfigStorage::new(cache);
```

After:
```rust
use common_utils::services::pg_cache::PgCacheService;
use common_utils::risk::pg_storage::PgRiskConfigStorage;

let cache = PgCacheService::new(app_state.pool.clone());
let storage = PgRiskConfigStorage::new(cache);
```

**Acceptance criteria:**
- `cargo clippy --all-targets` passes with zero redis references in router
- `cargo test` passes — risk config load/save still works
- `REDIS_URL` env var is no longer required to start the server

### Phase 2: Delete Dead Code

| ID | Requirement | File(s) | Priority |
|----|-------------|---------|----------|
| FR-2.1 | Delete `handle_user()` (lines 9-55), keep only `handle_user_pg()` | `engine/src/user.rs` | High |
| FR-2.2 | Delete `handle_order()` (lines 9-251), keep only `handle_order_pg()` | `engine/src/order.rs` | High |
| FR-2.3 | Remove deprecated `create_order` method and `RedisManager` import | `engine/src/engine/engine.rs` | High |
| FR-2.4 | Remove dead `RedisManager` import and unused publish methods | `engine/src/engine/ws_stream.rs` | Medium |
| FR-2.5 | Remove dead `WsManager` struct (replaced by `PgWsManager`) | `ws-stream/src/ws_manager.rs` | Medium |
| FR-2.6 | Remove `redis = { path = "../redis" }` from all Cargo.toml files | 4 Cargo.toml files | High |

**Acceptance criteria:**
- No `use redis::` import exists anywhere in the workspace
- `cargo clippy --all-targets` passes
- `cargo test` — all 972+ tests still pass

### Phase 3: Delete Redis Crate & Cleanup

| ID | Requirement | File(s) | Priority |
|----|-------------|---------|----------|
| FR-3.1 | Delete `crates/redis/` directory entirely | `crates/redis/` (231 lines) | High |
| FR-3.2 | Delete `CacheService` (Redis-backed) | `common_utils/src/services/cache.rs` (194 lines) | Medium |
| FR-3.3 | Delete `RiskConfigStorage` (Redis-backed) | `common_utils/src/risk/storage.rs` (146 lines) | Medium |
| FR-3.4 | Remove `redis` workspace dependency from root `Cargo.toml` | `Cargo.toml:21` | High |
| FR-3.5 | Remove `fred` crate dependency (Redis client library) if no longer referenced | All Cargo.toml files | Medium |
| FR-3.6 | Update stale "placeholder for future Task 3.2" comment on `ExchangeAdapter` trait | `router/src/exchange/mod.rs:117` | Low |

**Acceptance criteria:**
- `crates/redis/` does not exist
- `cargo build` succeeds without any redis/fred dependencies
- `cargo clippy --all-targets && cargo test` all green
- No `REDIS_URL` reference in any non-ops configuration

---

## 4. Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| `PgCacheService` API mismatch with `CacheService` | Low | Both implement get/set/get_or_set with same signatures |
| Risk config data loss during migration | None | Cache is ephemeral (24h TTL) — miss just reloads defaults |
| Removing dead functions breaks something | Low | Functions are unreachable (replaced by `_pg` variants, verified in engine/src/main.rs) |
| `fred` crate still needed elsewhere | Low | Grep confirms fred is only used via redis crate |

---

## 5. Lines of Code Impact

| Action | Lines Removed | Lines Added |
|--------|--------------|-------------|
| Delete `crates/redis/` | ~231 | 0 |
| Delete dead functions in engine | ~400 | 0 |
| Delete `CacheService` + `RiskConfigStorage` | ~340 | 0 |
| Delete dead `WsManager` | ~121 | 0 |
| Migrate 3 route call sites | ~6 | ~6 |
| Remove Cargo.toml deps | ~10 | 0 |
| **Total** | **~1,108** | **~6** |

Net reduction: **~1,100 lines** of dead code removed.
