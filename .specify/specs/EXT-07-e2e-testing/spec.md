# EXT-07: E2E Testing with Playwright

> Priority: P2 | Depends on: EXT-06 | Status: PENDING

## Overview
**Current:** Extension has Tier 1 unit tests (pure functions) and Tier 2 integration tests (mocked browser APIs) via Vitest. No E2E tests exist — all verification is manual (load in Chrome, open TradingView, interact).
**Target:** Playwright E2E test suite that loads the unpacked extension in Chromium, tests popup interaction, status indicators, and the Alt+X trade flow against a mock page.

## Functional Requirements

| ID | Requirement | Status |
|----|-------------|--------|
| FR-1 | Playwright config -- launch Chromium with `--load-extension=dist/chrome` and `--disable-extensions-except` | TODO |
| FR-2 | Popup tests -- open popup, verify settings save, execution mode toggle, login form renders | TODO |
| FR-3 | Status indicator test -- verify status dot reflects WS connection state (disconnected when no server) | TODO |
| FR-4 | Mock TradingView page -- local HTML page with Position Tool DOM structure for scraper testing | TODO |
| FR-5 | Alt+X hotkey test -- inject mock Position Tool, press Alt+X, verify modal appears with correct values | TODO |
| FR-6 | Trade flow test -- mock backend REST endpoint, confirm trade via modal, verify POST received | TODO |

## Architecture

### Playwright Extension Testing
Chromium supports loading unpacked extensions:
```typescript
const context = await chromium.launchPersistentContext(userDataDir, {
  headless: false, // extensions require headed mode
  args: [
    `--disable-extensions-except=${extensionPath}`,
    `--load-extension=${extensionPath}`,
  ],
});
```

### Mock Page Strategy
TradingView DOM changes frequently. Instead of testing against real TradingView:
- Create a local HTML page mimicking the Position Tool DOM structure
- Include `#header-toolbar-symbol-search` for ticker
- Include `#overlap-manager-root` with price level elements
- Content script injects via `matches` — use `"*://localhost/*"` in test manifest or inject manually

### Mock Backend
- Use Playwright's `page.route()` to intercept REST calls, or
- Spin up a simple local HTTP server that records requests

## Key Files

| File | Purpose |
|------|---------|
| `testudo-extension/playwright.config.ts` | Playwright config with extension loading |
| `testudo-extension/tests/e2e/popup.spec.ts` | Popup UI tests |
| `testudo-extension/tests/e2e/trade-flow.spec.ts` | Full Alt+X → modal → execute flow |
| `testudo-extension/tests/e2e/fixtures/mock-tradingview.html` | Mock page with Position Tool DOM |

## Verification
```bash
cd testudo-extension && npx playwright test
```

## Notes
- Extensions require `headless: false` in Playwright (Chromium limitation)
- Service worker must be waited for before testing popup messaging
- Content script injection timing may need `page.waitForFunction`
