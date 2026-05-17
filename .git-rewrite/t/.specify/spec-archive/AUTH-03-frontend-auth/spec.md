# Specification: Frontend Auth — Wallet Connect Login, Cookie-Based Sessions, Extension Pairing

**Spec ID:** AUTH-03-frontend-auth
**Date:** 2026-03-24
**Status:** Draft
**Class:** Feature / Auth
**Priority:** P0 — Frontend must adapt to wallet-primary SIWE backend; current email/password UI and localStorage tokens must be replaced
**Depends on:** AUTH-02-backend-auth (requires SIWE, cookie, and pairing endpoints)
**Series:** AUTH-01 through AUTH-03 (authentication architecture hardening)

---

## Problem Statement

The three frontend clients currently implement email/password authentication with tokens in JavaScript-accessible storage:

- **testudo-web**: `AuthContext.tsx` stores tokens in localStorage, `LoginPage.tsx` and `RegisterPage.tsx` present email/password forms, `api/client.ts` injects `Authorization: Bearer` headers via Axios interceptor.
- **testudo-journal**: `api/client.ts` reads `localStorage.getItem("access_token")` and manually constructs `Authorization` headers on every fetch call.
- **testudo-extension**: `token-sync.ts` (90 lines) monkey-patches `localStorage.setItem` to intercept token changes from the web app and bridge them to `chrome.storage.local`. The background worker manages tokens via `background/auth.ts`.

With the wallet-primary decision, ALL of this changes:

1. **Web app**: The only login is "Connect Wallet" via RainbowKit (already installed: `@rainbow-me/rainbowkit:^2.2.10`, `wagmi:^2.19.3`, `viem:^2.38.0`). After wallet connect, the app requests a SIWE signature and POSTs it to `/api/v1/auth/verify-siwe`. The browser receives HttpOnly cookies — no tokens in JavaScript, no localStorage. `LoginPage.tsx`, `RegisterPage.tsx`, and the email/password form are deleted.

2. **Journal**: Switches from manual `Authorization` headers to `credentials: "include"` on all fetch calls. Deletes `getToken()`, `refreshAccessToken()`, and the refresh promise mutex. The cookie handles everything.

3. **Extension**: Cannot access `window.ethereum` from its popup context (extensions run in isolated worlds). Uses a device-pairing flow: user clicks "Connect Extension" in the web app settings → gets a 6-digit code → enters it in the extension popup → extension exchanges code for JSON tokens via `/api/v1/auth/extension-pair` → stores in `chrome.storage.session`. Token-sync is deleted entirely.

---

## User Stories

- **As a trader**, I want to click "Connect Wallet" and sign one message to log in, so that I don't need to manage email/password credentials.
- **As a journal user**, I want the journal to share my web app session seamlessly, so that I never see a separate login screen.
- **As an extension user**, I want to pair my extension to my web session by entering a short code, so that I can authenticate without needing wallet signing in the extension context.
- **As a returning user**, I want `GET /api/v1/auth/me` to restore my session on page reload, so that I don't have to reconnect my wallet every visit.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Replace LoginPage with a single "Connect Wallet" screen using RainbowKit's `ConnectButton` | High | testudo-web |
| FR-2 | After wallet connect, construct EIP-4361 message (fetch nonce from `/auth/nonce`), request `personal_sign` via wagmi, POST to `/auth/verify-siwe` | High | testudo-web |
| FR-3 | Delete `LoginPage.tsx`, `RegisterPage.tsx`, and all email/password form code | High | testudo-web |
| FR-4 | Remove all `localStorage.getItem/setItem` for tokens from `AuthContext.tsx` | High | testudo-web |
| FR-5 | Configure Axios with `withCredentials: true` — cookies sent automatically | High | testudo-web |
| FR-6 | Remove manual `Authorization: Bearer` header injection from Axios request interceptor | High | testudo-web |
| FR-7 | Auth state on mount: call `GET /api/v1/auth/me` — if 200, user is authenticated; if 401, show connect screen | High | testudo-web |
| FR-8 | 401 interceptor: call `POST /api/v1/auth/refresh` with `withCredentials: true` (cookie, no body), retry original request | High | testudo-web |
| FR-9 | Add "Connect Extension" button in web app Settings page — calls `POST /api/v1/auth/pair-extension`, displays the returned 6-digit code with 5-minute countdown | High | testudo-web |
| FR-10 | Journal: replace all fetch calls with `credentials: "include"` | High | testudo-journal |
| FR-11 | Journal: remove `getToken()`, `refreshAccessToken()`, `refreshPromise`, manual `Authorization` headers | High | testudo-journal |
| FR-12 | Journal: 401 handling via `fetch("/api/v1/auth/refresh", { method: "POST", credentials: "include" })` then retry | High | testudo-journal |
| FR-13 | Delete `testudo-extension/src/token-sync.ts` entirely | High | testudo-extension |
| FR-14 | Remove `token-sync.js` content script entry from `manifest.json` | High | testudo-extension |
| FR-15 | Extension popup: add "Pair" screen where user enters 6-digit code | High | testudo-extension |
| FR-16 | Extension: POST code to `/api/v1/auth/extension-pair`, store returned tokens in `chrome.storage.session` | High | testudo-extension |
| FR-17 | Extension: `getTokens()` reads from `chrome.storage.session` (not `chrome.storage.local`) | High | testudo-extension |
| FR-18 | Extension: refresh calls `POST /api/v1/auth/extension-refresh` with JSON body | High | testudo-extension |
| FR-19 | Extension: on browser close, tokens are automatically cleared (`chrome.storage.session` behavior) — user re-pairs next session | Medium | testudo-extension |
| FR-20 | Remove `handleLogin`/`handleRegister` message handlers from extension background — replace with `handlePair` | High | testudo-extension |

---

## Technical Implementation

### 1. Web App — Login Screen

```tsx
// testudo-web/src/pages/LoginPage.tsx → rewritten
// BEFORE: email + password form
// AFTER: single wallet connect screen

import { ConnectButton } from "@rainbow-me/rainbowkit";
import { useAccount, useSignMessage } from "wagmi";

export function LoginPage() {
  const { address, isConnected } = useAccount();
  const { signMessageAsync } = useSignMessage();
  const { login } = useAuth();

  useEffect(() => {
    if (isConnected && address) {
      handleSiweLogin(address);
    }
  }, [isConnected, address]);

  const handleSiweLogin = async (addr: string) => {
    // 1. Fetch nonce
    const { nonce } = await apiClient.get("/auth/nonce").then(r => r.data);

    // 2. Construct EIP-4361 message
    const message = [
      `${window.location.host} wants you to sign in with your Ethereum account:`,
      addr, "", "Sign in to Testudo", "",
      `URI: ${window.location.origin}`,
      `Version: 1`, `Chain ID: 42161`,
      `Nonce: ${nonce}`,
      `Issued At: ${new Date().toISOString()}`,
    ].join("\n");

    // 3. Request signature
    const signature = await signMessageAsync({ message });

    // 4. Verify — cookie set by browser from Set-Cookie response
    const res = await apiClient.post("/auth/verify-siwe", { message, signature });
    login(res.data.user);
  };

  return (
    <div>
      <h1>Connect Wallet to Enter</h1>
      <ConnectButton />
    </div>
  );
}
```

### 2. Web App — AuthContext (Rewritten)

```tsx
// testudo-web/src/context/AuthContext.tsx
// NO localStorage. NO JWT decode. Cookie-based.

interface User { id: string; walletAddress: string }
interface AuthState { user: User | null; loading: boolean }

function AuthProvider({ children }) {
  const [state, setState] = useState<AuthState>({ user: null, loading: true });

  useEffect(() => {
    // On mount: check existing session via cookie
    apiClient.get("/auth/me")
      .then(res => setState({ user: res.data.user, loading: false }))
      .catch(() => setState({ user: null, loading: false }));
  }, []);

  const login = (user: User) => setState({ user, loading: false });

  const logout = async () => {
    await apiClient.post("/auth/logout"); // clears cookies server-side
    setState({ user: null, loading: false });
  };
}
```

### 3. Web App — Axios Client (Simplified)

```typescript
// testudo-web/src/api/client.ts
const apiClient = axios.create({
  baseURL: API_URL,
  withCredentials: true, // cookies on every request
});

// DELETED: request interceptor injecting Authorization header
// SIMPLIFIED: 401 interceptor
apiClient.interceptors.response.use(
  res => res,
  async error => {
    if (error.response?.status === 401 && !error.config._retry) {
      error.config._retry = true;
      try {
        await axios.post(`${API_URL}/auth/refresh`, {}, { withCredentials: true });
        return apiClient(error.config);
      } catch {
        window.location.href = "/login";
        return Promise.reject(error);
      }
    }
    return Promise.reject(error);
  }
);
```

### 4. Web App — Pairing Code Display

```tsx
// testudo-web/src/components/ExtensionPairing.tsx — new
export function ExtensionPairing() {
  const [code, setCode] = useState<string | null>(null);
  const [countdown, setCountdown] = useState(120);

  const generateCode = async () => {
    const res = await apiClient.post("/auth/pair-extension");
    setCode(res.data.code);
    setCountdown(300);
    // Start 5-minute countdown timer
  };

  return (
    <div>
      <button onClick={generateCode}>Pair Extension</button>
      {code && <div className="text-4xl font-mono tracking-widest">{code}</div>}
      {code && <p>Expires in {Math.floor(countdown / 60)}:{String(countdown % 60).padStart(2, '0')} — enter this code in the extension popup</p>}
    </div>
  );
}
```

### 5. Journal — Cookie-Based Fetch

```typescript
// testudo-journal/src/api/client.ts
// DELETED: getToken(), refreshAccessToken(), refreshPromise, manual Authorization header
// ALL fetch calls gain: credentials: "include"

async function fetchApi<T>(path: string, filters?: Filters): Promise<T> {
  const url = buildUrl(path, filters);
  const res = await fetch(url, { credentials: "include" });

  if (res.status === 401) {
    const refreshRes = await fetch(`${API_URL}/auth/refresh`, {
      method: "POST",
      credentials: "include",
    });
    if (!refreshRes.ok) throw new Error("Session expired");
    const retryRes = await fetch(url, { credentials: "include" });
    if (!retryRes.ok) throw new Error(`API error: ${retryRes.status}`);
    return retryRes.json();
  }

  if (!res.ok) throw new Error(`API error: ${res.status}`);
  return res.json();
}
```

### 6. Extension — Pairing Flow

```typescript
// testudo-extension/src/popup/components/PairView.tsx — new (replaces login form)
function PairView() {
  const [code, setCode] = createSignal("");
  const [error, setError] = createSignal("");

  const handlePair = async () => {
    const settings = await getSettings();
    const res = await fetch(`${settings.backendUrl}/api/v1/auth/extension-pair`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ code: code() }),
    });
    if (!res.ok) { setError("Invalid or expired code"); return; }
    const data = await res.json();
    await storeTokens(data.tokens); // chrome.storage.session
    scheduleTokenRefresh(data.tokens.expires_in);
  };

  return (
    <div>
      <label>Enter pairing code from web app:</label>
      <input value={code()} onInput={e => setCode(e.target.value)} maxLength={6} inputMode="numeric" pattern="[0-9]*" />
      <button onClick={handlePair}>Pair</button>
      {error() && <p class="text-red-500">{error()}</p>}
    </div>
  );
}
```

### 7. Extension — Storage Migration

```typescript
// testudo-extension/src/background/auth.ts
// BEFORE: chrome.storage.local
// AFTER: chrome.storage.session

export async function getTokens(): Promise<StoredTokens | null> {
  const data = await chrome.storage.session.get(["accessToken", "refreshToken", "tokenExpiry"]);
  return StoredTokensSchema.safeParse(data).success ? data : null;
}

export async function storeTokens(tokens: AuthTokens): Promise<void> {
  await chrome.storage.session.set({
    accessToken: tokens.access_token,
    refreshToken: tokens.refresh_token,
    tokenExpiry: Math.floor(Date.now() / 1000) + tokens.expires_in,
  });
}

export async function clearTokens(): Promise<void> {
  await chrome.storage.session.remove(["accessToken", "refreshToken", "tokenExpiry"]);
}
```

### 8. Deleted Code Inventory

| File | What's Removed | Why |
|------|----------------|-----|
| `testudo-web/src/pages/LoginPage.tsx` | Email/password form | Replaced by wallet connect |
| `testudo-web/src/pages/RegisterPage.tsx` | Entire file deleted | No registration — wallet creates account on first SIWE |
| `testudo-web/src/context/AuthContext.tsx` | localStorage token ops, `decodeJwtPayload()` | Cookies, `/auth/me` |
| `testudo-web/src/api/client.ts` | Request interceptor (Bearer header), `failedQueue` | `withCredentials: true` |
| `testudo-journal/src/api/client.ts` | `getToken()`, `refreshAccessToken()`, `refreshPromise`, manual headers | `credentials: "include"` |
| `testudo-extension/src/token-sync.ts` | Entire file deleted | No localStorage tokens to sync |
| `testudo-extension/manifest.json` | `token-sync.js` content script entry | File deleted |
| `testudo-extension/src/background/handlers.ts` | `handleLogin`, `handleRegister` | Replaced by `handlePair` |
| `testudo-extension/src/popup/components/` | Login/register form components | Replaced by PairView |

### Files

- `testudo-web/src/pages/LoginPage.tsx` — **rewritten** — wallet connect + SIWE
- `testudo-web/src/pages/RegisterPage.tsx` — **deleted**
- `testudo-web/src/context/AuthContext.tsx` — **rewritten** — cookie-based, `/auth/me`
- `testudo-web/src/api/client.ts` — **modified** — `withCredentials`, simplified interceptor
- `testudo-web/src/components/ExtensionPairing.tsx` — **new** — pairing code UI
- `testudo-journal/src/api/client.ts` — **modified** — `credentials: "include"`, remove token handling
- `testudo-extension/src/token-sync.ts` — **deleted**
- `testudo-extension/manifest.json` — **modified** — remove token-sync entry
- `testudo-extension/src/background/auth.ts` — **modified** — `chrome.storage.session`, extension endpoints
- `testudo-extension/src/background/handlers.ts` — **modified** — replace login/register with pair
- `testudo-extension/src/popup/components/PairView.tsx` — **new** — pairing code input (Solid.js)

### Dependencies Added

- None (RainbowKit, wagmi, viem already in testudo-web)

---

## Acceptance Criteria

- [ ] Web app shows "Connect Wallet" — no email/password fields
- [ ] Connecting wallet + signing SIWE message → authenticated (cookie set, user state loaded)
- [ ] `RegisterPage.tsx` does not exist
- [ ] No tokens in localStorage — verify DevTools Application → Local Storage
- [ ] HttpOnly cookies visible in DevTools Application → Cookies
- [ ] Page reload: `GET /auth/me` restores session from cookie
- [ ] Logout clears cookies and shows connect screen
- [ ] "Connect Extension" button in account page generates 6-digit code
- [ ] Extension popup shows pairing code input (no email/password form)
- [ ] Entering valid code in extension → authenticated → main view
- [ ] Extension tokens stored in `chrome.storage.session` (not `local`)
- [ ] Closing browser clears extension tokens (session storage behavior)
- [ ] Extension refresh calls `/auth/extension-refresh` with JSON body
- [ ] `token-sync.ts` does not exist in source tree
- [ ] `token-sync.js` not in manifest.json
- [ ] Journal loads authenticated data via cookie (`credentials: "include"`)
- [ ] Journal 401 refresh works via cookie round-trip
- [ ] Existing WalletConnect.tsx (agent wallet EIP-712 flow) still works
- [ ] `cd testudo-web && bun run build` passes
- [ ] `cd testudo-extension && bun run build` passes

---

## Risks

1. **`chrome.storage.session` clears on browser close** — Extension users must re-pair each browser session. Mitigation: This is a security feature, not a bug. Pairing takes 10 seconds. For users who find this burdensome, a future spec could add `chrome.storage.local` as an opt-in "remember this device" mode.
2. **Same-origin cookies** — Web app, journal, and API must share origin for `SameSite=Strict`. Mitigation: Dev uses Vite proxy; prod uses same domain with path routing. If cross-origin needed later, downgrade to `SameSite=Lax`.
3. **RainbowKit connect ≠ SIWE sign** — Connecting a wallet via RainbowKit is separate from signing the SIWE message. The user connects, THEN the app requests signature. Mitigation: Auto-trigger SIWE sign after wallet connect via `useEffect` watching `isConnected`. If user rejects signature, show "Sign message to continue" prompt.
4. **Extension login UX shift** — Currently users log in on web and tokens auto-sync. Now they must pair explicitly. Mitigation: The pairing flow is familiar (similar to TV streaming app device pairing). The extension popup guides users: "Log in at testudo.xyz, then click Pair Extension."

---

## Completion Signal

This spec is complete when:
1. Web app login is wallet-only via RainbowKit + SIWE (no email/password)
2. Journal uses cookie-based auth with zero JavaScript token access
3. Extension uses device-pairing + `chrome.storage.session`
4. `token-sync.ts` is deleted
5. `RegisterPage.tsx` is deleted
6. All three frontend builds pass
7. All acceptance criteria verified
8. Code committed to master
