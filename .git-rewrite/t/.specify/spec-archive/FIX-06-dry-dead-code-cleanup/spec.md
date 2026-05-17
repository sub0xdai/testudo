# Specification: DRY Consolidation and Dead Code Cleanup

**Spec ID:** FIX-06-dry-dead-code-cleanup
**Date:** 2026-03-16
**Status:** Complete
**Class:** Refactor / Maintainability
**Priority:** P2 — redundancy and dead code, no immediate correctness impact
**Depends on:** FIX-04 (exchange constants must exist first for string replacements)
**Series:** FIX-01 through FIX-07 (Hyperliquid audit remediation)
**Audit Refs:** High #12, High #13, Medium #24, #25, #26, #30

---

## Problem Statement

The Hyperliquid implementation has significant DRY violations and dead code that increase maintenance burden:

1. **Dead code**: `AgentRotationService` and `spawn_rotation_checker` exist but are never wired into `main.rs`. The AW-05 feature is marked complete but doesn't execute at runtime.

2. **Duplicated decrypt logic**: `load_credentials` and `load_credentials_for_approval` in `exchange_account.rs` share 30 identical lines of AES-GCM decrypt-and-construct code. A bug fix in one path will be missed in the other.

3. **Duplicated account lookup**: The pattern `list_by_user → find by id or first()` appears 3 times across `exchange_api.rs`, `routing.rs`, and `exchange_api.rs` (CexExchangeApi).

4. **Duplicated error mapping**: `CexClientError` → `HttpResponse` conversion appears 3 times in `routes/exchanges.rs` with inconsistent arms.

5. **Duplicated repo construction**: `PositionRepository::new(pg_pool.clone())` constructed 5 separate times in `main.rs`.

6. **Duplicated reconnect utilities**: `reconnect_delay()` and `wait_or_cancel()` are identical in `ws_fills.rs` and `ws_subscription_manager.rs`.

---

## User Stories

- **As a developer**, I want each piece of logic in exactly one place, so that bug fixes and changes propagate correctly.
- **As a developer**, I want dead code removed, so that the codebase reflects what actually runs.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Wire `AgentRotationService` into `main.rs` or remove it entirely | High | Router (main.rs, agent_rotation.rs) |
| FR-2 | Extract shared `decrypt_row` helper in `exchange_account.rs` | High | Repository |
| FR-3 | Extract shared `resolve_account` helper for the list→find pattern | Medium | Repository or utils |
| FR-4 | Extract `cex_error_to_response` helper in `routes/exchanges.rs` | Medium | Router (routes) |
| FR-5 | Create `PositionRepository` once in `main.rs`, clone where needed | Medium | Router (main.rs) |
| FR-6 | Extract `reconnect_delay` and `wait_or_cancel` into shared utils module | Medium | Router (utils) |
| FR-7 | Add shutdown signal to `spawn_rotation_checker` (if wired in) | Medium | Router (agent_rotation.rs) |

---

## Technical Implementation

### FR-1: AgentRotationService Decision

**Option A — Wire it in** (if rotation notifications are wanted):
```rust
// main.rs, alongside other HL service setup
if hl_enabled {
    let rotation_service = Arc::new(AgentRotationService::new(
        account_repo.clone(),
        rotation_notify_tx,
    ));
    services::hyperliquid::agent_rotation::spawn_rotation_checker(rotation_service);
}
```

**Option B — Remove it** (if the feature is not needed yet):
Delete `agent_rotation.rs` and remove the module declaration from `mod.rs`. Re-add when actually needed.

**Recommendation**: Option B (YAGNI — constitution says "delete dead code").

### FR-2: Decrypt Row Helper

```rust
// In exchange_account.rs
fn decrypt_credentials(&self, row: &CredentialRow) -> Result<DecryptedCredentials, RepoError> {
    let api_key = self.vault.decrypt(&row.api_key_encrypted)?;
    let api_secret = self.vault.decrypt(&row.api_secret_encrypted)?;
    let passphrase = row.passphrase_encrypted
        .as_ref()
        .map(|enc| self.vault.decrypt(enc))
        .transpose()?;
    Ok(DecryptedCredentials {
        api_key, api_secret, passphrase,
        exchange_name: row.exchange_name.clone(),
        auth_mode: row.auth_mode.clone().unwrap_or_else(|| "api_key".to_string()),
        wallet_address: row.wallet_address.clone(),
    })
}
```

### FR-6: Shared Reconnect Utils

```rust
// New: crates/router/src/utils/backoff.rs
use std::time::Duration;
use tokio::sync::watch;

/// Exponential backoff: 1s, 2s, 4s, 8s, 16s, 32s (capped).
pub fn reconnect_delay(attempt: u32) -> Duration {
    let capped = attempt.min(5);
    Duration::from_secs(1u64 << capped)
}

/// Sleep for `delay`, returning `true` if stop signal received.
pub async fn wait_or_cancel(delay: Duration, stop_rx: &mut watch::Receiver<bool>) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => false,
        changed = stop_rx.changed() => changed.is_ok() && *stop_rx.borrow(),
    }
}
```

### Files

- `crates/router/src/main.rs` — single PositionRepository, rotation service decision
- `crates/router/src/repositories/exchange_account.rs` — decrypt_row helper
- `crates/router/src/routes/exchanges.rs` — cex_error_to_response helper
- `crates/router/src/services/hyperliquid/ws_fills.rs` — use shared backoff
- `crates/router/src/services/ws_subscription_manager.rs` — use shared backoff
- `crates/router/src/services/hyperliquid/agent_rotation.rs` — wire or remove
- `crates/router/src/utils/backoff.rs` — new shared module
- `crates/router/src/utils/mod.rs` — register backoff module

---

## Acceptance Criteria

- [x] `AgentRotationService` is either wired into main.rs with shutdown signal, or removed
- [x] `decrypt_credentials` helper eliminates the 30-line duplication
- [x] `reconnect_delay` and `wait_or_cancel` exist in exactly one location
- [x] `PositionRepository` is created once in `main.rs`
- [x] `cex_error_to_response` helper replaces 3 duplicate match blocks
- [x] No `#[allow(dead_code)]` on the removed/wired code
- [x] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Removing AgentRotationService** — if the feature is wanted later, it must be reimplemented. Mitigation: it's 107 lines with full tests; easy to re-add from git history.

---

## Completion Signal

This spec is complete when:
1. Zero duplicated logic patterns
2. No dead code
3. All tests pass
4. Code committed to master
