# Specification: Enqueue Failed OCO Cancellations for At-Least-Once Retry

**Spec ID:** CON-03-oco-cancel-retry
**Date:** 2026-03-30
**Status:** Draft
**Class:** Infrastructure / Data Consistency
**Priority:** P1 — Failed OCO cancellations leave orphaned orders on the exchange that can fill unexpectedly, consuming margin. Not P0 because rehydration + reconciliation provide an eventual recovery path, but the window of inconsistency is dangerous.
**Depends on:** None (independent of CON-01, CON-02)
**Series:** CON-01 through CON-03 (Distributed data consistency hardening)

---

## Problem Statement

When a stop-loss fills, `FillDetectorService` cancels the corresponding take-profit on the exchange (and vice versa) via `cancel_all_related_orders()` (`fill_detector.rs:404-452`). If the cancel HTTP call fails with a non-`OrderNotFound` error (e.g., network timeout, sidecar unavailable, exchange rate limit), the error is logged and the flow continues:

```rust
Err(e) => {
    tracing::error!(
        "FillDetector: failed to cancel related order {} for {} (group {}): {}",
        order_id, symbol, group_id, e
    );
}
```

At this point:
- The engine has marked the group as `StoppedOut` (or `TookProfit`) — terminal state.
- The extension shows the position as closed.
- The counterpart order (TP or SL) is still live on the exchange.
- No retry mechanism exists. The order can fill, creating an unintended new position.

The current `handle_cancelled_event()` (line 457) handles cancellations from the exchange side, but it only processes entry order cancellations — not orphaned SL/TP orders.

The fix is to enqueue failed cancellations into `queue_orders` (existing pg_queue table with SKIP LOCKED + LISTEN/NOTIFY) for at-least-once retry. A new lightweight consumer processes the queue, retrying the cancel until it succeeds or the order is confirmed gone (`OrderNotFound`).

---

## User Stories

- **As a trader**, I want the system to reliably cancel my stop-loss when my take-profit fills (and vice versa), so that I don't get surprise fills on orphaned orders.
- **As the system operator**, I want failed exchange cancellations to retry automatically, so that I don't need to manually clean up orphaned orders after transient exchange API failures.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | When `cancel_all_related_orders()` encounters a non-`OrderNotFound` error, enqueue a `CancelRetry` job to `queue_orders` with the order details (user_id, exchange_account_id, symbol, order_id, group_id). | High | fill_detector |
| FR-2 | A `CancelRetryWorker` consumes `CancelRetry` jobs from `queue_orders`, calls `exchange_api.cancel_order()`, and marks the job complete on success or `OrderNotFound`. | High | cancel_retry_worker |
| FR-3 | On transient failure, the worker calls `queue.fail(job_id)` to reset the job to `pending` for retry. Maximum 5 retry attempts before marking complete and alerting via `pg_notify('system.alerts')`. | High | cancel_retry_worker |
| FR-4 | The `CancelRetry` payload is a new variant in the existing `queue_orders` job schema, distinguished by a `job_type: "cancel_retry"` field. | Medium | pg_queue |
| FR-5 | The cancel retry worker is started alongside existing background services in `main.rs` and respects the `CancellationToken` shutdown signal. | Medium | main |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Define `CancelRetryPayload` struct. Enqueue on cancel failure in `fill_detector.rs`. Unit test serialization. | Payload correctness |
| CP-2 | Implement `CancelRetryWorker` consuming from `queue_orders`. Test with mock exchange API (success, OrderNotFound, transient error paths). | Worker correctness |
| CP-3 | Wire worker into `main.rs` startup. End-to-end: simulate cancel failure → verify queue entry → verify retry succeeds. | Integration |

### CancelRetryPayload

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct CancelRetryPayload {
    pub job_type: String,  // "cancel_retry"
    pub user_id: Uuid,
    pub exchange_account_id: Option<Uuid>,
    pub symbol: String,
    pub order_id: String,
    pub group_id: Uuid,
    pub attempt: u32,
}
```

### Modified cancel_all_related_orders

```rust
// fill_detector.rs — cancel_all_related_orders()
Err(e) => {
    tracing::error!(
        "FillDetector: failed to cancel related order {} for {} (group {}): {}, enqueueing retry",
        order_id, symbol, group_id, e
    );
    if let Some(ref queue) = self.queue {
        let payload = CancelRetryPayload {
            job_type: "cancel_retry".to_string(),
            user_id,
            exchange_account_id,
            symbol: symbol.to_string(),
            order_id: order_id.clone(),
            group_id,
            attempt: 0,
        };
        if let Err(qe) = queue.push(QueueName::Orders, &payload).await {
            tracing::error!("Failed to enqueue cancel retry: {}", qe);
        }
    }
}
```

### CancelRetryWorker

```rust
pub struct CancelRetryWorker {
    queue: QueueRepository,
    exchange_api: Arc<dyn ExchangeApi>,
    listener: ListenerService,
}

impl CancelRetryWorker {
    const MAX_ATTEMPTS: u32 = 5;

    pub async fn run(self, shutdown: CancellationToken) {
        tracing::info!("CancelRetryWorker started");
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("CancelRetryWorker shutting down");
                    return;
                }
                result = self.queue.pop::<CancelRetryPayload>(QueueName::Orders) => {
                    match result {
                        Ok(Some(job)) => {
                            if job.payload.job_type != "cancel_retry" {
                                // Not our job — skip (complete to avoid reprocessing)
                                let _ = self.queue.complete(QueueName::Orders, job.id).await;
                                continue;
                            }
                            self.process_cancel(job).await;
                        }
                        Ok(None) => {
                            // No jobs — wait for notification or timeout
                            let _ = tokio::time::timeout(
                                Duration::from_secs(5),
                                self.listener.recv()
                            ).await;
                        }
                        Err(e) => {
                            tracing::error!("CancelRetryWorker: queue pop error: {}", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        }
    }

    async fn process_cancel(&self, job: Job<CancelRetryPayload>) {
        let p = &job.payload;
        match self.exchange_api
            .cancel_order(p.user_id, &p.order_id, &p.symbol, p.exchange_account_id)
            .await
        {
            Ok(()) => {
                tracing::info!("CancelRetry: cancelled order {} (group {})", p.order_id, p.group_id);
                let _ = self.queue.complete(QueueName::Orders, job.id).await;
            }
            Err(ExchangeApiError::OrderNotFound(_)) => {
                tracing::info!("CancelRetry: order {} already gone (group {})", p.order_id, p.group_id);
                let _ = self.queue.complete(QueueName::Orders, job.id).await;
            }
            Err(e) => {
                if p.attempt >= Self::MAX_ATTEMPTS {
                    tracing::error!("CancelRetry: giving up on order {} after {} attempts: {}", p.order_id, p.attempt, e);
                    let _ = self.queue.complete(QueueName::Orders, job.id).await;
                    // Alert
                    let _ = sqlx::query("SELECT pg_notify('system.alerts', $1)")
                        .bind(format!("cancel_retry_exhausted: order={} group={}", p.order_id, p.group_id))
                        .execute(&self.queue.pool())
                        .await;
                } else {
                    tracing::warn!("CancelRetry: attempt {} failed for order {}: {}", p.attempt, p.order_id, e);
                    let _ = self.queue.fail(QueueName::Orders, job.id).await;
                }
            }
        }
    }
}
```

### Paved Roads

- **`queue_orders` table + LISTEN/NOTIFY trigger** (`migrations/20260131000000_pg_queue_tables.up.sql`): Existing queue infrastructure with SKIP LOCKED consumer pattern. No schema changes needed.
- **`QueueRepository::push/pop/complete/fail`** (`pg_queue/src/queue.rs`): Full lifecycle already implemented. `fail()` resets status to `pending` for automatic retry.
- **`ImportWorker::run()`** (`services/import_worker.rs`): Established pattern for a queue consumer with `tokio::select!` + `CancellationToken`. `CancelRetryWorker` follows identical structure.
- **`ExchangeApiError::OrderNotFound`** (`services/exchange_api.rs`): Already a distinct variant for idempotent cancel handling.
- **`cancel_all_related_orders()` idempotency** (`fill_detector.rs:416-417`): Already skips the filled order ID and deduplicates. The enqueue-on-failure extension preserves this.

### Files

- `crates/router/src/services/fill_detector.rs` — Add `queue: Option<QueueRepository>` field. On cancel failure, enqueue `CancelRetryPayload`.
- `crates/router/src/services/cancel_retry_worker.rs` — New file: `CancelRetryWorker` consuming `cancel_retry` jobs from `queue_orders`.
- `crates/router/src/services/mod.rs` — Add `pub mod cancel_retry_worker;`
- `crates/router/src/main.rs` — Spawn `CancelRetryWorker` alongside other background services.

### Dependencies Added

None. `pg_queue` crate is already a dependency of `router`.

---

## Acceptance Criteria

- [ ] When `cancel_order()` fails with a transient error, a `cancel_retry` job is enqueued to `queue_orders`.
- [ ] `CancelRetryWorker` consumes and retries cancel jobs, marking complete on success or `OrderNotFound`.
- [ ] After 5 failed attempts, the job is marked complete and a `system.alerts` notification is sent.
- [ ] `OrderNotFound` responses are treated as success (order already cancelled by exchange or another process).
- [ ] Worker respects `CancellationToken` and shuts down gracefully.
- [ ] `queue_orders` is shared with other job types — `CancelRetryWorker` only processes `job_type = "cancel_retry"` payloads.
- [ ] `cancel_all_related_orders()` still handles the `Ok` and `OrderNotFound` paths synchronously (only enqueues on actual failure).
- [ ] `cargo clippy --all-targets && cargo test` passes.

---

## Risks

1. **Queue contention** — `queue_orders` is shared between cancel retries and any other order-related jobs. Mitigation: Cancel retry jobs are rare (only on transient exchange failures) and lightweight (single HTTP call). Volume is negligible. If contention becomes an issue in the future, add a dedicated `queue_cancel_retries` table — a 10-line migration.

2. **Retry storm on prolonged exchange outage** — If the exchange is down for an extended period, retry jobs accumulate and all fire simultaneously when the exchange recovers. Mitigation: The SKIP LOCKED pattern means only one worker processes at a time. Add an exponential backoff between retries by reading `attempt` count and sleeping `min(2^attempt, 30)` seconds before the cancel call.

3. **Job type discrimination** — `queue_orders` currently has no `job_type` field, so the worker must deserialize the full payload to check `job_type`. If another consumer pops a `cancel_retry` job, it won't know what to do with it. Mitigation: Currently no other consumer reads from `queue_orders`. If one is added later, add a `job_type` column to the migration for filtered queries. For now, the JSON-level `job_type` field is sufficient.

---

## Completion Signal

This spec is complete when:
1. Failed OCO cancellations are enqueued to `queue_orders` for automatic retry
2. `CancelRetryWorker` processes retries with at-least-once delivery
3. All acceptance criteria met
4. `cargo clippy --all-targets && cargo test` passes
5. Code committed to master
