# Specification: Extension UX Polish

**Spec ID:** AUD-08-extension-ux-polish
**Date:** 2026-03-07
**Status:** Complete
**Class:** Audit
**Phase:** 3 (Hardening)
**Audit Refs:** EX-14, EX-15, EX-20, EX-23, password reset

---

## Overview

Fix 4 UI state management bugs in the extension popup and add the missing password reset flow.

**Current state:**
- After switching exchange, if balance fetch fails, the OLD exchange's balance is displayed (EX-14)
- QuickTrade `enterCount` not reset on error — safety guard bypassed, single click fires trade (EX-15)
- `ActiveOrders` fires `fetchTrades()` on every `WS_ORDER_UPDATE` without debouncing — request storms during volatility (EX-20)
- `ExchangeSelector` shows stale accounts after deletion in sibling component (EX-23)
- Password reset page referenced in navigation but not implemented

**Target state:**
- Balance is cleared before fetching new exchange data
- QuickTrade safety guard resets on both success and error
- ActiveOrders debounces trade list refresh at 250ms (matching MainView's balance pattern)
- ExchangeSelector re-fetches on account changes
- Users can reset their password via email

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Clear balance state (`setBalance(null)`) when `activeExchangeId` changes, before fetching new balance | High | Popup / MainView |
| FR-2 | Reset `enterCount = 0` in QuickTrade's error/finally path, not just success | High | Popup / QuickTrade |
| FR-3 | Debounce `fetchTrades()` in ActiveOrders at 250ms on `WS_ORDER_UPDATE`, matching MainView's balance debounce pattern | High | Popup / ActiveOrders |
| FR-4 | Watch `browser.storage.onChanged` for `exchangeAccounts` key in ExchangeSelector — re-fetch data list on change | Medium | Popup / ExchangeSelector |
| FR-5 | Add `FORGOT_PASSWORD` message type to background worker — calls backend `/api/v1/auth/forgot-password` | Medium | Background / Auth |
| FR-6 | Add `/forgot-password` route to testudo-web with email input form | Medium | Web / Auth |
| FR-7 | Add `POST /api/v1/auth/forgot-password` endpoint to backend — sends password reset email with JWT token | Medium | Router / Auth |
| FR-8 | Add `POST /api/v1/auth/reset-password` endpoint — validates reset token and updates password | Medium | Router / Auth |

---

## Technical Implementation

### 1) Clear Balance on Exchange Switch (FR-1)

```typescript
// MainView.tsx handleStorageChange
function handleStorageChange(changes: Record<string, { oldValue?: unknown; newValue?: unknown }>) {
    if (changes.activeExchangeId) {
      setBalance(null);       // Clear stale balance
      setBalanceLoading(true);
      fetchBalance();
    }
}
```

### 2) QuickTrade enterCount Reset (FR-2)

```typescript
async function handleConfirm() {
    if (!isValid() || submitting()) return;
    enterCount++;
    if (enterCount < 2) return;
    setSubmitting(true);
    try {
        const response = await browser.runtime.sendMessage({ /* ... */ });
        // handle response...
    } catch (err) {
        setError(String(err));
    } finally {
        enterCount = 0;      // Always reset, not just on success
        setSubmitting(false);
    }
}
```

### 3) ActiveOrders Debounce (FR-3)

```typescript
let fetchTradesTimer: ReturnType<typeof setTimeout> | null = null;

function handleMessage(message: unknown) {
    const msg = message as { type: string };
    if (msg.type === "WS_ORDER_UPDATE") {
        if (fetchTradesTimer) clearTimeout(fetchTradesTimer);
        fetchTradesTimer = setTimeout(() => fetchTrades(), 250);
    }
}

onCleanup(() => {
    if (fetchTradesTimer) clearTimeout(fetchTradesTimer);
});
```

### 4) ExchangeSelector Re-Fetch (FR-4)

```typescript
onMount(() => {
    fetchData();
    browser.storage.onChanged.addListener(handleStorageChange);
});

onCleanup(() => {
    browser.storage.onChanged.removeListener(handleStorageChange);
});

function handleStorageChange(changes: Record<string, unknown>) {
    if (changes.exchangeAccounts || changes.activeExchangeId) {
        fetchData();
    }
}
```

### 5) Password Reset Flow (FR-5, FR-6, FR-7, FR-8)

Backend endpoints:

```rust
// POST /api/v1/auth/forgot-password
// Accepts { email: String }
// Generates JWT with sub=user_id, purpose="password_reset", exp=15min
// Sends email with reset link (or returns token in dev mode)

// POST /api/v1/auth/reset-password
// Accepts { token: String, new_password: String }
// Validates JWT purpose claim, updates bcrypt hash
```

Web frontend: Simple form at `/forgot-password` with email input. On submit, shows "Check your email" message.

---

## Verification

```bash
cd testudo-extension && bun run build
cd testudo-web && bun run build
cd testudo-exchange && cargo clippy --all-targets && cargo test
```

- [ ] Switching exchange shows loading state, not old balance
- [ ] Failed balance fetch shows $-- not old exchange balance
- [ ] QuickTrade requires double-Enter after errors (enterCount resets)
- [ ] ActiveOrders batches rapid WS updates into single fetch (250ms debounce)
- [ ] Deleting exchange account updates ExchangeSelector dropdown
- [ ] Password reset email sent with valid JWT
- [ ] Reset token accepted, password updated
- [ ] All builds succeed
