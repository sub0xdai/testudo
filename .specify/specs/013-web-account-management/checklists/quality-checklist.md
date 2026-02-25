# Quality Checklist - 013 Web Account Management

**Spec:** 013-web-account-management
**Date:** 2026-02-26

## Routing

- [ ] React Router v6 installed and configured in `main.tsx`
- [ ] `/` renders full landing page with SpotlightBackground (no regression)
- [ ] `/login` renders login form
- [ ] `/register` renders register form
- [ ] `/account` is protected — redirects to `/login` when unauthenticated
- [ ] Direct URL navigation works (SPA fallback configured)
- [ ] Header nav links update based on auth state (LOGIN vs ACCOUNT)

## Auth Context

- [ ] `AuthProvider` wraps app inside `BrowserRouter`
- [ ] `login()` stores tokens in `localStorage` and sets user state
- [ ] `register()` stores tokens and sets user state (backend returns tokens on register)
- [ ] `logout()` clears tokens, resets state, best-effort server logout call
- [ ] On mount: restores user from `localStorage` token (no flash of unauth state)
- [ ] `ProtectedRoute` redirects to `/login` while `loading` is false and user is null

## API Client

- [ ] Axios instance with configurable `baseURL` via `VITE_API_URL`
- [ ] Request interceptor attaches `Authorization: Bearer` header
- [ ] 401 response interceptor attempts token refresh before retry
- [ ] Refresh loop prevention (flag to block concurrent refresh attempts)
- [ ] Failed refresh clears tokens and redirects to `/login`
- [ ] Credentials (`api_key`, `secret`) never logged or persisted client-side beyond form state

## Auth Pages

- [ ] Login form: email + password + submit + error display + register link
- [ ] Register form: email + password + confirm password + submit + error display + login link
- [ ] Client-side validation: email format, password min 8 chars, confirm match
- [ ] Server errors displayed inline (invalid credentials, user exists, etc.)
- [ ] Successful auth redirects to `/account`
- [ ] Already-authenticated users redirected from auth pages to `/account`

## Account Page

- [ ] Fetches exchanges and accounts in parallel on mount
- [ ] Displays connected accounts as cards with exchange name and status
- [ ] Add form: exchange dropdown, API key, secret, conditional passphrase
- [ ] Exchange dropdown excludes already-connected exchanges
- [ ] Add account validates via backend (CCXT sidecar) and refreshes list
- [ ] Test connection shows latency ms on success, error on failure
- [ ] Delete with inline confirmation (DEL → CONFIRM/NO pattern)
- [ ] User email and logout button in page header
- [ ] Loading and error states handled gracefully

## Extension Simplification

- [ ] `ExchangeManager` removed from `SettingsView.tsx` render
- [ ] "MANAGE ACCOUNTS" link/button opens web `/account` in new tab
- [ ] Background message handlers for exchange operations preserved (no regression)
- [ ] Extension trade execution still reads active exchange from backend

## Design Consistency

- [ ] Background color: `#050505` (main-bg) on all pages
- [ ] Panel/card backgrounds: `#0A0A0A` (container-bg)
- [ ] Border color: `#333333` (container-border)
- [ ] Zero border-radius throughout (brutalist aesthetic)
- [ ] Typography: Unbounded for headings, Space Mono for body/forms
- [ ] Primary action color: `signal-green` (#00FF41)
- [ ] Error color: `signal-red` (#FF003C)
- [ ] Interactive accent: steel (#94A3B8) where appropriate
- [ ] Selection color matches landing site (`::selection` green on dark)

## Safety

- [ ] No backend code changes required — all endpoints reused as-is
- [ ] JWT tokens stored in `localStorage` only (no cookies, no sessionStorage)
- [ ] API credentials passed straight to backend — never stored in web app state beyond form inputs
- [ ] Form credential fields cleared on successful submission
- [ ] No sensitive data in URL parameters or browser history

## Testing

- [ ] `bun run build` passes with zero TypeScript errors
- [ ] `bun run lint` passes
- [ ] Manual test: full auth flow (register → login → add exchange → test → delete → logout)
- [ ] Manual test: token expiry triggers refresh and retries request
- [ ] Manual test: extension "MANAGE ACCOUNTS" link opens correct URL
- [ ] Landing page visual regression: all sections render identically to pre-change
