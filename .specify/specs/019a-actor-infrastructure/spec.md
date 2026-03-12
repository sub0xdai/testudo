# Specification: Actor Infrastructure

**Spec ID:** 019a-actor-infrastructure
**Date:** 2026-03-12
**Status:** Complete
**Class:** Architecture
**Parent:** 019-actor-model-event-sourcing
**Phase:** 1 of 6
**Depends on:** None (greenfield)

---

## Overview

Build `EngineCommand`, `EngineHandle`, and `EngineActor` alongside the existing `Arc<RwLock<ShadowEngine>>`. No existing callers are changed. The actor wraps `Arc<RwLock<ShadowEngine>>` internally as a temporary bridge — it acquires locks exactly like current callers do. This lets us test the actor in complete isolation without touching any existing code.

**Current state:**
- `Arc<RwLock<ShadowEngine>>` shared across 8 consumers.
- No actor infrastructure exists.

**Target state:**
- `EngineCommand` enum covers every ShadowEngine public method.
- `EngineHandle` wraps `mpsc::Sender<EngineCommand>` with async methods mirroring ShadowEngine's API.
- `EngineActor` dispatches commands to the engine via a sequential `recv()` loop.
- All new code compiles and is tested. Zero changes to existing callers.

---

## Constraint: No Existing Code Modified

This spec adds NEW files only. No existing file is modified. The actor uses `Arc<RwLock<ShadowEngine>>` internally to call the same methods current callers use. This is a temporary bridge removed in 019e.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Define `EngineCommand` enum with variants for every ShadowEngine public operation. Each variant carries its arguments and an embedded `oneshot::Sender<T>` for the typed response. | Critical | Engine |
| FR-2 | Define `EngineHandle` struct wrapping `mpsc::Sender<EngineCommand>`. Implement async methods mirroring ShadowEngine's public API. Each method constructs a command, sends it, and awaits the oneshot. | Critical | Engine |
| FR-3 | Define `EngineError` enum with at minimum `ActorShutdown` (channel closed) and `Internal(String)` variants. | Critical | Engine |
| FR-4 | Implement `EngineActor` struct holding `Arc<RwLock<ShadowEngine>>` and `mpsc::Receiver<EngineCommand>`. The `run()` method dispatches each command to the appropriate engine method via the existing lock-based API. | Critical | Engine |
| FR-5 | Channel capacity: `mpsc::channel(256)`. Callers block on backpressure — this is correct behavior (engine saturated, callers should wait). | Medium | Engine |
| FR-6 | Test: 10,000 sequential commands processed without deadlock or timeout (place_order, list_trade_groups, get_positions). | Critical | Test |
| FR-7 | Test: 10 concurrent tasks sending interleaved place/cancel/list commands through `EngineHandle` produce consistent state. | Critical | Test |
| FR-8 | Test: actor shutdown (drop the `EngineHandle`) causes pending `recv()` to return `None` and the actor exits cleanly. | High | Test |

---

## Technical Implementation

### 1) EngineCommand (FR-1)

**New file:** `crates/engine/src/shadow/actor.rs`

Every public method on `ShadowEngine`, `ShadowOrderManager`, `ShadowPositionManager`, `OrderGroupManager`, and `ShadowBalanceManager` that callers currently access through the RwLock gets a corresponding command variant.

Categorized by subsystem:

```rust
pub enum EngineCommand {
    // --- User management ---
    UserExists { user_id: Uuid, reply: oneshot::Sender<bool> },
    InitUser { user_id: Uuid, reply: oneshot::Sender<()> },

    // --- Orders ---
    PlaceOrder { user_id: Uuid, order: ShadowOrder, reply: oneshot::Sender<Result<ShadowOrder, EngineError>> },
    PlaceOrderNoGroup { user_id: Uuid, order: ShadowOrder, reply: oneshot::Sender<Result<ShadowOrder, EngineError>> },
    CancelOrder { user_id: Uuid, order_id: Uuid, reply: oneshot::Sender<Result<(), EngineError>> },
    CancelOrderNoCascade { user_id: Uuid, order_id: Uuid, reply: oneshot::Sender<Result<(), EngineError>> },
    GetOrder { order_id: Uuid, reply: oneshot::Sender<Option<ShadowOrder>> },
    GetOpenOrders { user_id: Uuid, reply: oneshot::Sender<Vec<ShadowOrder>> },

    // --- Balances ---
    GetBalances { user_id: Uuid, reply: oneshot::Sender<HashMap<String, ShadowBalance>> },
    ResetUser { user_id: Uuid, reply: oneshot::Sender<()> },

    // --- Positions ---
    GetPositions { user_id: Uuid, reply: oneshot::Sender<Vec<ShadowPosition>> },
    GetUnrealizedPnl { user_id: Uuid, reply: oneshot::Sender<Decimal> },

    // --- Price processing ---
    ProcessPriceUpdate { symbol: String, bid: Decimal, ask: Decimal, high: Decimal, low: Decimal,
                         reply: oneshot::Sender<PriceUpdateResult> },
    GetActiveSymbols { reply: oneshot::Sender<HashSet<String>> },
    CheckBreakEven { symbol: String, current_price: Decimal, reply: oneshot::Sender<Vec<BreakEvenResult>> },

    // --- Order groups ---
    ListTradeGroups { user_id: Uuid, reply: oneshot::Sender<Vec<OrderGroup>> },
    GetTradeGroup { group_id: Uuid, reply: oneshot::Sender<Option<OrderGroup>> },
    GetGroupByExchangeOrder { exchange_order_id: String, reply: oneshot::Sender<Option<OrderGroup>> },
    RegisterExchangeOrderId { group_id: Uuid, role: OrderRole, exchange_id: String,
                              reply: oneshot::Sender<Result<(), EngineError>> },
    UpdateGroupStatus { group_id: Uuid, status: OrderGroupStatus,
                        reply: oneshot::Sender<Result<(), EngineError>> },
    OnEntryFilled { group_id: Uuid, fill_price: Decimal, reply: oneshot::Sender<Result<(), EngineError>> },
    OnStopLossFilled { group_id: Uuid, reply: oneshot::Sender<Result<(), EngineError>> },
    OnTakeProfitFilled { group_id: Uuid, reply: oneshot::Sender<Result<(), EngineError>> },

    // --- GC ---
    PruneTerminal { cutoff: std::time::Instant, reply: oneshot::Sender<usize> },

    // --- Rehydration ---
    LoadOrderGroups { groups: Vec<OrderGroup>, reply: oneshot::Sender<()> },
}
```

**Discovery step before implementation:** Read every caller of `shadow_engine.read().await` and `shadow_engine.write().await` across the router crate. Grep for `.orders.read()`, `.orders.write()`, `.order_groups.read()`, `.order_groups.write()`, `.positions.read()`, `.positions.write()`. Every method called through a lock must have a corresponding command variant. If the grep reveals methods not listed above, add them.

### 2) EngineHandle (FR-2)

Same file: `crates/engine/src/shadow/actor.rs`

```rust
#[derive(Clone)]
pub struct EngineHandle {
    tx: mpsc::Sender<EngineCommand>,
}

impl EngineHandle {
    pub fn new(tx: mpsc::Sender<EngineCommand>) -> Self {
        Self { tx }
    }

    pub async fn place_order(&self, user_id: Uuid, order: ShadowOrder) -> Result<ShadowOrder, EngineError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx.send(EngineCommand::PlaceOrder { user_id, order, reply: reply_tx })
            .await
            .map_err(|_| EngineError::ActorShutdown)?;
        reply_rx.await.map_err(|_| EngineError::ActorShutdown)?
    }

    pub async fn list_trade_groups(&self, user_id: Uuid) -> Vec<OrderGroup> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.send(EngineCommand::ListTradeGroups { user_id, reply: reply_tx }).await;
        reply_rx.await.unwrap_or_default()
    }

    // One method per command variant. Methods that return Result propagate
    // EngineError::ActorShutdown if the channel closes. Methods that return
    // Option or Vec use unwrap_or_default() for graceful degradation.
}
```

### 3) EngineActor (FR-4)

```rust
pub struct EngineActor {
    engine: Arc<RwLock<ShadowEngine>>,  // TEMPORARY — removed in 019e
    rx: mpsc::Receiver<EngineCommand>,
}

impl EngineActor {
    pub fn new(engine: Arc<RwLock<ShadowEngine>>, rx: mpsc::Receiver<EngineCommand>) -> Self {
        Self { engine, rx }
    }

    pub async fn run(mut self) {
        tracing::info!("EngineActor started");
        while let Some(cmd) = self.rx.recv().await {
            self.dispatch(cmd).await;
        }
        tracing::info!("EngineActor shut down — channel closed");
    }

    async fn dispatch(&self, cmd: EngineCommand) {
        match cmd {
            EngineCommand::PlaceOrder { user_id, order, reply } => {
                let engine = self.engine.read().await;
                let result = engine.orders.write().await.place_order(user_id, order);
                let _ = reply.send(result.map_err(|e| EngineError::Internal(e.to_string())));
            }
            EngineCommand::ListTradeGroups { user_id, reply } => {
                let engine = self.engine.read().await;
                let groups = engine.order_groups.read().await.list_groups(user_id);
                let _ = reply.send(groups);
            }
            // ... dispatch all variants using the same lock patterns
            // that current callers use. Copy-paste the lock acquisition
            // from existing call sites.
        }
    }
}
```

**Implementation note:** The dispatch method acquires locks exactly like current callers do. This is intentionally temporary — it proves the actor interface works without changing locking semantics. Lock removal happens in 019e.

---

## Files to Create

| File | Contents |
|------|----------|
| `crates/engine/src/shadow/actor.rs` | `EngineCommand`, `EngineHandle`, `EngineActor`, `EngineError` |
| `crates/engine/src/shadow/actor_tests.rs` | Unit tests for FR-6, FR-7, FR-8 |

## Files to Modify

| File | Changes |
|------|---------|
| `crates/engine/src/shadow/mod.rs` | Add `pub mod actor;` |

---

## Verification

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

- [ ] `EngineCommand` has a variant for every ShadowEngine public method (verified by grep)
- [ ] `EngineHandle` compiles with same API surface as current lock-based access
- [ ] `EngineActor::run()` processes commands sequentially without deadlock
- [ ] 10,000 sequential commands test passes
- [ ] 10 concurrent callers test passes
- [ ] Actor shutdown test passes (clean exit on channel close)
- [ ] All existing tests pass (zero regression — no existing code modified)

---

## Completion Signal

All verification checkboxes green. `cargo clippy --all-targets && cargo test` passes. No new warnings.
