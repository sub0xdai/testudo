# EXT-01: Extension Scaffold & Build System

> Priority: P0 | Depends on: nothing | Status: COMPLETE

## Overview
**Current:** Empty directory with only `prd.md`.
**Target:** Buildable browser extension with Manifest V3, content script injecting on `tradingview.com`, background service worker, and popup settings page.

## Functional Requirements

| ID | Requirement | Status |
|----|-------------|--------|
| FR-1 | Manifest V3 with `content_scripts` matching `*://*.tradingview.com/*` | DONE |
| FR-2 | Content script skeleton injecting on TradingView chart pages | DONE |
| FR-3 | Background service worker for connection state management | DONE |
| FR-4 | Popup page with settings: backend URL, execution mode toggle | DONE |
| FR-5 | Build system (TypeScript + esbuild) producing Chrome and Firefox outputs | DONE |
| FR-6 | Cross-browser compat using webextension-polyfill | DONE |

## Key Files

| File | Purpose |
|------|---------|
| `testudo-extension/manifest.json` | Manifest V3 |
| `testudo-extension/src/content.ts` | Content script entry |
| `testudo-extension/src/background.ts` | Service worker |
| `testudo-extension/src/popup/popup.html` | Settings UI |
| `testudo-extension/src/popup/popup.ts` | Settings logic |
| `testudo-extension/build.ts` | Build script (Chrome + Firefox) |

## Verification
```bash
cd testudo-extension && bun install && bun run build
# dist/chrome/ and dist/firefox/ produced
# bun run typecheck passes clean
```

<promise>DONE</promise>
