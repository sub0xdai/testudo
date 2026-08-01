# Specification: Extension Error Recovery with Actionable Messages

**Spec ID:** UXA-03-extension-error-recovery
**Date:** 2026-04-01
**Status:** Draft
**Class:** Feature / Frontend
**Priority:** P1 — Raw hex error messages erode user trust during trading
**Depends on:** UXA-01-agent-wallet-visibility (backend must provide `error_code` field)
**Series:** UXA-01 through UXA-03 (Agent Wallet Resilience)

---

## Problem Statement

The extension displays backend error messages verbatim. When a trade fails, the content script shows `showToast(`Error: ${response.error}`, "error")` in `content.ts:214-254` — a 5-second red toast that disappears. For agent wallet errors, this produces messages like `"Error: Exchange error: User or API Wallet 0xb03c9ce4b61d446f4e0c7f978499f9556259a35e does not exist."` — raw hex addresses that mean nothing to the user and vanish in 5 seconds.

The background worker in `background/api.ts:263-292` passes errors straight through from `apiRequest()` without any client-side transformation. The only human-readable mapping happens server-side in `format_exchange_error()`, and that function (pre-UXA-01) only handles 2 patterns. After UXA-01 adds `error_code` to API responses, the extension can use structured codes for richer error UX — persistent banners for configuration errors, actionable next-step guidance, and direct links to the desk.

---

## User Stories

- **As a trader**, I want agent wallet errors to explain the problem and tell me how to fix it, so that I can resolve the issue myself instead of panicking.
- **As a trader**, I want configuration errors (broken auth, missing account) to persist on screen until I dismiss them, so that I don't miss a critical 5-second toast while looking at the chart.
- **As a trader**, I want a direct link to the desk Account page when my agent wallet needs re-authorization, so that I can fix it in one click.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Extension parses `error_code` from API error responses (from UXA-01) | High | Background |
| FR-2 | `error_code: "agent_wallet_inactive"` shows a persistent banner (not toast) with message "Agent wallet needs re-authorization" and a "Fix in Account Settings" link | High | Content |
| FR-3 | `error_code: "agent_wallet_expired"` shows same persistent banner treatment as `agent_wallet_inactive` | High | Content |
| FR-4 | `error_code: "rate_limited"` shows a toast with "Exchange is busy — retrying..." (auto-dismisses after 5s, no user action needed) | Medium | Content |
| FR-5 | `error_code: "insufficient_margin"` shows a toast with the existing margin message (no change in behavior, just verify) | Low | Content |
| FR-6 | Unrecognized `error_code` or missing code falls back to current toast behavior (raw error message, 5s) | Medium | Content |
| FR-7 | Persistent banners include a dismiss button (X) and do not auto-dismiss | High | Content |
| FR-8 | "Fix in Account Settings" link opens the desk Account page in a new tab (`{DESK_URL}/desk/#/account`) | High | Content |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | FR-1 + FR-6: Parse error_code from API responses, fallback behavior | Extension handles structured errors without breaking existing flow |
| CP-2 | FR-2 + FR-3 + FR-7 + FR-8: Persistent banner for agent wallet errors | Configuration errors persist with actionable link |
| CP-3 | FR-4 + FR-5: Toast refinements for transient errors | Rate limits and margin errors show appropriate messages |

### Error Code Parsing

```typescript
// background/api.ts — extend apiRequest result
interface ApiErrorResult {
  ok: false
  error: string
  error_code?: string  // NEW: from UXA-01 backend
}

// Modify apiRequest to extract error_code from response JSON
if (!res.ok) {
  const body = await res.json().catch(() => ({}))
  return {
    ok: false,
    error: body.error || body.message || `HTTP ${res.status}`,
    error_code: body.error_code || undefined,
  }
}
```

### Error Classification

```typescript
// NEW: content.ts or new file content-errors.ts
type ErrorSeverity = 'transient' | 'configuration'

interface ClassifiedError {
  severity: ErrorSeverity
  message: string
  action?: { label: string; url: string }
}

function classifyError(error: string, errorCode?: string): ClassifiedError {
  switch (errorCode) {
    case 'agent_wallet_inactive':
    case 'agent_wallet_expired':
      return {
        severity: 'configuration',
        message: 'Agent wallet needs re-authorization.',
        action: {
          label: 'Fix in Account Settings',
          url: `${DESK_URL}/desk/#/account`,
        },
      }
    case 'rate_limited':
      return {
        severity: 'transient',
        message: 'Exchange is busy — wait a moment and retry.',
      }
    case 'insufficient_margin':
      return {
        severity: 'transient',
        message: 'Insufficient margin — reduce size or increase leverage.',
      }
    default:
      return {
        severity: 'transient',
        message: error,
      }
  }
}
```

### Persistent Banner (Shadow DOM)

```typescript
// content.ts — new function alongside showToast
function showBanner(message: string, action?: { label: string; url: string }): void {
  // Inject into existing Shadow DOM host (same as toast)
  // Banner sits at top of viewport, does not auto-dismiss
  const banner = document.createElement('div')
  banner.className = 'testudo-banner error'
  banner.innerHTML = `
    <span class="icon"></span>
    <span class="message">${message}</span>
    ${action ? `<a href="${action.url}" target="_blank" class="action">${action.label}</a>` : ''}
    <button class="dismiss">&times;</button>
  `
  banner.querySelector('.dismiss')?.addEventListener('click', () => banner.remove())
  // ... append to shadow root
}
```

### Banner CSS (inline in Shadow DOM)

```css
/* Added to TOAST_CSS constant in modal.tsx */
.testudo-banner {
  position: fixed;
  top: 12px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 2147483647;
  font-family: 'JetBrains Mono', monospace;
  font-size: 12px;
  padding: 10px 16px;
  display: flex;
  align-items: center;
  gap: 10px;
  max-width: 500px;
}
.testudo-banner.error {
  background: #0a0a0a;
  color: #f59e0b;  /* amber, not red — config issue, not failure */
  border: 1px solid #3a2a0a;
}
.testudo-banner .action {
  color: #d4d4d4;
  text-decoration: underline;
  cursor: pointer;
  white-space: nowrap;
}
.testudo-banner .action:hover { color: #fff; }
.testudo-banner .dismiss {
  background: none;
  border: none;
  color: #666;
  cursor: pointer;
  font-size: 16px;
  padding: 0 4px;
}
.testudo-banner .dismiss:hover { color: #fff; }
```

### Trade Result Handler Update

```typescript
// content.ts — modify executeTrade result handling
if (response.success) {
  showToast("Order Sent", "success")
} else {
  const classified = classifyError(
    response.error || "Unknown error",
    response.error_code
  )
  if (classified.severity === 'configuration') {
    showBanner(classified.message, classified.action)
  } else {
    showToast(`Error: ${classified.message}`, "error")
  }
}
```

### Paved Roads

- `showToast()` in `modal.tsx:297-348` — existing toast infrastructure in Shadow DOM. Banner follows same injection pattern.
- `TOAST_CSS` constant in `modal.tsx` — shared CSS block. Banner CSS appended here.
- `DESK_URL` constant — already exists in extension for desk links (used by position cards, header links).
- `BackendResponseSchema` in `schemas.ts` — Zod schema for API responses. Add optional `error_code` field.

### Files

- `testudo-extension/src/background/api.ts` — extract `error_code` from API error responses
- `testudo-extension/src/schemas.ts` — add `error_code` to `BackendResponseSchema`
- `testudo-extension/src/content.ts` — add `classifyError()`, `showBanner()`, update trade result handler
- `testudo-extension/src/modal.tsx` — add banner CSS to `TOAST_CSS` constant

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Agent wallet errors (`error_code: "agent_wallet_inactive"` or `"agent_wallet_expired"`) show persistent amber banner with "Fix in Account Settings" link
- [ ] Banner persists until user clicks dismiss (X) — does not auto-dismiss
- [ ] "Fix in Account Settings" link opens desk Account page in new tab
- [ ] Rate limit errors show transient toast with retry message (5s auto-dismiss)
- [ ] Unrecognized error codes fall back to current toast behavior (raw message, 5s)
- [ ] `BackendResponseSchema` validates `error_code` as optional string
- [ ] Extension builds without errors: `bun run build` in testudo-extension
- [ ] Existing toast behavior unchanged for non-error-code responses

---

## Risks

1. **UXA-01 must ship first** — Without `error_code` in API responses, the extension has no structured signal to classify errors. Mitigation: FR-6 ensures graceful fallback — unrecognized/missing codes use current behavior. The extension can ship the classification logic even before UXA-01, with the fallback path covering all cases.
2. **Banner z-index conflicts** — Persistent banners at `z-index: 2147483647` could conflict with TradingView's own overlays or the trade confirmation modal. Mitigation: Use same z-index as existing toasts (already proven to work above TradingView). Banner dismisses on click, so it won't permanently block chart interaction.
3. **`DESK_URL` may not be configured** — If the desk URL env var isn't set, the "Fix in Account Settings" link will be broken. Mitigation: Fall back to `https://testudo.vip/desk/#/account` (production URL) if `DESK_URL` is empty.

---

## Completion Signal

This spec is complete when:
1. Agent wallet errors show persistent amber banner with actionable link
2. Transient errors (rate limit, margin) show appropriate toast messages
3. Error classification handles all UXA-01 error codes plus fallback
4. All acceptance criteria met
5. `bun run build` passes in testudo-extension
6. Code committed to master
