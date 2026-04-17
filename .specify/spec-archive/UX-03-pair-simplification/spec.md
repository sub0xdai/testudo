# Specification: Simplify Pair Page to Two-State Wallet-First Flow

**Spec ID:** UX-03-pair-simplification
**Date:** 2026-04-04
**Status:** Draft
**Class:** Feature / UX
**Priority:** P1 — Current 3-state pair page confuses new users; wallet connection should be the primary gateway
**Depends on:** UX-01-pair-page (complete)
**Series:** UX-01 through UX-03 (pairing UX evolution)

---

## Problem Statement

The Pair page at `/desk/pair` (`testudo-journal/src/pages/Pair.tsx`) currently has three sequential states: (1) no extension detected → show Chrome/Firefox download buttons, (2) extension detected but unauthenticated → show "Connect Wallet" button, (3) authenticated → show 6-digit pairing code. This 3-step funnel interrupts user momentum by forcing them to acknowledge an extension installation state before they can connect their primary identity — their wallet.

The extension detection mechanism (`window.postMessage` listener for `TESTUDO_INSTALLED`) is unreliable and creates a false gate. A user who has installed the extension but hasn't refreshed the page gets stuck at state 1. More fundamentally, the wallet is the user's root identity in a decentralized environment — establishing that connection should always be the primary gateway, with the extension treated as a companion tool.

Additionally, once the user generates a 6-digit code and enters it in the extension, the Pair page has no awareness that the code was consumed. The `PairingStore` uses one-time `take()` semantics that remove the code on consumption, but never records the redemption event. The user sits on a countdown timer with no feedback that pairing succeeded. The `localStorage` flag `testudo-extension-paired` is set optimistically on code generation, not on actual extension pairing completion.

This spec collapses states 1+2 into a single "Connect Wallet" view, adds backend redemption tracking to `PairingStore`, exposes a polling endpoint, and auto-transitions the Pair page to a "Successfully Linked" state when the extension redeems the code.

---

## User Stories

- **As a new user**, I want to see a clear "Connect Wallet" action when I arrive at the pair page, so that I immediately know how to begin onboarding without being blocked by extension detection.
- **As an authenticated user**, I want the pair page to automatically show me that pairing succeeded after I enter the code in the extension, so that I have confidence the flow is complete and can proceed to the trading desk.
- **As a user who forgot to install the extension**, I want to see download links on the same page as the wallet connection, so that I can install the extension without losing my place in the flow.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `PairingStore` tracks recently-redeemed user IDs in a `redeemed: DashMap<Uuid, Instant>` field, populated when `take()` succeeds | High | Backend |
| FR-2 | `PairingStore::check_paired(user_id)` returns `true` if user_id exists in `redeemed` and entry is within TTL | High | Backend |
| FR-3 | `PairingStore::cleanup()` prunes expired entries from both `codes` and `redeemed` maps | High | Backend |
| FR-4 | `GET /api/v1/auth/pair-status` authenticated endpoint returns `{ "paired": true/false }` by calling `check_paired(user.user_id)` | High | Backend |
| FR-5 | Route registered in authenticated auth scope in `main.rs` alongside `/pair-extension` and `/me` | High | Backend |
| FR-6 | Pair page shows single "Connect Wallet" state when unauthenticated, with Chrome/Firefox text links as secondary helper below | High | Frontend |
| FR-7 | Pair page removes extension detection logic (`extensionDetected` signal, `TESTUDO_INSTALLED` message listener) | High | Frontend |
| FR-8 | After code generation, Pair page polls `GET /api/v1/auth/pair-status` every 3 seconds | High | Frontend |
| FR-9 | When poll returns `{ paired: true }`, page transitions to "Extension Linked" success state with "Open Trading Desk" CTA | High | Frontend |
| FR-10 | Polling stops on: success, code expiry, component unmount, or navigation away from `/desk/pair` | Medium | Frontend |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Backend: `PairingStore` redeemed tracking + `check_paired()` + tests | `cargo test -p router` passes with new tests |
| CP-2 | Backend: `GET /api/v1/auth/pair-status` route + registration in `main.rs` | Endpoint returns correct JSON; `cargo clippy --all-targets` clean |
| CP-3 | Frontend: Redesign `Pair.tsx` (2 states + success) + `checkPairStatus()` API call + polling | `bun run build` passes; visual confirmation of all 3 states |

### Backend: PairingStore Changes

**File:** `testudo-exchange/crates/router/src/services/auth/pairing_store.rs`

```rust
pub struct PairingStore {
    codes: DashMap<String, (Uuid, Instant)>,
    redeemed: DashMap<Uuid, Instant>,  // NEW: tracks recently-paired users
    ttl: Duration,
}

impl PairingStore {
    pub fn new() -> Self {
        Self {
            codes: DashMap::new(),
            redeemed: DashMap::new(),
            ttl: Duration::from_secs(DEFAULT_PAIRING_TTL_SECS),
        }
    }

    pub fn take(&self, code: &str) -> Option<Uuid> {
        if let Some((_, (user_id, created_at))) = self.codes.remove(code) {
            if created_at.elapsed() < self.ttl {
                self.redeemed.insert(user_id, Instant::now());  // NEW
                return Some(user_id);
            }
        }
        None
    }

    /// Check if a user was recently paired (code redeemed within TTL).
    pub fn check_paired(&self, user_id: &Uuid) -> bool {
        self.redeemed
            .get(user_id)
            .map(|entry| entry.value().elapsed() < self.ttl)
            .unwrap_or(false)
    }

    fn cleanup(&self) {
        let ttl = self.ttl;
        self.codes.retain(|_, (_, created_at)| created_at.elapsed() < ttl);
        self.redeemed.retain(|_, created_at| created_at.elapsed() < ttl);  // NEW
    }
}
```

### Backend: New Route

**File:** `testudo-exchange/crates/router/src/routes/auth.rs`

```rust
pub async fn pair_status(
    user: AuthenticatedUser,
    pairing_store: web::Data<PairingStore>,
) -> Result<HttpResponse> {
    let paired = pairing_store.check_paired(&user.user_id);
    Ok(HttpResponse::Ok().json(serde_json::json!({ "paired": paired })))
}
```

**File:** `testudo-exchange/crates/router/src/main.rs` (line ~876, authenticated auth scope)

```rust
.route("/pair-status", web::get().to(auth::pair_status))
```

### Frontend: API Client

**File:** `testudo-journal/src/api/client.ts` (after `pairExtension()` at line 572)

```typescript
export async function checkPairStatus(): Promise<{ paired: boolean }> {
  const res = await fetchWithCredentials(`${API_BASE}/api/v1/auth/pair-status`)
  if (!res.ok) return { paired: false }
  return res.json()
}
```

### Frontend: Pair.tsx Redesign

**File:** `testudo-journal/src/pages/Pair.tsx`

Remove:
- `extensionDetected` signal
- `handleMessage()` function and `TESTUDO_INSTALLED` listener
- `ChromeIcon` and `FirefoxIcon` SVG components (30+ lines each)
- State 1 (no extension) and State 2 (extension detected, no auth) `<Show>` blocks

Add:
- `paired` signal (boolean, default false)
- `pollTimer` ref for cleanup
- Polling logic: after `generateCode()` succeeds, start `setInterval` every 3000ms calling `checkPairStatus()`. On `{ paired: true }`, set `paired(true)`, clear interval.
- Stop polling on `onCleanup`, on code expiry, on success.

Three UI states in content area:

| Condition | Content |
|-----------|---------|
| `!auth.isAuthenticated()` | "CONNECT WALLET" heading, Connect Wallet button, separator, "Need the extension? Chrome · Firefox" text links |
| `auth.isAuthenticated() && !paired()` | Existing code display: 6-digit code, countdown, copy, regenerate (unchanged logic) |
| `paired()` | "EXTENSION LINKED" heading with checkmark, "Your extension is now connected to your wallet.", "OPEN TRADING DESK" button → `/desk/` |

Keep unchanged:
- Header band (TESTUDO / TRADING TERMINAL / shield)
- Bottom nav cards (DESK / TESTUDO.VIP)
- `generateCode()`, `copyCode()`, countdown/timer logic
- `handleConnect()` function
- Background image + overlay
- `CHROME_STORE_URL` and `FIREFOX_STORE_URL` constants

### Paved Roads

- `DashMap` concurrent map already used in `PairingStore` for `codes` — same pattern for `redeemed`
- `AuthenticatedUser` extractor already used by `pair_extension()` — reuse for `pair_status()`
- `fetchWithCredentials()` in `client.ts` handles cookie-based auth — reuse for polling endpoint
- `handleConnect()` polling pattern (line 116-123 in Pair.tsx) already polls for auth state change — similar pattern for pair status

### Files

- `testudo-exchange/crates/router/src/services/auth/pairing_store.rs` — add `redeemed` field, `check_paired()`, update `cleanup()`, update `Default`
- `testudo-exchange/crates/router/src/routes/auth.rs` — add `pair_status()` handler
- `testudo-exchange/crates/router/src/main.rs` — register `/pair-status` GET route in authenticated auth scope (line ~876)
- `testudo-journal/src/api/client.ts` — add `checkPairStatus()` function
- `testudo-journal/src/pages/Pair.tsx` — full redesign: remove extension detection, collapse to 2 states + success state, add polling

### Dependencies Added

None — all required crates (`dashmap`, `uuid`, `serde_json`, `actix-web`) already in scope.

---

## Acceptance Criteria

- [ ] `PairingStore::check_paired()` returns `true` after a code is consumed via `take()`, and `false` before consumption
- [ ] `PairingStore::check_paired()` returns `false` after TTL expires on the redeemed entry
- [ ] `PairingStore::cleanup()` prunes both `codes` and `redeemed` maps
- [ ] `GET /api/v1/auth/pair-status` returns `{ "paired": false }` before code redemption
- [ ] `GET /api/v1/auth/pair-status` returns `{ "paired": true }` after extension redeems code
- [ ] `GET /api/v1/auth/pair-status` returns 401 without valid JWT
- [ ] Pair page shows Connect Wallet button when unauthenticated
- [ ] Chrome and Firefox links visible as secondary text on unauthenticated state
- [ ] After wallet connect, 6-digit code appears with countdown timer
- [ ] After code entry in extension, page auto-transitions to "Extension Linked" state
- [ ] Polling stops on code expiry, component unmount, and successful pairing
- [ ] `cargo clippy --all-targets && cargo test -p router` passes
- [ ] `cd testudo-journal && bun run build` passes

---

## Risks

1. **Polling overhead** — 3-second interval per active Pair page session creates backend load. Mitigation: `check_paired()` is a zero-allocation `DashMap::get()` lookup with no database or network I/O; supports thousands of concurrent polls trivially.
2. **Redeemed map memory leak** — Abandoned pairing attempts leave entries in `redeemed` DashMap indefinitely if `cleanup()` only runs on `generate()`. Mitigation: `cleanup()` is called on every `generate()`, pruning expired entries from both maps. For additional safety, a background Tokio task could periodically call `cleanup()`, but the current piggyback approach is sufficient for expected user volume.
3. **Race between expiry and poll** — Code expires at 60s, but poll runs every 3s. If extension redeems at second 59, the redeemed entry has ~1s before TTL prunes it. Mitigation: The `redeemed` TTL uses its own `Instant::now()` at insertion time, so the 60s window starts fresh from redemption — the redeemed entry lives a full 60s after the extension redeems, independent of when the code was generated.

---

## Completion Signal

This spec is complete when:
1. `PairingStore` tracks redeemed user IDs and `check_paired()` works with TTL expiry
2. `GET /api/v1/auth/pair-status` endpoint registered and returns correct paired status
3. `Pair.tsx` redesigned with wallet-first 2-state flow + success state + polling
4. All acceptance criteria met
5. `cargo clippy --all-targets && cargo test -p router` passes
6. `cd testudo-journal && bun run build` passes
7. Code committed to master
