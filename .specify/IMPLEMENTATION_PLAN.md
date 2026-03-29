# Implementation Plan

> Last updated: 2026-03-29
> Current spec: EXT-45-dexscreener-symbols
> Phase: COMPLETE

---

## Active Spec: EXT-45-dexscreener-symbols

DexScreener symbol extraction for Alt+X trade scraping. Adds platform detection and 4-strategy symbol scraper for DexScreener token pages.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Add `isDexScreener()` to content.ts; add `scrapeDexScreenerSymbol()` to scraper.ts with 4 strategies (legend, title parens, title slash, leaf element scan); integrate into `scrapeSymbol()` cascade | complete | low | — |
| T2 | Validate `bun run build` passes, update state files, commit | complete | low | T1 |

### Key Decisions

- **FR-5 already satisfied**: `isChartPlatform()` already includes `dexscreener.com` from EXT-43 — bridge injection works.
- **TradingView legend selectors may work**: DexScreener embeds TradingView charting lib directly (not iframe), so `[data-name="legend-source-item"]` may match.
- **4 fallback strategies**: legend → title parens → title slash → leaf element scan.

### Discoveries

- FR-5 pre-satisfied by EXT-43 (`isChartPlatform()` already included dexscreener.com)
- `isDexScreener()` added to content.ts but only used for documentation/readability — `isChartPlatform()` handles the actual bridge injection guard
- DexScreener check placed before generic SYMBOL_SELECTORS in `scrapeSymbol()` cascade (same pattern as Hyperliquid)
- content.js: 48.6kb → 49.3kb (+0.7kb)
- Build passes for both Chrome and Firefox targets

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
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
