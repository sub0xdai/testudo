# Specification: Extension Error Hardening

**Spec ID:** AUD-07-extension-error-hardening
**Date:** 2026-03-07
**Status:** Complete
**Class:** Audit
**Phase:** 1 (Safety-Critical)
**Audit Refs:** EX-3, EX-4, EX-6, EX-7, EX-8, EX-12, EX-16, EX-21

---

## Overview

Fix 8 error handling and lifecycle bugs in the extension's background worker that silently swallow errors, leak resources on logout, and allow duplicate trade execution.

**Current state:**
- `ensureActiveExchange()` not awaited after login — popup gets success before exchange is ready (EX-4)
- Sidecar health polling interval (30s) never stopped — keeps service worker alive forever (EX-6)
- `refreshTimer` not cleared on logout — fires after logout, fails silently (EX-7)
- WebSocket not disconnected on logout — receives updates for logged-out user (EX-8)
- `.parse()` used instead of `.safeParse()` — ZodError shown raw to user (EX-12)
- No concurrency guard on `executeTrade` — rapid clicks fire duplicate requests (EX-3)
- `cancelTrade` missing 401 retry logic, unlike all other API functions (EX-16)
- `AuthContext.checkAuth` has no error handling — popup hangs on background crash (EX-21)

**Target state:**
- Login flow awaits exchange initialization before returning success
- Logout cleanly stops all timers, disconnects WebSocket, clears state
- All Zod parsing uses `.safeParse()` with user-friendly error messages
- Trade execution is deduplicated with an in-flight guard
- All API functions have consistent 401 retry logic
- AuthContext handles background script failures gracefully

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Await `ensureActiveExchange()` in LOGIN handler before returning success | Critical | Background / Auth |
| FR-2 | Add `stopSidecarHealthPolling()` — clear interval and initial timeout | High | Background / Health |
| FR-3 | On LOGOUT: clear `refreshTimer`, call `disconnectWebSocket()`, call `stopSidecarHealthPolling()`, clear balance state | Critical | Background / Auth |
| FR-4 | Replace all `.parse()` calls with `.safeParse()` — return human-readable error on failure, not raw ZodError JSON | High | Background / Schemas |
| FR-5 | Add `tradeInFlight` guard to `executeTrade` — reject concurrent calls for same trade params | High | Background / Trade |
| FR-6 | Add 401 retry logic to `cancelTrade` matching the pattern in `executeTrade` and `listTrades` | Medium | Background / Trade |
| FR-7 | Wrap `AuthContext.checkAuth` in try/catch — on failure, set `authenticated(false)` and call `onReady(false)` | High | Popup / AuthContext |
| FR-8 | Add `self.addEventListener('unhandledrejection', ...)` to background worker for global error logging | Medium | Background |

---

## Technical Implementation

### 1) Await ensureActiveExchange (FR-1)

```typescript
// Before (line 929-933):
if (msg.type === "LOGIN") {
    return login(msg.email, msg.password).then((result) => {
      if (result.success) ensureActiveExchange(); // fire-and-forget
      return result;
    });
}

// After:
if (msg.type === "LOGIN") {
    return login(msg.email, msg.password).then(async (result) => {
      if (result.success) await ensureActiveExchange();
      return result;
    });
}
```

### 2) Logout Cleanup (FR-2, FR-3)

```typescript
function stopSidecarHealthPolling(): void {
  if (sidecarHealthTimer) {
    clearInterval(sidecarHealthTimer);
    sidecarHealthTimer = null;
  }
}

// LOGOUT handler
if (msg.type === "LOGOUT") {
    if (refreshTimer) clearTimeout(refreshTimer);
    refreshTimer = null;
    disconnectWebSocket();
    stopSidecarHealthPolling();
    return clearTokens().then(() => ({ success: true }));
}
```

### 3) SafeParse Everywhere (FR-4)

```typescript
// Before:
const json = LoginResponseSchema.parse(data);

// After:
const parsed = LoginResponseSchema.safeParse(data);
if (!parsed.success) {
    return { success: false, error: "Unexpected server response" };
}
const json = parsed.data;
```

Apply to all `.parse()` calls in `login()`, `register()`, `doRefresh()`.

### 4) Trade Deduplication (FR-5)

```typescript
let tradeInFlight = false;

async function executeTrade(params: TradeParams): Promise<TradeResult> {
    if (tradeInFlight) {
        return { success: false, error: "Trade already in progress" };
    }
    tradeInFlight = true;
    try {
        // ... existing trade logic
    } finally {
        tradeInFlight = false;
    }
}
```

### 5) cancelTrade 401 Retry (FR-6)

Add the same pattern used in `executeTrade`:

```typescript
if (response.status === 401) {
    const refreshed = await refreshAccessToken();
    if (refreshed) {
        // Retry the cancel request with new token
        return cancelTrade(groupId);
    }
}
```

### 6) AuthContext Error Handling (FR-7)

```typescript
async function checkAuth() {
    try {
        const response = await browser.runtime.sendMessage({ type: "AUTH_STATUS" });
        setAuthenticated(response.authenticated);
        if (response.email) setEmail(response.email);
        props.onReady(response.authenticated);
    } catch (err) {
        console.error("Auth check failed:", err);
        setAuthenticated(false);
        props.onReady(false);
    }
}
```

---

## Verification

```bash
cd testudo-extension && bun run build
```

- [ ] Login returns success only after active exchange is set
- [ ] Logout stops health polling, clears refresh timer, disconnects WS
- [ ] Zod parse failures show "Unexpected server response", not raw JSON
- [ ] Rapid trade clicks return "Trade already in progress" for duplicates
- [ ] cancelTrade retries on 401 with refreshed token
- [ ] Popup shows login screen (not blank) when background script crashes
- [ ] Build succeeds with no TypeScript errors
