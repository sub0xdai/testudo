# 013: Web Account Management

| Field    | Value                                    |
|----------|------------------------------------------|
| Status   | Complete                                 |
| Date     | 2026-02-26                               |
| Depends  | 012-ccxt-multi-exchange, EXT-15          |
| Phase    | Web — Account Management                 |

## 1. Overview

### Current State
- Landing site (`testudo-web/`) is a single-page marketing site with no routing, no auth, and no user-facing account functionality
- Exchange account CRUD lives exclusively in the browser extension (`ExchangeManager.tsx`, Solid.js)
- Backend auth (`/api/v1/auth/*`) and exchange (`/api/v1/exchanges/*`) endpoints are fully implemented and battle-tested via the extension
- Users must install the extension just to manage exchange credentials — no stable web interface exists

### Target State
- React Router added to the landing app: marketing page at `/`, auth pages at `/login` and `/register`, protected account page at `/account`
- Auth context provider handles JWT lifecycle (login, register, token storage, refresh, logout, protected routes)
- `/account` page provides the same exchange CRUD as the extension's ExchangeManager — list connected exchanges, add new accounts, test connections, delete accounts — but on a stable web page
- Extension's ExchangeManager component removed from popup settings, replaced with a link/button that opens the web `/account` page
- Shared JWT backend means a user authenticated on either surface (web or extension) is authenticated on the same backend
- Zero backend changes required — all endpoints already exist

## 2. User Stories

1. As a new user, I want to register an account on the website so I can manage my exchange connections without installing the extension
2. As a returning user, I want to log in on the website and manage my exchange API credentials on a full-size page
3. As a trader, I want to add exchange credentials (API key, secret, optional passphrase) via the web and have them validated immediately
4. As a trader, I want to test my exchange connection and see latency from the web account page
5. As a trader, I want to delete an exchange account from the web interface
6. As an extension user, I want a direct link from extension settings to the web account page so I can manage credentials on a proper page
7. As an unauthenticated visitor, I should be redirected to `/login` when trying to access `/account`

## 3. Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Routing library | React Router v6 (react-router-dom) | Standard for React SPAs; landing already uses React 18. Minimal dependency footprint. |
| Token storage | `localStorage` | Same approach as the extension's `browser.storage.local`. Simple, works for single-tab JWT. `httpOnly` cookies would require backend changes (out of scope). |
| Auth state management | React Context + useReducer | No external state library needed. Matches the app's zero-dependency philosophy. Context provides `user`, `login()`, `register()`, `logout()`, `isAuthenticated`. |
| API client | Axios with interceptors | Lightweight HTTP client. Bearer token interceptor + 401 refresh-and-retry pattern. |
| Account page design | React port of extension ExchangeManager | Same CRUD operations, same API endpoints. Adapted from Solid.js signals to React hooks. Full-width layout instead of constrained popup. |
| Extension simplification | Replace ExchangeManager with link | Extension should focus on trading. Account management is a settings task better suited to a full page. Keeps popup lean. |
| Typography | Match landing site: Unbounded (display) + Space Mono (body) | Maintains visual consistency with the existing landing page. Steel monochrome accent (`#94A3B8`) from extension for interactive elements. |
| Auth page style | Centered card on dark background | Consistent with landing site's `#050505` background, `#0A0A0A` panel containers, zero border-radius brutalist aesthetic. |
| Backend changes | None | All auth and exchange endpoints are implemented and tested. JWT middleware, CRUD, credential validation via CCXT sidecar — all working. |

## 4. Functional Requirements

### FR-1: React Router Setup

Modify `testudo-web/src/main.tsx` and `App.tsx`.

- **FR-1.1:** Add `react-router-dom` dependency to `testudo-web/package.json`.
- **FR-1.2:** Wrap app with `BrowserRouter` in `main.tsx`.
- **FR-1.3:** Define routes in `App.tsx`:
  - `/` — Existing landing page (all current sections)
  - `/login` — Login page
  - `/register` — Registration page
  - `/account` — Protected account management page
  - `/journal` — Placeholder page (future trading journal, not implemented in this spec)
- **FR-1.4:** Landing page route (`/`) renders all existing sections (Hero, Problem, Solution, etc.) unchanged. SpotlightBackground only renders on the landing page.
- **FR-1.5:** Header component updated:
  - Remove "BAGS.FM" external link
  - Replace "JOIN WAITLIST" button with auth-aware links:
    - Unauthenticated: "LOGIN" link → `/login`
    - Authenticated: "ACCOUNT" link → `/account`
  - Add "JOURNAL" nav link → `/journal` (disabled/dim styling with "COMING SOON" tooltip, visible to all users)
  - Preserve: PRICING, FAQ, GitHub, X links

### FR-2: Auth Context Provider

New file: `src/context/AuthContext.tsx`.

- **FR-2.1:** `AuthProvider` wraps the entire app inside `BrowserRouter`.
- **FR-2.2:** Context exposes:
  - `user: { id: string; email: string } | null`
  - `isAuthenticated: boolean`
  - `loading: boolean` (true during initial token validation)
  - `login(email: string, password: string): Promise<void>`
  - `register(email: string, password: string): Promise<void>`
  - `logout(): void`
- **FR-2.3:** On mount, check `localStorage` for `access_token`. If present, decode JWT to extract user info (no server validation needed — expired tokens will fail on first API call and trigger refresh).
- **FR-2.4:** `login()` calls `POST /api/v1/auth/login`, stores `access_token` and `refresh_token` in `localStorage`, sets user state.
- **FR-2.5:** `register()` calls `POST /api/v1/auth/register`, stores tokens (backend returns tokens on registration), sets user state.
- **FR-2.6:** `logout()` calls `POST /api/v1/auth/logout` (best-effort), clears `localStorage` tokens, resets user state to null.
- **FR-2.7:** `ProtectedRoute` component: wraps children, redirects to `/login` via `<Navigate>` if not authenticated and not loading.

### FR-3: API Client

New file: `src/api/client.ts`.

- **FR-3.1:** Axios instance with `baseURL` from environment variable `VITE_API_URL` (default: `http://localhost:8080/api/v1`).
- **FR-3.2:** Request interceptor: attach `Authorization: Bearer <access_token>` from `localStorage` on every request.
- **FR-3.3:** Response interceptor for 401 errors:
  1. If `refresh_token` exists in `localStorage`, call `POST /api/v1/auth/refresh`.
  2. On success: store new tokens, retry original request with new access token.
  3. On failure: clear tokens from `localStorage`, redirect to `/login`.
  4. Prevent infinite refresh loops (flag to track in-flight refresh).
- **FR-3.4:** Export typed API functions:
  - `authApi.login(email, password): Promise<LoginResponse>`
  - `authApi.register(email, password): Promise<LoginResponse>`
  - `authApi.refresh(refreshToken): Promise<TokenResponse>`
  - `authApi.logout(refreshToken): Promise<void>`
  - `exchangeApi.listExchanges(): Promise<ExchangeInfo[]>`
  - `exchangeApi.listAccounts(): Promise<ExchangeAccount[]>`
  - `exchangeApi.addAccount(payload): Promise<ExchangeAccount>`
  - `exchangeApi.deleteAccount(id): Promise<void>`
  - `exchangeApi.testConnection(id): Promise<TestConnectionResult>`

### FR-4: Auth Pages

New files: `src/pages/LoginPage.tsx`, `src/pages/RegisterPage.tsx`.

- **FR-4.1:** Both pages share the same layout: centered card (`max-w-md`, `bg-container-bg`, `border border-container-border`) on the dark `#050505` background.
- **FR-4.2:** Login form: email input, password input, submit button ("SIGN IN"), error display, link to `/register` ("Don't have an account? Register").
- **FR-4.3:** Register form: email input, password input, confirm password input, submit button ("CREATE ACCOUNT"), error display, link to `/login` ("Already have an account? Sign in").
- **FR-4.4:** Client-side validation:
  - Email: basic format check
  - Password: minimum 8 characters
  - Confirm password: must match password
- **FR-4.5:** On successful login/register, redirect to `/account` via `useNavigate()`.
- **FR-4.6:** If already authenticated, redirect from login/register pages to `/account`.
- **FR-4.7:** Styling: Space Mono body text, no border-radius (brutalist), `signal-green` for primary action buttons, `signal-red` for error text.

### FR-5: Account Page

New file: `src/pages/AccountPage.tsx`.

- **FR-5.1:** Protected route — requires authentication (via `ProtectedRoute` wrapper).
- **FR-5.2:** Page header: user email display, logout button.
- **FR-5.3:** Exchange accounts section — React port of extension's ExchangeManager:
  - List connected exchanges as cards with status indicators
  - "ADD EXCHANGE" button toggles the add form
  - Add form: exchange dropdown (from `GET /exchanges`), API key, secret, optional passphrase (shown for exchanges that need it: `okx`, `kucoin`)
  - "ADD EXCHANGE" submit button validates credentials via backend (which calls CCXT sidecar)
  - Test connection button per account — shows latency on success, error message on failure
  - Delete button with inline confirmation (same DEL → CONFIRM/NO pattern as extension)
- **FR-5.4:** Filter exchange dropdown to exclude already-connected exchanges (one account per exchange constraint).
- **FR-5.5:** Layout: full-width content area with `max-w-2xl` centered container. Cards use `bg-container-bg`, `border-container-border`.
- **FR-5.6:** Loading states: skeleton/spinner while fetching exchanges and accounts.
- **FR-5.7:** Error handling: display API errors inline (connection failures, invalid credentials, rate limiting).

### FR-6: Extension Simplification

Modify extension popup components.

- **FR-6.1:** Remove `ExchangeManager` component import and rendering from `SettingsView.tsx`.
- **FR-6.2:** Add "MANAGE ACCOUNTS" button/link in SettingsView that opens `${backendUrl.replace('/api/v1', '')}/account` (or a configurable web URL) in a new browser tab.
- **FR-6.3:** Keep all background message handlers (`LIST_EXCHANGES`, `LIST_EXCHANGE_ACCOUNTS`, etc.) — the extension still reads active exchange for trade execution.
- **FR-6.4:** `ExchangeManager.tsx` file can remain for now (not deleted) in case of rollback — just unused.

### FR-7: TypeScript Types

New file: `src/types/index.ts`.

- **FR-7.1:** Port relevant types from extension's `types.ts`:
  - `AuthTokens { access_token, refresh_token, expires_in }`
  - `LoginResponse { user: { id, email }, tokens: AuthTokens }`
  - `ExchangeInfo { id, name, type, description, supported_features, required_credentials, optional_credentials }`
  - `ExchangeAccount { id, exchange_name, account_name, is_active, permissions, created_at, last_used_at }`
  - `AddExchangeAccountPayload { exchange_name, account_name?, api_key, secret, passphrase? }`
  - `TestConnectionResult { account_id, exchange_name, status, message, tested_at, latency_ms }`

### FR-8: Landing Page CTA Conversion

Convert dead-end waitlist CTAs into product registration funnel.

- **FR-8.1:** `Hero.tsx` — Replace "JOIN WAITLIST" `<a href="#waitlist">` with `<Link to="/register">` labeled "GET STARTED". Keep "VIEW DOCS" GitHub link unchanged.
- **FR-8.2:** `FinalCTA.tsx` — Remove Formspree waitlist form entirely. Replace with a direct call-to-action: heading "READY TO TRADE?", body text, and a "CREATE ACCOUNT" button linking to `/register`. Remove `useState` for form status and Formspree fetch logic.
- **FR-8.3:** `Pricing.tsx` — Both tier CTA buttons ("GET STARTED" and "JOIN WAITLIST") change from `<a href="#waitlist">` to `<Link to="/register">`. Relabel both to "GET STARTED".
- **FR-8.4:** `Footer.tsx` — Update copyright year from 2024 to 2025.
- **FR-8.5:** `Exchanges.tsx` — No changes (informational section, no CTA).
- **FR-8.6:** `/journal` placeholder page — minimal page with "JOURNAL" heading, "Coming soon — trade history, P&L analytics, and performance insights" body text, and a back link to `/`. Styled consistently with auth pages (centered card on dark background).

## 5. File Context

### New Files

| File | Purpose |
|------|---------|
| `testudo-web/src/context/AuthContext.tsx` | Auth provider with JWT lifecycle |
| `testudo-web/src/api/client.ts` | Axios client with interceptors |
| `testudo-web/src/pages/LoginPage.tsx` | Login form page |
| `testudo-web/src/pages/RegisterPage.tsx` | Registration form page |
| `testudo-web/src/pages/AccountPage.tsx` | Protected exchange account management |
| `testudo-web/src/pages/LandingPage.tsx` | Extracted landing sections (moved from App.tsx) |
| `testudo-web/src/pages/JournalPage.tsx` | Placeholder page for future trading journal |
| `testudo-web/src/types/index.ts` | Shared TypeScript interfaces |

### Modified Files

| File | Changes |
|------|---------|
| `testudo-web/package.json` | Add `react-router-dom`, `axios` dependencies |
| `testudo-web/src/main.tsx` | Wrap with `BrowserRouter` |
| `testudo-web/src/App.tsx` | Replace single-page layout with `Routes` definition |
| `testudo-web/src/components/ui/Header.tsx` | Remove BAGS.FM, replace JOIN WAITLIST with LOGIN/ACCOUNT, add JOURNAL link |
| `testudo-web/src/components/sections/Hero.tsx` | Replace "JOIN WAITLIST" with "GET STARTED" → `/register` |
| `testudo-web/src/components/sections/FinalCTA.tsx` | Remove Formspree form, replace with "CREATE ACCOUNT" → `/register` |
| `testudo-web/src/components/sections/Pricing.tsx` | Both CTA buttons → `/register` |
| `testudo-web/src/components/sections/Footer.tsx` | Update copyright year |
| `testudo-web/vite.config.ts` | Add SPA fallback for client-side routing (if needed) |
| `testudo-extension/src/popup/components/SettingsView.tsx` | Remove ExchangeManager, add "MANAGE ACCOUNTS" link |

### Unchanged (Backend Complete)

| File | Status |
|------|--------|
| `testudo-exchange/crates/router/src/routes/user.rs` | Auth endpoints unchanged |
| `testudo-exchange/crates/router/src/routes/exchanges.rs` | Exchange CRUD unchanged |
| `testudo-exchange/crates/router/src/middleware/auth.rs` | JWT middleware unchanged |
| `testudo-ccxt/src/server.js` | CCXT sidecar unchanged |

## 6. Acceptance Criteria

1. `bun run build` in `testudo-web/` succeeds with zero TypeScript errors
2. `/` route renders the full landing page with all sections, SpotlightBackground, and updated header
3. `/login` renders login form; submitting valid credentials stores JWT and redirects to `/account`
4. `/register` renders register form; confirm password mismatch shows client-side error; valid submission creates account and redirects to `/account`
5. `/account` without auth redirects to `/login`
6. `/account` with auth shows exchange account list from `GET /api/v1/exchanges/accounts`
7. Adding an exchange account via the web form calls `POST /api/v1/exchanges/accounts` and shows the new account in the list
8. Testing an exchange connection shows latency in ms on success
9. Deleting an exchange account with inline confirmation removes it from the list
10. Logout clears tokens and redirects to `/login`
11. 401 on any API call triggers token refresh; if refresh fails, redirects to `/login`
12. Extension SettingsView shows "MANAGE ACCOUNTS" link that opens the web `/account` page in a new tab
13. Extension still reads active exchange from backend for trade execution (no regression)
14. Header shows "ACCOUNT" when authenticated, "LOGIN" when not; no BAGS.FM link; JOURNAL link present with "coming soon" indicator
15. All pages maintain the brutalist dark theme: `#050505` background, zero border-radius, Space Mono typography
16. Hero "GET STARTED" button navigates to `/register` (no `#waitlist` anchor references remain)
17. FinalCTA section has no Formspree integration — "CREATE ACCOUNT" button links to `/register`
18. Pricing tier CTAs both link to `/register`
19. `/journal` renders placeholder page with coming soon message
20. `grep -rn "formspree\|#waitlist\|bags.fm" src/` returns zero matches

## 7. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| CORS: landing site and backend on different origins | High | Backend must allow the landing site origin in CORS config. Verify `Access-Control-Allow-Origin` includes the landing domain. May need a backend env var addition (minor change, not code). |
| Token in localStorage — XSS exposure | Medium | Landing site has no user-generated content or third-party scripts. CSP headers from backend middleware provide additional protection. Same risk as extension's `browser.storage.local`. |
| Vite dev server proxying to backend | Low | Vite proxy config in `vite.config.ts` for `/api` prefix during development. Production uses same-origin or configured CORS. |
| Extension users confused by removal of inline ExchangeManager | Low | Clear "MANAGE ACCOUNTS" link with visual prominence. Tooltip or small label explaining accounts are now managed on web. |
| React Router SPA routing on static hosting | Low | Configure hosting (Railway/serve) with SPA fallback — all routes serve `index.html`. Vite build already handles this with `serve -s`. |

## 8. Implementation Order

1. **FR-1 + FR-7** (Router + types) — structural foundation, no visual changes yet
2. **FR-3** (API client) — enables all data fetching
3. **FR-2** (Auth context) — enables protected routes
4. **FR-4** (Auth pages) — login/register flows
5. **FR-8** (Landing page CTA conversion) — rewire all CTAs to registration funnel, add journal placeholder
6. **FR-5** (Account page) — core feature
7. **FR-6** (Extension simplification) — cleanup, done last

## 9. Completion Signal

```
feat: 013 web account management — auth pages, exchange CRUD on landing site
```
