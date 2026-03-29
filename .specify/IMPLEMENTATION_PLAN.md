# Implementation Plan

> Last updated: 2026-03-29
> Current spec: EXT-46-async-scraper-flow
> Phase: COMPLETE

---

## Active Spec: EXT-46-async-scraper-flow

Multi-platform Alt+X flow integration. Ties EXT-43/44/45 together: bridge-first strategy, TradingView-only DOM fallback, symbol-only fallback on all chart platforms.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Revise Alt+X flow: `isChartPlatform()` bridge guard, TradingView-only DOM fallback with no-arg `scrapeTradeSetup()`, export `scrapeTimeframe`, use platform-aware timeframe | complete | low | — |
| T2 | Validate `bun run build` passes, update state files, commit | complete | low | T1 |

### Key Decisions

- **Bridge guard**: Use `isChartPlatform()` instead of `bridgeReady` — `bridgeRequest()` already returns null if not ready internally.
- **DOM strategies TradingView-only**: Non-TV sites should not attempt DOM strategies (they fail in isolated world). Bridge handles chart API access on all platforms.
- **No-arg `scrapeTradeSetup()`**: FR-5 requires unchanged call path — remove `strategiesToTry` parameter usage.
- **Export `scrapeTimeframe()`**: On TradingView, bridge-sourced setups should get the real timeframe via `scrapeTimeframe()`. Other platforms get "chart".
- **Simplified symbol-only**: No bridge symbol request in symbol-only fallback — bridge was already tried in step 1.

### Discoveries

- `scrapeTimeframe()` was not exported from scraper.ts — added `export` keyword for content.ts to use in bridge path
- `strategiesToTry` parameter on `scrapeTradeSetup()` was the root cause of FR-5 violation — non-TV sites tried only Strategy 2 (dead in isolated world)
- content.js bundle size unchanged at 49.3kb — code is same length, just reorganized
- `bridgeRequest()` already returns null when `bridgeReady === false`, so `isChartPlatform()` guard is functionally equivalent to `bridgeReady` guard but semantically clearer

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| EXT-46-async-scraper-flow | 2026-03-29 |
| EXT-45-dexscreener-symbols | 2026-03-29 |
| EXT-44-hyperliquid-support | 2026-03-29 |
| EXT-43-main-world-bridge | 2026-03-29 |
| DOCS-01-comprehensive-documentation | 2026-03-27 |
| HIST-01-exchange-history-import | 2026-03-26 |
| ONBOARD-01-stepper-onboarding | 2026-03-26 |
| DESK-02-landing-strip | 2026-03-26 |
| EXT-41-desk-dashboard | 2026-03-24 |
| EXT-40-smart-card-grid | 2026-03-24 |
| EXT-39-pair-ux | 2026-03-24 |
| AUTH-03-frontend-auth | 2026-03-24 |
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
| EXT-38-background-decomposition | 2022-03-22 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |
| UXP-18-multi-theme | 2026-03-21 |
| HL-11-status-transition-fix | 2026-03-21 |

---

*This file is persistent state. Vox updates it each iteration.*
