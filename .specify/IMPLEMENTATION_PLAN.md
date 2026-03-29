# Implementation Plan

> Last updated: 2026-03-29
> Current spec: EXT-43-main-world-bridge
> Phase: COMPLETE

---

## Active Spec: EXT-43-main-world-bridge

Main-world bridge for TradingView Chart API access. Enables position tool scraping without properties dialog on TradingView, DexScreener, and Hyperliquid.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Create `page-bridge.ts` with widget probe, position tool extraction, symbol extraction; update `build.ts` IIFE entries; add to `manifest.json` web_accessible_resources | complete | medium | — |
| T2 | Update `content.ts` with `injectBridge()` + `bridgeRequest()` helper (postMessage protocol, 500ms timeout, request IDs) | complete | medium | T1 |
| T3 | Validate `bun run build` passes, verify `page-bridge.js` in dist/chrome + dist/firefox, update state files | complete | low | T2 |

### Key Decisions

- **Bridge runs in MAIN world**: Injected via `<script>` tag, accesses page's `window` directly (not isolated content script world).
- **postMessage protocol**: Content script ↔ bridge communicate via `window.postMessage` with typed messages and request IDs.
- **Position tool logic reused from scraper.ts**: `getTickSize()` and `findPositionToolByChartApi()` logic moved to bridge (runs in page context where Chart API is accessible).
- **No existing code removed**: Bridge is additive — existing DOM strategies unchanged.

### Discoveries

(populated during build iterations)

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
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
