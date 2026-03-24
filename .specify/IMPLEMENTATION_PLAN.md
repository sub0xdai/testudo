# Implementation Plan

> Last updated: 2026-03-24
> Current spec: AUTH-02-backend-auth
> Phase: BUILD

---

## Active Spec: AUTH-02-backend-auth

Backend auth hardening — SIWE-only authentication with HttpOnly cookies, session rotation, extension device pairing. Replaces email/password auth with wallet-primary SIWE.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Foundation rewrite — common_utils types (TokenClaims wallet_address, reduce lifetimes, remove AuthService/bcrypt/PasswordHasher) + fix all router consumers (AppState, middleware dual extraction, auth_helpers, user repo, trade_management, trade_events, order, exchanges tests) | complete | high | AUTH-01 |
| T2 | Create SessionRepository in router/repositories — concrete PgPool impl (create, find_by_hash, revoke, revoke_all, update_last_used, cleanup_expired) | complete | medium | T1 |
| T3 | Create nonce store + SIWE parser + pairing store — DashMap TTL stores, EIP-4361 parser, alloy signature recovery (services/auth/ module: nonce_store.rs, pairing_store.rs, siwe.rs) | complete | high | T1 |
| T4 | Create auth routes (auth.rs) — nonce, verify-siwe, refresh, logout, revoke-all, me, pair-extension, extension-pair, extension-refresh | complete | high | T2, T3 |
| T5 | Wire routes in main.rs + CORS credentials + delete old auth code (user.rs, old types) | pending | medium | T4 |
| T6 | Fix all tests + validate — cargo clippy --all-targets && cargo test | pending | medium | T5 |

### Key Decisions

- **TokenService replaces AuthService**: All middleware and route handlers now use `TokenService` (sync trait) instead of `AuthService` (async trait with email/password). Simpler, no DB access in middleware.
- **Dual token extraction**: Middleware reads Bearer header first, falls back to `access_token` cookie. Bearer priority ensures extension auth works alongside web cookie auth.
- **AuthenticatedUser.email → wallet_address**: All auth context and test fixtures updated.
- **User model simplified**: Removed email, password_hash, email_verified. Added wallet_address. No PasswordHasher/BcryptHasher/UserFactory.
- **Token lifetimes reduced**: Access 15min (was 1hr), Refresh 7 days (was 30 days).
- **SHA-256 token hashing**: `hash_token()` utility added for session storage.
- **SessionRepository in router crate**: Placed in `crates/router/src/repositories/session.rs` (not sqlx_postgres) to match the concrete type pattern used by `PostgresUserRepository` and `ExchangeAccountRepository`. Uses `AuthError` for consistency with user repo.
- **SIWE uses alloy 0.1.4 Signature**: `alloy::primitives::Signature` (not `PrimitiveSignature` — that name was introduced in 0.8+). `eip191_hash_message` and `from_bytes_and_parity` confirmed working.
- **Auth stores in services/auth/**: NonceStore and PairingStore use DashMap with cleanup-on-insert TTL pattern (matches AuthCache in hyperliquid/auth.rs). No AppState wiring yet — T4 will add them.
- **AuthError::Unauthorized(String)**: SIWE validation errors use `Unauthorized(msg)` not `InvalidToken` (unit variant, no payload). Matches semantic intent.
- **Auth routes use web::Data extractors**: NonceStore, PairingStore, SessionRepository, and PostgresUserRepository are injected as `web::Data<T>` (not embedded in AppState). This keeps AppState unchanged and allows independent testing. T5 will wire them via `.app_data()` in main.rs.
- **Cookie + JSON dual paths**: Web/journal gets HttpOnly cookies (verify-siwe, refresh, logout). Extension gets JSON body tokens (extension-pair, extension-refresh). Shared `rotate_refresh()` helper handles both paths.
- **`actix_web::test` shadows `#[test]`**: In test modules that `use actix_web::test`, the `#[test]` attribute resolves to `actix_web::test` macro which requires `async fn`. Renamed import to `actix_test` to avoid this.

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| AUTH-01-infra-hardening | 2026-03-24 |
| ANL-01-bloomberg-charts (Phase 1) | 2026-03-23 |
| JNL-18-storage-quotas | 2026-03-22 |
| JNL-17-nested-collections | 2026-03-22 |
| JNL-16-database-view | 2026-03-22 |
| JNL-15-export-with-images | 2026-03-22 |
| JNL-14-markdown-hardening | 2026-03-22 |
| UXP-21-light-theme-parity | 2026-03-22 |
| UXP-23-landing-typography | 2026-03-22 |
| UXP-20-strip-glassmorphism | 2026-03-22 |
| UXP-22-signal-color-calibration | 2026-03-22 |
| UXP-19-features-layout | 2026-03-22 |
| EXT-38-background-decomposition | 2026-03-22 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |
| UXP-18-multi-theme | 2026-03-21 |
| HL-11-status-transition-fix | 2026-03-21 |

---

*This file is persistent state. Vox updates it each iteration.*
