# Specification: External Call Resilience

**Spec ID:** FIX-07-external-call-resilience
**Date:** 2026-03-16
**Status:** Complete
**Class:** Refactor / Reliability
**Priority:** P2 — operational robustness improvements
**Depends on:** None
**Series:** FIX-01 through FIX-07 (Hyperliquid audit remediation)
**Audit Refs:** Medium #17, #18, #19, #20, #21, #22, #23

---

## Problem Statement

External HTTP calls to the Hyperliquid API have several resilience gaps:

1. **No connection pooling**: `reqwest::Client::new()` created per request in `agent_approval.rs` — each call creates a new connection pool instead of reusing one.

2. **No timeouts**: Neither `submit_approval` nor `verify_registration` set timeouts. A hung Hyperliquid API will block the Actix worker thread indefinitely.

3. **Nonce not validated**: The approval flow generates a nonce (timestamp) in `approve_data` and returns it to the frontend, but `approve_agent` accepts any client-supplied nonce without verifying it was issued by the server.

4. **Env var per-request**: `hl_network()` reads `HYPERLIQUID_TESTNET` from the environment on every request instead of reading from `AppState` (where it's already resolved at startup in `main.rs`).

5. **WsSubscriptionManager leaks entries**: No mechanism to remove stale subscription entries from the HashMap.

6. **Unsafe env var mutation in tests**: `std::env::set_var` / `remove_var` in agent_rotation tests — UB in multithreaded programs since Rust 1.66+.

---

## User Stories

- **As a platform operator**, I want external API calls to have timeouts, so that a hung upstream doesn't take down the backend.
- **As a developer**, I want configuration resolved once at startup, so that environment mutations don't affect running requests.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Store a shared `reqwest::Client` in `AppState`, reuse for all Hyperliquid API calls | High | Router (app.rs, agent_approval.rs) |
| FR-2 | Set 10-second timeout on all external HTTP calls to Hyperliquid | High | Router (agent_approval.rs) |
| FR-3 | Store resolved `Network` in `AppState`, eliminate `hl_network()` env reads | High | Router (app.rs, routes/exchanges.rs) |
| FR-4 | Add periodic cleanup of finished subscription entries in `WsSubscriptionManager` | Medium | Router (ws_subscription_manager.rs) |
| FR-5 | Remove `env::set_var` / `env::remove_var` from tests, pass TTL as constructor parameter | Medium | Router (agent_rotation.rs) |
| FR-6 | Persist nonce on `approve_data`, validate on `approve_agent` | Low | Router (routes/exchanges.rs) |

---

## Technical Implementation

### Shared HTTP Client

```rust
// types/app.rs
pub struct AppState {
    // ... existing fields
    pub hl_http_client: reqwest::Client,
    pub hl_network: Network,
}

// main.rs
let hl_http_client = reqwest::Client::builder()
    .timeout(Duration::from_secs(10))
    .build()
    .expect("Failed to build HTTP client");
```

### Agent Approval Changes

```rust
// BEFORE: agent_approval.rs
let client = reqwest::Client::new();
let resp = client.post(&url).json(&body).send().await?;

// AFTER: accept client as parameter
pub async fn submit_approval(
    client: &reqwest::Client,
    network: Network,
    // ... other params
) -> Result<(), ApprovalError> {
    let resp = client.post(&url).json(&body).send().await?;
    // ...
}
```

### Network in AppState

```rust
// BEFORE: routes/exchanges.rs
fn hl_network() -> Network {
    if std::env::var("HYPERLIQUID_TESTNET").unwrap_or_default() == "true" {
        Network::Testnet
    } else {
        Network::Mainnet
    }
}

// AFTER: read from AppState
let network = app_state.hl_network;
```

### Subscription Cleanup

```rust
// In WsSubscriptionManager — periodic prune
pub async fn prune_finished(&self) {
    let mut entries = self.entries.lock().await;
    entries.retain(|_key, entry| !entry.handle.is_finished());
}
```

### Test Fix for Env Vars

```rust
// BEFORE: agent_rotation.rs tests
std::env::set_var("AGENT_WALLET_TTL_HOURS", "12");

// AFTER: constructor takes TTL as parameter
impl AgentRotationService {
    pub fn new(repo: ExchangeAccountRepository, tx: mpsc::Sender<...>, ttl_hours: u64) -> Self {
        // ...
    }
}

// Test:
let service = AgentRotationService::new(repo, tx, 12);
```

### Files

- `crates/router/src/types/app.rs` — add `hl_http_client`, `hl_network`
- `crates/router/src/main.rs` — create shared client, pass Network to AppState
- `crates/router/src/services/hyperliquid/agent_approval.rs` — accept client parameter
- `crates/router/src/routes/exchanges.rs` — remove `hl_network()`, use AppState
- `crates/router/src/services/ws_subscription_manager.rs` — add `prune_finished()`
- `crates/router/src/services/hyperliquid/agent_rotation.rs` — constructor change, fix tests

---

## Acceptance Criteria

- [x] Single `reqwest::Client` in AppState, reused by all Hyperliquid HTTP calls
- [x] All external HTTP calls have a 10-second timeout
- [x] `hl_network()` function removed, Network read from AppState
- [x] `WsSubscriptionManager` prunes finished entries (either on access or periodically)
- [x] No `env::set_var` / `env::remove_var` in tests
- [x] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Nonce validation complexity** — storing nonces adds state. For a single-user system this is low risk. Mitigation: marked as Low priority; can use an in-memory HashMap with 5-minute TTL.

---

## Completion Signal

This spec is complete when:
1. ~~External calls are resilient (timeouts, connection pooling)~~ Done
2. ~~Configuration is resolved once at startup~~ Done
3. ~~No unsafe env mutations in tests~~ Done
4. ~~All tests pass~~ 920 tests pass, 0 failures
5. ~~Code committed to master~~ Committed (a694b24)
