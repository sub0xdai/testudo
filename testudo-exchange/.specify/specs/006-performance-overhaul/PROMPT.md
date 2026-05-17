# Ralph Loop Prompt: Performance Overhaul

You are implementing the Testudo Performance & Reliability Overhaul (006-performance-overhaul).

## Context

Read the PRD at `.specify/specs/006-performance-overhaul/PRD.md` for full requirements.

**Current State:**
- Order latency: ~65ms median, 100ms+ cold cache
- Global RwLocks causing contention at scale
- HTTP timeouts: 10-30s (dangerous)
- DB pool: 10 connections (bottleneck)

**Target State:**
- Order latency: <10ms
- Lock-free per-user operations
- HTTP timeouts: 2-5s with circuit breaker
- DB pool: 50+ connections with acquire timeout

## Your Task

Work through the phases in order. Each phase has specific functional requirements (FR-X.X.X).

### Phase 1: Stability (FR-2.1, FR-2.2, FR-3.1)

1. **HTTP Timeouts** (`crates/common_utils/src/services/binance_data.rs`, `crates/common_utils/src/adapters/binance_executor.rs`)
   - Change market data timeout: 10s -> 2s
   - Change execution timeout: 30s -> 5s

2. **DB Pool** (`crates/sqlx_postgres/src/lib.rs`)
   - Increase max_connections: 10 -> 50
   - Add acquire_timeout: 500ms

3. **Trade Caching** (`crates/router/src/routes/trade.rs`)
   - Add Redis cache for GET /trades with 5s TTL

### Phase 2: Concurrency (FR-2.3, FR-3.3)

1. **DashMap Migration** (`crates/engine/src/shadow/mod.rs`)
   - Add `dashmap = "5.5"` to engine/Cargo.toml
   - Replace `Arc<RwLock<ShadowBalanceManager>>` internals with DashMap
   - Replace `Arc<RwLock<ShadowOrderManager>>` internals with DashMap
   - Ensure all existing tests pass

2. **Lock Batching** (`crates/engine/src/engine/engine.rs`)
   - Refactor `update_balance_with_lock` calls to batch

### Phase 3: Algorithmics (FR-3.2, FR-3.4)

1. **User Order Index** (`crates/engine/src/engine/orderbook.rs`)
   - Add `user_orders: HashMap<String, HashSet<Uuid>>`
   - Update on add/remove/fill
   - Refactor `get_open_orders()` to use index

2. **Range Matching** (`crates/engine/src/engine/orderbook.rs`)
   - Use `self.asks.range(..=order.price)` for buys
   - Use `self.bids.range(order.price..)` for sells

## Completion Protocol

After EACH phase, run:
```bash
cargo test
cargo clippy --all-targets
```

When ALL phases complete and tests pass, output:
```
<promise>006-PERFORMANCE-OVERHAUL-COMPLETE</promise>
```

## Rules

1. **TDD**: Write/update tests before implementation
2. **Incremental**: Complete one FR at a time
3. **Verify**: Run tests after each change
4. **No Shortcuts**: Don't skip requirements
5. **Document**: Update CHANGELOG.md with changes

## Files to Modify

```
crates/common_utils/src/services/binance_data.rs    # HTTP timeout
crates/common_utils/src/adapters/binance_executor.rs # HTTP timeout
crates/sqlx_postgres/src/lib.rs                      # DB pool
crates/router/src/routes/trade.rs                    # Trade caching
crates/engine/src/shadow/mod.rs                      # DashMap migration
crates/engine/src/shadow/balances.rs                 # DashMap internals
crates/engine/src/shadow/orders.rs                   # DashMap internals
crates/engine/src/engine/engine.rs                   # Lock batching
crates/engine/src/engine/orderbook.rs                # User index + range matching
crates/engine/Cargo.toml                             # Add dashmap
CHANGELOG.md                                          # Document changes
```

## Progress Tracking

Check `.specify/specs/006-performance-overhaul/PROGRESS.md` for current state.
Update it as you complete each FR.
