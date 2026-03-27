# Specification: Comprehensive Frontend Test Suite

**Spec ID:** TEST-01-frontend-test-suite
**Date:** 2026-03-25
**Status:** Draft
**Class:** Testing / Infrastructure + Coverage
**Priority:** P1 — Three TypeScript frontends (10,098 LOC combined) have near-zero automated test coverage. The AUTH-03 SIWE rewrite shipped without verification. Extension test gaps span critical trade execution and schema validation paths.
**Depends on:** None (first in series)
**Series:** TEST-01 (frontend test infrastructure and coverage)

---

## Problem Statement

The Testudo frontend codebase has a severe test coverage imbalance. The Rust backend has 911 passing tests across 8 crates, while the three TypeScript frontends — testudo-web (2,247 LOC), testudo-journal (5,972 LOC), and testudo-extension (6,879 LOC) — have effectively zero or minimal automated testing.

**testudo-web** has no test runner installed. No vitest, jest, or @testing-library in `package.json`. The entire AUTH-03 rewrite — SIWE wallet authentication, HttpOnly cookie sessions, 401 refresh-and-retry interceptor, extension pairing code generation — has zero automated verification. The `AuthContext.tsx` (118 lines) contains a complex state machine with `wasDisconnected.current` tracking, `siweInFlight.current` guard, and auto-SIWE trigger logic that has never been tested. The `ProtectedRoute` component's relaxed guard (`!isAuthenticated && !isConnected`) was a pragmatic fix that could regress silently.

**testudo-journal** also has no test infrastructure. 398 lines of API client code (`src/api/client.ts`) with 25+ endpoints, cookie-based auth with 401 refresh, and data transformation pipelines for analytics (equity curves, daily PnL, symbol breakdowns, return distributions) are entirely untested.

**testudo-extension** has vitest (3.2.4) with 3 unit test files (938 lines) and 3 E2E files (722 lines), but major gaps remain: `background/api.ts` (443 lines, zero tests), `schemas.ts` (257 lines of Zod schemas, zero tests), `scraper.ts` (605 lines, E2E only), and all popup UI components (2,568 lines across 12 components, zero tests).

This spec establishes test infrastructure where missing, then builds coverage for the highest-risk paths first: authentication flows, data validation, and trade execution.

---

## User Stories

- **As a developer**, I want automated tests for the SIWE auth flow, so that auth regressions are caught before they reach production.
- **As a developer**, I want Zod schema validation tests for the extension's 23 message types, so that runtime type mismatches between extension and backend are detected at build time.
- **As a developer**, I want E2E coverage of the wallet connect → SIWE → account → pair extension flow, so that the critical onboarding path is verified end-to-end.
- **As a developer**, I want test infrastructure in testudo-web and testudo-journal, so that new features can be developed with TDD.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Install vitest + @testing-library/react + jsdom in testudo-web. Add `test` and `test:watch` scripts to package.json. Verify vitest runs with zero tests passing. | High | testudo-web |
| FR-2 | Test `AuthContext` SIWE flow: fresh connect triggers SIWE, page-load reconnect does NOT trigger SIWE, `siweInFlight` prevents duplicate attempts, failed SIWE sets `siweError`, successful SIWE sets `user` and `isAuthenticated`. | High | testudo-web |
| FR-3 | Test `ProtectedRoute`: unauthenticated + disconnected → redirects to `/`, authenticated → renders children, connected-but-unauthed → renders children (current behavior). | High | testudo-web |
| FR-4 | Test API client 401 interceptor: initial 401 triggers refresh, successful refresh retries original request, failed refresh rejects, auth-probe endpoints (`/auth/me`, `/auth/refresh`) skip refresh. | High | testudo-web |
| FR-5 | Test `Header` component render states: disconnected shows ConnectButton, connected shows AccountChip with truncated address, AccountChip dropdown opens/closes, DISCONNECT calls both `logout()` and `disconnect()`. | High | testudo-web |
| FR-6 | Test extension `schemas.ts`: every exported Zod schema validates valid input and rejects invalid input. Cover `TradePayloadSchema` edge cases (negative prices, zero risk, 100% risk, missing fields). | High | testudo-extension |
| FR-7 | Test extension `background/api.ts`: `apiRequest()` helper handles success, 401 refresh, network error, timeout. `normalizeBackendAck()` coerces response shapes. Exchange selection logic (`getActiveExchangeId`, `setActiveExchangeId`). | High | testudo-extension |
| FR-8 | Test extension `scraper.ts` price parsing: locale formats (1,234.56 vs 1.234,56), negative prices, prices with currency symbols, symbol extraction from TradingView DOM selectors. | Medium | testudo-extension |
| FR-9 | Install vitest + @solidjs/testing-library in testudo-journal. Add `test` and `test:watch` scripts. Verify vitest runs. | Medium | testudo-journal |
| FR-10 | Test journal `api/client.ts`: `fetchWithCredentials` sends cookies, 401 triggers refresh and retry, `buildParams()` serializes query parameters correctly, `fetchApi` and `fetchCrud` handle error responses. | Medium | testudo-journal |
| FR-11 | E2E test: landing page loads → click CONNECT → wallet modal appears → (mock) wallet connects → SIWE auto-triggers → account page accessible → exchange cards render. | Medium | E2E |
| FR-12 | Test extension popup `PairView.tsx`: 6-digit input auto-advances, paste fills all boxes, empty submit shows error, successful pair navigates to main view. | Low | testudo-extension |

---

## Technical Implementation

### Test Infrastructure Setup

**testudo-web** — new vitest config matching the project's React 18 + Vite stack:

```typescript
// testudo-web/vitest.config.ts
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: false,
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
    setupFiles: ['src/test/setup.ts'],
  },
})
```

```typescript
// testudo-web/src/test/setup.ts
import '@testing-library/jest-dom/vitest'
```

**testudo-journal** — new vitest config matching the Solid.js stack:

```typescript
// testudo-journal/vitest.config.ts
import { defineConfig } from 'vitest/config'
import solidPlugin from 'vite-plugin-solid'

export default defineConfig({
  plugins: [solidPlugin()],
  test: {
    environment: 'jsdom',
    globals: false,
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
    deps: {
      optimizer: { web: { include: ['solid-js'] } },
    },
    resolve: { conditions: ['development', 'browser'] },
  },
})
```

### AuthContext Test Strategy

Mock wagmi hooks (`useAccount`, `useSignMessage`, `useDisconnect`) and the API client (`authApi`). Test the state machine transitions:

```typescript
// testudo-web/src/context/AuthContext.test.tsx
import { render, waitFor } from '@testing-library/react'
import { describe, it, expect, vi } from 'vitest'

// Mock wagmi before import
vi.mock('wagmi', () => ({
  useAccount: vi.fn(),
  useSignMessage: vi.fn(),
  useDisconnect: vi.fn(),
}))

// Test cases:
// 1. Fresh connect (wasDisconnected→true, isConnected→true) → triggers SIWE
// 2. Page-load reconnect (wasDisconnected stays false) → skips SIWE
// 3. siweInFlight guard prevents duplicate calls
// 4. API nonce failure → siweError set, user remains null
// 5. signMessage rejection → siweError set
// 6. verifySiwe success → user set, isAuthenticated true
// 7. logout() → user null, disconnect called
```

### Schema Validation Tests

Test every exported schema from `testudo-extension/src/schemas.ts`:

```typescript
// testudo-extension/src/schemas.test.ts
import { describe, it, expect } from 'vitest'
import {
  TradePayloadSchema,
  SettingsSchema,
  AuthTokensSchema,
  RuntimeMessageSchema,
  // ... all exported schemas
} from './schemas'

describe('TradePayloadSchema', () => {
  it('accepts valid trade payload', () => {
    const result = TradePayloadSchema.safeParse({
      symbol: 'BTCUSDT',
      side: 'LONG',
      entry: '65000',
      stop: '64000',
      target: '67000',
      timeframe: '4H',
      management: { risk_percent: 1 },
    })
    expect(result.success).toBe(true)
  })

  it('rejects negative entry price', () => {
    const result = TradePayloadSchema.safeParse({
      symbol: 'BTCUSDT', side: 'LONG',
      entry: '-1', stop: '64000', target: '67000',
      timeframe: '4H', management: { risk_percent: 1 },
    })
    expect(result.success).toBe(false)
  })

  it('rejects risk_percent outside 0.1-100 range', () => { /* ... */ })
})
```

### API Client Interceptor Tests

Mock axios to verify the 401 refresh-and-retry interceptor in `testudo-web/src/api/client.ts`:

```typescript
// testudo-web/src/api/client.test.ts
import { describe, it, expect, vi, beforeEach } from 'vitest'
import axios from 'axios'

// Intercept axios.create to capture the interceptor
// Verify: 401 on /exchanges → refresh called → original retried
// Verify: 401 on /auth/me → NO refresh (auth probe skip)
// Verify: refresh fails → original rejected, no infinite loop
```

### File Map

| Action | File | Purpose |
|--------|------|---------|
| Create | `testudo-web/vitest.config.ts` | Vitest config for React 18 |
| Create | `testudo-web/src/test/setup.ts` | jest-dom matchers |
| Create | `testudo-web/src/context/AuthContext.test.tsx` | SIWE flow state machine (FR-2) |
| Create | `testudo-web/src/context/ProtectedRoute.test.tsx` | Route guard logic (FR-3) |
| Create | `testudo-web/src/api/client.test.ts` | 401 interceptor (FR-4) |
| Create | `testudo-web/src/components/ui/Header.test.tsx` | Header render states (FR-5) |
| Modify | `testudo-web/package.json` | Add vitest, @testing-library/react, @testing-library/jest-dom, jsdom, test scripts |
| Create | `testudo-extension/src/schemas.test.ts` | Zod schema validation (FR-6) |
| Create | `testudo-extension/src/background/api.test.ts` | API helper + exchange selection (FR-7) |
| Create | `testudo-extension/src/scraper.test.ts` | Price parsing + symbol extraction (FR-8) |
| Create | `testudo-extension/src/popup/components/PairView.test.tsx` | OTP input UX (FR-12) |
| Create | `testudo-journal/vitest.config.ts` | Vitest config for Solid.js |
| Create | `testudo-journal/src/api/client.test.ts` | API client + auth refresh (FR-10) |
| Modify | `testudo-journal/package.json` | Add vitest, @solidjs/testing-library, jsdom, test scripts |
| Create | `testudo-web/tests/e2e/auth-flow.spec.ts` | E2E wallet → SIWE → account (FR-11) |

### Dependencies Added

**testudo-web** (devDependencies):
- `vitest = "^3.2.4"` — test runner
- `@testing-library/react = "^16.3.0"` — React component testing
- `@testing-library/jest-dom = "^6.6.0"` — DOM matchers
- `@testing-library/user-event = "^14.6.0"` — user interaction simulation
- `jsdom = "^28.0.0"` — browser environment

**testudo-journal** (devDependencies):
- `vitest = "^3.2.4"` — test runner
- `@solidjs/testing-library = "^0.8.10"` — Solid.js component testing
- `jsdom = "^28.0.0"` — browser environment

---

## Acceptance Criteria

- [ ] `cd testudo-web && bun test` runs vitest and reports results (FR-1)
- [ ] AuthContext tests cover: fresh-connect SIWE trigger, reconnect skip, siweInFlight guard, nonce failure, sign rejection, verify success, logout (FR-2)
- [ ] ProtectedRoute tests cover: redirect when unauthed+disconnected, render when authed, render when connected-but-unauthed (FR-3)
- [ ] API interceptor tests cover: 401→refresh→retry, auth-probe skip, refresh-failure rejection (FR-4)
- [ ] Header tests cover: ConnectButton when disconnected, AccountChip when connected, dropdown open/close, DISCONNECT handler (FR-5)
- [ ] All exported Zod schemas in `schemas.ts` have valid-input and invalid-input tests (FR-6)
- [ ] `apiRequest()` tests cover success, 401 refresh, network error, timeout (FR-7)
- [ ] Scraper price parsing tests cover: US locale, EU locale, negative, currency symbols, symbol extraction (FR-8)
- [ ] `cd testudo-journal && bun test` runs vitest and reports results (FR-9)
- [ ] Journal API client tests cover: cookie sending, 401 refresh, buildParams serialization (FR-10)
- [ ] E2E auth flow test exists and can run against local dev environment (FR-11)
- [ ] All existing tests still pass: `cd testudo-extension && bun test` (regression check)
- [ ] `cd testudo-web && bun run build` passes
- [ ] `cd testudo-extension && bun run build` passes
- [ ] `cd testudo-journal && bun run build` passes

---

## Risks

1. **wagmi/RainbowKit mock complexity** — These libraries use React context providers extensively. Mocking `useAccount`, `useSignMessage`, etc. requires careful setup to avoid "missing provider" errors. Mitigation: create a shared test wrapper (`renderWithProviders`) that wraps components in minimal mock providers.

2. **Solid.js testing library maturity** — `@solidjs/testing-library` is less mature than the React equivalent. Some patterns (async state updates, signal tracking) may require manual `waitFor` patterns. Mitigation: reference existing working tests in `testudo-extension/src/background.test.ts` which already use the Solid testing setup.

3. **E2E wallet mocking** — Real wallet connections can't be automated without browser extension injection. Mitigation: use Playwright's page.evaluate to mock `window.ethereum` provider, or use a test-only wallet connector that auto-approves.

4. **Test maintenance burden** — Adding 15+ test files increases maintenance surface. Mitigation: focus tests on behavior contracts (state machine transitions, data validation), not implementation details. Avoid snapshot tests that break on CSS changes.

---

## Completion Signal

This spec is complete when:
1. vitest installed and running in testudo-web with test/test:watch scripts
2. vitest installed and running in testudo-journal with test/test:watch scripts
3. AuthContext SIWE state machine tested (7+ test cases)
4. ProtectedRoute guard tested (3 cases)
5. API interceptor tested (3 cases)
6. Header render states tested (4 cases)
7. All Zod schemas validated (valid + invalid for each)
8. Extension api.ts helper tested (4 cases)
9. Scraper price parsing tested (5+ locales)
10. Journal API client tested (3 cases)
11. E2E auth flow test exists
12. All builds pass (web, extension, journal)
13. All tests pass across all three frontends
14. Code committed to master
