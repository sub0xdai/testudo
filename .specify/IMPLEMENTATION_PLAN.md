# Implementation Plan

> Last updated: 2026-03-29
> Current spec: EXT-44-hyperliquid-support
> Phase: COMPLETE

---

## Active Spec: EXT-44-hyperliquid-support

Hyperliquid platform support for Alt+X trade scraping. Adds manifest permissions, platform detection, and Hyperliquid-specific symbol extraction.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Add `*://app.hyperliquid.xyz/*` to manifest `host_permissions` + `content_scripts.matches`; add `isHyperliquid()` to content.ts; add `scrapeHyperliquidSymbol()` to scraper.ts with leaf-div walk + title fallback; integrate into Alt+X flow | complete | low | — |
| T2 | Validate `bun run build` passes, update state files, commit | complete | low | T1 |

### Key Decisions

- **FR-7 already satisfied**: `isChartPlatform()` already includes `hyperliquid` from EXT-43 — bridge injection works.
- **Text-content matching over class selectors**: Hyperliquid uses styled-components with unstable hash classes. Match leaf divs by `/^[A-Z0-9]{2,10}-USDC$/` regex.
- **Symbol format conversion**: `BTC-USDC` → `BTCUSDC` (strip hyphen).

### Discoveries

- FR-7 pre-satisfied by EXT-43 (`isChartPlatform()` already included hyperliquid)
- DOM strategy [2] won't work in isolated world on Hyperliquid — bridge handles position tool scraping
- Leaf div walk with text regex is the only stable selector approach (styled-components hash classes are unstable)
- content.js: 48.2kb → 48.6kb (+0.8kb)

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
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
