# Specification: Actor Adapter Migration

**Spec ID:** 019b-actor-adapter-migration
**Date:** 2026-03-12
**Status:** Complete
**Class:** Architecture
**Parent:** 019-actor-model-event-sourcing
**Phase:** 2 of 6
**Depends on:** 019a-actor-infrastructure

---

## Overview

Spawn the `EngineActor` in the router's `main.rs` and migrate the two adapter layers (`ShadowExchangeApi`, `ShadowEngineAdapter`) from `Arc<RwLock<ShadowEngine>>` to `EngineHandle`. These adapters are the interface boundary between the engine and the rest of the system — migrating them first proves the actor works under real traffic before touching routes or services.

**Current state:**
- `ShadowExchangeApi` holds `Arc<RwLock<ShadowEngine>>` and calls methods through read/write locks.
- `ShadowEngineAdapter` holds `Arc<RwLock<ShadowEngine>>` and exposes engine methods to the decision loop.
- Both are constructed in `main.rs` and passed to services.

**Target state:**
- `EngineActor` spawned as a Tokio task in `main.rs` during startup.
- `EngineHandle` cloned and distributed to `ShadowExchangeApi` and `ShadowEngineAdapter`.
- Both adapters use `EngineHandle` async methods instead of direct lock access.
- `Arc<RwLock<ShadowEngine>>` still exists for unmigrated callers (routes, services).

---

## Constraint: Dual-Path Coexistence

Both `Arc<RwLock<ShadowEngine>>` and `EngineHandle` coexist. The actor wraps the same `Arc<RwLock<ShadowEngine>>` that other callers still use directly. This means the same locks are acquired by both paths — behavior is identical, just routed through the actor for migrated callers.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Spawn `EngineActor` in `main.rs` during startup, after `ShadowEngine` is created but before services are constructed. The actor task runs until the `EngineHandle` is dropped (application shutdown). | Critical | Router |
| FR-2 | Add `engine_handle: EngineHandle` to `AppState`. Keep `shadow_engine: Arc<RwLock<ShadowEngine>>` for unmigrated callers. | Critical | Router |
| FR-3 | Migrate `ShadowExchangeApi`: replace `Arc<RwLock<ShadowEngine>>` field with `EngineHandle`. Update all method implementations to use `engine_handle.method().await`. | Critical | Router |
| FR-4 | Migrate `ShadowEngineAdapter`: replace `Arc<RwLock<ShadowEngine>>` field with `EngineHandle`. Update all method implementations. | Critical | Router |
| FR-5 | Update `ShadowExchangeApi` and `ShadowEngineAdapter` constructors in `main.rs` to receive `EngineHandle` instead of `Arc<RwLock<ShadowEngine>>`. | Critical | Router |
| FR-6 | Test: trade execution through `ShadowExchangeApi` via actor produces same results as direct lock-based access. | High | Test |

---

## Technical Implementation

### 1) Spawn Actor in main.rs (FR-1, FR-2)

```rust
// After ShadowEngine creation:
let shadow_engine = Arc::new(RwLock::new(ShadowEngine::new(/* ... */)));

// Create actor channel and handle
let (engine_tx, engine_rx) = tokio::sync::mpsc::channel(256);
let engine_handle = EngineHandle::new(engine_tx);

// Spawn actor — it shares the same Arc<RwLock<ShadowEngine>>
let actor = EngineActor::new(shadow_engine.clone(), engine_rx);
tokio::spawn(actor.run());

// Both shadow_engine AND engine_handle are available
// Migrated callers get engine_handle
// Unmigrated callers still get shadow_engine.clone()
```

### 2) ShadowExchangeApi Migration (FR-3)

**File:** `crates/router/src/services/exchange_api.rs`

Before:
```rust
pub struct ShadowExchangeApi {
    engine: Arc<RwLock<ShadowEngine>>,
}

impl ExchangeApi for ShadowExchangeApi {
    async fn place_order(&self, user_id: Uuid, order: ShadowOrder) -> Result<...> {
        let engine = self.engine.read().await;
        engine.orders.write().await.place_order(user_id, order)
    }
}
```

After:
```rust
pub struct ShadowExchangeApi {
    engine: EngineHandle,
}

impl ExchangeApi for ShadowExchangeApi {
    async fn place_order(&self, user_id: Uuid, order: ShadowOrder) -> Result<...> {
        self.engine.place_order(user_id, order).await
    }
}
```

### 3) ShadowEngineAdapter Migration (FR-4)

**File:** `crates/router/src/services/shadow_adapter.rs`

Same pattern — replace `Arc<RwLock<ShadowEngine>>` with `EngineHandle`, replace lock-based method calls with `engine_handle.method().await`.

**Discovery step:** Read the full `ShadowEngineAdapter` implementation before modifying. Identify every method and which `EngineCommand` it maps to. Methods that call multiple engine operations in sequence may need multiple handle calls.

---

## Files to Modify

| File | Changes |
|------|---------|
| `crates/router/src/main.rs` | Spawn `EngineActor`, create `EngineHandle`, add to `AppState` |
| `crates/router/src/types/app.rs` | Add `engine_handle: EngineHandle` field to `AppState` |
| `crates/router/src/services/exchange_api.rs` | Replace `Arc<RwLock<ShadowEngine>>` with `EngineHandle` |
| `crates/router/src/services/shadow_adapter.rs` | Replace `Arc<RwLock<ShadowEngine>>` with `EngineHandle` |

---

## Verification

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

- [ ] `EngineActor` spawned in `main.rs` during startup
- [ ] `EngineHandle` available in `AppState`
- [ ] `ShadowExchangeApi` uses `EngineHandle` — no direct lock access
- [ ] `ShadowEngineAdapter` uses `EngineHandle` — no direct lock access
- [ ] `Arc<RwLock<ShadowEngine>>` still present for unmigrated callers
- [ ] All existing tests pass (zero regression)

---

## Completion Signal

All verification checkboxes green. `cargo clippy --all-targets && cargo test` passes. No new warnings.
