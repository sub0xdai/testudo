# Specification: Enforce JWT Authentication on Trade Management Routes

**Spec ID:** SEC-01-trade-auth-bypass
**Date:** 2026-03-30
**Status:** Complete
**Class:** Core / Security
**Priority:** P0 — Unauthenticated callers can cancel real exchange orders via X-User-Id header spoofing
**Depends on:** None (first in series)
**Series:** SEC-01 through SEC-04 (Security review remediation)

---

## Problem Statement

The `/api/v1/trades` scope in `main.rs` (line 991) is mounted WITHOUT `JwtMiddleware`. The comment confirms this was intentional for paper trading: `// Paper trading routes - shadow engine (uses X-User-Id header, not JWT)`. However, the scope has since grown to handle both shadow AND live trade operations.

The `extract_user_id` function in `trade_management.rs` (line 41) implements a dual-path identity extraction: JWT Bearer token first, then fallback to a bare `X-User-Id` header. When the header fallback is used, `is_authenticated` is set to `false`. Most handlers correctly gate live exchange operations behind `if is_authenticated`, but `cleanup_stale_trades` (line 1904) discards the flag entirely (`_is_authenticated`) and unconditionally calls `trade_manager_live.cancel_order()`.

This means any caller who knows a user's UUID can cancel all their pending exchange orders on real exchanges without any authentication. The root cause is scope creep — the `/trades` endpoint started as shadow-only but gained live capabilities without updating its auth posture.

---

## User Stories

- **As a trader**, I want my exchange orders to only be cancellable by me (authenticated), so that no one can manipulate my live positions.
- **As a platform operator**, I want all routes that touch live exchange operations to require JWT authentication, so that the attack surface is minimized.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Apply `JwtMiddleware` to the `/api/v1/trades` scope in `main.rs` | High | Router |
| FR-2 | Gate all live trade manager calls in `cleanup_stale_trades` behind `is_authenticated` | High | Trade Management |
| FR-3 | Remove or restrict the `X-User-Id` header fallback in `extract_user_id` to shadow-only mode | Medium | Trade Management |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Add `JwtMiddleware` to `/trades` scope in `main.rs` | Unauthenticated requests to `/trades/*` return 401 |
| CP-2 | Gate `cleanup_stale_trades` live operations behind `is_authenticated` | Cleanup with X-User-Id header cannot cancel real exchange orders |
| CP-3 | Restrict `extract_user_id` X-User-Id fallback | Shadow-mode-only identity extraction works; live mode requires JWT |

### Changes to `main.rs`

Add JwtMiddleware to the `/trades` scope, matching how `/order`, `/orders`, `/exchanges`, `/risk-config`, and `/journal` are configured.

```rust
// main.rs — around line 991
// Trade management routes — JWT required for all operations
web::scope("/trades")
    .wrap(JwtMiddleware::new(token_service.clone()))
    // ... existing route registrations unchanged
```

### Changes to `trade_management.rs`

**`cleanup_stale_trades` (line 1904):** Replace `_is_authenticated` with `is_authenticated` and gate live trade manager calls:

```rust
let (user_id, is_authenticated) = extract_user_id(&req)?;

// ... existing shadow cleanup logic ...

// Only cancel real exchange orders if authenticated
if is_authenticated {
    if let Some(ref tm) = state.trade_manager_live {
        tm.cancel_order(/* ... */).await?;
    }
}
```

**`extract_user_id` (line 41):** Consider restricting the X-User-Id fallback. With JwtMiddleware applied, the fallback will never be reached for the `/trades` scope since the middleware rejects requests without valid JWT. The fallback can be left as dead code (for future shadow-only scopes) or removed for clarity.

### Paved Roads

- `cancel_trade` (line 1704) already correctly gates exchange operations behind `if is_authenticated` — follow this exact pattern.
- `/order`, `/orders`, `/exchanges` scopes already use `.wrap(JwtMiddleware::new(token_service.clone()))` — reuse the same pattern.

### Files

- `testudo-exchange/crates/router/src/main.rs` — Add JwtMiddleware to `/trades` scope
- `testudo-exchange/crates/router/src/routes/trade_management.rs` — Gate `cleanup_stale_trades` live ops behind `is_authenticated`

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `/api/v1/trades/*` returns 401 without valid JWT Bearer token
- [ ] `POST /trades/cleanup` with only `X-User-Id` header cannot cancel real exchange orders
- [ ] `POST /trades/cleanup` with valid JWT can still cancel real exchange orders
- [ ] Shadow trade operations (create, list, cancel) still work with JWT auth
- [ ] Existing tests pass: `cargo clippy --all-targets && cargo test`

---

## Risks

1. **Shadow-mode regression** — Paper trading relied on X-User-Id without JWT. Mitigation: Ensure the web app / extension sends JWT for all trade requests (it already does for `/order` routes).
2. **Test breakage** — Tests may use X-User-Id directly without JWT. Mitigation: Update test helpers to include mock JWT tokens.

---

## Completion Signal

This spec is complete when:
1. JwtMiddleware is applied to the `/trades` scope
2. `cleanup_stale_trades` gates live operations behind `is_authenticated`
3. All acceptance criteria met
4. `cargo clippy --all-targets && cargo test` passes
5. Code committed to master
