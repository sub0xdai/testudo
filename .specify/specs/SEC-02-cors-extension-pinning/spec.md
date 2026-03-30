# Specification: Pin CORS Extension Origins to Testudo Sniper Extension IDs

**Spec ID:** SEC-02-cors-extension-pinning
**Date:** 2026-03-30
**Status:** Complete
**Class:** Core / Security
**Priority:** P1 — Any browser extension can make credentialed cross-origin requests to the API via cookie auth
**Depends on:** None
**Series:** SEC-01 through SEC-04 (Security review remediation)

---

## Problem Statement

The `is_origin_allowed` function in `main.rs` (line 73) accepts ANY origin starting with `chrome-extension://` or `moz-extension://` — no extension ID validation is performed. Combined with `.supports_credentials()` (line 813) and `SameSite=None` cookies, this creates a cross-origin authentication bypass for any installed browser extension.

The auth system uses dual token extraction (`middleware/auth.rs:149-160`): Bearer header first, then `access_token` HttpOnly cookie fallback. When a user authenticates via the Testudo web app, an HttpOnly cookie with `SameSite=None` is set. Any malicious extension installed alongside Testudo Sniper can make credentialed requests to the API, and the browser will attach the user's auth cookie because CORS allows any extension origin with credentials.

The fix is straightforward: pin the allowed extension origins to the specific Chrome and Firefox extension IDs for Testudo Sniper.

---

## User Stories

- **As a trader**, I want only the Testudo Sniper extension to access my account via cookie auth, so that a compromised or malicious extension cannot trade on my behalf.
- **As a platform operator**, I want CORS to follow the principle of least privilege, so that the attack surface from browser extensions is minimized.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `is_origin_allowed` must validate the full extension origin (including ID), not just the scheme prefix | High | Router / CORS |
| FR-2 | Allowed extension IDs must be configurable via environment variable | Medium | Router / Config |
| FR-3 | Requests from non-allowlisted extension origins must be rejected by CORS | High | Router / CORS |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Replace prefix check with exact extension ID match, configurable via env var | Only Testudo Sniper extension origin passes CORS |
| CP-2 | Verify cookie-auth path is blocked for non-allowlisted extensions | Malicious extension `fetch()` with `credentials: "include"` is rejected |

### Changes to `main.rs`

Replace the prefix-based check with an explicit allowlist. Chrome extension IDs are deterministic from the signing key. Firefox extension UUIDs are declared in `manifest.json` under `browser_specific_settings.gecko.id`.

```rust
fn is_origin_allowed(origin: &str, allowed_origins: &str) -> bool {
    // Check explicit origin allowlist (web app origins)
    if allowed_origins.split(',').any(|o| o.trim() == origin) {
        return true;
    }

    // Check extension-specific allowlist
    let extension_origins = std::env::var("ALLOWED_EXTENSION_ORIGINS")
        .unwrap_or_default();
    if !extension_origins.is_empty() {
        return extension_origins.split(',').any(|o| o.trim() == origin);
    }

    false
}
```

The `ALLOWED_EXTENSION_ORIGINS` env var would contain:
```
chrome-extension://<chrome-extension-id>,moz-extension://<firefox-extension-uuid>
```

### Obtaining Extension IDs

- **Chrome:** The extension ID is deterministic from the CRX signing key. After publishing to Chrome Web Store, the ID is visible in the store URL (e.g., `chrome-extension://abcdef1234567890abcdef1234567890`).
- **Firefox:** The extension ID is declared in `manifest.json` → `browser_specific_settings.gecko.id`. Currently set to `testudo-sniper@testudo.vip`.

### Paved Roads

- `allowed_origins` env var already configures web app origins — follow the same pattern for extension origins.
- The CORS middleware at line 806 already uses `is_origin_allowed` — no structural changes needed.

### Files

- `testudo-exchange/crates/router/src/main.rs` — Modify `is_origin_allowed` function
- `testudo-exchange/crates/router/src/config.rs` — Add `allowed_extension_origins` config field (optional)

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Requests from `chrome-extension://<testudo-id>` are allowed by CORS
- [ ] Requests from `chrome-extension://<random-other-id>` are rejected by CORS
- [ ] Requests from `moz-extension://<testudo-uuid>` are allowed by CORS
- [ ] Requests from `moz-extension://<random-other-uuid>` are rejected by CORS
- [ ] Extension IDs are configurable via `ALLOWED_EXTENSION_ORIGINS` env var
- [ ] Web app origins continue to work via existing `allowed_origins` config
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Extension ID changes on republish** — If the Chrome extension is removed and re-uploaded, the ID changes. Mitigation: Document the current extension IDs; use env var for easy rotation.
2. **Firefox UUID format** — Firefox uses email-style IDs (`testudo-sniper@testudo.vip`), not hash-based. The `moz-extension://` origin uses an internal UUID assigned per installation. Mitigation: Research whether Firefox uses a stable origin or per-install UUID; if per-install, the `moz-extension://` prefix check may need to remain for Firefox only.

---

## Completion Signal

This spec is complete when:
1. `is_origin_allowed` validates full extension origin, not just scheme prefix
2. Extension IDs are configurable via environment variable
3. All acceptance criteria met
4. `cargo clippy --all-targets && cargo test` passes
5. Code committed to master
