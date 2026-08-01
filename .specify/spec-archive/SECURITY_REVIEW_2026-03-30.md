# Testudo Security Review — 2026-03-30

**Scope:** Full codebase review of `testudo-exchange` (Rust backend) and `testudo-extension` (browser extension), with secondary coverage of `testudo-cex` (CCXT sidecar).

**Methodology:** Static analysis with data flow tracing across authentication boundaries, CORS configuration, and inter-service communication channels.

**Findings:** 4 confirmed vulnerabilities (1 HIGH, 3 MEDIUM). 1 candidate rejected as false positive (browser extension storage model misunderstanding).

---

## Vuln 1: Unauthenticated Trade Operations via X-User-Id Header Spoofing

**File:** `testudo-exchange/crates/router/src/routes/trade_management.rs:41-82`, `testudo-exchange/crates/router/src/main.rs:991-1024`

* **Severity:** HIGH
* **Category:** Authentication Bypass
* **Confidence:** 9/10

**Description:** The `/api/v1/trades` scope is mounted WITHOUT `JwtMiddleware`. The comment on line 991 of `main.rs` confirms this is intentional: `// Paper trading routes - shadow engine (uses X-User-Id header, not JWT)`. However, the scope now handles both shadow AND live trade operations. The `extract_user_id` function (line 41) accepts either a JWT Bearer token or a bare `X-User-Id` header — any caller can self-assert any user identity.

Critically, the `cleanup_stale_trades` handler (line 1904) discards the `is_authenticated` flag (`_is_authenticated`) and unconditionally calls `trade_manager_live.cancel_order()` to cancel real exchange orders. This differs from `cancel_trade` (line 1704) which correctly gates exchange operations behind `if is_authenticated`.

**Exploit Scenario:**
```
POST /api/v1/trades/cleanup
X-User-Id: <victim-uuid>
Content-Type: application/json
```
An attacker with knowledge of a user's UUID can cancel all pending exchange orders on the victim's real exchange account without any authentication. While UUIDs are 128-bit random values, they may be leaked through logs, error responses, WebSocket messages, or other API endpoints.

**Recommendation:**
1. Apply `.wrap(JwtMiddleware::new(token_service.clone()))` to the `/trades` scope in `main.rs`
2. Gate all live trade manager operations in `cleanup_stale_trades` behind `if is_authenticated`, consistent with `cancel_trade`
3. Remove or restrict the `X-User-Id` fallback in `extract_user_id` to dev/test environments only

---

## Vuln 2: CORS Allows Any Browser Extension Origin with Credentials

**File:** `testudo-exchange/crates/router/src/main.rs:73-83`

* **Severity:** MEDIUM
* **Category:** Cross-Origin Authentication Bypass
* **Confidence:** 8/10

**Description:** The `is_origin_allowed` function accepts ANY origin starting with `chrome-extension://` or `moz-extension://` — no extension ID validation is performed. Combined with `.supports_credentials()` (line 813), the server responds with `Access-Control-Allow-Credentials: true` for any extension origin.

The auth system uses **dual token extraction** (`middleware/auth.rs:149-160`): Bearer header first, then `access_token` HttpOnly cookie fallback. Cookies are configured with `SameSite=None` + `Secure`, meaning they are sent on all cross-origin HTTPS requests when CORS allows credentials.

**Exploit Scenario:**
1. User authenticates via the Testudo web app, receiving an `access_token` HttpOnly cookie (`SameSite=None`)
2. User installs a malicious browser extension (disguised as a utility)
3. Malicious extension makes `fetch("https://api.testudo.vip/api/v1/orders", { credentials: "include" })`
4. Server allows the request (origin `chrome-extension://malicious-id` passes `is_origin_allowed`)
5. Browser attaches the user's HttpOnly cookie automatically
6. Malicious extension can read balances, positions, and execute trades as the victim

**Recommendation:** Pin allowed extension origins to the specific Testudo Sniper extension IDs:
```rust
const ALLOWED_EXTENSION_IDS: &[&str] = &[
    "chrome-extension://<your-chrome-extension-id>",
    "moz-extension://<your-firefox-extension-uuid>",
];
```
Or make extension IDs configurable via environment variable alongside `allowed_origins`.

---

## Vuln 3: CCXT Sidecar PSK Guard Fails Open When Unset

**File:** `testudo-cex/src/middleware/psk.ts:3-12`

* **Severity:** MEDIUM
* **Category:** Authentication Bypass (Defense-in-Depth)
* **Confidence:** 8/10

**Description:** The Pre-Shared Key middleware bypasses authentication when `SIDECAR_PSK` is not set:
```typescript
if (!SIDECAR_PSK) return next();
```
This is fail-open behavior — a missing environment variable silently disables the only network-level authentication for the sidecar.

**Mitigating factors:** The sidecar requires caller-supplied exchange credentials per-request (`parseEnvelope()` rejects missing `apiKey`/`secret`), and production Docker/K8s deployments isolate the sidecar on internal networks. However, this is defense-in-depth — if credentials are obtained through other means (e.g., database breach), the PSK is the last barrier preventing lateral movement through the sidecar.

**Exploit Scenario:** In a deployment where `SIDECAR_PSK` is accidentally unset (misconfigured K8s secret, missing `.env` entry), any pod or process with network access to port 3100 can call sidecar endpoints. If the attacker also has exchange credentials (from another breach vector), the sidecar becomes an unauthenticated trade execution proxy.

**Recommendation:** Fail closed when PSK is unset:
```typescript
if (!SIDECAR_PSK) {
  return res.status(503).json({ error: "PSK not configured" });
}
```
Or validate `SIDECAR_PSK` at startup and refuse to start the server without it.

---

## Vuln 4: Missing Ownership Check on GET /trades/{id}/management (IDOR)

**File:** `testudo-exchange/crates/router/src/routes/trade_management.rs:1995-2045`

* **Severity:** MEDIUM
* **Category:** Insecure Direct Object Reference (IDOR)
* **Confidence:** 9/10

**Description:** The `get_trade_management` handler extracts `_user_id` (note: prefixed with `_`, Rust convention for unused variables) but never verifies the requested position belongs to the caller. It queries `trade_manager.get_position(position_id)` using only the UUID from the URL path.

The underlying service (`trade_manager/service.rs:538-540`) performs a bare HashMap lookup with no user scoping:
```rust
pub async fn get_position(&self, id: Uuid) -> Option<ManagedPosition> {
    self.positions.read().await.get(&id).cloned()
}
```

Other handlers in the same file consistently check ownership:
- `get_trade_group` (line 1149): `if group.user_id != user_id { return Forbidden }`
- `cancel_trade` (line 1200): `if group.user_id != user_id { return Forbidden }`
- `edit_order_entry` (line 1471): `if group.user_id != user_id { return Forbidden }`

This omission is inconsistent with the established pattern.

**Exploit Scenario:** If a position UUID is leaked (through logs, WebSocket messages, or other API responses), any caller can query the position's management state — `current_stop`, `remaining_quantity`, `be_triggered`, `trailing_active`, and `partial_tp_fired` — revealing the victim's trading strategy and risk parameters.

Combined with Vuln 1 (no JWT middleware on `/trades`), this endpoint requires zero authentication.

**Recommendation:** Add ownership check after line 2022:
```rust
if pos.user_id != user_id {
    return HttpResponse::Forbidden()
        .json(ApiResponse::<()>::error("Access denied".to_string()));
}
```

---

## Summary

| # | Finding | Severity | Confidence | Category |
|---|---------|----------|------------|----------|
| 1 | Trade cleanup cancels real exchange orders without JWT auth | **HIGH** | 9/10 | Auth Bypass |
| 2 | CORS allows any browser extension origin with credentials | MEDIUM | 8/10 | Cross-Origin Auth |
| 3 | CCXT sidecar PSK fails open when env var unset | MEDIUM | 8/10 | Auth Bypass |
| 4 | Missing ownership check on trade management endpoint | MEDIUM | 9/10 | IDOR |

## Priority Remediation Order

1. **Vuln 1** — Immediate. Gate `cleanup_stale_trades` live operations behind `is_authenticated`. Apply JwtMiddleware to `/trades` scope.
2. **Vuln 4** — Quick fix. Add one ownership check line, consistent with existing patterns.
3. **Vuln 2** — Before next release. Pin CORS extension origins to specific extension IDs.
4. **Vuln 3** — Next deploy cycle. Change PSK guard to fail-closed.

---

*Review conducted 2026-03-30. Co-Authored-By: Claude Opus 4.6 (1M context)*
