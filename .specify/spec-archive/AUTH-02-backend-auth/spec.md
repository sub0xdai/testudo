# Specification: Backend Auth — SIWE-Only Authentication with HttpOnly Cookies & Session Rotation

**Spec ID:** AUTH-02-backend-auth
**Date:** 2026-03-24
**Status:** Draft
**Class:** Core / Auth
**Priority:** P0 — Current email/password auth must be replaced with SIWE; tokens in localStorage are XSS-vulnerable
**Depends on:** AUTH-01-infra-hardening (requires `users.wallet_address` column and `user_sessions` table)
**Series:** AUTH-01 through AUTH-03 (authentication architecture hardening)

---

## Problem Statement

The Rust backend currently implements email/password authentication (`crates/common_utils/src/auth/mod.rs`). The `StandardAuthService` hashes passwords with bcrypt, issues JWTs in JSON body responses, and has a no-op logout. The platform is moving to wallet-primary identity — SIWE (Sign-In With Ethereum) becomes the sole authentication method.

After AUTH-01, the `users` table has `wallet_address` as its identity column (no email, no password_hash). The `user_sessions` table exists but is unused. This spec rewires the entire auth layer:

1. Replace `register`/`login` with a single `POST /api/v1/auth/verify-siwe` endpoint that recovers the wallet address from an EIP-4361 signature using `alloy` (already in `Cargo.toml:43` for Hyperliquid EIP-712 signing).
2. Issue tokens as `HttpOnly; Secure; SameSite=Strict` cookies for web/journal clients.
3. Provide `POST /api/v1/auth/pair-extension` for device pairing — web generates a one-time code, extension exchanges it for tokens in JSON body.
4. Implement strict refresh token rotation via `user_sessions` — every refresh revokes the old token and issues a new one.
5. Delete all email/password auth code: `register`, `login`, `forgot_password`, `reset_password`, bcrypt dependency, `PasswordHasher` trait.

The `alloy` crate already provides `PrivateKeySigner` and `Address` types used in `crates/router/src/services/hyperliquid/auth.rs`. SIWE signature recovery uses the same `alloy::signers` module — no new crate needed.

---

## User Stories

- **As a trader**, I want to sign in by connecting my wallet and signing a message, so that I don't need to manage yet another email/password.
- **As a web/journal user**, I want auth handled via HttpOnly cookies, so that my tokens are never exposed to JavaScript.
- **As an extension user**, I want to pair my extension to my web session via a one-time code, so that I can authenticate without needing wallet access in the extension context.
- **As a user**, I want logout to immediately invalidate my session, so that a stolen token is useless the moment I log out.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add `POST /api/v1/auth/verify-siwe` — accepts EIP-4361 message + signature, recovers address, creates or finds user, issues HttpOnly cookie session | High | router/routes |
| FR-2 | Add `GET /api/v1/auth/nonce` — generates a random nonce, stores it server-side with 5-minute TTL, returns it to client | High | router/routes |
| FR-3 | SIWE verification: parse message, validate domain + nonce + timestamps, recover signer via `alloy`, match against message address | High | router/routes |
| FR-4 | Set tokens as `HttpOnly; Secure; SameSite=Strict` cookies. Access cookie: `Path=/api`, `Max-Age=900`. Refresh cookie: `Path=/api/v1/auth`, `Max-Age=604800` | High | router/routes |
| FR-5 | Reduce token lifetimes: access = 15 minutes, refresh = 7 days | High | common_utils/auth |
| FR-6 | Every refresh token issuance inserts a `user_sessions` row with `SHA-256(token)` hash, client IP, User-Agent | High | router/routes |
| FR-7 | Refresh endpoint reads refresh token from cookie, rotates (revokes old session, creates new), sets new cookies | High | router/routes |
| FR-8 | Refresh validation: reject if `is_revoked = TRUE` or `expires_at < NOW()` in `user_sessions` | High | router/routes |
| FR-9 | Logout revokes session row and clears cookies | High | router/routes |
| FR-10 | Add `POST /api/v1/auth/revoke-all` — revokes all sessions for authenticated user | Medium | router/routes |
| FR-11 | Add `POST /api/v1/auth/pair-extension` (authenticated) — generates a one-time 6-digit pairing code, stores with 5-minute TTL tied to user_id, returns code | High | router/routes |
| FR-12 | Add `POST /api/v1/auth/extension-pair` (unauthenticated) — accepts pairing code, validates, consumes (one-time use), issues extension-scoped tokens in JSON body, creates session | High | router/routes |
| FR-13 | Add `POST /api/v1/auth/extension-refresh` — accepts refresh token in JSON body, rotates, returns new tokens in JSON body | High | router/routes |
| FR-14 | Auth middleware accepts token from EITHER `Authorization: Bearer` header (extension) OR `access_token` cookie (web) — Bearer takes priority | High | router/middleware |
| FR-15 | Add `GET /api/v1/auth/me` — returns `{ user_id, wallet_address }` from access token claims | Medium | router/routes |
| FR-16 | Delete: `register`, `login`, `forgot_password`, `reset_password` handlers. Delete `PasswordHasher` trait, bcrypt usage | High | router/routes, common_utils |
| FR-17 | Update `TokenClaims` to use `wallet_address` instead of `email` | High | common_utils/auth |
| FR-18 | Add CORS `Access-Control-Allow-Credentials: true` | Medium | router/config |

---

## Technical Implementation

### 1. Token Claims (Updated)

```rust
// crates/common_utils/src/auth/mod.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,             // user_id (UUID as string)
    pub wallet_address: String,  // 0x-prefixed Ethereum address (was: email)
    pub exp: i64,
    pub iat: i64,
    pub token_type: TokenType,
}

const ACCESS_TOKEN_EXPIRY_SECONDS: u64 = 900;      // 15 min (was 3600)
const REFRESH_TOKEN_EXPIRY_SECONDS: u64 = 604_800;  // 7 days (was 2_592_000)
```

### 2. SIWE Verification

```rust
// crates/router/src/routes/auth.rs (replaces user.rs auth handlers)
use alloy::primitives::{Address, eip191_hash_message};
use alloy::signers::Signature;

#[derive(Deserialize)]
pub struct SiweRequest {
    pub message: String,     // EIP-4361 plaintext
    pub signature: String,   // 0x-prefixed hex signature
}

async fn verify_siwe(
    app_state: web::Data<AppState>,
    req: web::Json<SiweRequest>,
    http_req: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    // 1. Parse EIP-4361 fields from message string
    let parsed = parse_siwe_message(&req.message)?;

    // 2. Validate: domain matches, nonce is valid + unused, not expired
    validate_siwe_fields(&parsed, &app_state)?;

    // 3. Recover signer address
    let message_hash = eip191_hash_message(req.message.as_bytes());
    let sig = Signature::from_hex(&req.signature)?;
    let recovered = sig.recover_address_from_prehash(&message_hash)?;

    // 4. Compare recovered address to address in message
    if recovered != parsed.address {
        return Err(ApiError::Unauthorized("signature mismatch"));
    }

    // 5. Find or create user by wallet_address
    let user = find_or_create_user(&app_state, &recovered.to_string()).await?;

    // 6. Generate tokens
    let access = app_state.token_service.generate_access_token(&user.id, &user.wallet_address)?;
    let refresh = app_state.token_service.generate_refresh_token(&user.id, &user.wallet_address)?;

    // 7. Create session
    create_session(&app_state, &user.id, &refresh, &http_req).await?;

    // 8. Set HttpOnly cookies
    Ok(build_cookie_response(&user, &access, &refresh))
}
```

EIP-4361 message parsing is a simple line-by-line text parse — no `siwe` crate needed. The message format is standardized plaintext.

### 3. Cookie Helper

```rust
fn build_cookie_response(user: &User, access: &str, refresh: &str) -> HttpResponse {
    let access_cookie = Cookie::build("access_token", access)
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Strict)
        .path("/api")
        .max_age(time::Duration::seconds(900))
        .finish();

    let refresh_cookie = Cookie::build("refresh_token", refresh)
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Strict)
        .path("/api/v1/auth")
        .max_age(time::Duration::seconds(604_800))
        .finish();

    HttpResponse::Ok()
        .cookie(access_cookie)
        .cookie(refresh_cookie)
        .json(json!({ "user": { "id": user.id, "wallet_address": user.wallet_address } }))
}
```

### 4. Extension Device Pairing

```rust
// POST /api/v1/auth/pair-extension (authenticated — requires valid cookie)
async fn generate_pairing_code(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ApiError> {
    let code = generate_numeric_code(6); // e.g. "847293"
    // Store: code → user_id mapping with 5-minute TTL (in-memory DashMap or DB)
    app_state.pairing_store.insert(code.clone(), user.user_id, Duration::from_secs(300));
    Ok(HttpResponse::Ok().json(json!({ "code": code })))
}

// POST /api/v1/auth/extension-pair (unauthenticated — extension sends the code)
#[derive(Deserialize)]
pub struct PairRequest { pub code: String }

async fn pair_extension(
    app_state: web::Data<AppState>,
    req: web::Json<PairRequest>,
    http_req: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let user_id = app_state.pairing_store
        .take(&req.code) // consume: one-time use
        .ok_or(ApiError::Unauthorized("invalid or expired code"))?;

    let user = app_state.user_repo.find_by_id(&user_id).await?;
    let access = app_state.token_service.generate_access_token(&user.id, &user.wallet_address)?;
    let refresh = app_state.token_service.generate_refresh_token(&user.id, &user.wallet_address)?;
    create_session(&app_state, &user.id, &refresh, &http_req).await?;

    // JSON body (not cookies) — extension stores in chrome.storage.session
    Ok(HttpResponse::Ok().json(json!({
        "tokens": {
            "access_token": access,
            "refresh_token": refresh,
            "expires_in": 900
        },
        "user": { "id": user.id, "wallet_address": user.wallet_address }
    })))
}
```

### 5. Refresh Rotation Flow

```
Client sends refresh cookie (web) or JSON body (extension)
  → SHA-256(token) → lookup in user_sessions
  → IF not found OR is_revoked OR expired → 401
  → Revoke current session (is_revoked = TRUE)
  → Generate new access + refresh tokens
  → Insert new session row with SHA-256(new_refresh)
  → Return new tokens (cookie or JSON body)
```

### 6. Dual Token Extraction

```rust
// crates/router/src/middleware/auth.rs
fn extract_token(req: &ServiceRequest) -> Option<String> {
    // Priority 1: Authorization: Bearer header (extension)
    if let Some(auth) = req.headers().get("Authorization") {
        if let Ok(val) = auth.to_str() {
            if let Some(token) = val.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }
    // Priority 2: HttpOnly cookie (web/journal)
    req.cookie("access_token").map(|c| c.value().to_string())
}
```

### 7. SessionRepository

```rust
// crates/sqlx_postgres/src/session_repo.rs
#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create_session(&self, session: NewSession) -> Result<UserSession, RepoError>;
    async fn find_by_token_hash(&self, hash: &str) -> Result<Option<UserSession>, RepoError>;
    async fn revoke_session(&self, session_id: Uuid) -> Result<(), RepoError>;
    async fn revoke_all_for_user(&self, user_id: Uuid) -> Result<u64, RepoError>;
    async fn update_last_used(&self, session_id: Uuid) -> Result<(), RepoError>;
    async fn cleanup_expired(&self) -> Result<u64, RepoError>;
}
```

Token hashing: `SHA-256(refresh_token)` — fast lookup, not password verification.

### 8. Nonce Store

Nonces prevent SIWE replay attacks. Store in-memory with TTL:

```rust
// Simple DashMap<String, (Instant, String)> — nonce → (created_at, _)
// Cleanup: remove entries older than 5 minutes on each insert
// Alternative: PostgreSQL table if horizontal scaling needed later
```

### 9. Route Map (Final)

| Endpoint | Method | Auth | Token Delivery | Purpose |
|----------|--------|------|----------------|---------|
| `/api/v1/auth/nonce` | GET | No | — | Generate SIWE nonce |
| `/api/v1/auth/verify-siwe` | POST | No | HttpOnly cookies | Wallet sign-in |
| `/api/v1/auth/refresh` | POST | No | HttpOnly cookies | Cookie-based refresh + rotate |
| `/api/v1/auth/logout` | POST | Yes | Clear cookies | Revoke session |
| `/api/v1/auth/revoke-all` | POST | Yes | Clear cookies | Revoke all sessions |
| `/api/v1/auth/me` | GET | Yes | — | Current user info |
| `/api/v1/auth/pair-extension` | POST | Yes | JSON body | Generate pairing code |
| `/api/v1/auth/extension-pair` | POST | No | JSON body | Exchange code for tokens |
| `/api/v1/auth/extension-refresh` | POST | No | JSON body | Extension refresh + rotate |

### 10. Deleted Code

| Item | Location | Why |
|------|----------|-----|
| `register()` handler | `routes/user.rs:18-55` | No email registration |
| `login()` handler | `routes/user.rs:59-92` | Replaced by SIWE |
| `forgot_password()` | `routes/user.rs:176-232` | No passwords |
| `reset_password()` | `routes/user.rs:236-312` | No passwords |
| `PasswordHasher` trait | `common_utils/auth/mod.rs` | No passwords |
| `bcrypt` dependency | `common_utils/Cargo.toml` | No passwords |
| `StandardAuthService` | `common_utils/auth/mod.rs:176-314` | Replaced by SIWE service |
| `RegisterRequest`/`LoginRequest` | `routes/user.rs` | No email auth |
| `ForgotPasswordRequest`/`ResetPasswordRequest` | `routes/user.rs` | No passwords |
| `email` field in `TokenClaims` | `common_utils/auth/mod.rs:11` | Replaced by `wallet_address` |

### Files

- `crates/router/src/routes/auth.rs` — **new** — SIWE verify, nonce, pairing, refresh, logout, me
- `crates/router/src/routes/user.rs` — **gutted** — remove all auth handlers (keep if other user routes exist, else delete)
- `crates/router/src/routes/mod.rs` — **modified** — register new auth routes
- `crates/router/src/main.rs` — **modified** — register auth scope, CORS credentials, pairing store
- `crates/router/src/middleware/auth.rs` — **modified** — dual extraction (Bearer + cookie)
- `crates/router/src/config.rs` — **modified** — CORS credentials config
- `crates/common_utils/src/auth/mod.rs` — **rewritten** — remove email/password, update claims, new lifetimes
- `crates/sqlx_postgres/src/session_repo.rs` — **new** — SessionRepository
- `crates/sqlx_postgres/src/user_repo.rs` — **modified** — `find_by_wallet_address`, `find_or_create_by_wallet`
- `crates/sqlx_postgres/src/lib.rs` — **modified** — export session_repo

### Dependencies Added

- `sha2 = "0.10"` in common_utils — SHA-256 for token hashing (may already be transitive)

### Dependencies Removed

- `bcrypt = "0.15"` from common_utils — no password hashing

---

## Acceptance Criteria

- [ ] `POST /api/v1/auth/verify-siwe` recovers wallet address from valid EIP-4361 signature
- [ ] SIWE endpoint creates user if wallet_address is new
- [ ] SIWE endpoint finds existing user if wallet_address already exists
- [ ] Response sets `Set-Cookie` with `HttpOnly; Secure; SameSite=Strict`
- [ ] Response body contains user info but NOT tokens
- [ ] Access cookie: `Path=/api`, `Max-Age=900`
- [ ] Refresh cookie: `Path=/api/v1/auth`, `Max-Age=604800`
- [ ] `GET /api/v1/auth/nonce` returns unique nonce; same nonce cannot be used twice
- [ ] Protected endpoints accept token from `access_token` cookie
- [ ] Protected endpoints accept `Authorization: Bearer` header (extension path)
- [ ] Bearer header takes priority over cookie when both present
- [ ] `POST /api/v1/auth/refresh` reads refresh token from cookie, not body
- [ ] After refresh, old session is revoked in `user_sessions`
- [ ] After refresh, new session row exists with new hash
- [ ] Revoked refresh token returns 401
- [ ] Expired refresh token returns 401
- [ ] `POST /api/v1/auth/logout` sets `is_revoked = TRUE` and clears cookies
- [ ] `POST /api/v1/auth/revoke-all` revokes all user sessions
- [ ] `POST /api/v1/auth/pair-extension` returns 6-digit numeric code
- [ ] Code expires after 5 minutes
- [ ] `POST /api/v1/auth/extension-pair` exchanges valid code for JSON tokens
- [ ] Code is one-time use (second attempt fails)
- [ ] `POST /api/v1/auth/extension-refresh` accepts JSON body, returns JSON tokens
- [ ] `GET /api/v1/auth/me` returns user_id and wallet_address
- [ ] No `/auth/register`, `/auth/login`, `/auth/forgot-password`, `/auth/reset-password` routes exist
- [ ] `bcrypt` is not in Cargo.lock
- [ ] CORS includes `Access-Control-Allow-Credentials: true`
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Cookie + SPA CORS** — `SameSite=Strict` blocks cookies on cross-origin requests. Mitigation: Web and API share origin (Vite proxy in dev, same domain in prod). Downgrade to `Lax` if cross-origin needed.
2. **SIWE message parsing** — EIP-4361 has optional fields and edge cases. Mitigation: Parse only required fields (domain, address, nonce, issued-at, expiration-time). Add `siwe` crate later if manual parsing proves fragile.
3. **Refresh rotation race** — Two browser tabs refreshing simultaneously: first succeeds, second gets 401 (old token revoked). Mitigation: This is intended behavior — the second tab re-authenticates. Standard pattern for stolen-token detection.
4. **Pairing code brute force** — 6-digit numeric = 1 million combinations with 5-minute TTL. Mitigation: Rate-limit `/extension-pair` to 5 attempts per minute per IP. Add lockout after 10 failed attempts. Sufficient for pre-launch; increase to 8 digits if needed.
5. **Existing tests** — 972 Rust tests reference email/password auth. Mitigation: Tests must be updated to use SIWE auth fixtures. This is the bulk of the migration effort.

---

## Completion Signal

This spec is complete when:
1. SIWE is the sole authentication method (no email/password routes)
2. Web/journal get HttpOnly cookies; extension gets JSON tokens via pairing
3. Refresh tokens rotate on every use with server-side tracking
4. Logout revokes sessions; revoke-all signs out all devices
5. All auth-related tests updated for SIWE
6. `cargo clippy --all-targets && cargo test` passes
7. Code committed to master
