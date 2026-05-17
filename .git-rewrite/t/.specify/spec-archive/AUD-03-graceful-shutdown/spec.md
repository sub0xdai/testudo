# Specification: Graceful Shutdown & Process Lifecycle

**Spec ID:** AUD-03-graceful-shutdown
**Date:** 2026-03-07
**Status:** Complete
**Class:** Audit
**Phase:** 2 (Reliability)
**Audit Refs:** ML-6, ML-7, ML-10, ML-13, RC-8, RC-11

---

## Overview

Add coordinated shutdown to the router process. Currently 6+ background tasks run as orphaned `tokio::spawn` with no cancellation mechanism. On SIGTERM (K8s pod termination), tasks continue until the Tokio runtime drops them, losing in-flight events and holding database connections.

**Current state:**
- 6 `tokio::spawn` tasks in `main.rs` (sidecar health, mgmt event forwarder, PriceFeedService, TradeManagerService x2, FillDetectorService) — no shutdown signal
- No `tokio::signal::ctrl_c()` handler
- Management event channel is unbounded (`mpsc::unbounded_channel`) — backpressure impossible
- `SyncService` uses `Arc<RwLock<bool>>` stop flag checked after blocking sync() — delayed shutdown
- `PgWsManager::remove_user` leaves stale `reverse_subscriptions` and `active_channels` entries
- `SidecarHealthState` uses `std::sync::RwLock` with `unwrap()` — panics on poison

**Target state:**
- All background tasks observe a `CancellationToken` and shut down gracefully within 5 seconds
- SIGTERM triggers coordinated shutdown: stop accepting requests, drain in-flight work, close connections
- Management event channel is bounded with backpressure
- PgWsManager properly cleans up on user disconnect

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Create a `CancellationToken` in `main()`, pass clone to every spawned task | Critical | Router / Main |
| FR-2 | Wire `tokio::signal::ctrl_c()` to cancel the token | Critical | Router / Main |
| FR-3 | Modify `PriceFeedService::run()` to `tokio::select!` on cancellation token | High | Router / PriceFeed |
| FR-4 | Modify sidecar health check loop to `tokio::select!` on cancellation token | High | Router / Main |
| FR-5 | Modify management event forwarder to `tokio::select!` on cancellation token | High | Router / Main |
| FR-6 | Replace `mpsc::unbounded_channel` with `mpsc::channel(1024)` for management events | High | Router / Main |
| FR-7 | Handle `SendError` on bounded channel — log and drop old events | Medium | Router / Services |
| FR-8 | Fix `SyncService` shutdown — use `tokio::select!` with cancellation token instead of `Arc<RwLock<bool>>` | Medium | Router / SyncService |
| FR-9 | Fix `PgWsManager::remove_user` — remove empty `reverse_subscriptions` entries and corresponding `active_channels`, issue UNLISTEN | Medium | WS-Stream |
| FR-10 | Replace `std::sync::RwLock` on `SidecarHealthState` with `tokio::sync::RwLock`, remove `unwrap()` | Low | Router / Main |
| FR-11 | Log shutdown sequence with structured messages | Medium | Router / Main |

---

## Technical Implementation

### 1) CancellationToken Setup (FR-1, FR-2)

```rust
use tokio_util::sync::CancellationToken;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let shutdown = CancellationToken::new();

    // Wire SIGTERM/SIGINT
    let shutdown_signal = shutdown.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
        log::info!("Shutdown signal received, initiating graceful shutdown...");
        shutdown_signal.cancel();
    });

    // Pass shutdown.clone() to every spawned task
    // ...
}
```

### 2) Task Modification Pattern (FR-3, FR-4, FR-5)

```rust
// Before: infinite loop
tokio::spawn(async move {
    loop {
        interval.tick().await;
        do_work().await;
    }
});

// After: cancellation-aware
let shutdown = shutdown.clone();
tokio::spawn(async move {
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                log::info!("Task shutting down gracefully");
                break;
            }
            _ = interval.tick() => {
                do_work().await;
            }
        }
    }
});
```

### 3) Bounded Channel (FR-6, FR-7)

```rust
let (mgmt_event_tx, mut mgmt_event_rx) =
    tokio::sync::mpsc::channel::<ManagementEvent>(1024);

// On send failure
if let Err(e) = mgmt_event_tx.try_send(event) {
    log::warn!("Management event channel full, dropping event: {:?}", e);
}
```

### 4) PgWsManager Cleanup (FR-9)

```rust
pub fn remove_user(&mut self, id: &str) {
    self.users.remove(id);
    if let Some(channels) = self.subscriptions.remove(id) {
        for channel in &channels {
            if let Some(subscribers) = self.reverse_subscriptions.get_mut(channel) {
                subscribers.retain(|uid| uid != id);
                if subscribers.is_empty() {
                    self.reverse_subscriptions.remove(channel);
                    self.active_channels.remove(channel);
                    // Issue UNLISTEN for this channel
                }
            }
        }
    }
}
```

---

## Verification

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

- [ ] SIGTERM triggers cancellation token
- [ ] All 6+ background tasks exit within 5 seconds of cancellation
- [ ] Management event channel is bounded at 1024
- [ ] SendError on full channel is logged, not panicked
- [ ] PgWsManager removes empty reverse_subscriptions on user disconnect
- [ ] SidecarHealthState uses tokio::sync::RwLock without unwrap
- [ ] Shutdown sequence is logged
- [ ] All existing tests still pass
