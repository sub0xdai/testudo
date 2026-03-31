# Specification: Add Ownership Check to Trade Management Endpoint

**Spec ID:** SEC-04-trade-management-idor
**Date:** 2026-03-30
**Status:** Complete
**Class:** Core / Security
**Priority:** P1 — Missing authorization check allows reading any user's trade management state
**Depends on:** SEC-01-trade-auth-bypass (JWT middleware must be in place first)
**Series:** SEC-01 through SEC-04 (Security review remediation)

---

## Problem Statement

The `get_trade_management` handler in `trade_management.rs` (line 1995) extracts `_user_id` but never uses it to verify the requested position belongs to the caller. The leading underscore is Rust's convention for intentionally unused variables, confirming the ownership check was simply missed.

The underlying service (`trade_manager/service.rs:538-540`) performs a bare HashMap lookup by position ID with no user scoping:

```rust
pub async fn get_position(&self, id: Uuid) -> Option<ManagedPosition> {
    self.positions.read().await.get(&id).cloned()
}
```

Every other handler in the file follows the pattern `if group.user_id != user_id { return Forbidden }`:
- `get_trade_group` (line 1149)
- `cancel_trade` (line 1200)
- `edit_order_entry` (line 1471)

This is an Insecure Direct Object Reference (IDOR) — the authorization check is present everywhere else but missing on this specific endpoint. Combined with SEC-01 (no JWT middleware on `/trades`), this endpoint currently requires zero authentication.

---

## User Stories

- **As a trader**, I want my trade management state (stops, quantities, strategies) to be visible only to me, so that competitors cannot see my positions.
- **As a platform operator**, I want all data-access endpoints to enforce ownership checks, so that IDOR vulnerabilities don't leak user data.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `get_trade_management` must verify `position.user_id == authenticated_user_id` before returning data | High | Trade Management |
| FR-2 | Return 403 Forbidden when the position belongs to a different user | High | Trade Management |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Add ownership check to `get_trade_management` handler | Requesting another user's position returns 403 |

### Changes to `trade_management.rs`

At line ~2022, after retrieving the position, add the ownership check before returning data:

```rust
async fn get_trade_management(
    req: HttpRequest,
    path: web::Path<Uuid>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let (user_id, _is_authenticated) = match extract_user_id(&req) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let position_id = path.into_inner();

    if let Some(ref tm) = state.trade_manager_shadow {
        if let Some(pos) = tm.get_position(position_id).await {
            // Ownership check — consistent with get_trade_group, cancel_trade, etc.
            if pos.user_id != user_id {
                return HttpResponse::Forbidden()
                    .json(ApiResponse::<()>::error("Access denied".to_string()));
            }
            // ... return position data
        }
    }

    // Same pattern for trade_manager_live
    if let Some(ref tm) = state.trade_manager_live {
        if let Some(pos) = tm.get_position(position_id).await {
            if pos.user_id != user_id {
                return HttpResponse::Forbidden()
                    .json(ApiResponse::<()>::error("Access denied".to_string()));
            }
            // ... return position data
        }
    }

    HttpResponse::NotFound()
        .json(ApiResponse::<()>::error("Position not found".to_string()))
}
```

### Paved Roads

- `get_trade_group` (line 1149): `if group.user_id != user_id { return Forbidden }` — follow this exact pattern.
- `ManagedPosition` struct already has a `user_id` field (`types.rs:77`) — no schema changes needed.

### Files

- `testudo-exchange/crates/router/src/routes/trade_management.rs` — Add ownership check to `get_trade_management`

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `GET /trades/{id}/management` returns 403 when position belongs to a different user
- [ ] `GET /trades/{id}/management` returns position data when position belongs to the authenticated user
- [ ] `GET /trades/{id}/management` returns 404 when position does not exist
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **None significant** — This is a one-line fix following an established pattern. The only risk is a typo in the field name, caught by the compiler.

---

## Completion Signal

This spec is complete when:
1. Ownership check is added to `get_trade_management`
2. All acceptance criteria met
3. `cargo clippy --all-targets && cargo test` passes
4. Code committed to master
