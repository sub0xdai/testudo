# Specification: Actor Model + Event Sourcing

**Spec ID:** 019-actor-model-event-sourcing
**Date:** 2026-03-12
**Status:** Decomposed
**Class:** Architecture
**Origin:** Deep architecture audit — RwLock contention analysis + crash-safety gap analysis

> **This spec has been decomposed into 6 sequential sub-specs.**
> Implement them in order. Each is independently deployable.
>
> | Phase | Spec ID | Summary |
> |-------|---------|---------|
> | 1 | [019a](../019a-actor-infrastructure/spec.md) | Build EngineCommand, EngineHandle, EngineActor alongside existing locks |
> | 2 | [019b](../019b-actor-adapter-migration/spec.md) | Spawn actor, migrate ShadowExchangeApi + ShadowEngineAdapter |
> | 3 | [019c](../019c-actor-route-migration/spec.md) | Migrate routes, decouple create_trade() network I/O |
> | 4 | [019d](../019d-actor-service-migration/spec.md) | Migrate services, fire-and-forget price updates |
> | 5 | [019e](../019e-lock-removal-zombie-detection/spec.md) | Remove all RwLocks, add in-flight zombie detection |
> | 6 | [019f](../019f-event-log-single-writer/spec.md) | Event log + single-writer persistence (Freqtrade amendment) |
>
> **Amendments incorporated from Freqtrade comparison analysis (2026-03-12):**
> - **In-Flight Zombies (019e):** `pending_placements` tracker + 15s sweep for orders that never reached the exchange
> - **Single-Writer (019f):** `TradeEventWriter` is sole PG writer — event log + mutable state in one transaction
> - **Fire-and-Forget Price (019d):** `ProcessPriceUpdate` drops oneshot, fills emitted to downstream channel

---

## Overview

Two complementary architectural changes to the ShadowEngine:

1. **Actor Model**: Replace `Arc<RwLock<ShadowEngine>>` with a Tokio actor pattern — the engine becomes a single task that owns its state, receives commands via `mpsc`, and returns responses via `oneshot`. Eliminates lock contention, race conditions, and the problem of holding locks during network I/O.

2. **Event Log**: Append-only PostgreSQL table recording every state transition. Provides financial auditability, time-travel debugging, and crash-recovery evidence. Not full event sourcing (state is NOT rebuilt from events) — this is an audit log that runs alongside existing persistence.

**Current state:**
- `Arc<RwLock<ShadowEngine>>` shared across 8 consumers. Outer lock is never write-locked at runtime (redundant). Inner `RwLock<ShadowOrderManager>`, `RwLock<OrderGroupManager>`, `RwLock<ShadowPositionManager>` are the real contention points.
- `create_trade()` holds `order_groups` write lock during 1-3 exchange round-trips (100-500ms each), blocking `list_trades()`, `process_price_update()` Phase 3, and GC.
- `process_price_update()` Phase 3 holds three write locks simultaneously, blocking all order/position/group operations.
- Read-Compute-Write race: between Phase 1 read and Phase 3 write, an HTTP handler could cancel a triggered order, leading to filling a cancelled order.
- Write-behind persistence: in-memory state changes first, DB writes after. Crash between mutation and `persist_closed()` causes state divergence.
- No audit trail for state transitions. Ghost position bugs (#3315-3318) required manual code-level investigation to diagnose.

**Target state:**
- `EngineHandle` (cheap-to-clone `mpsc::Sender` wrapper) replaces `Arc<RwLock<ShadowEngine>>` everywhere.
- Single `EngineActor` task owns all engine state — no locks, no contention, no races.
- Network I/O happens OUTSIDE the actor — callers send commands, await responses, then do I/O, then send follow-up commands.
- Every state transition is appended to `trade_events` table with full context.
- Debugging any issue = `SELECT * FROM trade_events WHERE group_id = $1 ORDER BY seq`.

---

## Constraint: Incremental Migration

This is a large refactor touching every ShadowEngine consumer. It MUST be done incrementally:

- **Phase 1**: Build `EngineHandle` + `EngineActor` alongside existing `Arc<RwLock<ShadowEngine>>`. Both work simultaneously. Existing callers unchanged.
- **Phase 2**: Migrate callers one-by-one from `shadow_engine.read().await.method()` to `engine_handle.method().await`. Each migration is independently testable and committable.
- **Phase 3**: Remove `Arc<RwLock<ShadowEngine>>` once all callers migrated. Remove inner RwLocks (actor is single-threaded).
- **Phase 4**: Wire event log into actor command processing.

Each phase compiles, passes tests, and can be deployed independently. No big-bang cutover.

---

## Functional Requirements

### Part 1: Actor Model

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Define `EngineCommand` enum with variants for every ShadowEngine operation currently called through the RwLock. Each variant carries its arguments and an embedded `oneshot::Sender` for the response. | Critical | Engine |
| FR-2 | Define `EngineHandle` struct wrapping `mpsc::Sender<EngineCommand>`. Implement async methods mirroring ShadowEngine's public API. Each method constructs a command, sends it, and awaits the oneshot response. | Critical | Engine |
| FR-3 | Implement `EngineActor` that owns `ShadowEngine` (not behind any lock) and runs a `while let Some(cmd) = rx.recv().await` loop dispatching each command to the appropriate engine method. | Critical | Engine |
| FR-4 | Spawn `EngineActor` in `main.rs` during startup. Clone `EngineHandle` to all consumers that currently receive `Arc<RwLock<ShadowEngine>>`. | Critical | Router |
| FR-5 | Migrate `ShadowExchangeApi` to use `EngineHandle` instead of `Arc<RwLock<ShadowEngine>>`. | Critical | Router |
| FR-6 | Migrate `ShadowEngineAdapter` to use `EngineHandle`. | Critical | Router |
| FR-7 | Migrate `PriceFeedService` to use `EngineHandle`. | Critical | Router |
| FR-8 | Migrate trade management routes (`create_trade`, `list_trades`, `get_trade`, `cancel_trade`, `update_stop_loss`, `update_take_profit`, `update_entry_price`, `enable_break_even`) to use `EngineHandle`. | Critical | Router |
| FR-9 | Migrate order routes (`execute_order`, `get_open_orders`, `cancel_all_orders`) to use `EngineHandle`. | High | Router |
| FR-10 | Migrate paper balance routes to use `EngineHandle`. | High | Router |
| FR-11 | Migrate GC task to use `EngineHandle`. | High | Router |
| FR-12 | Migrate `RehydrationService` to use `EngineHandle`. Rehydration commands populate the actor's owned state before the HTTP server starts. | High | Router |
| FR-13 | Remove `Arc<RwLock<ShadowEngine>>` from `main.rs` and `AppState`. Remove inner `RwLock` wrappers on `ShadowOrderManager`, `ShadowPositionManager`, `OrderGroupManager` — they are no longer needed since only the actor task touches them. | Medium | Engine |
| FR-14 | `EngineHandle` channel capacity: 256 commands. If backpressure occurs, callers block (`.send().await`) — this is correct, it means the engine is saturated and callers should wait. | Medium | Engine |

### Part 2: Event Log

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-15 | Create `trade_events` table: `(seq BIGSERIAL PRIMARY KEY, event_type TEXT NOT NULL, group_id UUID, user_id UUID NOT NULL, symbol TEXT, payload JSONB NOT NULL, created_at TIMESTAMPTZ DEFAULT now())`. Index on `(group_id, seq)` and `(user_id, created_at)`. | Critical | Database |
| FR-16 | Define `TradeEvent` enum: `TradeCreated`, `EntryPlaced`, `EntryFilled { fill_price }`, `StopLossPlaced`, `StopLossFilled`, `TakeProfitPlaced`, `TakeProfitFilled`, `OrderCancelled { reason }`, `GroupStatusChanged { from, to }`, `BreakEvenTriggered`, `StopLossAmended { old_price, new_price }`, `ReconciliationAction { action }`. | Critical | Engine |
| FR-17 | The `EngineActor` emits events to an `mpsc::Sender<TradeEvent>` after processing each command that mutates state. Events are fire-and-forget from the actor's perspective (non-blocking `try_send`). | Critical | Engine |
| FR-18 | Spawn a `TradeEventWriter` background task that batches events from the mpsc channel and writes them to `trade_events` via `INSERT ... VALUES` in batches (flush every 100ms or 50 events, whichever comes first). | High | Router |
| FR-19 | Add `GET /api/v1/trades/{id}/events` endpoint returning the event history for a trade group, ordered by `seq`. Authenticated, user-scoped. | Medium | Router |
| FR-20 | Live trades MUST have events logged. Shadow/paper trades SHOULD have events logged (for debugging) but MAY be gated by a config flag if the write volume is too high. | Medium | Router |

### Part 3: Testing

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-21 | Test: `EngineHandle` can process 10,000 sequential commands without deadlock or timeout. | Critical | Test |
| FR-22 | Test: concurrent callers (10 tasks) sending interleaved place/cancel/list commands through `EngineHandle` produce consistent state. | Critical | Test |
| FR-23 | Test: `process_price_update` through the actor does not race with `cancel_order` — a cancelled order is never filled. | Critical | Test |
| FR-24 | Test: `create_trade` through the actor does NOT hold engine state during simulated exchange I/O delay. Verify that `list_trades` returns immediately even when a `create_trade` is mid-flight. | Critical | Test |
| FR-25 | Test: events emitted by the actor for a full trade lifecycle (create -> entry fill -> SL fill) match expected sequence. | High | Test |

---

## Technical Implementation

### 1) EngineCommand Enum (FR-1)

**New file:** `crates/engine/src/shadow/actor.rs`

```rust
use tokio::sync::oneshot;

pub enum EngineCommand {
    // User management
    UserExists { user_id: Uuid, reply: oneshot::Sender<bool> },
    InitUser { user_id: Uuid, reply: oneshot::Sender<()> },

    // Orders
    PlaceOrder { user_id: Uuid, order: ShadowOrder, reply: oneshot::Sender<Result<ShadowOrder, EngineError>> },
    PlaceOrderNoGroup { user_id: Uuid, order: ShadowOrder, reply: oneshot::Sender<Result<ShadowOrder, EngineError>> },
    CancelOrder { user_id: Uuid, order_id: Uuid, reply: oneshot::Sender<Result<(), EngineError>> },
    CancelOrderNoCascade { user_id: Uuid, order_id: Uuid, reply: oneshot::Sender<Result<(), EngineError>> },
    GetOrder { order_id: Uuid, reply: oneshot::Sender<Option<ShadowOrder>> },
    GetOpenOrders { user_id: Uuid, reply: oneshot::Sender<Vec<ShadowOrder>> },

    // Balances (could bypass actor via DashMap, but routing through for consistency)
    GetBalances { user_id: Uuid, reply: oneshot::Sender<HashMap<String, ShadowBalance>> },
    ResetUser { user_id: Uuid, reply: oneshot::Sender<()> },

    // Positions
    GetPositions { user_id: Uuid, reply: oneshot::Sender<Vec<ShadowPosition>> },
    GetUnrealizedPnl { user_id: Uuid, reply: oneshot::Sender<Decimal> },

    // Price processing
    ProcessPriceUpdate { symbol: String, bid: Decimal, ask: Decimal, high: Decimal, low: Decimal,
                         reply: oneshot::Sender<PriceUpdateResult> },
    GetActiveSymbols { reply: oneshot::Sender<HashSet<String>> },
    CheckBreakEven { symbol: String, current_price: Decimal, reply: oneshot::Sender<Vec<BreakEvenResult>> },

    // Order groups
    ListTradeGroups { user_id: Uuid, reply: oneshot::Sender<Vec<OrderGroup>> },
    GetTradeGroup { group_id: Uuid, reply: oneshot::Sender<Option<OrderGroup>> },
    GetGroupByExchangeOrder { exchange_order_id: String, reply: oneshot::Sender<Option<OrderGroup>> },

    // Mutations on groups (currently done by callers holding inner write locks)
    RegisterExchangeOrderId { group_id: Uuid, role: OrderRole, exchange_id: String, reply: oneshot::Sender<Result<(), EngineError>> },
    UpdateGroupStatus { group_id: Uuid, status: OrderGroupStatus, reply: oneshot::Sender<Result<(), EngineError>> },
    OnEntryFilled { group_id: Uuid, fill_price: Decimal, reply: oneshot::Sender<Result<(), EngineError>> },
    OnStopLossFilled { group_id: Uuid, reply: oneshot::Sender<Result<(), EngineError>> },
    OnTakeProfitFilled { group_id: Uuid, reply: oneshot::Sender<Result<(), EngineError>> },

    // GC
    PruneTerminal { cutoff: std::time::Instant, reply: oneshot::Sender<usize> },

    // Rehydration (bulk load at startup)
    LoadOrderGroups { groups: Vec<OrderGroup>, reply: oneshot::Sender<()> },
}
```

### 2) EngineHandle (FR-2)

```rust
#[derive(Clone)]
pub struct EngineHandle {
    tx: mpsc::Sender<EngineCommand>,
}

impl EngineHandle {
    pub async fn place_order(&self, user_id: Uuid, order: ShadowOrder) -> Result<ShadowOrder, EngineError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx.send(EngineCommand::PlaceOrder { user_id, order, reply: reply_tx }).await
            .map_err(|_| EngineError::ActorShutdown)?;
        reply_rx.await.map_err(|_| EngineError::ActorShutdown)?
    }

    pub async fn list_trade_groups(&self, user_id: Uuid) -> Vec<OrderGroup> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.send(EngineCommand::ListTradeGroups { user_id, reply: reply_tx }).await;
        reply_rx.await.unwrap_or_default()
    }

    // ... one method per command variant
}
```

### 3) EngineActor (FR-3)

```rust
pub struct EngineActor {
    engine: ShadowEngine,  // OWNED, not behind any lock
    rx: mpsc::Receiver<EngineCommand>,
    event_tx: Option<mpsc::Sender<TradeEvent>>,  // FR-17
}

impl EngineActor {
    pub async fn run(mut self) {
        while let Some(cmd) = self.rx.recv().await {
            match cmd {
                EngineCommand::PlaceOrder { user_id, order, reply } => {
                    let result = self.engine.place_order(user_id, order).await;
                    if let Ok(ref placed) = result {
                        self.emit_event(TradeEvent::EntryPlaced { /* ... */ });
                    }
                    let _ = reply.send(result);
                }
                EngineCommand::ProcessPriceUpdate { symbol, bid, ask, high, low, reply } => {
                    // No more 3-phase RCW needed — we own the state exclusively
                    let result = self.engine.process_price_update_atomic(&symbol, bid, ask, high, low);
                    for fill in &result.filled {
                        self.emit_event(TradeEvent::from_fill(fill));
                    }
                    let _ = reply.send(result);
                }
                // ... dispatch all variants
            }
        }
    }

    fn emit_event(&self, event: TradeEvent) {
        if let Some(tx) = &self.event_tx {
            let _ = tx.try_send(event);  // non-blocking, drop if full
        }
    }
}
```

**Key insight:** `process_price_update` no longer needs the 3-phase Read-Compute-Write pattern. Since the actor owns the state exclusively, it can read, compute, and write in one uninterrupted sequence. No concurrent access is possible. The race condition where a cancelled order gets filled is structurally eliminated.

### 4) Decoupling Network I/O from Engine State (FR-8)

The biggest win. Current `create_trade()` flow:

```
acquire engine read lock
  acquire order_groups write lock
    place shadow order
    place entry on exchange (network I/O, 100-500ms)  <-- LOCK HELD
    place SL on exchange (network I/O)                 <-- LOCK HELD
    place TP on exchange (network I/O)                 <-- LOCK HELD
    register exchange order IDs
  release order_groups write lock
release engine read lock
```

New flow with actor:

```
engine_handle.place_order(order).await          // actor processes instantly, returns
place entry on exchange (network I/O)           // NO ENGINE STATE HELD
place SL on exchange (network I/O)              // NO ENGINE STATE HELD
place TP on exchange (network I/O)              // NO ENGINE STATE HELD
engine_handle.register_exchange_ids(ids).await  // actor processes instantly, returns
```

The actor processes each command in microseconds. Network I/O happens entirely outside the actor's command loop. Other callers (list_trades, process_price_update) are never blocked by exchange latency.

### 5) trade_events Table (FR-15)

**New migration:** `crates/sqlx_postgres/migrations/YYYYMMDD_trade_events.up.sql`

```sql
CREATE TABLE trade_events (
    seq         BIGSERIAL PRIMARY KEY,
    event_type  TEXT NOT NULL,
    group_id    UUID,
    user_id     UUID NOT NULL,
    symbol      TEXT,
    payload     JSONB NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_trade_events_group ON trade_events (group_id, seq);
CREATE INDEX idx_trade_events_user  ON trade_events (user_id, created_at);
```

### 6) TradeEventWriter (FR-18)

```rust
pub struct TradeEventWriter {
    rx: mpsc::Receiver<TradeEvent>,
    pool: PgPool,
}

impl TradeEventWriter {
    pub async fn run(mut self) {
        let mut batch: Vec<TradeEvent> = Vec::with_capacity(50);
        let mut flush_interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                Some(event) = self.rx.recv() => {
                    batch.push(event);
                    if batch.len() >= 50 {
                        self.flush(&mut batch).await;
                    }
                }
                _ = flush_interval.tick() => {
                    if !batch.is_empty() {
                        self.flush(&mut batch).await;
                    }
                }
            }
        }
    }

    async fn flush(&self, batch: &mut Vec<TradeEvent>) {
        // Bulk INSERT via sqlx query builder
        // On failure: log error, retain batch for retry
    }
}
```

### 7) Balances: Keep DashMap (Optimization)

The `ShadowBalanceManager` already uses `DashMap` for lock-free per-user access. Routing balance reads through the actor would serialize them unnecessarily. Two options:

**Option A (Simple):** Route everything through actor. Balances are fast (DashMap lookup), so the serialization cost is negligible for current scale.

**Option B (Optimized):** Keep `ShadowBalanceManager` as `Arc<ShadowBalanceManager>` shared directly. The actor and callers both access it without going through the command channel.

**Decision:** Start with Option A for simplicity. Optimize to Option B only if profiling shows the actor is a bottleneck on balance reads.

---

## Files to Modify

| File | Changes |
|------|---------|
| `crates/engine/src/shadow/actor.rs` | **NEW** — `EngineCommand`, `EngineHandle`, `EngineActor` |
| `crates/engine/src/shadow/mod.rs` | Remove `Arc<RwLock<...>>` wrappers on inner fields. Add `process_price_update_atomic()` method. Export actor module. |
| `crates/engine/src/shadow/orders.rs` | Remove `RwLock` wrapper. Methods take `&mut self` instead of being called through `.write().await`. |
| `crates/engine/src/shadow/positions.rs` | Remove `RwLock` wrapper. Methods take `&mut self`. |
| `crates/engine/src/shadow/order_group.rs` | Remove `RwLock` wrapper. Methods take `&mut self`. |
| `crates/router/src/main.rs` | Spawn `EngineActor`. Create `EngineHandle`. Replace `Arc<RwLock<ShadowEngine>>` with `EngineHandle` in all service constructors. Spawn `TradeEventWriter`. |
| `crates/router/src/types/app.rs` | Replace `shadow_engine: Arc<RwLock<ShadowEngine>>` with `engine: EngineHandle` in `AppState`. |
| `crates/router/src/routes/trade_management.rs` | Replace all `state.engine.read().await.method()` with `state.engine.method().await`. Restructure `create_trade()` to do network I/O outside actor commands. |
| `crates/router/src/routes/order.rs` | Replace lock-based access with `EngineHandle` calls. |
| `crates/router/src/routes/paper_balance.rs` | Replace lock-based access with `EngineHandle` calls. |
| `crates/router/src/services/exchange_api.rs` | `ShadowExchangeApi` holds `EngineHandle` instead of `Arc<RwLock<ShadowEngine>>`. |
| `crates/router/src/services/shadow_adapter.rs` | `ShadowEngineAdapter` holds `EngineHandle`. |
| `crates/router/src/services/price_feed.rs` | Replace lock-based access with `EngineHandle` calls. |
| `crates/router/src/services/fill_detector.rs` | Replace `Arc<RwLock<OrderGroupManager>>` with `EngineHandle`. |
| `crates/router/src/services/reconciliation.rs` | Replace `Arc<RwLock<OrderGroupManager>>` with `EngineHandle`. |
| `crates/router/src/services/rehydration.rs` | Replace `Arc<RwLock<ShadowEngine>>` with `EngineHandle`. Use `LoadOrderGroups` command. |
| `crates/router/src/routes/trade_events.rs` | **NEW** — `GET /api/v1/trades/{id}/events` endpoint. |
| `crates/router/src/services/trade_event_writer.rs` | **NEW** — `TradeEventWriter` background task. |
| `crates/router/src/services/trade_manager/types.rs` | **NEW** — `TradeEvent` enum definition. |
| `crates/sqlx_postgres/migrations/YYYYMMDD_trade_events.up.sql` | **NEW** — `trade_events` table + indexes. |

---

## Migration Strategy

### Phase 1: Build Actor (FR-1, FR-2, FR-3) — No callers changed

Create `actor.rs` with `EngineCommand`, `EngineHandle`, `EngineActor`. The actor wraps an `Arc<RwLock<ShadowEngine>>` internally (temporary — it acquires locks just like current callers). This lets us test the actor in isolation without changing any existing code.

**Verify:** `cargo test` — all existing tests pass, new actor unit tests pass.

### Phase 2: Spawn Actor + Migrate Callers (FR-4 through FR-12)

Spawn actor in `main.rs`. Migrate callers one file at a time. Each file migration is a separate commit. The actor still uses `Arc<RwLock<ShadowEngine>>` internally, so behavior is identical.

**Verify:** After each file migration, `cargo clippy --all-targets && cargo test`.

### Phase 3: Remove Locks (FR-13)

Once all callers use `EngineHandle`, the actor is the sole accessor of `ShadowEngine`. Remove `Arc<RwLock<...>>` from inner fields. The actor owns `ShadowEngine` directly.

**Verify:** `cargo clippy --all-targets && cargo test`. Lock-free compilation.

### Phase 4: Event Log (FR-15 through FR-20)

Add `trade_events` table. Wire `TradeEvent` emission into actor. Spawn `TradeEventWriter`. Add events endpoint.

**Verify:** Full test suite + manual verification of event output.

---

## Verification

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

- [ ] `EngineCommand` enum covers all ShadowEngine public methods
- [ ] `EngineHandle` compiles with same API surface as current lock-based access
- [ ] `EngineActor` processes commands sequentially without deadlock
- [ ] All callers migrated from `Arc<RwLock<ShadowEngine>>` to `EngineHandle`
- [ ] `Arc<RwLock<ShadowEngine>>` removed from codebase
- [ ] Inner `RwLock` wrappers removed from ShadowEngine fields
- [ ] `create_trade()` does NOT hold engine state during exchange I/O
- [ ] `list_trades()` responds immediately even during concurrent `create_trade()`
- [ ] `process_price_update` cannot race with `cancel_order`
- [ ] `trade_events` table created with indexes
- [ ] Events emitted for full trade lifecycle
- [ ] `GET /api/v1/trades/{id}/events` returns event history
- [ ] 10,000 sequential commands processed without deadlock
- [ ] 10 concurrent callers produce consistent state
- [ ] All existing tests pass (zero regression)

---

## Completion Signal

All verification checkboxes green. `cargo clippy --all-targets && cargo test` passes. No new warnings. Lock contention eliminated. Event audit trail queryable.
