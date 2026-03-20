# Specification: Credential State Machine Safety

**Spec ID:** FIX-03-credential-state-safety
**Date:** 2026-03-16
**Status:** Complete
**Class:** Refactor / Security
**Priority:** P0 — race conditions on credential state transitions
**Depends on:** None
**Series:** FIX-01 through FIX-07 (Hyperliquid audit remediation)
**Audit Refs:** Critical #4, Critical #5

---

## Problem Statement

The agent wallet lifecycle (approve, migrate, revoke) has no atomicity guarantees on state transitions. Two concurrent `POST /agent-wallet/approve` requests can both succeed — both submit to the Hyperliquid API, both set `is_active = true`. More critically, a concurrent `migrate_to_agent_wallet` (which replaces the keypair) and `approve_agent` (which activates the old keypair) can race, leaving the account active with a replaced key.

These operations manage **real exchange credentials** that control trading permissions. A corrupted state means either an unauthorized key is active or a valid key is incorrectly deactivated.

---

## User Stories

- **As a trader**, I want credential operations to be atomic, so that my account is never in an inconsistent state.
- **As a security auditor**, I want state transitions to use compare-and-swap semantics, so that concurrent requests cannot corrupt credential state.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `approve_agent` must verify `is_active = false` before submitting to Hyperliquid API | High | Router (routes/exchanges.rs) |
| FR-2 | `update_agent_approved` must use `UPDATE ... WHERE is_active = false RETURNING *` to atomically guard the transition | High | Repository (exchange_account.rs) |
| FR-3 | `migrate_to_agent_wallet` must use `SELECT ... FOR UPDATE` to lock the row during keypair replacement | High | Repository (exchange_account.rs) |
| FR-4 | `revoke_agent` must use `UPDATE ... WHERE is_active = true AND auth_mode = 'agent_wallet' RETURNING *` | High | Repository (exchange_account.rs) |
| FR-5 | All state transition methods return `RowNotFound`/`ConflictError` when the precondition is not met | High | Repository (exchange_account.rs) |
| FR-6 | Route handlers translate precondition failures to HTTP 409 Conflict responses | Medium | Router (routes/exchanges.rs) |

---

## Technical Implementation

### Atomic Approval Guard

```rust
// BEFORE: update_agent_approved just sets is_active = true
// AFTER: conditional update with precondition
pub async fn update_agent_approved(&self, account_id: Uuid, user_id: Uuid) -> Result<bool, RepoError> {
    let result = sqlx::query!(
        r#"UPDATE exchange_accounts
           SET is_active = true
           WHERE id = $1 AND user_id = $2 AND is_active = false AND auth_mode = 'agent_wallet'
           RETURNING id"#,
        account_id,
        user_id,
    )
    .fetch_optional(&self.pool)
    .await?;

    Ok(result.is_some())  // false = precondition not met (already active or wrong mode)
}
```

### Row Locking for Migration

```rust
pub async fn migrate_to_agent_wallet(
    &self,
    account_id: Uuid,
    user_id: Uuid,
    new_credentials: EncryptedCredentials,
) -> Result<(), RepoError> {
    let mut tx = self.pool.begin().await?;

    // Lock the row to prevent concurrent approve/revoke
    let row = sqlx::query!(
        "SELECT id FROM exchange_accounts WHERE id = $1 AND user_id = $2 FOR UPDATE",
        account_id, user_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(RepoError::NotFound)?;

    // Perform the migration under the lock
    sqlx::query!(
        r#"UPDATE exchange_accounts
           SET auth_mode = 'agent_wallet',
               api_key_encrypted = $3,
               api_secret_encrypted = $4,
               wallet_address = $5,
               is_active = false
           WHERE id = $1 AND user_id = $2"#,
        account_id, user_id,
        new_credentials.api_key, new_credentials.api_secret, new_credentials.wallet_address,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}
```

### Route Handler Conflict Response

```rust
// In approve_agent handler:
let updated = account_repo.update_agent_approved(account_id, user_id).await?;
if !updated {
    return Ok(HttpResponse::Conflict().json(serde_json::json!({
        "error": "Account already approved or not in agent_wallet mode"
    })));
}
```

### Files

- `crates/router/src/repositories/exchange_account.rs` — all state-transition queries
- `crates/router/src/routes/exchanges.rs` — approve, migrate, revoke handlers
- `crates/router/src/types/exchanges.rs` — error response types if needed

---

## Acceptance Criteria

- [x] `update_agent_approved` uses `WHERE is_active = false` precondition
- [x] `migrate_to_agent_wallet` uses `SELECT ... FOR UPDATE` row locking within a transaction
- [x] `revoke_agent` uses `WHERE is_active = true` precondition
- [x] Concurrent approve requests: only one succeeds, other gets 409
- [x] Concurrent migrate + approve: migrate locks row, approve waits and sees new state
- [x] Unit tests verify precondition failure paths
- [x] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Deadlock potential** — `FOR UPDATE` can deadlock if multiple transactions lock rows in different order. Mitigation: all operations lock by single `(account_id, user_id)` — no cross-row locks.
2. **Performance** — Row-level locking adds minor overhead. Mitigation: these operations are rare (setup, not per-trade) — negligible impact.

---

## Completion Signal

This spec is complete when:
1. All credential state transitions are atomic
2. Concurrent requests produce correct results (one succeeds, others get 409)
3. No corrupted credential state is possible under concurrent access
4. All tests pass
5. Code committed to master
