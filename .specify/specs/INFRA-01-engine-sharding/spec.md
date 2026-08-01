# Specification: Engine Sharding by Asset Pair with CQRS Routing

**Spec ID:** INFRA-01-engine-sharding
**Date:** 2026-07-14
**Status:** Draft
**Class:** Infrastructure / Scaling
**Priority:** P1 — noisy-neighbor IOPS contention degrades live order matching latency
**Depends on:** 009-redis-removal (pg_queue must be the sole data layer)
**Series:** INFRA-01 (standalone — horizontal scaling foundation)

---

## Problem Statement

The current architecture runs everything on a single PostgreSQL 16 instance: OLTP order
matching, historical trade imports, analytics queries, job queues, pub/sub, and caching.
All four `pg_queue` tables (`queue_orders`, `queue_users`, `queue_database`,
`queue_imports`) share the same buffer pool and WAL, so a single heavy process — a
50,000-row CSV import flooding `queue_imports` — competes for IOPS with live order
matching on `queue_orders`.

The matching engine is a single Tokio actor (`EngineHandle` wrapping an `mpsc` channel).
It cannot scale horizontally because orderbook consistency demands a single source of
truth per asset pair. Running multiple engine instances behind a load balancer would
produce conflicting orderbook state. But the asset-pair-level isolation is absolute:
SOL_USDC and BTC_USDC orderbooks never interact. This is the canonical sharding boundary
for an execution engine.

The CQRS pattern described in the architectural assessment is a natural fit: a
centralized routing table maps symbol → engine instance, the router loads this into an
in-memory `DashMap`, and `pg_notify` pushes invalidation signals when the routing table
changes. The router then resolves the target engine without an external network hop,
preserving sub-millisecond execution latency.

---

## User Stories

- **As a trader**, I want my live order matching to stay fast even when another user is importing 50,000 historical trades, so that my fills don't get delayed by background work.
- **As an operator**, I want to add new asset pairs without restarting the router, so that market coverage can expand during trading hours.
- **As the platform**, I want the engine to scale horizontally by sharding orderbooks across instances, so that throughput grows with market coverage without degrading per-pair latency.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `queue_imports` connects to a dedicated Postgres instance (`DATABASE_IMPORTS_URL`), isolated from the OLTP instance | High | pg_queue, sqlx_postgres |
| FR-2 | `QueueRepository` resolves which pool to use based on `QueueName` — imports go to the imports pool, everything else to the OLTP pool | High | pg_queue |
| FR-3 | A `routing` table exists in the OLTP Postgres instance, mapping `asset_pair TEXT PRIMARY KEY` → `engine_instance TEXT NOT NULL` | High | sqlx_postgres |
| FR-4 | A `RoutingTable` struct wraps `Arc<DashMap<String, String>>` for lock-free concurrent reads, with a `resolve(asset_pair) -> Option<String>` method | High | router |
| FR-5 | A `RoutingListener` subscribes to `pg_notify('routing_change', ...)` and mutates the `DashMap` on invalidation. Payload format: `"SOL_USDC:engine-B"` | High | router |
| FR-6 | The router resolves the engine instance from `RoutingTable` before dispatching to `EngineHandle`. Unknown pairs fall back to a default engine. | High | router |
| FR-7 | Multiple `EngineHandle` instances coexist in `AppState` — keyed by instance ID — and the signal/order dispatch path selects the correct one | Medium | router, engine |
| FR-8 | The `routing` table is populated by an operator tool or migration. Runtime mutation comes only via `NOTIFY` — no REST endpoint for routing changes in v1 | Medium | sqlx_postgres |
| FR-9 | The `imports_database` Docker Compose service is added, with `DB_IMPORTS_MAX_CONNECTIONS` env var (default 20) | Medium | docker |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Dedicated Postgres instance for `queue_imports` only. `QueueRepository` routes imports to the isolated pool. Docker Compose adds `imports-db` service. | Import a large CSV while placing orders — orders complete without latency spike. `cargo test` passes. |
| CP-2 | `RoutingTable` struct + `RoutingListener` wired into `AppState`. Table resolves from `DashMap`, listener hot-reloads via `pg_notify`. No engine changes yet — routing resolves but always lands on the single default engine. | `cargo test` for routing table resolution + listener invalidation. Integration: insert a row in `routing`, send `NOTIFY`, verify `DashMap` updates within 100ms. |
| CP-3 | Multiple `EngineHandle` instances in `AppState`. Dispatch selects instance from `RoutingTable`. Engine instances are independent actors with their own orderbooks. | `cargo test` for dispatch to correct engine. Integration: route SOL_USDC to engine-B, place an order, verify it lands on engine-B's orderbook and not engine-A's. |

### Architecture

```
                        ┌─────────────────────────┐
                        │      Router (actix)      │
                        │  ┌───────────────────┐   │
                        │  │   RoutingTable    │   │
                        │  │ Arc<DashMap<      │   │
                        │  │   String, String> │   │
                        │  │ >                 │   │
                        │  └───────┬───────────┘   │
                        │          │ resolve()     │
                        │          ▼               │
                        │  ┌───────────────────┐   │
                        │  │ EngineHandles     │   │
                        │  │ HashMap<          │   │
                        │  │  String,          │   │
                        │  │  EngineHandle>    │   │
                        │  └───┬─────┬─────┬───┘   │
                        └──────┼─────┼─────┼───────┘
                               │     │     │
                    ┌──────────┘     │     └──────────┐
                    ▼                ▼                ▼
            ┌──────────┐    ┌──────────┐    ┌──────────┐
            │ engine-A │    │ engine-B │    │ engine-C │
            │ (default)│    │SOL_USDC  │    │BTC_USDC  │
            │ orders   │    │ orders   │    │ orders   │
            └────┬─────┘    └────┬─────┘    └────┬─────┘
                 │               │               │
                 └───────┬───────┴───────┬───────┘
                         │               │
                 ┌───────┴───┐   ┌───────┴───────┐
                 │ OLTP PG  │   │ Imports PG    │
                 │ (trades, │   │ (queue_imports│
                 │  orders, │   │  only)        │
                 │  routing,│   │               │
                 │  cache)  │   │               │
                 └──────────┘   └───────────────┘
```

### Key Types

```rust
// crates/router/src/routing/mod.rs

use dashmap::DashMap;
use std::sync::Arc;

/// Thread-safe, lock-free routing table: asset_pair → engine_instance_id.
/// Read path is O(1) with no contention. Write path is pg_notify-driven.
pub struct RoutingTable {
    inner: Arc<DashMap<String, String>>,
}

impl RoutingTable {
    pub fn new() -> Self {
        Self { inner: Arc::new(DashMap::new()) }
    }

    /// Resolve the engine instance for an asset pair.
    /// Returns the instance ID, or "default" if no mapping exists.
    pub fn resolve(&self, asset_pair: &str) -> String {
        self.inner
            .get(asset_pair)
            .map(|r| r.value().clone())
            .unwrap_or_else(|| "default".to_string())
    }

    /// Upsert a routing entry. Called by RoutingListener on NOTIFY.
    pub fn upsert(&self, asset_pair: String, instance: String) {
        self.inner.insert(asset_pair, instance);
    }

    /// Remove a routing entry.
    pub fn remove(&self, asset_pair: &str) {
        self.inner.remove(asset_pair);
    }
}
```

```rust
// crates/router/src/routing/listener.rs

use crate::routing::RoutingTable;
use pg_queue::ListenerService;
use std::sync::Arc;

/// Spawns a background task that listens for routing changes via pg_notify.
/// Parses payload format "SYMBOL:INSTANCE" and mutates the RoutingTable.
pub async fn spawn_routing_listener(
    mut listener: ListenerService,
    table: Arc<RoutingTable>,
) {
    listener.listen("routing_change").await
        .expect("Failed to listen on routing_change channel");

    tokio::spawn(async move {
        loop {
            match listener.recv().await {
                Ok(notification) => {
                    let payload = notification.payload();
                    if let Some((symbol, instance)) = payload.split_once(':') {
                        if instance.is_empty() {
                            table.remove(symbol);
                        } else {
                            table.upsert(symbol.to_string(), instance.to_string());
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Routing listener error: {:?}, reconnecting...", e);
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    });
}
```

```rust
// crates/pg_queue/src/queue.rs — amended QueueRepository

impl QueueRepository {
    /// Returns the pool for a given queue name.
    /// Imports queue uses the dedicated imports pool; everything else uses OLTP.
    pub fn pool_for(&self, queue: QueueName) -> &PgPool {
        match queue {
            QueueName::TradeImports => self.imports_pool.as_ref()
                .unwrap_or(&self.pool),
            _ => &self.pool,
        }
    }
}
```

### AppState Changes

```rust
// crates/router/src/types/app.rs — additions

pub struct AppState {
    // ... existing fields ...

    /// INFRA-01: Symbol-to-engine routing table. Read path is lock-free.
    pub routing_table: Arc<RoutingTable>,

    /// INFRA-01: Engine handles keyed by instance ID.
    /// "default" handle always exists. Additional handles from routing config.
    pub engine_handles: HashMap<String, EngineHandle>,

    /// INFRA-01: Postgres pool for trade imports (isolated from OLTP).
    pub imports_pool: Option<sqlx::Pool<sqlx::Postgres>>,
}
```

### Migration

```sql
-- INFRA-01: Routing table for symbol → engine instance mapping
CREATE TABLE IF NOT EXISTS routing (
    asset_pair  TEXT PRIMARY KEY,
    engine_instance TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Trigger: NOTIFY routing_change on INSERT/UPDATE/DELETE
CREATE OR REPLACE FUNCTION notify_routing_change() RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM pg_notify('routing_change', OLD.asset_pair || ':');
    ELSE
        PERFORM pg_notify('routing_change', NEW.asset_pair || ':' || NEW.engine_instance);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER routing_change_notify
    AFTER INSERT OR UPDATE OR DELETE ON routing
    FOR EACH ROW EXECUTE FUNCTION notify_routing_change();
```

### Docker Compose Changes

```yaml
# New service alongside existing `db`
imports-db:
  container_name: exchange-imports-postgres
  image: postgres:16-alpine
  command: -c 'max_connections=200'
  shm_size: 512mb
  restart: always
  env_file: .env
  environment:
    POSTGRES_DB: testudo_imports
  ports:
    - "5001:5432"
  volumes:
    - ./imports-postgres-data:/var/lib/postgresql/data
  networks:
    - gateway
```

### Files

| File | Status | Purpose |
|------|--------|---------|
| `crates/router/src/routing/mod.rs` | New | `RoutingTable` struct |
| `crates/router/src/routing/listener.rs` | New | `spawn_routing_listener` background task |
| `crates/pg_queue/src/queue.rs` | Modified | `QueueRepository` gains `imports_pool` field and `pool_for()` method |
| `crates/sqlx_postgres/src/lib.rs` | Modified | `PostgresDb` gains optional `imports_pool` for `DATABASE_IMPORTS_URL` |
| `crates/router/src/types/app.rs` | Modified | Add `routing_table`, `engine_handles`, `imports_pool` to `AppState` |
| `crates/router/src/main.rs` | Modified | Wire `RoutingListener`, `EngineHandle` map, route dispatch |
| `crates/sqlx_postgres/migrations/INFRA01_routing.sql` | New | `routing` table + notify trigger |
| `docker/docker-compose.yml` | Modified | Add `imports-db` service |
| `.env.example` | Modified | Add `DATABASE_IMPORTS_URL` |
| `crates/router/src/routes/order.rs` | Modified | Resolve engine instance from `RoutingTable` before dispatch |
| `crates/router/src/routes/signal.rs` | Modified | Resolve engine instance from `RoutingTable` before dispatch |

### Dependencies Added

None. All primitives exist: `DashMap` (already in `Cargo.toml`), `pg_queue::ListenerService`
(already wraps `sqlx::PgListener`), `sqlx::PgPool` (already the connection primitive).
No new crates.

---

## Acceptance Criteria

- [ ] CP-1: `queue_imports` writes to a dedicated Postgres instance. Heavy import does not increase `queue_orders` poll latency (measured via `POP_LATENCY_MS` metric).
- [ ] CP-1: `cargo clippy --all-targets && cargo test` passes with no regressions.
- [ ] CP-2: `RoutingTable::resolve("SOL_USDC")` returns `"engine-B"` after `pg_notify('routing_change', 'SOL_USDC:engine-B')` within 100ms.
- [ ] CP-2: `RoutingTable::resolve("UNKNOWN_PAIR")` returns `"default"`.
- [ ] CP-2: `RoutingListener` reconnects on connection loss (test: kill Postgres, restart, verify listener resumes).
- [ ] CP-3: Order for SOL_USDC dispatched to engine-B appears on engine-B's orderbook and not engine-A's.
- [ ] CP-3: Unknown pairs fall back to the `"default"` engine without error.
- [ ] CP-3: `cargo clippy --all-targets && cargo test` passes.
- [ ] All three checkpoints committed with passing CI.

---

## Risks

1. **EngineHandle map grows unbounded** — if routing entries proliferate, the `HashMap` in `AppState` holds a handle per instance. Mitigation: engine instances are operator-managed, not user-created. The map size is bounded by the number of asset pairs.
2. **NOTIFY delivery is at-most-once** — if the listener is disconnected when a NOTIFY fires, it misses the update. Mitigation: on listener reconnect, full-reload the routing table from the database via `SELECT * FROM routing`. This is included in CP-2.
3. **Imports pool failure blocks imports but not trading** — if `DATABASE_IMPORTS_URL` is unreachable, `pool_for()` falls back to the OLTP pool (`unwrap_or(&self.pool)`). This preserves import functionality at the cost of re-introducing noisy-neighbor risk. Mitigation: log a warning so operators know to fix the imports DB. This is the correct failure mode — trading must never be blocked by an imports infrastructure issue.

---

## Completion Signal

This spec is complete when:
1. All three checkpoints are implemented, tested, and committed.
2. All acceptance criteria pass.
3. `cargo clippy --all-targets && cargo test` exits 0 across the workspace.
4. Docker Compose spins up both Postgres instances and the router starts cleanly.
