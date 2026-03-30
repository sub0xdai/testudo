# Implementation Plan

> Last updated: 2026-03-30
> Current spec: SEC-02-cors-extension-pinning
> Phase: COMPLETE

---

## Active Spec: SEC-02-cors-extension-pinning

Pin CORS extension origins. Chrome pinned to ALLOWED_EXTENSION_ORIGINS env var. Firefox keeps prefix check (per-install UUIDs not pinnable).

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Replace blanket chrome-extension:// check with env-var pinning, keep moz-extension:// prefix | complete | low | — |
| T2 | Update CORS tests for pinned/unpinned Chrome + Firefox prefix | complete | low | T1 |

### Key Decisions

- **Firefox prefix check kept**: MDN confirms moz-extension:// UUIDs are random per install — cannot pin
- **Dev mode fallback**: When ALLOWED_EXTENSION_ORIGINS unset, any chrome-extension:// allowed (enables local dev)
- **No config.rs change**: env var read inline in is_origin_allowed — simple, follows existing ALLOWED_ORIGINS pattern

### Discoveries

- MDN docs confirm: "In Firefox: Resources are assigned a random UUID that changes for every instance of Firefox"
- Chrome IDs are deterministic from signing key — pinnable after Web Store publish
- Extension uses Bearer tokens not cookies, so CORS pinning is defense-in-depth (not primary auth gate)

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
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
