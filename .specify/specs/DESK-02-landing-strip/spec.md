# Specification: Landing Page Strip — Remove Web3 Dependencies

**Spec ID:** DESK-02-landing-strip
**Date:** 2026-03-25
**Status:** Draft
**Class:** Refactor / Cleanup
**Priority:** P1 — After DESK-01 moves all auth and account management to the Desk, testudo-web retains dead wallet infrastructure (wagmi, RainbowKit, viem, MetaMask SDK) that bloats the landing page bundle and maintains orphaned code paths.
**Depends on:** DESK-01-unified-dashboard
**Series:** DESK-01 through DESK-02 (unified dashboard migration)

---

## Problem Statement

Once DESK-01 is complete, the Desk (testudo-journal) owns the entire authenticated experience: wallet connection, SIWE, exchange management, extension pairing, and analytics. The testudo-web landing page no longer needs any Web3 functionality — but it still ships wagmi (2.19), RainbowKit (2.2), viem (2.38), MetaMask SDK, and @tanstack/react-query as dependencies. The `AuthContext.tsx` still contains the SIWE flow, `ProtectedRoute` guard, and wagmi hooks. The `AccountPage.tsx` (439 lines) remains in the codebase alongside `ExchangeCard.tsx` (215 lines), `WalletConnect.tsx` (252 lines), `ExtensionPairingBanner.tsx` (115 lines), and `AddExchangeCard.tsx` (14 lines).

This dead code inflates the landing page from what should be a ~50KB marketing site to a ~300KB+ gzip payload. The `Header.tsx` still lazy-loads a `ConnectButton` that should be replaced with a simple "LAUNCH DESK" link. The orphaned `/account` route still exists behind a `ProtectedRoute` that references wagmi hooks.

This spec strips testudo-web to a pure marketing site: Hero, Features, Pricing, About — with links to the Desk, Docs, and Extension download. No wallet, no auth, no account management.

---

## User Stories

- **As a visitor**, I want the landing page to load instantly, so that I can evaluate Testudo without downloading wallet infrastructure.
- **As a developer**, I want the testudo-web codebase to only contain code relevant to the marketing site, so that it's simple to maintain.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Remove `wagmi`, `@rainbow-me/rainbowkit`, `viem`, `@tanstack/react-query` from testudo-web `package.json` dependencies. | High | Dependencies |
| FR-2 | Delete `src/context/AuthContext.tsx` — no auth context needed on landing page. | High | Auth |
| FR-3 | Delete `src/pages/AccountPage.tsx`. | High | Pages |
| FR-4 | Delete `src/components/WalletConnect.tsx`, `src/components/ExchangeCard.tsx`, `src/components/AddExchangeCard.tsx`, `src/components/ExtensionPairingBanner.tsx`. | High | Components |
| FR-5 | Simplify `src/components/ui/Header.tsx`: remove wallet chip (`AccountChip`), remove lazy `ConnectButton`, remove wagmi imports (`useAccount`, `useDisconnect`), remove `useAuth` import. Replace with a static "LAUNCH DESK" link pointing to `/desk/`. | High | Header |
| FR-6 | Simplify `src/main.tsx`: remove `WagmiProvider`, `QueryClientProvider`, `RainbowKitProvider`, `RainbowKitThemeWrapper`, `AuthProvider`. The app tree becomes: `ThemeProvider → BrowserRouter → App`. | High | Entry |
| FR-7 | Update `src/App.tsx`: remove `/account` route and `ProtectedRoute` import. Routes become: `/` (LandingPage), `/about` (AboutPage). Remove lazy import of AccountPage. | High | Routing |
| FR-8 | Delete `src/api/client.ts` or strip to only what's needed (if any landing page feature calls the API). If no API calls remain, delete entirely. | High | API |
| FR-9 | Delete `src/types.ts` if it only contains auth-related types (`User`, `ExchangeAccount`, etc.) that are no longer used. | Medium | Types |
| FR-10 | Remove `@rainbow-me/rainbowkit/styles.css` import from `main.tsx`. | Medium | Styles |
| FR-11 | Remove RainbowKit CSS overrides (`.rk-header-btn` rules) from `src/index.css`. | Medium | Styles |
| FR-12 | Update the Pricing section's "GET STARTED" link from `/account` to `/desk/account`. | Medium | Content |
| FR-13 | Verify build output: the landing page bundle should have zero Web3-related chunks (no metamask-sdk, no walletconnect, no viem, no wagmi chunks). Total gzip payload for initial page load should be under 100KB. | Medium | Build |
| FR-14 | Update any extension code that references `/account` to point to `/desk/account` (check background/api.ts, handlers.ts for redirect URLs). | Medium | Extension |
| FR-15 | Delete `src/config/wagmi.ts` — no wagmi configuration needed. | High | Config |

---

## Technical Implementation

### Header Simplification

```tsx
// testudo-web/src/components/ui/Header.tsx — after strip
import { Link } from 'react-router-dom'
import { useTheme, THEME_LABELS } from '../../context/ThemeContext'

export function Header() {
  const { theme, cycleTheme } = useTheme()

  return (
    <header className="fixed top-0 left-0 right-0 z-50 px-6 md:px-8 py-4 bg-main-bg/90 border-b border-container-border/30">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Link to="/" className="font-mono text-lg tracking-widest text-text-primary hover:text-accent-steel transition-colors">
            TESTUDO
          </Link>
          {/* theme toggle button */}
        </div>
        <nav className="flex items-center gap-6 md:gap-8">
          <Link to="/about" className="font-mono text-xs tracking-wider text-text-secondary hover:text-text-primary transition-colors hidden md:block">ABOUT</Link>
          <a href="#pricing" className="font-mono text-xs tracking-wider text-text-secondary hover:text-text-primary transition-colors hidden md:block">PRICING</a>
          <a href="/docs/" target="_blank" rel="noopener noreferrer" className="font-mono text-xs tracking-wider text-text-secondary hover:text-text-primary transition-colors hidden md:block">DOCS</a>
          <a href="https://chromewebstore.google.com" target="_blank" rel="noopener noreferrer" className="font-mono text-xs tracking-wider text-text-secondary hover:text-text-primary transition-colors hidden md:block">EXTENSION</a>
          <a href="/desk/" className="font-mono text-xs tracking-wider text-text-primary hover:text-accent-steel transition-colors">LAUNCH DESK</a>
        </nav>
      </div>
    </header>
  )
}
```

### main.tsx Simplification

```tsx
// testudo-web/src/main.tsx — after strip
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import { ThemeProvider } from './context/ThemeContext'
import App from './App'
import './index.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ThemeProvider>
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </ThemeProvider>
  </StrictMode>,
)
```

### Deletion Manifest

| Action | File | Lines Removed |
|--------|------|---------------|
| Delete | `src/context/AuthContext.tsx` | ~130 |
| Delete | `src/pages/AccountPage.tsx` | ~439 |
| Delete | `src/components/WalletConnect.tsx` | ~252 |
| Delete | `src/components/ExchangeCard.tsx` | ~215 |
| Delete | `src/components/AddExchangeCard.tsx` | ~14 |
| Delete | `src/components/ExtensionPairingBanner.tsx` | ~115 |
| Delete | `src/api/client.ts` | ~111 |
| Delete | `src/config/wagmi.ts` | ~12 |
| Simplify | `src/components/ui/Header.tsx` | ~80 removed |
| Simplify | `src/main.tsx` | ~30 removed |
| Simplify | `src/App.tsx` | ~15 removed |
| Simplify | `src/index.css` | ~15 removed |
| **Total** | | **~1,430 lines deleted** |

### Dependencies Removed

From `package.json` dependencies:
- `@rainbow-me/rainbowkit` — wallet UI (no longer needed)
- `@tanstack/react-query` — required by wagmi (no longer needed)
- `wagmi` — wallet hooks (no longer needed)
- `viem` — Ethereum utilities (no longer needed)
- `zod` — only used for account form validation (no longer needed)
- `axios` — only used for API client (no longer needed)

### Files

**Delete:**
- `testudo-web/src/context/AuthContext.tsx`
- `testudo-web/src/pages/AccountPage.tsx`
- `testudo-web/src/components/WalletConnect.tsx`
- `testudo-web/src/components/ExchangeCard.tsx`
- `testudo-web/src/components/AddExchangeCard.tsx`
- `testudo-web/src/components/ExtensionPairingBanner.tsx`
- `testudo-web/src/api/client.ts`
- `testudo-web/src/config/wagmi.ts`

**Modify:**
- `testudo-web/src/components/ui/Header.tsx` — strip wallet UI, add "LAUNCH DESK" (FR-5)
- `testudo-web/src/main.tsx` — remove wallet providers (FR-6)
- `testudo-web/src/App.tsx` — remove /account route (FR-7)
- `testudo-web/src/index.css` — remove RainbowKit overrides (FR-11)
- `testudo-web/src/components/sections/Pricing.tsx` — update link to `/desk/account` (FR-12)
- `testudo-web/package.json` — remove 6 dependencies (FR-1)

**Check:**
- `testudo-extension/src/background/api.ts` — any references to `/account` path (FR-14)
- `testudo-extension/src/background/handlers.ts` — any redirect URLs (FR-14)

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] `package.json` has zero Web3 dependencies (no wagmi, rainbowkit, viem, react-query) (FR-1)
- [ ] `src/context/AuthContext.tsx` does not exist (FR-2)
- [ ] `src/pages/AccountPage.tsx` does not exist (FR-3)
- [ ] All 5 account-related components deleted (FR-4)
- [ ] Header shows "LAUNCH DESK" link instead of ConnectButton/AccountChip (FR-5)
- [ ] `main.tsx` provider tree is: ThemeProvider → BrowserRouter → App (FR-6)
- [ ] Only two routes exist: `/` and `/about` (FR-7)
- [ ] No RainbowKit CSS imports or overrides remain (FR-10, FR-11)
- [ ] Pricing "GET STARTED" links to `/desk/account` (FR-12)
- [ ] Build output contains zero Web3-related chunks — no metamask, walletconnect, viem, wagmi files (FR-13)
- [ ] Total initial page load gzip under 100KB (FR-13)
- [ ] `cd testudo-web && bun run build` passes
- [ ] Landing page renders correctly with all sections (Hero, Features, Pricing, About)
- [ ] "LAUNCH DESK" navigates to `/desk/` successfully
- [ ] Theme toggle (amoled/light) still works
- [ ] Test files updated or removed to match new code (no broken imports)

---

## Risks

1. **Shared test files reference deleted modules** — The TEST-01 test suite created `AuthContext.test.tsx`, `client.test.ts`, and `Header.test.tsx` which all import from modules being deleted. Mitigation: delete all testudo-web test files that test deleted modules. Write a minimal Header test for the simplified component. The auth and API tests are no longer needed (that logic lives in testudo-journal now).

2. **Extension references to `/account`** — The extension's background worker or popup may contain URLs or redirect logic pointing to the old `/account` path. Mitigation: grep the extension codebase for `/account` and update to `/desk/account`.

3. **SEO / external links** — If any external pages link to `testudo.app/account`, those links break. Mitigation: add a redirect rule in the vite config or a catch-all route that redirects `/account` to `/desk/account`.

---

## Completion Signal

This spec is complete when:
1. All Web3 dependencies removed from testudo-web package.json
2. All 8 auth/account files deleted
3. Header simplified to static nav with "LAUNCH DESK"
4. main.tsx stripped to ThemeProvider → BrowserRouter → App
5. Build output has zero Web3 chunks, total gzip under 100KB
6. Landing page renders and navigates correctly
7. Extension references updated from `/account` to `/desk/account`
8. `bun run build` passes
9. Code committed to master
