# Implementation Plan

> Last updated: 2026-03-30
> Current spec: UX-01-pair-page
> Phase: COMPLETE

---

## Active Spec: UX-01-pair-page

Standalone /pair page for extension pairing. 3-state UI with extension auto-detection.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Create /pair route and standalone Pair.tsx with 3-state UI | complete | medium | — |
| T2 | Add content script TESTUDO_INSTALLED postMessage for auto-detection | complete | low | T1 |
| T3 | Update extension PairView instructions + landing site redirect | complete | low | T1 |

### Key Decisions

- **Router restructured**: Changed from `root={Layout}` to nested route pattern so /pair bypasses Layout entirely
- **postMessage for detection**: Content scripts run in isolated world — can't set window globals, so use postMessage bridge (same pattern as EXT-43)
- **DESK_URL/pair for dev**: Extension links to DESK_URL/pair (not testudo.vip/pair) so local dev works without redirect
- **Cloudflare _redirects**: Landing site uses Cloudflare Pages _redirects for /pair → desk.testudo.vip/pair
- **Auto-generate code**: Code generates automatically on auth — no extra button click needed

### Discoveries

- SolidJS Router v0.15 supports nested route layouts cleanly — just remove `root=` and use parent Route
- Manifest needed desk.testudo.vip and localhost added to content_scripts matches for detection to work
- Pair.tsx is lazy-loaded and code-split automatically by Vite

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| UX-01-pair-page | 2026-03-30 |
| SEC-04-trade-management-idor | 2026-03-30 |
| SEC-03-psk-fail-closed | 2026-03-30 |
| SEC-02-cors-extension-pinning | 2026-03-30 |
| SEC-01-trade-auth-bypass | 2026-03-30 |
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
