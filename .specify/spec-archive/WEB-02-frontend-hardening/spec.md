# Specification: Web Frontend Hardening — Bundle Size, Auth Guard, Header Stability

**Spec ID:** WEB-02-frontend-hardening
**Date:** 2026-03-25
**Status:** Draft
**Class:** Refactor / Performance + Security + Stability
**Priority:** P1 — Landing page loads 2MB+ of wallet infrastructure eagerly (affecting conversion), ProtectedRoute allows unauthenticated access to /account (UX bug), Header wallet component has no regression protection after 6 sequential bug fixes.
**Depends on:** None
**Series:** WEB-02 (testudo-web frontend improvements)

---

## Problem Statement

Three related issues in testudo-web degrade user experience and developer confidence.

**Bundle size (988KB main chunk).** Every page — including the public landing page and about page — eagerly loads the entire Web3 stack: wagmi, RainbowKit, viem, MetaMask SDK, and WalletConnect. The provider stack in `main.tsx` wraps the entire app tree: `ThemeProvider → WagmiProvider → QueryClientProvider → RainbowKitProvider → BrowserRouter → AuthProvider → App`. The `Header.tsx` component imports `ConnectButton` from `@rainbow-me/rainbowkit` (line 3) and renders it on every page (line 110), forcing tree-shaking to include the full wallet infrastructure. Zero dynamic `import()` calls exist in the codebase. The result: 988KB (index), 556KB (metamask-sdk), 497KB (core), 416KB (wagmi), 312KB (rainbowkit), 143KB (walletconnect) — all loaded before the user sees any content. For a landing page whose job is conversion, this is a ~610KB gzip penalty on first paint.

**ProtectedRoute guard relaxed.** The `ProtectedRoute` component in `AuthContext.tsx` (line 111) uses OR logic: `if (!isAuthenticated && !isConnected) return <Navigate to="/" replace />`. This means a wallet-connected but unauthenticated user passes through to `/account`. The backend is secure — all `/exchanges` endpoints enforce JWT via `JwtMiddleware` at `main.rs:884` — but the frontend UX is broken. The user reaches AccountPage, sees "LOADING...", then gets "Failed to load exchange data" error banners when every API call returns 401. The guard was relaxed to solve a chicken-and-egg problem: the user needs to reach `/account` to complete SIWE, but `/account` requires authentication. The correct fix is auto-triggering SIWE when a connected-but-unauthed user hits a protected route, not lowering the guard.

**Header wallet component fragility.** The Mar 24 evening session produced 6 sequential bug fixes for `Header.tsx`: button flashing on auto-connect, undefined `openConnectModal`, stale loading state, wagmi auto-reconnect race condition, disconnect not clearing both auth states, and AccountChip crash when `user` is null but `isConnected` is true. `ConnectButton.Custom` was tried twice and reverted both times. The final implementation uses standard `ConnectButton` with CSS overrides, which delegates lifecycle to RainbowKit. The remaining fragility is the wagmi auto-reconnect race: page load restores a stale MetaMask session (`isConnected=true`) before AuthContext runs SIWE (`user` is still null), causing AccountChip to render with fallback address. No regression tests exist to catch future breakage.

---

## User Stories

- **As a visitor**, I want the landing page to load quickly, so that I can evaluate Testudo without waiting for wallet infrastructure I haven't asked for.
- **As a user who just connected my wallet**, I want to seamlessly complete SIWE authentication, so that I don't see error banners on the account page.
- **As a developer**, I want the Header wallet component to have regression tests, so that auth UI changes don't require a 6-fix debug session.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Lazy-load RainbowKit `ConnectButton` in Header via `React.lazy()` + `Suspense`. Fallback renders a static "CONNECT" text styled identically to the loaded button. | High | Header |
| FR-2 | Route-split `AccountPage` via React Router `lazy()`. The `/account` route loads its component and dependencies on navigation, not on initial page load. | High | App routing |
| FR-3 | Move `WagmiProvider`, `QueryClientProvider`, and `RainbowKitProvider` out of the global app tree. Wrap only routes that need wallet functionality (currently only `/account` and the Header ConnectButton). Create a `WalletProviders` wrapper component. | High | main.tsx |
| FR-4 | Public pages (landing `/`, about `/about`) must NOT load wagmi, RainbowKit, viem, or MetaMask SDK in their initial JavaScript bundles. Verify via build output analysis. | High | Build |
| FR-5 | When a user reaches a ProtectedRoute with `isConnected=true` but `isAuthenticated=false`, auto-trigger the SIWE flow instead of showing the page with 401 errors. Show a signing prompt overlay: "Sign to verify your wallet" with a spinner. | High | AuthContext |
| FR-6 | If SIWE auto-trigger fails (user rejects signature, nonce error), redirect to `/` with an error toast or query param, rather than showing the account page with broken API calls. | High | AuthContext |
| FR-7 | Header regression test: mount → shows CONNECT → connect wallet → shows AccountChip with address → open dropdown → click DISCONNECT → shows CONNECT again. | Medium | Header |
| FR-8 | Header regression test: page load with stale wagmi session → AccountChip renders with fallback address → SIWE completes → AccountChip updates with `user.wallet_address`. | Medium | Header |
| FR-9 | Measure and verify bundle size improvement. The landing page's initial JS payload (gzip) must decrease by at least 300KB compared to the pre-change build. | Medium | Build |

---

## Technical Implementation

### Bundle Size: Lazy Loading Architecture

The key insight: the landing page doesn't need wallet functionality. Only the Header's CONNECT button and the /account page use wagmi/RainbowKit. Move wallet providers to a lazy boundary.

**Option A (recommended): Conditional provider + lazy ConnectButton**

```tsx
// testudo-web/src/components/ui/Header.tsx
import { lazy, Suspense } from 'react'

const LazyConnectButton = lazy(() =>
  import('@rainbow-me/rainbowkit').then(m => ({
    default: m.ConnectButton,
  }))
)

// In render:
<Suspense fallback={
  <span className="font-mono text-xs tracking-wider text-text-secondary">
    CONNECT
  </span>
}>
  <div className="rk-header-btn">
    <LazyConnectButton label="CONNECT" showBalance={false} chainStatus="none" accountStatus="address" />
  </div>
</Suspense>
```

**Route-level code splitting for AccountPage:**

```tsx
// testudo-web/src/App.tsx
import { lazy, Suspense } from 'react'

const AccountPage = lazy(() => import('./pages/AccountPage'))

// In routes:
<Route path="/account" element={
  <ProtectedRoute>
    <Suspense fallback={<div className="p-8 font-mono text-text-secondary">LOADING...</div>}>
      <AccountPage />
    </Suspense>
  </ProtectedRoute>
} />
```

**Provider restructuring in main.tsx:**

The wallet providers (`WagmiProvider`, `QueryClientProvider`, `RainbowKitProvider`) must remain in the tree for `useAccount` and `useSignMessage` hooks used by `AuthContext`. However, we can defer their initialization until the first wallet interaction by wrapping them in a lazy-loaded component:

```tsx
// testudo-web/src/providers/WalletProviders.tsx
import { WagmiProvider } from 'wagmi'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RainbowKitProvider } from '@rainbow-me/rainbowkit'
import { wagmiConfig } from '../config/wagmi'

const queryClient = new QueryClient()

export function WalletProviders({ children, theme }: { children: React.ReactNode; theme: any }) {
  return (
    <WagmiProvider config={wagmiConfig}>
      <QueryClientProvider client={queryClient}>
        <RainbowKitProvider theme={theme}>
          {children}
        </RainbowKitProvider>
      </QueryClientProvider>
    </WagmiProvider>
  )
}
```

Since `AuthContext` calls `useAccount()` at the top level, the providers must stay above it. The real savings come from lazy-loading the `ConnectButton` (which pulls in the RainbowKit modal UI, all wallet connectors, and MetaMask SDK) and route-splitting `AccountPage` (which pulls in `WalletConnect.tsx` with its agent wallet logic).

### ProtectedRoute: SIWE Auto-Trigger

Replace the current pass-through behavior with an active SIWE prompt:

```tsx
// testudo-web/src/context/AuthContext.tsx — updated ProtectedRoute
export function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, loading, siweError } = useAuth()
  const { isConnected } = useAccount()

  if (loading) {
    return <div className="flex items-center justify-center h-screen font-mono text-text-secondary">LOADING...</div>
  }

  // Not connected at all → redirect
  if (!isAuthenticated && !isConnected) {
    return <Navigate to="/" replace />
  }

  // Connected but not authenticated → SIWE will auto-trigger via AuthContext useEffect
  // Show signing prompt instead of broken account page
  if (!isAuthenticated && isConnected) {
    if (siweError) {
      return <Navigate to="/?auth_error=signature_rejected" replace />
    }
    return (
      <div className="flex flex-col items-center justify-center h-screen gap-4">
        <div className="w-4 h-4 border-2 border-text-secondary border-t-text-primary rounded-full animate-spin" />
        <p className="font-mono text-xs text-text-secondary tracking-wider">VERIFYING WALLET...</p>
      </div>
    )
  }

  return <>{children}</>
}
```

The existing `AuthContext` useEffect (lines 45-84) already auto-triggers SIWE when `isConnected` transitions from false→true while `wasDisconnected.current` is true. The ProtectedRoute change just provides a proper loading state instead of showing the broken account page.

**Edge case**: If the user navigated directly to `/account` with a stale session (wagmi auto-reconnects), `wasDisconnected.current` starts false and SIWE won't auto-trigger. The `AuthContext` useEffect needs a second trigger path:

```tsx
// In AuthContext useEffect, add:
// If on a protected route, connected but not authed, and SIWE hasn't run yet
if (isConnected && !user && !siweInFlight.current && location.pathname === '/account') {
  // Trigger SIWE regardless of wasDisconnected state
  runSiwe()
}
```

### Header Regression Tests

```typescript
// testudo-web/src/components/ui/Header.test.tsx
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { describe, it, expect, vi } from 'vitest'
import { Header } from './Header'

// Mock dependencies
vi.mock('wagmi', () => ({ useAccount: vi.fn(), useDisconnect: vi.fn() }))
vi.mock('@rainbow-me/rainbowkit', () => ({
  ConnectButton: (props: any) => <button data-testid="rk-connect-button">{props.label}</button>,
}))
vi.mock('../../context/AuthContext', () => ({
  useAuth: vi.fn(),
}))

describe('Header wallet lifecycle', () => {
  it('shows CONNECT when disconnected', () => { /* ... */ })
  it('shows AccountChip with truncated address when connected', () => { /* ... */ })
  it('dropdown: ACCOUNT navigates, DISCONNECT calls both logout and disconnect', () => { /* ... */ })
  it('handles stale session: renders fallback address from useAccount when user is null', () => { /* ... */ })
})
```

### Files

- `testudo-web/src/components/ui/Header.tsx` — lazy-load ConnectButton (FR-1)
- `testudo-web/src/App.tsx` — lazy-load AccountPage route (FR-2)
- `testudo-web/src/main.tsx` — restructure provider wrapping (FR-3)
- `testudo-web/src/providers/WalletProviders.tsx` — extracted provider wrapper (FR-3)
- `testudo-web/src/context/AuthContext.tsx` — ProtectedRoute SIWE auto-trigger + signing prompt (FR-5, FR-6)
- `testudo-web/src/components/ui/Header.test.tsx` — regression tests (FR-7, FR-8)

### Dependencies Added

None — all lazy loading uses React built-ins (`React.lazy`, `Suspense`). Testing deps covered by TEST-01.

---

## Acceptance Criteria

- [ ] Landing page (`/`) initial JS payload does not include RainbowKit modal UI, MetaMask SDK, or WalletConnect connector code (FR-1, FR-3, FR-4)
- [ ] `ConnectButton` loads lazily when Header renders; a static "CONNECT" text shows during load (FR-1)
- [ ] AccountPage chunk loads only when navigating to `/account` (FR-2)
- [ ] A wallet-connected but unauthenticated user on `/account` sees "VERIFYING WALLET..." spinner, not error banners (FR-5)
- [ ] SIWE auto-triggers for connected-but-unauthed users on protected routes, including stale wagmi sessions (FR-5)
- [ ] If SIWE fails (user rejects signature), user is redirected to `/` — not left on a broken page (FR-6)
- [ ] Header regression test passes: mount → CONNECT → connect → AccountChip → disconnect → CONNECT (FR-7)
- [ ] Header regression test passes: stale session → fallback address → SIWE completes → user address (FR-8)
- [ ] Build output shows landing page initial JS gzip decreased by ≥300KB vs. baseline (FR-9)
- [ ] `cd testudo-web && bun run build` passes with no new warnings
- [ ] All existing functionality preserved: wallet connect, SIWE, account page, exchange management

---

## Risks

1. **Lazy ConnectButton flash** — If the lazy chunk takes >200ms to load, users may see the static "CONNECT" text briefly replaced by the styled RainbowKit button. Mitigation: style the fallback identically (same font, size, color, position) so the swap is invisible. Use `<link rel="modulepreload">` in index.html for the RainbowKit chunk if needed.

2. **AuthContext hooks require WagmiProvider** — `useAccount()` and `useSignMessage()` in `AuthContext` will throw if rendered outside `WagmiProvider`. Moving providers to a lazy boundary requires careful nesting. Mitigation: keep `WagmiProvider` + `QueryClientProvider` at the root (they're lightweight — the heavy imports are the RainbowKit UI components and MetaMask SDK). Only lazy-load the _rendering_ components (`ConnectButton`, `AccountPage`).

3. **SIWE auto-trigger infinite loop** — If `verifySiwe` fails silently (network error, not signature rejection), the useEffect could re-trigger. Mitigation: `siweInFlight.current` ref already prevents re-entry. Add `siweError` state check: if error is set, don't retry — redirect instead.

4. **Stale wagmi session edge case** — wagmi persists wallet connection in localStorage. If a user clears cookies but not localStorage, wagmi reports `isConnected=true` but the backend has no session. SIWE will fire, creating a new session. Mitigation: this is actually the correct behavior — SIWE creates a fresh session. Verify this path in Header regression test (FR-8).

---

## Completion Signal

This spec is complete when:
1. ConnectButton lazy-loaded in Header with invisible fallback
2. AccountPage route-split via React Router lazy
3. Landing page initial JS payload reduced by ≥300KB gzip
4. ProtectedRoute shows signing prompt for connected-but-unauthed users
5. SIWE auto-triggers on protected routes (fresh connect + stale session)
6. SIWE failure redirects to landing page
7. Header regression tests pass (2 lifecycle scenarios)
8. `bun run build` passes
9. Code committed to master