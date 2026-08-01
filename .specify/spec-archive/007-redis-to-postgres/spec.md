# Spec: 007-redis-to-postgres - Unified Data Layer

> Migrate ALL Redis functionality to PostgreSQL
> Priority: P0 (Infrastructure Critical)
> Status: Implemented

---

## Overview

Eliminate Redis dependency by migrating queues, pub/sub, and caching to PostgreSQL. This consolidates infrastructure to a single database.

**Current:** Redis handles queues (ORDERS, USERS, DATABASE), pub/sub (trade/depth channels), and caching (trade history, binance data, risk config).

**Target:** PostgreSQL handles all via SKIP LOCKED queues, LISTEN/NOTIFY pub/sub, and UNLOGGED cache tables.

---

## Design Decisions (Per User Input)

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Queue wake mechanism | **LISTEN/NOTIFY** | Lower latency - consumer wakes immediately on new job |
| Cache TTL enforcement | **Per-query check** | Exact expiration - check `expires_at` on every read |
| Connection pool size | **Keep 50** | Sufficient if queries are fast |

---

## Database Schema

```sql
-- Queue Tables (LISTEN/NOTIFY trigger for immediate wake)
CREATE TABLE queue_orders (
    id BIGSERIAL PRIMARY KEY,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ
);
CREATE INDEX idx_queue_orders_pending ON queue_orders(created_at) WHERE status = 'pending';

-- Trigger: Notify on new job
CREATE OR REPLACE FUNCTION notify_queue_orders() RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('queue_orders', NEW.id::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER queue_orders_notify
    AFTER INSERT ON queue_orders
    FOR EACH ROW EXECUTE FUNCTION notify_queue_orders();

-- Same pattern for queue_users and queue_database tables

-- Cache Table (UNLOGGED for performance, per-query TTL check)
CREATE UNLOGGED TABLE cache_entries (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);

-- Request-Response Table
CREATE TABLE request_responses (
    request_id UUID PRIMARY KEY,
    response JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE OR REPLACE FUNCTION notify_response() RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('response_' || NEW.request_id::text, '');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER response_notify
    AFTER INSERT ON request_responses
    FOR EACH ROW EXECUTE FUNCTION notify_response();
```

---

## Implementation Tasks

### Phase 1: Create pg_queue Crate

| Task | File | Status |
|------|------|--------|
| T1.1 | `crates/pg_queue/Cargo.toml` | complete |
| T1.2 | `crates/pg_queue/src/lib.rs` | complete |
| T1.3 | `crates/pg_queue/src/queue.rs` | complete |
| T1.4 | `crates/pg_queue/src/notify.rs` | complete |
| T1.5 | `crates/pg_queue/src/listen.rs` | complete |
| T1.6 | `crates/pg_queue/src/cache.rs` | complete |
| T1.7 | `crates/pg_queue/src/request_response.rs` | complete |
| T1.8 | `crates/pg_queue/src/errors.rs` | complete |

### Phase 2: Migrate Cache Operations

| Task | File | Status |
|------|------|--------|
| T2.1 | `crates/common_utils/src/services/pg_cache.rs` | complete |
| T2.2 | `crates/router/src/routes/trade.rs` | complete |
| T2.3 | `crates/common_utils/src/risk/pg_storage.rs` | complete |

### Phase 3: Migrate Queue Operations

| Task | File | Status |
|------|------|--------|
| T3.1 | `crates/engine/src/main.rs` | complete |
| T3.2 | `crates/engine/src/order.rs` | complete |
| T3.3 | `crates/db-processor/src/main.rs` | complete |

### Phase 4: Migrate Pub/Sub

| Task | File | Status |
|------|------|--------|
| T4.1 | `crates/engine/src/engine/ws_stream.rs` | complete |
| T4.2 | `crates/ws-stream/src/main.rs` | complete |
| T4.3 | `crates/ws-stream/src/pg_ws_manager.rs` | complete |

### Phase 5: Migrate Request-Response

| Task | File | Status |
|------|------|--------|
| T5.1 | `crates/router/src/routes/depth.rs` | complete |
| T5.2 | `crates/router/src/types/app.rs` | complete |

### Phase 6: Cleanup

| Task | File | Status |
|------|------|--------|
| T6.1 | `crates/redis/src/lib.rs` | complete (deprecated) |
| T6.2 | Remove fred dependency | deferred |
| T6.3 | Docker/K8s configs | deferred |

---

## Key Patterns

### Queue Pop with LISTEN/NOTIFY Wake

```rust
pub async fn consume_loop(pool: &PgPool, queue: &str) {
    let mut listener = PgListener::connect_with(pool).await.unwrap();
    listener.listen(&format!("queue_{}", queue)).await.unwrap();

    loop {
        // Try to claim a job
        if let Some(job) = pop_job(pool, queue).await {
            process_job(job).await;
            continue;
        }
        // No jobs - wait for notification
        let _ = listener.recv().await;
    }
}
```

### Cache Get with Per-Query TTL Check

```rust
pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, CacheError> {
    let row: Option<(serde_json::Value,)> = sqlx::query_as(
        "SELECT value FROM cache_entries WHERE key = $1 AND expires_at > NOW()"
    )
    .bind(key)
    .fetch_optional(&self.pool)
    .await?;

    row.map(|(v,)| serde_json::from_value(v)).transpose()
}
```

---

## Files Created

| File | Purpose |
|------|---------|
| `crates/pg_queue/` | New crate (8 files) |
| `crates/sqlx_postgres/migrations/20260131000000_pg_queue_tables.up.sql` | Schema |
| `crates/sqlx_postgres/migrations/20260131000000_pg_queue_tables.down.sql` | Rollback |
| `crates/common_utils/src/services/pg_cache.rs` | PG cache service |
| `crates/common_utils/src/risk/pg_storage.rs` | PG risk storage |
| `crates/ws-stream/src/pg_ws_manager.rs` | PG websocket manager |

---

## Acceptance Criteria

- [x] All 3 queues (ORDERS, USERS, DATABASE) work via PostgreSQL
- [x] WebSocket receives trade/depth updates via LISTEN/NOTIFY
- [x] Cache read with exact TTL enforcement
- [x] Request-response pattern works via PG
- [x] `cargo clippy --all-targets && cargo test` passes
- [ ] 100 orders/sec throughput maintained (needs integration test)
- [ ] Zero message loss (needs integration test)

---

## Verification Commands

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| NOTIFY 8KB limit | Trade/depth messages are <1KB, safe |
| Connection exhaustion | Keep pool at 50, monitor under load |
| UNLOGGED data loss | Cache only - acceptable on crash |
| Migration downtime | Feature flag for gradual rollout |

---

*Spec implemented 2026-01-31*
