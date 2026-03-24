# Implementation Plan

> Last updated: 2026-03-24
> Current spec: AUTH-03-frontend-auth
> Phase: BUILD

---

## Active Spec: AUTH-03-frontend-auth

Frontend auth migration — wallet connect login (SIWE), cookie-based sessions, extension device pairing. Replaces email/password UI and localStorage tokens across all three frontends.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Web: API client + AuthContext rewrite — withCredentials cookies, /auth/me session restore, remove Bearer injection + localStorage tokens + refresh queue. Update types (User wallet_address). Delete RegisterPage/ForgotPasswordPage, stub LoginPage, clean routes. | complete | high | AUTH-02 |
| T2 | Web: LoginPage SIWE flow — fetch nonce, construct EIP-4361 message, sign via wagmi, POST verify-siwe, auto-trigger after wallet connect | complete | high | T1 |
| T3 | Web: Extension pairing UI + AccountPage cleanup — ExtensionPairing.tsx component, AccountPage removes email display + adds pairing section | complete | medium | T1 |
| T4 | Journal: Cookie-based auth migration — credentials: "include" on all fetches, remove getToken/refreshAccessToken/refreshPromise/manual Authorization headers, cookie-based 401 refresh | complete | medium | AUTH-02 |
| T5 | Extension: Token storage migration — delete token-sync.ts, remove from manifest.json, auth.ts → chrome.storage.session, update schemas | complete | medium | AUTH-02 |
| T6 | Extension: Pairing flow + UI migration — replace login/register handlers with handlePair, api.ts pair endpoint, PairView.tsx replaces AuthSection, update popup AuthContext + App.tsx | complete | high | T5 |
| T7 | Build validation — bun run build for web + extension + journal, verify acceptance criteria | pending | low | T1-T6 |

### Key Decisions

- **Cookie-based auth, no localStorage**: `withCredentials: true` on Axios, no Bearer header injection. 401 interceptor does cookie-based refresh (empty POST to `/auth/refresh` with `withCredentials`), then retries. No queue needed — cookies handle concurrent requests.
- **AuthContext uses /auth/me on mount**: No JWT decoding, no localStorage init. `useEffect` calls `/auth/me` to restore session from cookie. `login(user: User)` is called by the SIWE flow (T2), not by AuthContext itself.
- **User model: wallet_address replaces email**: `User { id, wallet_address }`. AccountPage displays truncated address. Old email/password types (AuthTokens, LoginResponse, TokenResponse) deleted.
- **RegisterPage + ForgotPasswordPage deleted**: No registration — wallet creates account on first SIWE. No password to forget. Routes removed from App.tsx.
- **LoginPage stubbed with ConnectButton**: Shows RainbowKit `ConnectButton` only. T2 adds the SIWE signature flow after wallet connect.
- **LoginFormSchema + RegisterFormSchema deleted**: Only ExchangeAccountFormSchema remains in validation/forms.ts.

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| AUTH-02-backend-auth | 2026-03-24 |
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
