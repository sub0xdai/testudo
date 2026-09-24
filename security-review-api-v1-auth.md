# API v1 auth review - testudo-exchange router

Review date: 2026-09-24
Target: `https://api.testudo.vip/api/v1/*` and `testudo-exchange/crates/router`
Trigger: findings in `report-0362ff70-cc14-4663-b113-55fbf9ad0e70.json`
Status: findings open, no remediation applied

## 1. Scope correction

The scan report graded `https://testudo.vip`, which is the Astro marketing site on
Cloudflare (worker `testudo-web`, exit 404 on every `/api/*` path).

The API lives on a different host: `api.testudo.vip`, served by `nginx/1.28.3 (Ubuntu)`
in front of the actix router on `127.0.0.1:8080` (`testudo-exchange/.env` `SERVER_ADDR`).
The web app is `desk.testudo.vip`. That is where auth, exchange credentials, and trade
execution actually are, and the report never touched it.

The report's six header findings are still true, in two separate places:

| Surface | Fix location | Applied in this change |
|---|---|---|
| `testudo.vip` (scanned) | `testudo-web/public/_headers` | yes |
| `api.testudo.vip`, unauth and error paths | nginx snippet in section 6 | yes |

Evidence that the API sends no security headers on unauthenticated responses:

```
$ curl -sD- -o/dev/null https://api.testudo.vip/api/v1/health
HTTP/1.1 200 OK
Server: nginx/1.28.3 (Ubuntu)
x-request-id: 8237ac75-d929-4f83-9de9-6aa85d0375ea
access-control-expose-headers: x-request-id
access-control-allow-credentials: true
```

The router sets HSTS, CSP, X-Frame-Options, X-Content-Type-Options, Referrer-Policy on
authenticated responses only, inside `JwtMiddleware` (`middleware/auth.rs:216-241`). Every
unauthenticated response, every 401/403/404, `/health`, `/metrics`, and the whole public
market-data surface ship bare. Headers belong at the edge, not in the JWT layer.

## 2. Method

Code read: `middleware/auth.rs`, `routes/auth.rs`, `services/auth/{siwe,siws,nonce_store,
pairing_store}.rs`, `routes/{agent_keys,paper_balance,sync,trade_management,order,signal}.rs`,
`policy/mod.rs`, `common_utils/src/auth/mod.rs`, and the route table in `main.rs`.

Live checks against `api.testudo.vip`: unauthenticated GETs, CORS preflights from five
origins, and two 60-request bursts to check for edge rate limiting.

Deliberately not tested, because either would have caused a production outage or abused
authorization: the global-lockout DoS in H4, and the pairing-code brute force in H1.

## 3. Findings

### H1 - Pairing code brute force: the limiter is a no-op (critical)

`POST /api/v1/auth/extension-pair` exchanges a 6-digit code for a full access and refresh
token pair (`routes/auth.rs:534-604`).

`pairing_store.rs:20` sets `CODE_LENGTH = 6`, so the search space is 1e6. `pairing_store.rs:22`
gives codes a 300-second TTL.

The rate limit intended to protect it is dead code. `routes/auth.rs:549` calls
`pair_rate_limiter.is_allowed(ip)`, and nothing in the router ever calls
`pair_rate_limiter.record_attempt(...)`. Confirmed by grep across `crates/router/src`:
`record_attempt` is called at `middleware/auth.rs:196,247,648,659,660` and
`routes/public_profile.rs:31,37`, never for the pairing limiter. The bucket stays empty
forever, `is_allowed` returns true forever, and the configured 5-attempts-per-60s
(`main.rs:967`) never applies.

Impact: an unauthenticated attacker guesses codes with unlimited throughput. A code is
live only while a victim is mid-pairing, so the attacker floods continuously. At 1000
requests per second an attacker covers 300k of the 1e6 space inside one 300-second
window, roughly a 30 percent hit rate per victim click, and retries on every click. A hit
returns `access_token` and `refresh_token` for that user (`routes/auth.rs:596-603`). The
account holds exchange API credentials and can place live trades.

Fix, both parts: lengthen the code to at least 8 base32 characters (or bind the code to a
device-generated key pair), and call `record_attempt` on every failed redemption.

### H2 - Agent keys can escalate themselves to unlimited trade execution (critical)

`POST /api/v1/agent-keys` (`routes/agent_keys.rs:31-83`) and `PATCH /api/v1/agent-keys/{id}`
(`routes/agent_keys.rs:167-240`) take `AuthenticatedUser` and write `req.permissions`
straight into the row. Neither checks `auth_method`, and neither calls `authorize()`.

`JwtMiddleware` accepts an `X-Agent-Key` header on the same code path as a SIWE token
(`middleware/auth.rs:258-287`), and `/agent-keys` is wrapped in that middleware
(`main.rs:1410-1417`). `update_key` scopes its UPDATE by `user_id`, so a key can patch the
row it is authenticating with.

Consequence: an agent key minted with `JournalRead` only can PATCH itself to
`Permission::TradeExecute { symbols: None, exchanges: None, max_risk_per_trade: None }`
(`policy/mod.rs:24-55`) and then trade without limit. It can also mint a fresh,
non-expiring key with those permissions, so revoking the original credential does not
remove the attacker's access. That is privilege escalation plus persistence, against a
system that holds exchange API keys.

Fix: require `AuthMethod::Siwe` on all four `/agent-keys` handlers, and reject any
requested permission set that is not a subset of the caller's current set.

### H3 - CORS trusts any browser extension, with credentials (high)

`is_origin_allowed` (`main.rs:80-102`) returns true unconditionally for any
`moz-extension://` origin (`main.rs:97-99`), and returns true for any
`chrome-extension://` origin whenever `ALLOWED_EXTENSION_ORIGINS` is unset
(`main.rs:87-93`). The CORS policy enables `supports_credentials()` (`main.rs:1119`).

Verified live on `https://api.testudo.vip/api/v1/agent-keys`:

```
Origin: moz-extension://11111111-2222-3333-4444-555555555555   -> 200
  access-control-allow-origin: moz-extension://11111111-2222-3333-4444-555555555555
  access-control-allow-credentials: true

Origin: chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa    -> 200
  access-control-allow-origin: chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
  access-control-allow-credentials: true
```

A random 32-character Chrome extension ID was accepted, so the dev-mode fallback is
shipping to production and `ALLOWED_EXTENSION_ORIGINS` is unset there.

Combined with the session cookies, which are `SameSite=None; Secure; HttpOnly` on
`COOKIE_DOMAIN=.testudo.vip` (`routes/auth.rs:54-85`), any Firefox extension in a user's
browser can read that user's journal, balances, and agent keys, and can place trades. It
does not need a permission prompt to do it. Firefox extensions are the single largest
malware surface in the browser.

Fix: delete the `moz-extension://` branch and require the Chrome allowlist to be non-empty
at startup (fail closed when unset). The extension already receives Bearer tokens through
the pairing flow and stores them in `chrome.storage.session`, so cookie-based extension
auth is not needed.

### H4 - Rate limiter keyed on the proxy address: unauthenticated global lockout (high)

`middleware/auth.rs:187-192` derives the rate-limit key from
`connection_info().peer_addr()`. No `set_realip` call and no `X-Forwarded-For` handling
exists anywhere in `crates/router/src` or `crates/common_utils/src`. The router sits behind
nginx on the same host, so the key is always nginx's address.

Two consequences, both from the same root cause.

First, one global bucket for the entire internet. `is_allowed` is evaluated before the
token is read (`middleware/auth.rs:195-202`), so a full bucket rejects requests that carry
valid tokens too. Ten requests bearing an invalid Bearer token fill the 10-attempts-per-15
minutes bucket (`middleware/auth.rs:96,247`) and every authenticated user gets 429 on every
JWT-protected route until it drains. That is an outage triggered by roughly ten
unauthenticated requests. Not executed during this review, precisely because it would have
taken the API down.

Second, `user_sessions.ip_address` records the proxy for every session
(`routes/auth.rs:115-119,150`), so the IP column is unusable for incident response.

Fix, both halves. nginx must overwrite rather than append, since nginx's default
`$proxy_add_x_forwarded_for` prepends an attacker-controlled value:

```nginx
proxy_set_header X-Forwarded-For $remote_addr;
proxy_set_header X-Real-IP $remote_addr;
```

The router must then read that header instead of the socket peer address. Do not simply
switch to `realip_remote_addr()` while nginx is still appending, or the limit becomes
spoofable and unlimited.

Sequencing: fix H1 before H4. Per-IP limits are the wrong control for a 6-digit code, and
restoring them without lengthening the code opens the brute force wider.

### M1 - The policy engine is not wired in: agent keys are effectively unscoped (medium)

`authorize()` has seven call sites in the whole router: `routes/agent_journal.rs:32,77,108`,
`routes/signal.rs:83`, `routes/exchanges.rs:155,289`, `routes/risk_config.rs:111`.

Four route files that touch money and credentials contain none:
`routes/trade_management.rs`, `routes/order.rs`, `routes/journal.rs`, `routes/agent_keys.rs`.

An agent key with only `JournalRead` can therefore create trades via `POST /api/v1/trades`,
execute live orders via `POST /api/v1/order`, and write journal entries via
`POST /api/v1/journal/entries`. Combined with H2 it can also widen its own scope. The
permission model in `policy/mod.rs` and `AgentKeyClaims` is decorative on those paths.

Fix: call `authorize()` on every handler that mutates state or reads account data, and make
the missing call a test failure rather than a code review item.

### M2 - TradeExecute limits fail open (medium)

`PolicyEngine::authorize_trade` (`policy/mod.rs:292-337`) enforces each limit only when the
permission field and the context field are both present, using
`if let (Some(allowed), Some(ctx_...))`. A missing context field silently skips the check.

`routes/signal.rs:83-87` builds the only `ActionContext` in that file and populates `symbol`
alone. The comment above it claims exchange, risk, and position checks are deferred until
the account and size are known, but no second check exists. `exchanges`, `max_risk_per_trade`,
and `max_open_positions` are never enforced on `POST /api/v1/signals`.

A key minted with `max_risk_per_trade: Some(50)` and `exchanges: Some(["hyperliquid"])` can
trade any venue at any size.

Fix: perform the deferred authorization once the account and size are resolved, and treat a
missing context field for an agent key as a hard failure instead of a skip.

### M3 - Paper balance endpoints trust a client-supplied identity (medium)

`/paper` is not wrapped in `JwtMiddleware` (`main.rs:1430-1438`), and both handlers read the
user from an `X-User-Id` request header (`routes/paper_balance.rs:23-33`).

Verified live:

```
$ curl -H "X-User-Id: 1025c0f4-e30b-4646-b600-39aa4239aacc" \
    https://api.testudo.vip/api/v1/paper/balances
{"success":true,"data":[{"asset":"USDT","available":"10000","locked":"0"}],"error":null}
```

No token required. The UUID did not exist and a 10000 USDT book was created for it.
`POST /api/v1/paper/reset` is likewise unauthenticated and accepts any UUID.

Walking versions 4 UUIDs is not practical, so this is not exploitable at scale today. It is
still a broken authorization model: identity comes from a header the client controls, and
any UUID leak (shared trade export, support ticket, log) grants read and reset on that
user's book.

Note that `/trades` is correctly protected, and a request there with only `X-User-Id` gets
401 from the middleware, so the identical fallback in `trade_management.rs:71-84` is
unreachable dead code. Delete it so it cannot be re-exposed by a future route change.

Fix: wrap `/paper` in `JwtMiddleware` and delete the header-derived identity paths.

### L1 - `/api/v1/metrics` is public (low)

`main.rs:1146` mounts the Prometheus handler with no middleware. Anyone can read business
and infrastructure metrics. Bind it to a separate internal listener or gate it on an
allowlist.

### L2 - Agent-key responses skip the security headers (low)

The agent-key branch returns at `middleware/auth.rs:264-267` without the header block that
the JWT branch applies at lines 216-241. Any authenticated response through an agent key is
header-less. Hoist the header insertion so both branches share it, or delete it once nginx
owns the headers.

### L3 - Public auth endpoints are unrated, and account creation is free (low)

`/auth/nonce`, `/verify-siwe`, `/verify-siws`, and `/auth/refresh` sit outside
`JwtMiddleware`, and no other limiter covers them. `/auth/nonce` allocates a store entry per
call, and `find_or_create_by_wallet` (`routes/auth.rs:226,301`) creates a user row for any
wallet that presents a valid signature. An attacker can generate key pairs and create users
without bound. 20 rapid `/auth/nonce` calls returned 200 for all 20, with no edge limiting
either.

Fix: per-IP limits on the public auth scope once H4 is resolved, plus a cheap cost on
first-time account creation.

### L4 - `/sync` is unauthenticated and returns a hardcoded success (low)

`main.rs:1191-1196` mounts three handlers with no middleware; all three are stubs that
return `success: true, is_synced: true` regardless of state (`routes/sync.rs:139-185`).
Confirmed live with an unauthenticated GET. Not a vulnerability by itself, but any agent
built against `AGENT_TRADING.md` that trusts `is_synced` is being told a lie. Delete the
routes or implement them behind auth.

### L5 - Limiter state and process-local stores (low)

`RateLimiter` keeps a `HashMap<IpAddr, Vec<Instant>>` whose empty per-IP entries are never
removed (`middleware/auth.rs:57-82`), so the map grows without bound once real IPs are used.

`NonceStore` and `PairingStore` are in-process `DashMap`s. Login and extension pairing break
if the router is ever run as more than one replica. Fine at one container, worth a note
before any horizontal scale-out.

## 4. What is done well

Stated because it constrains the fixes above and should not be regressed.

- JWT secrets fail closed. `JWT_ACCESS_SECRET` is required via `expect()` (`main.rs:160-161`)
  with no default fallback. The deployed values are 41 and 42 characters, not known defaults.
- No algorithm confusion. `jsonwebtoken 9.3` with `Validation::default()` pins HS256
  (`common_utils/src/auth/mod.rs:104-131`), the issuer is pinned, `validate_exp` is on, there
  is a redundant explicit expiry check, and `token_type` is compared so a refresh token
  cannot be used as an access token.
- SIWE and SIWS validation is sound: nonce is consumed before validation and is single-use
  with a 300-second TTL and 32 alphanumeric characters (`nonce_store.rs`), domain and
  expiration are checked, the signer is recovered, and SIWS additionally compares
  `parsed.address` against `body.address` (`routes/auth.rs:288-292`).
- Refresh rotation revokes the old session before issuing new tokens
  (`routes/auth.rs:403-430`), so a rotated token cannot be replayed.
- Cookies are `HttpOnly`, `Secure`, and path-scoped narrowly to `/api` and `/api/v1/auth`.
  Raw tokens are never returned to the web client.
- Agent keys are 32 bytes from `OsRng`, formatted `tudo_sk_<base64url>`, stored as SHA-256
  and never logged, with revocation and expiry checked per request (`services/agent_key.rs`).
- `X-User-Id` is not trusted on the protected `/trades` surface, verified live.

## 5. Fix order

1. H1 and H3, today. Both are reachable by an unauthenticated attacker with no user action
   beyond installing an extension or clicking pair.
2. H2, today. One handler-level `AuthMethod` check closes escalation and persistence.
3. H4 with the nginx real-IP change, and lengthen the pairing code in the same commit.
4. M1 and M2, then add a test that fails when a mutating handler omits `authorize()`.
5. M3, L1 through L5, and delete the dead `X-User-Id` fallbacks.
6. Deploy the header changes and re-scan to close the original report.

## 6. Deployment notes for the header changes

- `testudo-web/public/_headers` requires a deploy of the `testudo-web` worker. CSP is in
  report-only mode on purpose: the site has many inline `script` tags and inline `style`
  attributes, so an enforcing policy needs a pass over real violation reports first.
- The API header block is section 7. Include it in the `api.testudo.vip` server block. Two
  nginx behaviours to respect: `add_header` directives are not inherited into a `location`
  that declares its own `add_header`, and upstream copies of the same headers should be
  dropped with `proxy_hide_header` in the proxy location so one layer owns the values.
- Verify before enforcing that nothing embeds `api.testudo.vip` responses in an iframe, since
  the block sets `X-Frame-Options: DENY`.
- HSTS with `includeSubDomains` on `api.testudo.vip` covers only `*.api.testudo.vip`. Only
  `api` and `desk` subdomains of `testudo.vip` resolve today, and both serve HTTPS, so the
  `includeSubDomains` carried into the apex site's `_headers` is low risk. Confirm in the
  Cloudflare zone settings if other subdomains are added later.

## 7. nginx header block

Include in the `api.testudo.vip` server block. The `always` flag covers 401, 403, 404, and
5xx responses, which is exactly the surface the security report found bare.

```nginx
add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
add_header X-Content-Type-Options "nosniff" always;
add_header X-Frame-Options "DENY" always;
add_header Referrer-Policy "strict-origin-when-cross-origin" always;
add_header Permissions-Policy "camera=(), microphone=(), geolocation=(), payment=()" always;
add_header Content-Security-Policy "default-src 'none'; frame-ancestors 'none'" always;
```

The CSP is deliberately a JSON-API policy. Do not copy it to `testudo.vip`, where the site
is HTML with inline scripts and would break.
