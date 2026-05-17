# PRD: Testudo Core Performance & Reliability Overhaul

| Metadata | Details |
|----------|---------|
| **Project** | Testudo Exchange Core |
| **Component** | Infrastructure & Matching Engine |
| **Status** | **Critical (Immediate Action)** |
| **Priority** | **P0** |
| **Spec ID** | 006-performance-overhaul |
| **Owner** | Engineering Lead |

## 1. Executive Summary

Current analysis of the Testudo production environment reveals critical latency spikes (up to 30s) and scalability bottlenecks that threaten the platform's viability. The system suffers from excessive locking, unoptimized database usage, and dangerous default timeout configurations.

This initiative is an **emergency stabilization sprint** to bring order latency down from ~65ms (median) to <10ms, and to eliminate "stop-the-world" locking events that block trading during high volume.

---

## 2. Critical Issues (P0 - Immediate Fix)

### 2.1. Dangerous HTTP Timeouts

**Severity:** Critical (Causes thread starvation)
**Location:** `crates/common_utils/src/services/binance_data.rs:57-60`, `crates/common_utils/src/adapters/binance_executor.rs:39-42`

**Problem:** The HTTP client has a 30-second timeout for order execution and 10 seconds for market data. During network congestion, a single slow request blocks a worker thread for half a minute.

**Requirements:**
- [ ] FR-2.1.1: Hard cap timeouts to **2s** for market data
- [ ] FR-2.1.2: Hard cap timeouts to **5s** for order execution
- [ ] FR-2.1.3: Implement Circuit Breaker that fails fast after 3 consecutive timeouts
- [ ] FR-2.1.4: No thread blocks longer than 5s under any condition

**Acceptance Criteria:**
```rust
// binance_data.rs - market data client
.timeout(Duration::from_secs(2))

// binance_executor.rs - order execution client
.timeout(Duration::from_secs(5))
```

---

### 2.2. Database Connection Starvation

**Severity:** Critical (Limits concurrency)
**Location:** `crates/sqlx_postgres/src/lib.rs:27`

**Problem:** The connection pool is capped at `max_connections(10)`. With >100 concurrent users/requests, 90% of requests immediately block waiting for a DB handle.

**Requirements:**
- [ ] FR-2.2.1: Increase `max_connections` to **50** (configurable via env var)
- [ ] FR-2.2.2: Implement connection acquire timeout of **500ms**
- [ ] FR-2.2.3: Add metrics logging for pool exhaustion events

**Acceptance Criteria:**
```rust
PgPoolOptions::new()
    .max_connections(env::var("DB_MAX_CONNECTIONS").unwrap_or("50"))
    .acquire_timeout(Duration::from_millis(500))
```

---

### 2.3. Global Lock Contention

**Severity:** Critical (Throughput Bottleneck)
**Location:** `crates/engine/src/shadow/mod.rs:78-83`

**Problem:** `ShadowBalanceManager`, `ShadowOrderManager`, `ShadowPositionManager`, and `OrderGroupManager` are protected by global `RwLock`s.

*Impact:* Every trade operation serializes the entire engine. 100 traders = 99 waiting.

**Requirements:**
- [ ] FR-2.3.1: Replace `Arc<RwLock<ShadowBalanceManager>>` with sharded or `DashMap` approach
- [ ] FR-2.3.2: Replace `Arc<RwLock<ShadowOrderManager>>` with sharded or `DashMap` approach
- [ ] FR-2.3.3: Replace `Arc<RwLock<ShadowPositionManager>>` with sharded or `DashMap` approach
- [ ] FR-2.3.4: Replace `Arc<RwLock<OrderGroupManager>>` with sharded or `DashMap` approach
- [ ] FR-2.3.5: Concurrent access for different users must be lock-free relative to each other

**Acceptance Criteria:**
- Two users placing orders simultaneously must not block each other
- Benchmark: 100 concurrent order placements complete in <100ms total

---

## 3. High Priority Optimizations (P1)

### 3.1. Trade History Caching

**Severity:** High (DB Load)
**Location:** `crates/router/src/routes/trade.rs:17-27`

**Problem:** Every `GET /trades` request hits PostgreSQL directly. High-frequency polling causes massive unnecessary DB load.

**Requirements:**
- [ ] FR-3.1.1: Implement Redis cache for trade history with 5s TTL
- [ ] FR-3.1.2: Cache key format: `trades:{symbol}:{limit}`
- [ ] FR-3.1.3: Invalidate cache on new trade insertion

---

### 3.2. Orderbook User Index

**Severity:** High (CPU Load)
**Location:** `crates/engine/src/engine/orderbook.rs:143-150`

**Problem:** Finding a user's orders currently requires iterating the *entire* orderbook (O(n)).

**Requirements:**
- [ ] FR-3.2.1: Add secondary index `HashMap<UserId, HashSet<OrderId>>`
- [ ] FR-3.2.2: Update index on order add/remove/fill
- [ ] FR-3.2.3: `get_open_orders(user_id)` must be O(1)

---

### 3.3. Lock Batching in Fills

**Severity:** High (Latency)
**Location:** `crates/engine/src/engine/engine.rs:395-407`

**Problem:** A trade with 10 fills currently triggers 40 separate lock acquisitions (4 per fill).

**Requirements:**
- [ ] FR-3.3.1: Collect all balance changes before acquiring locks
- [ ] FR-3.3.2: Acquire locks once per affected user
- [ ] FR-3.3.3: Apply all changes atomically, then release

---

### 3.4. Efficient Orderbook Matching

**Severity:** High (Matching Speed)
**Location:** `crates/engine/src/engine/orderbook.rs:61-93`

**Problem:** Matching engine iterates through *all* price levels to find matches.

**Requirements:**
- [ ] FR-3.4.1: Use `BTreeMap::range(..=order.price)` for buy orders
- [ ] FR-3.4.2: Use `BTreeMap::range(order.price..)` for sell orders
- [ ] FR-3.4.3: Matching complexity must be O(log n + k) where k = matched orders

---

## 4. Medium Priority Cleanups (P2)

| ID | Issue | Location | Proposed Fix |
|----|-------|----------|--------------|
| FR-4.1 | Redis RTT | `redis/src/lib.rs:78-110` | Use Redis Pipelining for batched commands |
| FR-4.2 | JSON Allocations | `routes/order.rs:178-192` | Derive `Serialize` directly on structs |
| FR-4.3 | HTTP Keep-Alive | HTTP Clients | Enable connection pooling in reqwest |
| FR-4.4 | Slow Startup | `redis/src/lib.rs:39-45` | Use `tokio::join!` for parallel init |

---

## 5. Latency Goals

| Step | Current (Cold) | Target | Optimization |
|------|----------------|--------|--------------|
| Account Lookup | ~50ms (DB) | <1ms | Redis Cache |
| Lock Wait | ~10ms+ | <0.1ms | DashMap/Sharding |
| Matching | O(n) | O(log n) | BTreeMap Range |
| **Total Latency** | **~65ms** | **<10ms** | All of the above |

---

## 6. Implementation Phases

### Phase 1: Stability ("Stop the Bleeding") - Week 1
- FR-2.1.1, FR-2.1.2: HTTP timeout reduction
- FR-2.2.1, FR-2.2.2: DB pool expansion
- FR-3.1.1: Redis caching for trades

### Phase 2: Concurrency ("Unlock") - Week 2
- FR-2.3.1 through FR-2.3.5: DashMap migration
- FR-3.3.1 through FR-3.3.3: Lock batching

### Phase 3: Algorithmics ("Speed") - Week 3
- FR-3.2.1 through FR-3.2.3: User order index
- FR-3.4.1 through FR-3.4.3: Range-based matching

---

## 7. Verification

All changes must pass:
1. `cargo test` - All 580+ tests passing
2. `cargo clippy --all-targets` - No warnings
3. Benchmark: 100 concurrent orders < 100ms total execution time

---

## 8. Dependencies

```toml
# Add to Cargo.toml for DashMap support
dashmap = "5.5"
```
