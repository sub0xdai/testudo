# Specification: Cross-Origin Cookie Auth

**Spec ID:** DEPLOY-02-cross-origin-cookies
**Date:** 2026-03-28
**Status:** Complete
**Class:** Bug Fix
**Priority:** P0 — Exchange dropdown empty, all authenticated API calls fail in production.
**Depends on:** DEPLOY-01-production-hosting (deployed)

---

## Problem Statement

The Desk SPA runs on `desk.testudo.vip` and makes API calls to `api.testudo.vip`. Auth cookies are set with `SameSite::Strict`, which prevents the browser from sending them on cross-origin requests. Result: SIWE login succeeds (cookies are set) but all subsequent authenticated requests return 401 because the cookies are never sent.

The exchange dropdown appears empty because `GET /api/v1/exchanges` requires authentication and the cookies aren't sent.

---

## Root Cause

`crates/router/src/routes/auth.rs` lines 42-55: both `access_token` and `refresh_token` cookies are built with `SameSite::Strict`.

For cross-origin requests (different subdomains), cookies must use:
- `SameSite::None` — allows cross-origin sending
- `Secure` — required when SameSite is None (already set)
- `Domain=.testudo.vip` — allows the cookie to be sent to any subdomain

---

## Implementation

### T1: Update cookie builder in auth.rs

Change all four cookie builders (access set, refresh set, access clear, refresh clear):

```rust
// Before
Cookie::build("access_token", token.to_string())
    .path("/api")
    .http_only(true)
    .same_site(SameSite::Strict)
    .secure(true)

// After
let cookie_domain = std::env::var("COOKIE_DOMAIN").ok();

let mut builder = Cookie::build("access_token", token.to_string())
    .path("/api")
    .http_only(true)
    .same_site(SameSite::None)
    .secure(true);

if let Some(ref domain) = cookie_domain {
    builder = builder.domain(domain.clone());
}
```

### T2: Add COOKIE_DOMAIN to .env

On the droplet, add to `/opt/testudo/.env`:
```
COOKIE_DOMAIN=.testudo.vip
```

The leading dot allows the cookie to be sent to `desk.testudo.vip`, `api.testudo.vip`, etc.

### T3: Update tests

Update test assertions in auth.rs from `SameSite::Strict` to `SameSite::None`.

### T4: Verify

- `cargo clippy --all-targets` clean
- `cargo test` passes
- Deploy and verify exchange dropdown populates on `desk.testudo.vip`

---

## Acceptance Criteria

- [ ] Cookies set with `SameSite::None; Secure; Domain=.testudo.vip`
- [ ] `COOKIE_DOMAIN` env var configurable (no domain set = local dev works as before)
- [ ] Exchange dropdown populates after SIWE login
- [ ] All authenticated Desk API calls work (analytics, journal, trades)
- [ ] `cargo test` passes
