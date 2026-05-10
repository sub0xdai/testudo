# Specification: Asynchronous Order Pipeline (SDD V2)

**Spec ID:** ENG-04-async-order-pipeline
**Date:** 2026-04-25
**Status:** Draft
**Class:** Infrastructure / Core
**Priority:** P1 — Removes HTTP-worker head-of-line blocking on sidecar latency, unblocks reliable retry/replay semantics, and establishes the paved road for worker-scaled execution.
**Depends on:** None
**Supersedes:** ENG-03-risk-raas-extraction (see *Supersession* below)

---

## Problem Statement

The current `POST /order` path is fully synchronous. The Actix handler blocks on:
1. `DecisionLoop::validate` (in-process, sub-ms — not the bottleneck)
2. `CexClient` HTTP call to the CCXT / Hyperliquid sidecar (~5–20 ms happy path, multi-second on sidecar stalls)
3. Response construction from the sidecar body

Consequences:
- **HTTP worker head-of-line blocking.** A single slow sidecar response holds an Actix worker, which under a TradingView burst exhausts the pool and tail-latencies every other caller.
- **No replay / retry semantics.** If the sidecar succeeds but the HTTP response is lost (client disconnect, worker panic), there is no durable "in-flight" record to reconcile against. FIX-08 (Bybit fill reconciliation) had to be bolted on precisely because of this gap.
- **No pipeline visibility.** Orders don't have a persisted state machine — ops has to reconstruct "what happened" from trade_events logs after the fact.
- **Coupling of concerns.** Risk-gating and exchange-dispatch run serialized under one worker lifetime; scaling either is scaling the entire handler.

We already own the primitives to fix this: `pg_queue` ships `queue_orders` with `FOR UPDATE SKIP LOCKED` claiming and LISTEN/NOTIFY. Today it is used only as a FIFO buffer; no explicit pipeline state machine sits on the row, and the HTTP handler doesn't round-trip through it.

This spec introduces an async pipeline where HTTP ingress writes only to `queue_orders`, dedicated workers run risk-gating and sidecar dispatch off-request, and the HTTP handler awaits a completion event reassembled via a single shared `PgListener` + a per-request `oneshot` channel. Target: **<25 ms P99 end-to-end** while decoupling failure modes.

### Supersession of ENG-03

ENG-03 proposed extracting risk to a gRPC microservice (`testudo-raas`) on the premise that `Decimal` math and monolith coupling imposed a "Risk Tax" on every trade. Investigation (see ENG-03 supersession note) showed the "Risk Tax" does not exist on the hot path — `RiskService::validate()` is synchronous Decimal math over an in-memory config, sub-microsecond per call. Moving it behind gRPC *adds* latency.

ENG-04 targets the real hot-path tax — the sidecar roundtrip — by moving it off the HTTP request path entirely. Under ENG-04, `DecisionLoop` runs in a queue worker, not on the user-visible thread, so whatever residual cost it has stops being user-visible without any extraction. ENG-03 is therefore retired as redundant.

---

## User Stories

- **As a trader**, I want my order ack within 25 ms P99 even when the exchange sidecar stalls, so that my TradingView Alt+X flow never feels stuck.
- **As a backend operator**, I want every order to have a durable pipeline state (RECEIVED → GATED → DISPATCHED → FILLED), so that I can replay or reconcile without reading trade_events.
- **As a reliability engineer**, I want HTTP workers decoupled from sidecar latency, so that a slow exchange doesn't exhaust the Actix pool and tail-latency every caller.
- **As a quant**, I want risk-gating and exchange-dispatch scalable independently, so that I can add risk workers without scaling HTTP ingress.

---

## Functional Requirements

| ID   | Requirement | Priority | Subsystem |
|------|-------------|----------|-----------|
| FR-1 | Define `InboundOrderRequest` JSON schema as the stable HTTP ingress boundary | High | Boundary |
| FR-2 | Add `pipeline_state` enum column to `queue_orders` with monotonic CHECK constraint (`RECEIVED → GATING → GATED → DISPATCHED → FILLED`, plus terminal `REJECTED`/`FAILED`) | High | Queue |
| FR-3 | `RiskWorker` claims `RECEIVED` rows via `FOR UPDATE SKIP LOCKED`, runs `DecisionLoop`, transitions to `GATED` or `REJECTED` | High | Worker |
| FR-4 | `DispatchWorker` claims `GATED` rows, calls `CexClient`, transitions to `DISPATCHED` on ack or `FAILED` on timeout/error | High | Worker |
| FR-5 | Existing fill pipeline transitions `DISPATCHED` → `FILLED` when the fill_detector observes exchange confirmation | High | Detector |
| FR-6 | `ListenDispatcher` — single `tokio` task on a shared `PgListener` consuming the `order_updates` channel, routing `(request_id, state, payload)` notifications via `DashMap<Uuid, oneshot::Sender<OrderResponse>>` | High | Boundary |
| FR-7 | HTTP handler writes row, registers oneshot in the `DashMap`, awaits with a per-mode timeout (LIVE: 25 ms soft / 2 s hard; SHADOW: 100 ms hard), removes entry on drop | High | Boundary |
| FR-8 | Prometheus histograms per stage (`ingress_write`, `gating_claim_to_decision`, `dispatch_send_to_ack`, `e2e_ingress_to_response`) with P99 alerts | Medium | Observability |
| FR-9 | Migration backward-compatible: old rows default to `FILLED` or `REJECTED` terminal state based on existing `status`/`processed_at`; add-only columns, no data rewrite | High | Migration |
| FR-10 | Feature flag `async_order_pipeline_enabled` (env-driven) allows canary rollout; legacy sync path remains executable until flag is flipped in prod for 1 week | Medium | Rollout |

---

## Technical Implementation

### Vertical Checkpoints

Each checkpoint is end-to-end, committable, and independently tested.

| CP  | Scope | Validates |
|-----|-------|-----------|
| CP-1 | Migration + `InboundOrderRequest` DTO + HTTP handler writes `RECEIVED` row (no worker yet, handler returns 202 with request_id) | Schema change applies; ingress write succeeds; row visible in `RECEIVED` state |
| CP-2 | `RiskWorker` loop with `SKIP LOCKED` + DecisionLoop integration; transitions `RECEIVED` → `GATED`/`REJECTED` | Integration test: seeded row → worker claims → state transition matches DecisionLoop verdict |
| CP-3 | `DispatchWorker` with `CexClient` mock; transitions `GATED` → `DISPATCHED`/`FAILED` | Integration test: seeded `GATED` row → worker calls mock sidecar → state + exchange_order_id persisted |
| CP-4 | `ListenDispatcher` + oneshot reassembly; HTTP handler awaits and returns synthesized `OrderResponse` | Integration test: POST /order → handler returns within 25 ms when workers + sidecar mocked to instant |
| CP-5 | Prometheus wiring + k6/oha load harness; feature flag plumbing | Load test: 100 req/s burst, all stages report, P99 < 25 ms under happy-path mock sidecar |
| CP-6 | Legacy sync path deletion + flag removal after 1 week soak | `cargo clippy --all-targets` clean, no dead code warnings |

### I/O Boundary: `InboundOrderRequest`

Stable JSON contract at `POST /order`. Existing handler input unchanged shape except for the addition of `request_id` (client-generated UUIDv4) and `timestamp_ns` (client monotonic clock for end-to-end tracing).

```json
{
  "request_id": "018f2b9a-...-uuidv4",
  "user_id": "018f2b9a-...-uuidv4",
  "symbol": "BTC_USDT",
  "side": "LONG",
  "type": "LIMIT",
  "quantity": "0.015",
  "price": "65400.50",
  "leverage": 10,
  "stop_loss": "64000.00",
  "take_profit": "68000.00",
  "execution_mode": "LIVE",
  "timestamp_ns": 1713961200000000000
}
```

All decimal fields remain strings (CLAUDE.md rule: no f64 on the wire). `request_id` is the oneshot correlation key.

### Pipeline State Machine

```
RECEIVED ──(RiskWorker claim)──▶ GATING ──(decision)──┬──▶ GATED ──(DispatchWorker claim)──▶ DISPATCHING ──(sidecar ack)──┬──▶ DISPATCHED ──(fill_detector)──▶ FILLED
                                                       │                                                                    │
                                                       └──▶ REJECTED (terminal)                                              └──▶ FAILED (terminal)
```

Enforced at the DB layer via a CHECK constraint that validates the `(old_state, new_state)` transition is in the allowed set. Intermediate `GATING`/`DISPATCHING` states are the "claimed" marker so a crashed worker leaves an orphan row reclaimable by a separate sweeper after `claim_timeout_ms`.

```sql
ALTER TABLE queue_orders
  ADD COLUMN request_id UUID UNIQUE,
  ADD COLUMN pipeline_state TEXT NOT NULL DEFAULT 'RECEIVED',
  ADD COLUMN pipeline_state_updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  ADD COLUMN claimed_by TEXT,              -- worker id, for stale-claim recovery
  ADD COLUMN claim_deadline TIMESTAMPTZ,   -- when to reclaim if not progressed
  ADD COLUMN exchange_order_id TEXT,
  ADD COLUMN rejection_reason TEXT,
  ADD CONSTRAINT pipeline_state_valid
    CHECK (pipeline_state IN ('RECEIVED','GATING','GATED','DISPATCHING',
                              'DISPATCHED','FILLED','REJECTED','FAILED'));

CREATE INDEX idx_queue_orders_pipeline
  ON queue_orders (pipeline_state, created_at)
  WHERE pipeline_state IN ('RECEIVED','GATED','DISPATCHED');
```

### Concurrency Topology: `ListenDispatcher`

One `PgListener` on channel `order_updates` routes completion notifications back to pending HTTP handles:

```rust
// crates/router/src/services/listen_dispatcher.rs  (new)
pub struct ListenDispatcher {
    pending: Arc<DashMap<Uuid, oneshot::Sender<OrderResponse>>>,
}

impl ListenDispatcher {
    pub async fn run(self, pool: PgPool) -> anyhow::Result<()> {
        let mut listener = PgListener::connect_with(&pool).await?;
        listener.listen("order_updates").await?;
        while let Some(notif) = listener.try_recv().await? {
            let evt: OrderUpdateEvent = serde_json::from_str(notif.payload())?;
            if let Some((_, tx)) = self.pending.remove(&evt.request_id) {
                let _ = tx.send(evt.into_response());
            }
        }
        Ok(())
    }
}
```

Single instance guaranteed by an advisory lock (`pg_advisory_lock(ENG04_LOCK_ID)`) held for the task lifetime — multiple router replicas can start it; only one holds the channel.

The HTTP handler:

```rust
pub async fn post_order(
    req: web::Json<InboundOrderRequest>,
    pending: web::Data<Arc<DashMap<Uuid, oneshot::Sender<OrderResponse>>>>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let request_id = req.request_id;
    let (tx, rx) = oneshot::channel();
    pending.insert(request_id, tx);

    // Defensive: remove on drop so timeouts don't leak DashMap entries
    let _guard = PendingGuard::new(pending.clone(), request_id);

    queue_orders::insert_received(&pool, &req).await?;

    let timeout = match req.execution_mode {
        ExecutionMode::Live => Duration::from_secs(2),
        ExecutionMode::Shadow => Duration::from_millis(100),
    };

    match tokio::time::timeout(timeout, rx).await {
        Ok(Ok(resp)) => Ok(HttpResponse::Ok().json(resp)),
        Ok(Err(_)) => Ok(HttpResponse::InternalServerError().finish()),
        Err(_) => Ok(HttpResponse::Accepted().json(PendingAck { request_id })),
    }
}
```

Soft deadline (25 ms P99) is a monitoring target; hard timeout (2 s live / 100 ms shadow) bounds the response. On hard timeout the client gets `202 Accepted` with the request_id and can poll `GET /order/{request_id}/status` — reconciliation remains correct because the row is durable.

### Workers

```rust
// crates/router/src/services/risk_worker.rs (new)
pub async fn risk_worker_loop(pool: PgPool, risk: Arc<RiskService>) {
    loop {
        let Some(row) = claim_one(&pool, "RECEIVED", "GATING").await? else {
            sleep(100.ms).await; continue; // replaced by NOTIFY wakeup in CP-5
        };
        let verdict = risk.validate(&row.into_input()).await;
        transition(&pool, row.id, verdict).await?;
    }
}
```

`claim_one` uses a single atomic UPDATE + RETURNING:

```sql
UPDATE queue_orders SET
  pipeline_state = $2,
  claimed_by = $3,
  claim_deadline = NOW() + interval '10 seconds',
  pipeline_state_updated_at = NOW()
WHERE id = (
  SELECT id FROM queue_orders
  WHERE pipeline_state = $1
    AND (claimed_by IS NULL OR claim_deadline < NOW())
  ORDER BY created_at
  FOR UPDATE SKIP LOCKED
  LIMIT 1
)
RETURNING *;
```

### NOTIFY Payload

Currently `notify_queue_orders()` fires `pg_notify('queue_orders', NEW.id::text)` on insert only. Replaced by a state-transition trigger that emits a richer payload on the `order_updates` channel:

```sql
CREATE OR REPLACE FUNCTION notify_order_state_change() RETURNS TRIGGER AS $$
BEGIN
  IF NEW.pipeline_state IS DISTINCT FROM OLD.pipeline_state THEN
    PERFORM pg_notify('order_updates', json_build_object(
      'request_id', NEW.request_id,
      'state', NEW.pipeline_state,
      'exchange_order_id', NEW.exchange_order_id,
      'rejection_reason', NEW.rejection_reason
    )::text);
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;
```

### Integration Points

| Existing surface | Change | Notes |
|------------------|--------|-------|
| `crates/router/src/routes/order.rs` POST handler | Rewritten per CP-4 | Old handler removed in CP-6 |
| `crates/router/src/decision_loop.rs` | Unchanged callable | Invoked by RiskWorker instead of handler |
| `crates/router/src/services/ccxt_client.rs` | Unchanged callable | Invoked by DispatchWorker |
| `crates/router/src/services/fill_detector.rs` | Emits state transition `DISPATCHED → FILLED` | One-line addition to existing fill path |
| `crates/router/src/services/ws_subscription_manager.rs` | Unchanged | Still listens on `queue_orders` insert channel for WS clients |
| `crates/pg_queue/src/listen.rs` | Reused primitive | `ListenDispatcher` builds on existing `PgListener` wrapper |

### Paved Roads

- `pg_queue::PgListener` already wraps `sqlx::PgListener` with reconnect semantics (`crates/pg_queue/src/listen.rs:16`).
- `pg_queue::QueueName::Orders` already maps to `queue_orders` (`crates/pg_queue/src/queue.rs:35`). No new enum variant needed.
- `fill_detector` already owns the DISPATCHED→FILLED transition conceptually; only needs to write the state explicitly.
- Migration pattern matches `20260131000000_pg_queue_tables` — add forward and down migrations in `crates/sqlx_postgres/migrations/`.
- Feature flags follow the existing env-driven pattern used by `AUTH-02`'s toggles.

### Files

- `crates/sqlx_postgres/migrations/20260425000000_order_pipeline_state.up.sql` — schema change + trigger
- `crates/sqlx_postgres/migrations/20260425000000_order_pipeline_state.down.sql` — rollback
- `crates/router/src/services/listen_dispatcher.rs` — new, single-instance NOTIFY consumer
- `crates/router/src/services/risk_worker.rs` — new, claim-and-gate worker
- `crates/router/src/services/dispatch_worker.rs` — new, claim-and-dispatch worker
- `crates/router/src/services/mod.rs` — register new modules
- `crates/router/src/routes/order.rs` — rewritten handler (gated by flag in CP-1..CP-5, unconditional in CP-6)
- `crates/router/src/services/fill_detector.rs` — emit DISPATCHED→FILLED transition
- `crates/router/src/main.rs` — spawn ListenDispatcher + worker pools under advisory-lock gate
- `crates/pg_queue/src/request_response.rs` — extend existing request/response helper if reusable

### Dependencies Added

None new — the primitives (`sqlx`, `tokio::oneshot`, `DashMap`, `PgListener`, Actix) are already in the workspace.

---

## Acceptance Criteria

- [ ] Migration applies cleanly to a populated `queue_orders` table (add-only, no data rewrite). Verified by restoring a recent staging dump and running `sqlx migrate run`.
- [ ] `POST /order` returns within **25 ms P99** under a 100 req/s burst with mock sidecar ack at 5 ms (FR-7, FR-8).
- [ ] A killed `DispatchWorker` does not lose `GATED` rows — the orphaned claim is reclaimed within `claim_deadline` and processed by the next worker (FR-3/4 resilience).
- [ ] State transitions violating the CHECK constraint fail at the DB level, not silently (FR-2).
- [ ] All four stage histograms emit samples on every order and are scraped by the existing Prometheus config (FR-8).
- [ ] `LIVE` mode hard-timeout path returns `202 Accepted` + request_id; follow-up GET resolves to the final state (FR-7 fallback).
- [ ] After CP-6: no references to the legacy sync `post_order` remain; `cargo clippy --all-targets && cargo test` passes in testudo-exchange.

---

## Risks

1. **NOTIFY payload truncation.** PostgreSQL NOTIFY has an 8 KB limit; the proposed payload (request_id + state + optional fields) is well under 200 bytes but could grow. Mitigation: payload schema capped at the listed fields; full order state is always reread from the row if the worker needs it.
2. **`oneshot` leak on handler drop before NOTIFY arrives.** If the HTTP client disconnects and the handler is dropped but the entry remains in `DashMap`, memory grows. Mitigation: `PendingGuard` removes on drop; a periodic sweeper removes entries older than 5 minutes as defense-in-depth.
3. **Single ListenDispatcher is a SPOF.** If the task or its connection dies, responses stall. Mitigation: advisory lock with heartbeat; secondary replicas attempt reacquisition every 1 s; handler-side hard timeout (2 s live) bounds blast radius even if dispatcher is gone.
4. **Transition CHECK constraint false negatives.** A legal transition could be blocked by an overly-strict constraint, losing orders. Mitigation: constraint validates *membership of state set*, not the edge; edge validation lives in Rust (`transition()`), with unit tests for every pair. This keeps the DB's role to "no impossible states" and the edge logic testable.
5. **Feature-flag complexity.** Running two handler paths simultaneously risks divergence bugs. Mitigation: CP-5 flags the new path for ≤1 week canary, CP-6 removes the old path atomically; do not ship the flag long-term.
6. **Fill detector race.** If fill_detector fires the DISPATCHED→FILLED transition *before* DispatchWorker commits DISPATCHED, the transition is invalid. Mitigation: fill_detector retries DISPATCHED→FILLED on constraint failure after a 100 ms backoff; DispatchWorker commits DISPATCHED in the same txn as the sidecar-ack write.
7. **`testudo-raas` partial implementation exists on disk.** It contributes zero to this spec and its presence could mislead future readers. Mitigation: deleted as part of ENG-03 archival (tracked separately).

---

## Observability

Each transition emits a `trade_event` row and a Prometheus histogram sample. Dashboards:

| Metric | Type | Target |
|--------|------|--------|
| `order_ingress_write_ms` | histogram | P99 < 2 ms |
| `order_gating_claim_to_decision_ms` | histogram | P99 < 5 ms |
| `order_dispatch_send_to_ack_ms` | histogram | P99 < 15 ms |
| `order_e2e_ingress_to_response_ms` | histogram | P99 < 25 ms |
| `order_pipeline_state_total{state=…}` | counter | Sanity: RECEIVED == GATED+REJECTED over window |
| `order_orphan_reclaim_total` | counter | Alert if > 0.1 / min |

---

## Completion Signal

This spec is complete when:
1. All six checkpoints merged and the legacy sync path removed (CP-6).
2. Load test shows P99 < 25 ms sustained for 10 minutes at 100 req/s.
3. Prometheus dashboards live and alert rules configured.
4. One week of prod canary with the flag on shows no regression in fill rate or order latency vs the previous sync baseline.
5. Spec moved to `.specify/spec-archive/ENG-04-async-order-pipeline/` with a completion note.

